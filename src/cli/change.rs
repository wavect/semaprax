//! Read-only `semaprax change` workflows over Universal Semantic Transaction v1
//! and, additively, ordered multi-step Universal Semantic Transaction v2
//! workflows (`change workflow`).

use std::path::PathBuf;
use std::sync::Arc;

use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    self, ProjectCandidate, ProjectRevision, SemanticQuery, SemanticTransaction,
    SemanticTransactionAddContract, SemanticTransactionAddDeclaration,
    SemanticTransactionMergeOrder, SemanticTransactionRenameDisplayName,
    SemanticTransactionReplaceExpression, SemanticTransactionV2, SemanticTransactionV2Workflow,
    SemanticWorkspaceService, SemanticWorkspaceStructuralDiff,
    MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS, SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA,
};

use super::project::{is_project_manifest, resolve_positional};

pub(crate) enum ChangeCommand {
    Preview(ChangePreview),
    Rebase(ChangeRebase),
    Merge(ChangeMerge),
    Workflow(ChangeWorkflow),
}

pub(crate) struct ChangePreview {
    manifest: PathBuf,
    operation: PreviewOperation,
    revision: Option<String>,
    output: PreviewOutput,
}

enum PreviewOperation {
    RenameDisplayName {
        target: String,
        new_name: String,
    },
    AddContract {
        target: String,
        phase: String,
        predicate: serde_json::Value,
    },
    AddDeclaration {
        target: String,
        declaration: serde_json::Value,
    },
    ReplaceExpression {
        target: String,
        expression_id: String,
        replacement: serde_json::Value,
    },
}

pub(crate) struct ChangeRebase {
    base_manifest: PathBuf,
    onto_manifest: PathBuf,
    target: String,
    new_name: String,
    revision: Option<String>,
    onto_revision: Option<String>,
}

pub(crate) struct ChangeMerge {
    manifest: PathBuf,
    left_target: String,
    left_new_name: String,
    right_target: String,
    right_new_name: String,
    revision: Option<String>,
    order: MergeOrder,
}

pub(crate) struct ChangeWorkflow {
    manifest: PathBuf,
    steps: Vec<WorkflowStep>,
    revision: Option<String>,
    output: WorkflowOutput,
}

struct WorkflowStep {
    target: String,
    expression_id: String,
    replacement: serde_json::Value,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreviewOutput {
    Result,
    Evidence,
    StructuralDiff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum WorkflowOutput {
    Result,
    StructuralDiff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeOrder {
    LeftThenRight,
    RightThenLeft,
}

const PREVIEW_USAGE: &str = "change requires preview <project> <rename-display-name <stable-id> <new-name>|replace-expression <stable-id> <expression-id> <replacement-json>|add-contract <stable-id> <requires|ensures> <predicate-json>|add-declaration <anchor-stable-id> <declaration-json>> [--revision digest] [--evidence|--structural-diff]";
const REBASE_USAGE: &str = "change rebase requires <base-project> rename-display-name <stable-id> <new-name> --onto <onto-project> [--revision digest] [--onto-revision digest]";
const MERGE_USAGE: &str = "change merge requires <project> rename-display-name <left-id> <left-new-name> --with rename-display-name <right-id> <right-new-name> [--revision digest] --order <left-then-right|right-then-left>";
const WORKFLOW_USAGE: &str = "change workflow requires <project> replace-expression <stable-id> <expression-id> <replacement-json> [--then replace-expression <stable-id> <expression-id> <replacement-json>]... [--revision digest] [--structural-diff]";

pub(crate) fn parse(args: &[String]) -> Result<ChangeCommand, u8> {
    match args.first().map(String::as_str) {
        Some("preview") => parse_preview(args).map(ChangeCommand::Preview),
        Some("rebase") => parse_rebase(args).map(ChangeCommand::Rebase),
        Some("merge") => parse_merge(args).map(ChangeCommand::Merge),
        Some("workflow") => parse_workflow(args).map(ChangeCommand::Workflow),
        _ => Err(preview_usage()),
    }
}

fn parse_preview(args: &[String]) -> Result<ChangePreview, u8> {
    let manifest = args.get(1).map(PathBuf::from).ok_or_else(preview_usage)?;
    let manifest = resolve_positional(manifest);
    if !is_project_manifest(&manifest) {
        eprintln!("change preview requires a Project directory or semaprax.toml");
        return Err(2);
    }
    let (operation, mut index) = match args.get(2).map(String::as_str) {
        Some("rename-display-name") => (
            PreviewOperation::RenameDisplayName {
                target: required(args, 3, preview_usage)?,
                new_name: required(args, 4, preview_usage)?,
            },
            5,
        ),
        Some("add-contract") => {
            let phase = required(args, 4, preview_usage)?;
            if !matches!(phase.as_str(), "requires" | "ensures") {
                eprintln!("change preview add-contract phase must be requires or ensures");
                return Err(2);
            }
            let predicate =
                serde_json::from_str(&required(args, 5, preview_usage)?).map_err(|_| {
                    eprintln!("change preview add-contract predicate must be valid JSON");
                    2
                })?;
            (
                PreviewOperation::AddContract {
                    target: required(args, 3, preview_usage)?,
                    phase,
                    predicate,
                },
                6,
            )
        }
        Some("add-declaration") => {
            let declaration =
                serde_json::from_str(&required(args, 4, preview_usage)?).map_err(|_| {
                    eprintln!("change preview add-declaration constructor must be valid JSON");
                    2
                })?;
            (
                PreviewOperation::AddDeclaration {
                    target: required(args, 3, preview_usage)?,
                    declaration,
                },
                5,
            )
        }
        Some("replace-expression") => {
            let replacement =
                serde_json::from_str(&required(args, 5, preview_usage)?).map_err(|_| {
                    eprintln!("change preview replace-expression replacement must be valid JSON");
                    2
                })?;
            (
                PreviewOperation::ReplaceExpression {
                    target: required(args, 3, preview_usage)?,
                    expression_id: required(args, 4, preview_usage)?,
                    replacement,
                },
                6,
            )
        }
        _ => return Err(preview_usage()),
    };
    let mut revision = None;
    let mut output = PreviewOutput::Result;
    while index < args.len() {
        match args[index].as_str() {
            "--evidence" if output == PreviewOutput::Result => {
                output = PreviewOutput::Evidence;
                index += 1;
            }
            "--evidence" => {
                eprintln!("change preview output options are mutually exclusive and unique");
                return Err(2);
            }
            "--structural-diff" if output == PreviewOutput::Result => {
                output = PreviewOutput::StructuralDiff;
                index += 1;
            }
            "--structural-diff" => {
                eprintln!("change preview output options are mutually exclusive and unique");
                return Err(2);
            }
            "--revision" if revision.is_none() => {
                revision = Some(required(args, index + 1, preview_usage)?);
                index += 2;
            }
            "--revision" => {
                eprintln!("change preview option `--revision` may not be repeated");
                return Err(2);
            }
            option => {
                eprintln!("unknown change preview option `{option}`");
                return Err(2);
            }
        }
    }
    Ok(ChangePreview {
        manifest,
        operation,
        revision,
        output,
    })
}

fn parse_rebase(args: &[String]) -> Result<ChangeRebase, u8> {
    if args.get(2).map(String::as_str) != Some("rename-display-name") {
        return Err(rebase_usage());
    }
    let base_manifest = project_operand(args, 1, "change rebase base", rebase_usage)?;
    let target = required(args, 3, rebase_usage)?;
    let new_name = required(args, 4, rebase_usage)?;
    let mut onto_manifest = None;
    let mut revision = None;
    let mut onto_revision = None;
    let mut index = 5;
    while index < args.len() {
        let option = args[index].as_str();
        let slot = match option {
            "--onto" => &mut onto_manifest,
            "--revision" => &mut revision,
            "--onto-revision" => &mut onto_revision,
            _ => {
                eprintln!("unknown change rebase option `{option}`");
                return Err(2);
            }
        };
        if slot.is_some() {
            eprintln!("change rebase option `{option}` may not be repeated");
            return Err(2);
        }
        *slot = Some(required(args, index + 1, rebase_usage)?);
        index += 2;
    }
    let onto_manifest = onto_manifest.ok_or_else(rebase_usage)?;
    let onto_manifest = project_path(onto_manifest, "change rebase destination")?;
    Ok(ChangeRebase {
        base_manifest,
        onto_manifest,
        target,
        new_name,
        revision,
        onto_revision,
    })
}

fn parse_merge(args: &[String]) -> Result<ChangeMerge, u8> {
    if args.get(2).map(String::as_str) != Some("rename-display-name")
        || args.get(5).map(String::as_str) != Some("--with")
        || args.get(6).map(String::as_str) != Some("rename-display-name")
    {
        return Err(merge_usage());
    }
    let manifest = project_operand(args, 1, "change merge", merge_usage)?;
    let left_target = required(args, 3, merge_usage)?;
    let left_new_name = required(args, 4, merge_usage)?;
    let right_target = required(args, 7, merge_usage)?;
    let right_new_name = required(args, 8, merge_usage)?;
    let mut revision = None;
    let mut order = None;
    let mut index = 9;
    while index < args.len() {
        let option = args[index].as_str();
        match option {
            "--revision" if revision.is_none() => {
                revision = Some(required(args, index + 1, merge_usage)?);
            }
            "--order" if order.is_none() => {
                order = Some(match required(args, index + 1, merge_usage)?.as_str() {
                    "left-then-right" => MergeOrder::LeftThenRight,
                    "right-then-left" => MergeOrder::RightThenLeft,
                    value => {
                        eprintln!("unknown change merge order `{value}`");
                        return Err(2);
                    }
                });
            }
            "--revision" | "--order" => {
                eprintln!("change merge option `{option}` may not be repeated");
                return Err(2);
            }
            _ => {
                eprintln!("unknown change merge option `{option}`");
                return Err(2);
            }
        }
        index += 2;
    }
    let order = order.ok_or_else(merge_usage)?;
    Ok(ChangeMerge {
        manifest,
        left_target,
        left_new_name,
        right_target,
        right_new_name,
        revision,
        order,
    })
}

fn parse_workflow(args: &[String]) -> Result<ChangeWorkflow, u8> {
    let manifest = project_operand(args, 1, "change workflow", workflow_usage)?;
    let mut steps = Vec::new();
    let mut index = 2;
    loop {
        if args.get(index).map(String::as_str) != Some("replace-expression") {
            return Err(workflow_usage());
        }
        let target = required(args, index + 1, workflow_usage)?;
        let expression_id = required(args, index + 2, workflow_usage)?;
        let replacement = serde_json::from_str(&required(args, index + 3, workflow_usage)?)
            .map_err(|_| {
                eprintln!("change workflow replace-expression replacement must be valid JSON");
                2
            })?;
        steps.push(WorkflowStep {
            target,
            expression_id,
            replacement,
        });
        index += 4;
        if args.get(index).map(String::as_str) == Some("--then") {
            index += 1;
            continue;
        }
        break;
    }
    if steps.len() > MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS {
        eprintln!(
            "change workflow accepts at most {MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS} steps"
        );
        return Err(2);
    }
    let mut revision = None;
    let mut output = WorkflowOutput::Result;
    while index < args.len() {
        match args[index].as_str() {
            "--structural-diff" if output == WorkflowOutput::Result => {
                output = WorkflowOutput::StructuralDiff;
                index += 1;
            }
            "--structural-diff" => {
                eprintln!("change workflow output options are mutually exclusive and unique");
                return Err(2);
            }
            "--revision" if revision.is_none() => {
                revision = Some(required(args, index + 1, workflow_usage)?);
                index += 2;
            }
            "--revision" => {
                eprintln!("change workflow option `--revision` may not be repeated");
                return Err(2);
            }
            option => {
                eprintln!("unknown change workflow option `{option}`");
                return Err(2);
            }
        }
    }
    Ok(ChangeWorkflow {
        manifest,
        steps,
        revision,
        output,
    })
}

fn project_operand(
    args: &[String],
    index: usize,
    label: &str,
    usage: fn() -> u8,
) -> Result<PathBuf, u8> {
    project_path(required(args, index, usage)?, label)
}

fn project_path(path: String, label: &str) -> Result<PathBuf, u8> {
    let path = resolve_positional(PathBuf::from(path));
    if !is_project_manifest(&path) {
        eprintln!("{label} requires a Project directory or semaprax.toml");
        return Err(2);
    }
    Ok(path)
}

fn required(args: &[String], index: usize, usage: fn() -> u8) -> Result<String, u8> {
    args.get(index)
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
        .cloned()
        .ok_or_else(usage)
}

fn preview_usage() -> u8 {
    eprintln!("{PREVIEW_USAGE}");
    2
}

fn rebase_usage() -> u8 {
    eprintln!("{REBASE_USAGE}");
    2
}

fn merge_usage() -> u8 {
    eprintln!("{MERGE_USAGE}");
    2
}

fn workflow_usage() -> u8 {
    eprintln!("{WORKFLOW_USAGE}");
    2
}

pub(crate) fn run(command: ChangeCommand, report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let output = match command {
        ChangeCommand::Preview(options) => run_preview(options),
        ChangeCommand::Rebase(options) => run_rebase(options),
        ChangeCommand::Merge(options) => run_merge(options),
        ChangeCommand::Workflow(options) => run_workflow(options),
    }
    .map_err(|errors| report(&errors))?;
    print!("{output}");
    Ok(())
}

fn run_preview(options: ChangePreview) -> Result<String, Vec<Diagnostic>> {
    project::with_authenticated_project(&options.manifest, |snapshot| {
        let service = SemanticWorkspaceService::open(snapshot.retain_revision())?;
        let expected = selected_revision(&service, options.revision.as_deref());
        let transaction = match options.operation {
            PreviewOperation::RenameDisplayName { target, new_name } => {
                rename_transaction(&service, &expected, &target, &new_name)?
            }
            PreviewOperation::AddContract {
                target,
                phase,
                predicate,
            } => add_contract_transaction(&service, &expected, &target, &phase, predicate)?,
            PreviewOperation::AddDeclaration {
                target,
                declaration,
            } => add_declaration_transaction(&service, &expected, &target, declaration)?,
            PreviewOperation::ReplaceExpression {
                target,
                expression_id,
                replacement,
            } => {
                let transaction = replace_expression_transaction(
                    &service,
                    &expected,
                    &target,
                    &expression_id,
                    replacement,
                )?;
                let artifacts =
                    service.validate_transaction_v2(transaction.to_json().as_bytes())?;
                return match options.output {
                    PreviewOutput::Result => Ok(artifacts.result().to_owned()),
                    PreviewOutput::Evidence => Ok(artifacts.evidence().to_owned()),
                    PreviewOutput::StructuralDiff => Ok(SemanticWorkspaceStructuralDiff::derive(
                        artifacts.candidate(),
                        artifacts.candidate().candidate_digest(),
                    )?
                    .to_json()
                    .to_owned()),
                };
            }
        };
        let artifacts = service.validate_transaction(transaction.to_json().as_bytes())?;
        match options.output {
            PreviewOutput::Result => Ok(artifacts.result().to_owned()),
            PreviewOutput::Evidence => Ok(artifacts.evidence().to_owned()),
            PreviewOutput::StructuralDiff => Ok(SemanticWorkspaceStructuralDiff::derive(
                artifacts.candidate(),
                artifacts.candidate().candidate_digest(),
            )?
            .to_json()
            .to_owned()),
        }
    })
}

fn replace_expression_transaction(
    service: &SemanticWorkspaceService,
    expected: &str,
    target: &str,
    expression_id: &str,
    replacement: serde_json::Value,
) -> Result<SemanticTransactionV2, Vec<Diagnostic>> {
    if expected != service.active_generation().workspace_revision() {
        return Err(vec![Diagnostic::io(
            "SPX-G527",
            "ReplaceExpression requested workspace revision is stale",
        )]);
    }
    replace_expression_transaction_against(
        service.active_generation().revision(),
        expected,
        target,
        expression_id,
        replacement,
    )
}

/// Build one v2 ReplaceExpression transaction by looking up `target`'s
/// expression catalog directly against `revision`, rather than against a
/// service's currently active generation. `change workflow`'s steps after
/// the first select their expression identity and old-source precondition
/// against the exact intermediate revision the prior step produced, which
/// has no `SemanticWorkspaceService` of its own.
fn replace_expression_transaction_against(
    revision: &Arc<ProjectRevision>,
    expected: &str,
    target: &str,
    expression_id: &str,
    replacement: serde_json::Value,
) -> Result<SemanticTransactionV2, Vec<Diagnostic>> {
    let candidate = ProjectCandidate::open(revision.clone(), revision.project_revision())?;
    let catalog: serde_json::Value = serde_json::from_str(&candidate.expression_catalog(target)?)
        .map_err(|_| {
        vec![Diagnostic::io(
            "SPX-G525",
            "ReplaceExpression catalog is not valid JSON",
        )]
    })?;
    let entry = catalog["expressions"]
        .as_array()
        .and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry["expression_id"].as_str() == Some(expression_id))
        })
        .ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G527",
                "ReplaceExpression identity is stale or unavailable",
            )]
        })?;
    if entry["replaceable"] != serde_json::Value::Bool(true) {
        return Err(vec![Diagnostic::io(
            "SPX-G525",
            "ReplaceExpression selection is not replaceable",
        )]);
    }
    let path = catalog["source"]["path"].as_str().ok_or_else(|| {
        vec![Diagnostic::io(
            "SPX-G525",
            "ReplaceExpression source path is missing",
        )]
    })?;
    let start = entry["source_span"]["start"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G525",
                "ReplaceExpression source span is invalid",
            )]
        })?;
    let end = entry["source_span"]["end"]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G525",
                "ReplaceExpression source span is invalid",
            )]
        })?;
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == path)
        .ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G527",
                "ReplaceExpression source is stale",
            )]
        })?;
    let expected_old_expression = source.source().get(start..end).ok_or_else(|| {
        vec![Diagnostic::io(
            "SPX-G527",
            "ReplaceExpression source span is stale",
        )]
    })?;
    SemanticTransactionV2::replace_expression(
        expected,
        SemanticTransactionReplaceExpression::new(
            target,
            expression_id,
            expected_old_expression,
            replacement,
        ),
    )
}

fn run_rebase(options: ChangeRebase) -> Result<String, Vec<Diagnostic>> {
    project::with_authenticated_project(&options.base_manifest, |base_snapshot| {
        let original_base = base_snapshot.retain_revision();
        let service = SemanticWorkspaceService::open(original_base.clone())?;
        let expected = selected_revision(&service, options.revision.as_deref());
        let transaction =
            rename_transaction(&service, &expected, &options.target, &options.new_name)?;
        project::with_authenticated_project(&options.onto_manifest, |onto_snapshot| {
            let onto = onto_snapshot.retain_revision();
            let onto_service = SemanticWorkspaceService::open(onto.clone())?;
            let expected_onto = selected_revision(&onto_service, options.onto_revision.as_deref());
            Ok(transaction
                .rebase(original_base, onto, &expected_onto)?
                .to_json()
                .to_owned())
        })
    })
}

fn run_merge(options: ChangeMerge) -> Result<String, Vec<Diagnostic>> {
    project::with_authenticated_project(&options.manifest, |snapshot| {
        let base = snapshot.retain_revision();
        let service = SemanticWorkspaceService::open(base.clone())?;
        let expected = selected_revision(&service, options.revision.as_deref());
        let left = rename_transaction(
            &service,
            &expected,
            &options.left_target,
            &options.left_new_name,
        )?;
        let right = rename_transaction(
            &service,
            &expected,
            &options.right_target,
            &options.right_new_name,
        )?;
        let order = match options.order {
            MergeOrder::LeftThenRight => SemanticTransactionMergeOrder::LeftThenRight,
            MergeOrder::RightThenLeft => SemanticTransactionMergeOrder::RightThenLeft,
        };
        Ok(left.merge(&right, base, order)?.to_json().to_owned())
    })
}

/// Compose `options.steps` into one Universal Semantic Transaction v2
/// workflow: step 0 selects against the project's active revision, and each
/// later step selects against the exact revision its predecessor produced,
/// exactly mirroring [`SemanticTransactionV2Workflow::derive`]'s own base
/// evolution (invoked below to produce the returned evidence, so the CLI's
/// own step-by-step reconstruction and the frozen workflow core agree on the
/// final candidate by construction, not by trusting this loop alone).
fn run_workflow(options: ChangeWorkflow) -> Result<String, Vec<Diagnostic>> {
    project::with_authenticated_project(&options.manifest, |snapshot| {
        let service = SemanticWorkspaceService::open(snapshot.retain_revision())?;
        let expected = selected_revision(&service, options.revision.as_deref());
        if expected != service.active_generation().workspace_revision() {
            return Err(vec![Diagnostic::io(
                "SPX-G527",
                "workflow requested workspace revision is stale",
            )]);
        }
        let base = service.active_generation().revision().clone();
        let mut current_revision = base.clone();
        let mut current_workspace_revision = expected;
        let mut transactions = Vec::with_capacity(options.steps.len());
        for step in &options.steps {
            let transaction = replace_expression_transaction_against(
                &current_revision,
                &current_workspace_revision,
                &step.target,
                &step.expression_id,
                step.replacement.clone(),
            )?;
            let artifacts = transaction.validate(current_revision.clone())?;
            current_revision = artifacts.candidate().revision().clone();
            current_workspace_revision = artifacts
                .candidate_program_root()
                .workspace_revision()
                .to_owned();
            transactions.push(transaction);
        }
        let workflow = SemanticTransactionV2Workflow::derive(base, &transactions)?;
        match options.output {
            WorkflowOutput::Result => Ok(workflow.to_json().to_owned()),
            WorkflowOutput::StructuralDiff => Ok(workflow.structural_diff().to_json().to_owned()),
        }
    })
}

fn selected_revision(service: &SemanticWorkspaceService, requested: Option<&str>) -> String {
    requested
        .unwrap_or_else(|| service.active_generation().workspace_revision())
        .to_owned()
}

fn rename_transaction(
    service: &SemanticWorkspaceService,
    expected: &str,
    target: &str,
    new_name: &str,
) -> Result<SemanticTransaction, Vec<Diagnostic>> {
    let discovery = SemanticQuery::available_operations(expected, target)?;
    let discovery = service.query(discovery.to_json().as_bytes())?;
    let payload: serde_json::Value = serde_json::from_str(discovery.payload()).map_err(|_| {
        vec![Diagnostic::io(
            "SPX-G531",
            "available operations payload is not valid JSON",
        )]
    })?;
    if payload.get("schema").and_then(serde_json::Value::as_str)
        != Some(SEMANTIC_QUERY_AVAILABLE_OPERATIONS_SCHEMA)
    {
        return Err(vec![Diagnostic::io(
            "SPX-G531",
            "available operations payload schema is unsupported",
        )]);
    }
    let operation = payload
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .and_then(|operations| {
            operations
                .iter()
                .find(|operation| operation["kind"] == "rename_display_name")
        })
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G531",
                "available operations payload has no operation entry",
            )]
        })?;
    let old_name = operation
        .get("expected_old_value")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    SemanticTransaction::rename_display_name(
        expected,
        SemanticTransactionRenameDisplayName::new(target, old_name, new_name),
    )
}

fn add_contract_transaction(
    service: &SemanticWorkspaceService,
    expected: &str,
    target: &str,
    phase: &str,
    predicate: serde_json::Value,
) -> Result<SemanticTransaction, Vec<Diagnostic>> {
    let discovery = SemanticQuery::available_operations(expected, target)?;
    let discovery = service.query(discovery.to_json().as_bytes())?;
    let payload: serde_json::Value = serde_json::from_str(discovery.payload()).map_err(|_| {
        vec![Diagnostic::io(
            "SPX-G531",
            "available operations payload is not valid JSON",
        )]
    })?;
    let operation = payload
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .and_then(|operations| {
            operations
                .iter()
                .find(|operation| operation["kind"] == "add_contract")
        })
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G531",
                "available operations payload has no AddContract entry",
            )]
        })?;
    let expected_old_contract = operation
        .get("expected_old_contract")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    SemanticTransaction::add_contract(
        expected,
        SemanticTransactionAddContract::new(target, expected_old_contract, phase, predicate),
    )
}

fn add_declaration_transaction(
    service: &SemanticWorkspaceService,
    expected: &str,
    target: &str,
    declaration: serde_json::Value,
) -> Result<SemanticTransaction, Vec<Diagnostic>> {
    let discovery = SemanticQuery::available_operations(expected, target)?;
    let discovery = service.query(discovery.to_json().as_bytes())?;
    let payload: serde_json::Value = serde_json::from_str(discovery.payload()).map_err(|_| {
        vec![Diagnostic::io(
            "SPX-G531",
            "available operations payload is not valid JSON",
        )]
    })?;
    let operation = payload
        .get("operations")
        .and_then(serde_json::Value::as_array)
        .and_then(|operations| {
            operations
                .iter()
                .find(|operation| operation["kind"] == "add_declaration")
        })
        .and_then(serde_json::Value::as_object)
        .ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G531",
                "available operations payload has no AddDeclaration entry",
            )]
        })?;
    let expected_old_module = operation
        .get("expected_old_module")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    SemanticTransaction::add_declaration(
        expected,
        SemanticTransactionAddDeclaration::new(target, expected_old_module, declaration),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn preview_grammar_is_closed() {
        let ChangeCommand::Preview(parsed) = parse(&strings(&[
            "preview",
            "fixtures/semaprax.toml",
            "rename-display-name",
            "app.run",
            "execute",
            "--revision",
            "sha256:abc",
            "--evidence",
        ]))
        .unwrap() else {
            panic!("preview grammar selected another command");
        };
        let PreviewOperation::RenameDisplayName { target, new_name } = parsed.operation else {
            panic!("preview grammar selected another operation");
        };
        assert_eq!(target, "app.run");
        assert_eq!(new_name, "execute");
        assert_eq!(parsed.revision.as_deref(), Some("sha256:abc"));
        assert_eq!(parsed.output, PreviewOutput::Evidence);
        let ChangeCommand::Preview(contract) = parse(&strings(&[
            "preview",
            "fixtures/semaprax.toml",
            "add-contract",
            "app.run",
            "ensures",
            r#"{"kind":"bool","value":true}"#,
        ]))
        .unwrap() else {
            panic!("preview grammar selected another command");
        };
        let PreviewOperation::AddContract {
            target,
            phase,
            predicate,
        } = contract.operation
        else {
            panic!("preview grammar selected another operation");
        };
        assert_eq!(target, "app.run");
        assert_eq!(phase, "ensures");
        assert_eq!(predicate, serde_json::json!({"kind":"bool","value":true}));
        let ChangeCommand::Preview(declaration) = parse(&strings(&[
            "preview",
            "fixtures/semaprax.toml",
            "add-declaration",
            "app.run",
            r#"{"id":"app.helper"}"#,
        ]))
        .unwrap() else {
            panic!("preview grammar selected another command");
        };
        let PreviewOperation::AddDeclaration {
            target,
            declaration,
        } = declaration.operation
        else {
            panic!("preview grammar selected another operation");
        };
        assert_eq!(target, "app.run");
        assert_eq!(declaration, serde_json::json!({"id":"app.helper"}));
        for malformed in [
            vec![],
            vec![
                "preview",
                "m.spx",
                "rename-display-name",
                "app.run",
                "execute",
            ],
            vec![
                "apply",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
                "execute",
            ],
            vec![
                "preview",
                "fixtures/semaprax.toml",
                "rename",
                "app.run",
                "execute",
            ],
            vec![
                "preview",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
            ],
            vec![
                "preview",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
                "execute",
                "--unknown",
            ],
            vec![
                "preview",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
                "execute",
                "--evidence",
                "--structural-diff",
            ],
        ] {
            assert!(parse(&strings(&malformed)).is_err(), "{malformed:?}");
        }
    }

    #[test]
    fn rebase_and_merge_grammars_are_closed() {
        let ChangeCommand::Rebase(rebase) = parse(&strings(&[
            "rebase",
            "fixtures/semaprax.toml",
            "rename-display-name",
            "app.run",
            "execute",
            "--onto",
            "fixtures/semaprax.toml",
            "--revision",
            "sha256:base",
            "--onto-revision",
            "sha256:onto",
        ]))
        .unwrap() else {
            panic!("rebase grammar selected another command");
        };
        assert_eq!(rebase.target, "app.run");
        assert_eq!(rebase.new_name, "execute");
        assert_eq!(rebase.revision.as_deref(), Some("sha256:base"));
        assert_eq!(rebase.onto_revision.as_deref(), Some("sha256:onto"));

        for order in ["left-then-right", "right-then-left"] {
            let ChangeCommand::Merge(merge) = parse(&strings(&[
                "merge",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.left",
                "renamed_left",
                "--with",
                "rename-display-name",
                "app.right",
                "renamed_right",
                "--order",
                order,
            ]))
            .unwrap() else {
                panic!("merge grammar selected another command");
            };
            assert_eq!(merge.left_target, "app.left");
            assert_eq!(merge.right_target, "app.right");
            assert_eq!(
                merge.order,
                if order == "left-then-right" {
                    MergeOrder::LeftThenRight
                } else {
                    MergeOrder::RightThenLeft
                }
            );
        }

        for malformed in [
            vec![
                "rebase",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
                "execute",
            ],
            vec![
                "rebase",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
                "execute",
                "--onto",
                "fixtures/semaprax.toml",
                "--onto",
                "fixtures/semaprax.toml",
            ],
            vec![
                "merge",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.left",
                "left",
                "--with",
                "rename-display-name",
                "app.right",
                "right",
            ],
            vec![
                "merge",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.left",
                "left",
                "--with",
                "rename-display-name",
                "app.right",
                "right",
                "--order",
                "automatic",
            ],
        ] {
            assert!(parse(&strings(&malformed)).is_err(), "{malformed:?}");
        }
    }

    #[test]
    fn workflow_grammar_is_closed() {
        let ChangeCommand::Workflow(single) = parse(&strings(&[
            "workflow",
            "fixtures/semaprax.toml",
            "replace-expression",
            "app.run",
            "expr.0",
            r#"{"kind":"int","value":1}"#,
            "--revision",
            "sha256:abc",
            "--structural-diff",
        ]))
        .unwrap() else {
            panic!("workflow grammar selected another command");
        };
        assert_eq!(single.steps.len(), 1);
        assert_eq!(single.steps[0].target, "app.run");
        assert_eq!(single.steps[0].expression_id, "expr.0");
        assert_eq!(
            single.steps[0].replacement,
            serde_json::json!({"kind":"int","value":1})
        );
        assert_eq!(single.revision.as_deref(), Some("sha256:abc"));
        assert_eq!(single.output, WorkflowOutput::StructuralDiff);

        let ChangeCommand::Workflow(chained) = parse(&strings(&[
            "workflow",
            "fixtures/semaprax.toml",
            "replace-expression",
            "app.run",
            "expr.0",
            r#"{"kind":"int","value":1}"#,
            "--then",
            "replace-expression",
            "app.helper",
            "expr.1",
            r#"{"kind":"int","value":2}"#,
            "--then",
            "replace-expression",
            "app.other",
            "expr.2",
            r#"{"kind":"int","value":3}"#,
        ]))
        .unwrap() else {
            panic!("workflow grammar selected another command");
        };
        assert_eq!(chained.steps.len(), 3);
        assert_eq!(chained.steps[1].target, "app.helper");
        assert_eq!(chained.steps[2].expression_id, "expr.2");
        assert_eq!(chained.output, WorkflowOutput::Result);
        assert_eq!(chained.revision, None);

        let mut too_many = vec![
            "workflow".to_owned(),
            "fixtures/semaprax.toml".to_owned(),
            "replace-expression".to_owned(),
            "app.0".to_owned(),
            "expr.0".to_owned(),
            r#"{"kind":"int","value":0}"#.to_owned(),
        ];
        for index in 1..=MAX_SEMANTIC_TRANSACTION_V2_WORKFLOW_STEPS {
            too_many.extend([
                "--then".to_owned(),
                "replace-expression".to_owned(),
                format!("app.{index}"),
                format!("expr.{index}"),
                r#"{"kind":"int","value":0}"#.to_owned(),
            ]);
        }
        assert!(parse(&too_many).is_err());

        for malformed in [
            vec!["workflow", "fixtures/semaprax.toml"],
            vec![
                "workflow",
                "m.spx",
                "replace-expression",
                "app.run",
                "expr.0",
                r#"{"kind":"int","value":1}"#,
            ],
            vec![
                "workflow",
                "fixtures/semaprax.toml",
                "rename-display-name",
                "app.run",
                "execute",
            ],
            vec![
                "workflow",
                "fixtures/semaprax.toml",
                "replace-expression",
                "app.run",
                "expr.0",
                "not json",
            ],
            vec![
                "workflow",
                "fixtures/semaprax.toml",
                "replace-expression",
                "app.run",
                "expr.0",
                r#"{"kind":"int","value":1}"#,
                "--then",
            ],
            vec![
                "workflow",
                "fixtures/semaprax.toml",
                "replace-expression",
                "app.run",
                "expr.0",
                r#"{"kind":"int","value":1}"#,
                "--structural-diff",
                "--structural-diff",
            ],
            vec![
                "workflow",
                "fixtures/semaprax.toml",
                "replace-expression",
                "app.run",
                "expr.0",
                r#"{"kind":"int","value":1}"#,
                "--revision",
                "sha256:a",
                "--revision",
                "sha256:b",
            ],
            vec![
                "workflow",
                "fixtures/semaprax.toml",
                "replace-expression",
                "app.run",
                "expr.0",
                r#"{"kind":"int","value":1}"#,
                "--unknown",
            ],
        ] {
            assert!(parse(&strings(&malformed)).is_err(), "{malformed:?}");
        }
    }
}
