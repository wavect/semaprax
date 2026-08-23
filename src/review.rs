//! Deterministic, read-only Semantic Review v1.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::ast::{Expr, ExprKind, Program, TypeDeclarationKind};
use crate::bounded_output::BudgetedJoin as _;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::patch::{self, PatchPreflight, PreflightChange, PreflightOperation};
use crate::{graph, hir, impact, parse};

macro_rules! format {
    ($($argument:tt)*) => {
        crate::bounded_output::budgeted_format(format_args!($($argument)*))
    };
}

const REVIEW_SCHEMA: &str = "semaprax.semantic-review.v1";
const IDENTITY_REBASE_SCHEMA: &str = "semaprax.identity-rebase.v1";
pub(crate) const MAX_SOURCE_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_PATCH_BYTES: usize = 4 * 1024 * 1024;
pub(crate) const MAX_OPERATIONS: usize = 4096;
pub(crate) const MAX_DECLARATIONS: usize = 4096;
pub(crate) const MAX_CALLABLES: usize = 1024;
pub(crate) const MAX_CALL_SITES: usize = 65_536;
pub(crate) const MAX_IMPACT_DEPTH: usize = 1024;
pub(crate) const MAX_IMPACT_NODES: usize = 1024;
pub(crate) const MAX_IMPACT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;
const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-review.source-digest.v1\0";
const PATCH_DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-review.patch-digest.v1\0";
const IMPACT_DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-review.impact-digest.v1\0";
const IDENTITY_REBASE_DIGEST_DOMAIN: &[u8] =
    b"semaprax.semantic-review.identity-rebase-digest.v1\0";
const EVIDENCE_ID: &str = "evidence:0";

pub(crate) fn source_digest(source: &[u8]) -> String {
    domain_digest(SOURCE_DIGEST_DOMAIN, source)
}

#[derive(Clone, Copy)]
pub(crate) struct ReviewUsage {
    source_bytes: usize,
    patch_bytes: usize,
    operations: usize,
    declarations: usize,
    callables: usize,
    call_sites: usize,
    impact_depth: usize,
    impact_nodes: usize,
    impact_bytes: usize,
}

impl ReviewUsage {
    pub(crate) fn source_bytes(self) -> usize {
        self.source_bytes
    }

    pub(crate) fn patch_bytes(self) -> usize {
        self.patch_bytes
    }

    pub(crate) fn operations(self) -> usize {
        self.operations
    }

    pub(crate) fn declarations(self) -> usize {
        self.declarations
    }

    pub(crate) fn callables(self) -> usize {
        self.callables
    }

    pub(crate) fn call_sites(self) -> usize {
        self.call_sites
    }

    pub(crate) fn impact_depth(self) -> usize {
        self.impact_depth
    }

    pub(crate) fn impact_nodes(self) -> usize {
        self.impact_nodes
    }

    pub(crate) fn impact_bytes(self) -> usize {
        self.impact_bytes
    }
}

struct AstUsage {
    declarations: usize,
    callables: usize,
    call_sites: usize,
}

struct Evidence {
    json: String,
    kind: &'static str,
    schema: &'static str,
    digest: String,
    impact_depth: usize,
    impact_nodes: usize,
    impact_bytes: usize,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SectionKind {
    Behavior,
    ApiIdentity,
    SecurityAuthority,
    MemoryOwnership,
    TargetArtifact,
    Migration,
    Unsafe,
}

impl SectionKind {
    const ALL: [Self; 7] = [
        Self::Behavior,
        Self::ApiIdentity,
        Self::SecurityAuthority,
        Self::MemoryOwnership,
        Self::TargetArtifact,
        Self::Migration,
        Self::Unsafe,
    ];

    const fn text(self) -> &'static str {
        match self {
            Self::Behavior => "behavior",
            Self::ApiIdentity => "api_identity",
            Self::SecurityAuthority => "security_authority",
            Self::MemoryOwnership => "memory_ownership",
            Self::TargetArtifact => "target_artifact",
            Self::Migration => "migration",
            Self::Unsafe => "unsafe",
        }
    }
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum Disposition {
    Change,
    BoundedNoChange,
    MigrationRequired,
    Unknown,
    NotApplicable,
}

impl Disposition {
    const fn text(self) -> &'static str {
        match self {
            Self::Change => "change",
            Self::BoundedNoChange => "bounded_no_change",
            Self::MigrationRequired => "migration_required",
            Self::Unknown => "unknown",
            Self::NotApplicable => "not_applicable",
        }
    }
}

struct Finding {
    code: &'static str,
    disposition: Disposition,
    statement: String,
    operation_index: usize,
}

pub(crate) struct ReviewAssessment {
    key: &'static str,
    value: &'static str,
}

impl ReviewAssessment {
    pub(crate) fn key(&self) -> &'static str {
        self.key
    }

    pub(crate) fn value(&self) -> &'static str {
        self.value
    }
}

pub(crate) struct ReviewSupportingEvidence {
    kind: &'static str,
    schema: &'static str,
    digest: String,
}

impl ReviewSupportingEvidence {
    pub(crate) fn kind(&self) -> &'static str {
        self.kind
    }

    pub(crate) fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }
}

pub(crate) struct ReviewBuild {
    preflight: patch::PatchPreflight,
    before_resolved: hir::ResolvedProgram,
    candidate_resolved: hir::ResolvedProgram,
    report: String,
    source_graph_schema: &'static str,
    base_revision: String,
    candidate_revision: String,
    source_digest: String,
    patch_schema: &'static str,
    patch_digest: String,
    assessments: Vec<ReviewAssessment>,
    supporting_evidence: ReviewSupportingEvidence,
    usage: ReviewUsage,
}

#[derive(Clone, Copy)]
pub(crate) struct WorkspaceEvidenceLimits {
    pub(crate) max_impact_nodes: usize,
    pub(crate) max_impact_bytes: usize,
    pub(crate) max_review_bytes: usize,
}

impl ReviewBuild {
    pub(crate) fn preflight(&self) -> &patch::PatchPreflight {
        &self.preflight
    }

    pub(crate) fn before_resolved(&self) -> &hir::ResolvedProgram {
        &self.before_resolved
    }

    pub(crate) fn candidate_resolved(&self) -> &hir::ResolvedProgram {
        &self.candidate_resolved
    }

    pub(crate) fn report(&self) -> &str {
        &self.report
    }

    pub(crate) fn into_report(self) -> String {
        self.report
    }

    pub(crate) fn source_graph_schema(&self) -> &'static str {
        self.source_graph_schema
    }

    pub(crate) fn base_revision(&self) -> &str {
        &self.base_revision
    }

    pub(crate) fn candidate_revision(&self) -> &str {
        &self.candidate_revision
    }

    pub(crate) fn source_digest(&self) -> &str {
        &self.source_digest
    }

    pub(crate) fn patch_schema(&self) -> &'static str {
        self.patch_schema
    }

    pub(crate) fn patch_digest(&self) -> &str {
        &self.patch_digest
    }

    pub(crate) fn assessments(&self) -> &[ReviewAssessment] {
        &self.assessments
    }

    pub(crate) fn supporting_evidence(&self) -> &ReviewSupportingEvidence {
        &self.supporting_evidence
    }

    pub(crate) fn usage(&self) -> ReviewUsage {
        self.usage
    }
}

pub fn preview(source_path: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    preview_with_hook(source_path, patch_path, |_, _| Ok(()))
}

fn preview_with_hook(
    source_path: &Path,
    patch_path: &Path,
    mut before_final_check: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot =
        patch::read_source_snapshot_bounded(&canonical_source_path, MAX_SOURCE_BYTES, "SPX-G120")?;
    let patch_source = read_patch_bounded(patch_path)?;
    let build = build_owned(
        snapshot.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
    )?;
    before_final_check(&canonical_source_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("semantic review final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged_bounded(
        &canonical_source_path,
        source_path,
        &snapshot,
        build.base_revision(),
        MAX_SOURCE_BYTES,
    )?;
    Ok(build.into_report())
}

pub(crate) fn build_owned(
    source: String,
    patch_source: String,
    diagnostic_path: std::path::PathBuf,
) -> Result<ReviewBuild, Vec<Diagnostic>> {
    build_owned_with_candidate_limit(source, patch_source, diagnostic_path, None)
}

pub(crate) fn build_target_owned(
    source: String,
    patch_source: String,
    diagnostic_path: std::path::PathBuf,
    max_candidate_bytes: usize,
) -> Result<ReviewBuild, Vec<Diagnostic>> {
    build_owned_with_candidate_limit(
        source,
        patch_source,
        diagnostic_path,
        Some(max_candidate_bytes),
    )
}

fn build_owned_with_candidate_limit(
    source: String,
    patch_source: String,
    diagnostic_path: std::path::PathBuf,
    max_candidate_bytes: Option<usize>,
) -> Result<ReviewBuild, Vec<Diagnostic>> {
    let parsed = parse(&source, &diagnostic_path).map_err(|error| vec![error])?;
    let ast_usage = precheck_program(&parsed)?;
    let preflight = if let Some(max_candidate_bytes) = max_candidate_bytes {
        let (result, overflowed) = crate::bounded_output::with_limit(max_candidate_bytes, || {
            patch::preflight_target_owned(
                source,
                patch_source,
                diagnostic_path,
                MAX_OPERATIONS,
                max_candidate_bytes,
            )
        });
        if overflowed {
            return Err(vec![Diagnostic::io(
                "SPX-G140",
                format!(
                    "Semantic Target Evidence source canonicalization exceeds {max_candidate_bytes} bytes"
                ),
            )]);
        }
        result?
    } else {
        patch::preflight_review_owned(source, patch_source, diagnostic_path, MAX_OPERATIONS)?
    };
    build_from_preflight_with_candidate_limit(preflight, ast_usage, max_candidate_bytes)
}

/// Builds immutable Review v1 facts from an already authenticated pure Patch
/// preflight. Workspace callers use this seam to avoid parsing or preflighting
/// an embedded patch a second time; no Target-specific limit or diagnostic is
/// introduced.
#[allow(
    dead_code,
    reason = "consumed by the private Workspace Evidence Phase A build"
)]
pub(crate) fn build_from_preflight(
    preflight: PatchPreflight,
) -> Result<ReviewBuild, Vec<Diagnostic>> {
    let ast_usage = precheck_program(preflight.before())?;
    build_from_preflight_with_limits(preflight, ast_usage, None, None)
}

pub(crate) fn build_from_preflight_for_workspace(
    preflight: PatchPreflight,
    limits: WorkspaceEvidenceLimits,
) -> Result<ReviewBuild, Vec<Diagnostic>> {
    let ast_usage = precheck_program(preflight.before())?;
    build_from_preflight_with_limits(preflight, ast_usage, None, Some(limits))
}

fn build_from_preflight_with_candidate_limit(
    preflight: PatchPreflight,
    ast_usage: AstUsage,
    max_candidate_bytes: Option<usize>,
) -> Result<ReviewBuild, Vec<Diagnostic>> {
    build_from_preflight_with_limits(preflight, ast_usage, max_candidate_bytes, None)
}

fn build_from_preflight_with_limits(
    preflight: PatchPreflight,
    ast_usage: AstUsage,
    max_candidate_bytes: Option<usize>,
    workspace_limits: Option<WorkspaceEvidenceLimits>,
) -> Result<ReviewBuild, Vec<Diagnostic>> {
    let resolve_checked = || -> Result<_, Vec<Diagnostic>> {
        let before_resolved = hir::resolve(preflight.before())?;
        let candidate_resolved = hir::resolve(preflight.candidate())?;
        hir::validate(&before_resolved).map_err(|error| vec![error])?;
        hir::validate(&candidate_resolved).map_err(|error| vec![error])?;
        graph::reject_native_rust_imports(&before_resolved).map_err(|error| vec![error])?;
        graph::reject_native_rust_imports(&candidate_resolved).map_err(|error| vec![error])?;
        Ok((before_resolved, candidate_resolved))
    };
    let (before_resolved, candidate_resolved) = if let Some(limit) = max_candidate_bytes {
        let (result, overflowed) = crate::bounded_output::with_limit(limit, resolve_checked);
        if overflowed {
            return Err(vec![Diagnostic::io(
                "SPX-G140",
                format!("Semantic Target Evidence HIR identity construction exceeds {limit} bytes"),
            )]);
        }
        result?
    } else {
        resolve_checked()?
    };
    let source_graph_schema = graph::graph_schema(&before_resolved);
    if graph::graph_schema(&candidate_resolved) != source_graph_schema {
        return Err(vec![invariant_error(
            "semantic review base and candidate Graph schemas differ",
        )]);
    }
    prove_review_classifications(&preflight)?;
    let (sections, assessments) = if let Some(limits) = workspace_limits {
        let (rendered, overflowed) =
            crate::bounded_output::with_limit(limits.max_review_bytes, || {
                sections_json(preflight.operations())
            });
        if overflowed {
            return Err(vec![limit_error(
                "semantic review aggregate output budget is exhausted",
            )]);
        }
        rendered
    } else {
        sections_json(preflight.operations())
    };
    let source_digest = source_digest(preflight.source().as_bytes());
    let patch_digest = domain_digest(PATCH_DIGEST_DOMAIN, preflight.patch_source().as_bytes());
    let preliminary_usage = ReviewUsage {
        source_bytes: preflight.source().len(),
        patch_bytes: preflight.patch_source().len(),
        operations: preflight.operations().len(),
        declarations: ast_usage.declarations,
        callables: ast_usage.callables,
        call_sites: ast_usage.call_sites,
        impact_depth: 0,
        impact_nodes: 0,
        impact_bytes: 0,
    };
    let workspace_limits = if preflight.identity_rebase().is_none() {
        workspace_limits
            .map(|limits| {
                let envelope_input = ReviewRender {
                    preflight: &preflight,
                    source_graph_schema,
                    source_digest: &source_digest,
                    patch_digest: &patch_digest,
                    evidence: "{}",
                    sections: &sections,
                    usage: preliminary_usage,
                };
                let envelope = render_with_budget(&envelope_input, limits.max_review_bytes)?;
                // The Impact evidence wrapper has only fixed schema/id/digest
                // text plus three bounded decimal counters. 512 bytes is a
                // closed upper bound for that wrapper and all counter growth.
                let available = limits
                    .max_review_bytes
                    .saturating_sub(envelope.len())
                    .saturating_sub(512);
                Ok::<_, Vec<Diagnostic>>(WorkspaceEvidenceLimits {
                    max_impact_bytes: limits.max_impact_bytes.min(available),
                    ..limits
                })
            })
            .transpose()?
    } else {
        workspace_limits
    };
    let evidence = evidence_json(&preflight, workspace_limits)?;
    let usage = ReviewUsage {
        impact_depth: evidence.impact_depth,
        impact_nodes: evidence.impact_nodes,
        impact_bytes: evidence.impact_bytes,
        ..preliminary_usage
    };
    let report_input = ReviewRender {
        preflight: &preflight,
        source_graph_schema,
        source_digest: &source_digest,
        patch_digest: &patch_digest,
        evidence: &evidence.json,
        sections: &sections,
        usage,
    };
    let report = render_with_budget(
        &report_input,
        workspace_limits.map_or(MAX_OUTPUT_BYTES, |limits| limits.max_review_bytes),
    )?;
    Ok(ReviewBuild {
        before_resolved,
        candidate_resolved,
        report,
        source_graph_schema,
        base_revision: preflight.base_revision().to_owned(),
        candidate_revision: preflight.candidate_revision().to_owned(),
        source_digest,
        patch_schema: preflight.schema_label(),
        patch_digest,
        assessments,
        supporting_evidence: ReviewSupportingEvidence {
            kind: evidence.kind,
            schema: evidence.schema,
            digest: evidence.digest,
        },
        usage,
        preflight,
    })
}

pub(crate) fn read_patch_bounded(path: &Path) -> Result<String, Vec<Diagnostic>> {
    let file = File::open(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("cannot read {}: {error}", path.display()),
        )]
    })?;
    let metadata = file.metadata().map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("cannot inspect {}: {error}", path.display()),
        )]
    })?;
    if metadata.len() > MAX_PATCH_BYTES as u64 {
        return Err(vec![limit_error(format!(
            "semantic review patch exceeds {MAX_PATCH_BYTES} bytes"
        ))]);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((MAX_PATCH_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I202",
                format!("cannot read {}: {error}", path.display()),
            )]
        })?;
    if bytes.len() > MAX_PATCH_BYTES {
        return Err(vec![limit_error(format!(
            "semantic review patch exceeds {MAX_PATCH_BYTES} bytes"
        ))]);
    }
    String::from_utf8(bytes).map_err(|_| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("semantic review patch {} is not UTF-8", path.display()),
        )]
    })
}

fn precheck_program(program: &Program) -> Result<AstUsage, Vec<Diagnostic>> {
    let mut declarations = program.types.len() + program.interfaces.len() + program.functions.len();
    let mut callables = program.functions.len();
    for ty in &program.types {
        declarations += match &ty.kind {
            TypeDeclarationKind::Resource { lifecycles } => lifecycles.len(),
            TypeDeclarationKind::Record { fields } => fields.len(),
            TypeDeclarationKind::Variant { cases } => {
                cases.len() + cases.iter().map(|case| case.fields.len()).sum::<usize>()
            }
        };
    }
    for interface in &program.interfaces {
        declarations += interface.imports.len();
        callables += interface.imports.len();
    }
    if declarations > MAX_DECLARATIONS {
        return Err(vec![limit_error(format!(
            "semantic review program exceeds {MAX_DECLARATIONS} declarations"
        ))]);
    }
    if callables > MAX_CALLABLES {
        return Err(vec![limit_error(format!(
            "semantic review program exceeds {MAX_CALLABLES} authored callables"
        ))]);
    }
    let mut stack = Vec::<&Expr>::new();
    for function in &program.functions {
        stack.extend(function.requires.iter());
        stack.extend(function.ensures.iter());
        stack.push(&function.body);
    }
    let mut call_sites = 0usize;
    while let Some(expression) = stack.pop() {
        match &expression.kind {
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::Var(_) => {}
            ExprKind::Call { args, .. } => {
                call_sites += 1;
                if call_sites > MAX_CALL_SITES {
                    return Err(vec![limit_error(format!(
                        "semantic review program exceeds {MAX_CALL_SITES} call sites"
                    ))]);
                }
                stack.extend(args);
            }
            ExprKind::Unary { value, .. }
            | ExprKind::Try { operand: value }
            | ExprKind::Project { base: value, .. } => stack.push(value),
            ExprKind::Binary { left, right, .. } => {
                stack.push(right);
                stack.push(left);
            }
            ExprKind::Block { statements, tail } => {
                stack.push(tail);
                for statement in statements.iter().rev() {
                    stack.push(statement.value());
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                stack.push(else_branch);
                stack.push(then_branch);
                stack.push(condition);
            }
            ExprKind::ConstructRecord { fields, .. }
            | ExprKind::ConstructVariant { fields, .. }
            | ExprKind::UpdateRecord { fields, .. } => {
                if let ExprKind::UpdateRecord { base, .. } = &expression.kind {
                    stack.push(base);
                }
                stack.extend(fields.iter().map(|field| &field.value));
            }
            ExprKind::Match { scrutinee, arms } => {
                stack.push(scrutinee);
                stack.extend(arms.iter().map(|arm| &arm.value));
            }
        }
    }
    Ok(AstUsage {
        declarations,
        callables,
        call_sites,
    })
}

pub(crate) fn workspace_ast_counts(
    program: &Program,
) -> Result<(usize, usize, usize), Vec<Diagnostic>> {
    let usage = precheck_program(program)?;
    Ok((usage.declarations, usage.callables, usage.call_sites))
}

#[cfg(test)]
pub(crate) fn precheck_counts_for_test(
    program: &Program,
) -> Result<(usize, usize, usize), Vec<Diagnostic>> {
    let usage = precheck_program(program)?;
    Ok((usage.declarations, usage.callables, usage.call_sites))
}

fn evidence_json(
    preflight: &PatchPreflight,
    workspace_limits: Option<WorkspaceEvidenceLimits>,
) -> Result<Evidence, Vec<Diagnostic>> {
    if preflight.identity_rebase().is_some() {
        if let Some(limits) = workspace_limits {
            let (evidence, overflowed) =
                crate::bounded_output::with_limit(limits.max_review_bytes, || {
                    identity_rebase_evidence(preflight)
                });
            if overflowed {
                return Err(vec![limit_error(
                    "semantic review aggregate output budget is exhausted",
                )]);
            }
            return evidence;
        }
        return identity_rebase_evidence(preflight);
    }
    let impact = if let Some(limits) = workspace_limits {
        impact::complete_review_evidence_bounded(
            preflight,
            limits.max_impact_nodes,
            limits.max_impact_bytes,
        )?
    } else {
        impact::complete_review_evidence(preflight)?
    };
    if impact.report().len() > MAX_IMPACT_BYTES {
        return Err(vec![limit_error(format!(
            "semantic review Impact v1 evidence exceeds {MAX_IMPACT_BYTES} bytes"
        ))]);
    }
    let digest = domain_digest(IMPACT_DIGEST_DOMAIN, impact.report().as_bytes());
    Ok(Evidence {
        json: format!(
            "{{\"id\":\"{EVIDENCE_ID}\",\"kind\":\"semantic_impact_v1\",\"schema\":\"semaprax.semantic-impact.v1\",\"digest\":{},\"report\":{}}}",
            quote_json(&digest),
            impact.report()
        ),
        kind: "semantic_impact_v1",
        schema: "semaprax.semantic-impact.v1",
        digest,
        impact_depth: impact.used_depth(),
        impact_nodes: impact.used_nodes(),
        impact_bytes: impact.report().len(),
    })
}

fn identity_rebase_evidence(preflight: &PatchPreflight) -> Result<Evidence, Vec<Diagnostic>> {
    if let Some(rebase) = preflight.identity_rebase() {
        let callers = rebase
            .direct_callers()
            .iter()
            .map(|caller| {
                crate::bounded_output::budgeted_format(format_args!(
                    "{{\"id\":{},\"identity_origin\":{},\"site_count\":{}}}",
                    quote_json(caller.id()),
                    quote_json(caller.identity_origin().text()),
                    caller.site_count()
                ))
            })
            .collect::<Vec<_>>()
            .as_slice()
            .budgeted_join(",");
        let identity_rebase = crate::bounded_output::budgeted_format(format_args!(
            "{{\"before_id\":{},\"after_id\":{},\"name\":{},\"direct_callers\":[{}],\"derived_id_count\":{},\"derived_id_digest\":{}}}",
            quote_json(rebase.before_id()),
            quote_json(rebase.after_id()),
            quote_json(rebase.name()),
            callers,
            rebase.derived_id_count(),
            quote_json(rebase.derived_id_digest()),
        ));
        let digest = domain_digest(IDENTITY_REBASE_DIGEST_DOMAIN, identity_rebase.as_bytes());
        return Ok(Evidence {
            json: crate::bounded_output::budgeted_format(format_args!(
                "{{\"id\":\"{EVIDENCE_ID}\",\"kind\":\"identity_rebase_v1\",\"schema\":\"{IDENTITY_REBASE_SCHEMA}\",\"digest\":{},\"identity_rebase\":{identity_rebase}}}",
                quote_json(&digest)
            )),
            kind: "identity_rebase_v1",
            schema: IDENTITY_REBASE_SCHEMA,
            digest,
            impact_depth: 0,
            impact_nodes: 0,
            impact_bytes: 0,
        });
    }
    Err(vec![invariant_error(
        "identity-rebase evidence was requested without a typed rebase proof",
    )])
}

fn prove_review_classifications(preflight: &PatchPreflight) -> Result<(), Vec<Diagnostic>> {
    if preflight.identity_rebase().is_some() {
        return Ok(());
    }
    if preflight.operations().iter().any(|operation| {
        matches!(
            operation,
            PreflightOperation::Rename { .. }
                | PreflightOperation::RenameMember { .. }
                | PreflightOperation::RenameCase { .. }
        )
    }) {
        prove_rename_graph_delta(preflight)?;
    }
    if security_facts(preflight.before()) != security_facts(preflight.candidate()) {
        return Err(vec![invariant_error(
            "semantic review security facts exceed the admitted patch delta",
        )]);
    }
    Ok(())
}

fn prove_rename_graph_delta(preflight: &PatchPreflight) -> Result<(), Vec<Diagnostic>> {
    let mut patch_source = String::new();
    if preflight.schema_label() == "semaprax.semantic-patch.v2" {
        patch_source.push_str("schema semaprax.semantic-patch.v2\n");
    }
    patch_source.push_str(&format!("base {}\n", preflight.base_revision()));
    for operation in preflight.operations() {
        match operation {
            PreflightOperation::Rename { target, to, .. } => {
                patch_source.push_str(&format!("rename {target} to {to}\n"));
            }
            PreflightOperation::RenameMember {
                owner, member, to, ..
            } => patch_source.push_str(&format!(
                "rename-member owner {owner} member {member} to {to}\n"
            )),
            PreflightOperation::RenameCase {
                owner, case, to, ..
            } => patch_source.push_str(&format!("rename-case owner {owner} case {case} to {to}\n")),
            PreflightOperation::AssignFunctionId { .. }
            | PreflightOperation::ReplaceCallTypeArgument { .. }
            | PreflightOperation::RequireNoNewEffects { .. } => {}
        }
    }
    let rename = patch::preflight_review_owned(
        preflight.source().to_owned(),
        patch_source,
        Path::new("semantic-review-rename-proof.spx").to_path_buf(),
        MAX_OPERATIONS,
    )?;
    let rename_before = hir::resolve(rename.before())?;
    let rename_candidate = hir::resolve(rename.candidate())?;
    hir::validate(&rename_before).map_err(|error| vec![error])?;
    hir::validate(&rename_candidate).map_err(|error| vec![error])?;
    let before_json = graph::to_json(rename.before())?;
    let candidate_json = graph::to_json(rename.candidate())?;
    let before: serde_json::Value = serde_json::from_str(&before_json).map_err(|_| {
        vec![invariant_error(
            "semantic review base Graph is not canonical JSON",
        )]
    })?;
    let mut candidate: serde_json::Value = serde_json::from_str(&candidate_json).map_err(|_| {
        vec![invariant_error(
            "semantic review candidate Graph is not canonical JSON",
        )]
    })?;
    candidate["revision"] = before["revision"].clone();
    let names = rename
        .changes()
        .iter()
        .filter_map(|change| match change {
            PreflightChange::Rename {
                target,
                before,
                after,
                ..
            } => Some((target, before, after)),
            PreflightChange::CallInstance { .. } => None,
        })
        .collect::<Vec<_>>();
    let nodes = candidate["nodes"].as_array_mut().ok_or_else(|| {
        vec![invariant_error(
            "semantic review candidate Graph has no canonical node array",
        )]
    })?;
    for (target, before_name, after_name) in names {
        let node = nodes
            .iter_mut()
            .find(|node| node["id"].as_str() == Some(target.as_str()))
            .ok_or_else(|| {
                vec![invariant_error(format!(
                    "semantic review rename target `{target}` is absent from candidate Graph"
                ))]
            })?;
        if node["name"].as_str() != Some(after_name.as_str()) {
            return Err(vec![invariant_error(format!(
                "semantic review rename target `{target}` has an unexpected candidate name"
            ))]);
        }
        node["name"] = serde_json::Value::String(before_name.clone());
    }
    if before != candidate {
        return Err(vec![invariant_error(
            "semantic review rename exceeds normalized Graph/Cleanup name projection",
        )]);
    }
    Ok(())
}

fn security_facts(program: &Program) -> String {
    let mut facts = Vec::new();
    facts.push(format!("permits:{:?}", program.permits));
    for function in &program.functions {
        facts.push(format!(
            "function:{}:effects:{:?}",
            function.stable_id, function.effects
        ));
    }
    for interface in &program.interfaces {
        facts.push(format!(
            "interface:{}:permits:{:?}",
            interface.stable_id, interface.permits
        ));
        for import in &interface.imports {
            facts.push(format!(
                "import:{}:effects:{:?}:failure:{:?}:consumes:{}",
                import.stable_id, import.effects, import.failure, import.consumes
            ));
        }
    }
    facts.join("\0")
}

fn sections_json(operations: &[PreflightOperation]) -> (String, Vec<ReviewAssessment>) {
    let rendered = SectionKind::ALL
        .into_iter()
        .map(|section| section_json(section, operations))
        .collect::<Vec<_>>();
    (
        rendered
            .iter()
            .map(|(json, _)| json.as_str())
            .collect::<Vec<_>>()
            .as_slice()
            .budgeted_join(","),
        rendered
            .into_iter()
            .map(|(_, assessment)| assessment)
            .collect(),
    )
}

fn section_json(
    section: SectionKind,
    operations: &[PreflightOperation],
) -> (String, ReviewAssessment) {
    let findings = operations
        .iter()
        .map(|operation| finding(section, operation))
        .collect::<Vec<_>>();
    let assessment = assessment(&findings);
    let findings_json = findings
        .iter()
        .map(|finding| {
            format!(
                "{{\"code\":{},\"disposition\":{},\"statement\":{},\"operation_indices\":[{}],\"evidence_ids\":[\"{EVIDENCE_ID}\"]}}",
                quote_json(finding.code),
                quote_json(finding.disposition.text()),
                quote_json(&finding.statement),
                finding.operation_index,
            )
        })
        .collect::<Vec<_>>()
        .as_slice()
        .budgeted_join(",");
    (
        format!(
            "{}:{{\"kind\":{},\"assessment\":{},\"findings\":[{}]}}",
            quote_json(section.text()),
            quote_json(section.text()),
            quote_json(assessment),
            findings_json
        ),
        ReviewAssessment {
            key: section.text(),
            value: assessment,
        },
    )
}

fn assessment(findings: &[Finding]) -> &'static str {
    let dispositions = findings
        .iter()
        .map(|finding| finding.disposition)
        .collect::<BTreeSet<_>>();
    if dispositions.len() != 1 {
        return "mixed";
    }
    match dispositions.first().copied() {
        Some(Disposition::Change) | Some(Disposition::MigrationRequired) => "change_proven",
        Some(Disposition::BoundedNoChange) => "unchanged_within_admitted_domain",
        Some(Disposition::Unknown) => "unknown",
        Some(Disposition::NotApplicable) | None => "not_applicable",
    }
}

fn finding(section: SectionKind, operation: &PreflightOperation) -> Finding {
    let index = operation_index(operation);
    let (family, subject) = operation_subject(operation);
    let (code, disposition, statement) = match (family, section) {
        ("rename", SectionKind::Behavior) => ("SRV-B101", Disposition::BoundedNoChange, format!("Operation {index} changes only the source name projection of {subject}; both projections pass checked HIR, and the name-normalized Graph including its emitted Cleanup projection is unchanged.")),
        ("rename", SectionKind::ApiIdentity) => ("SRV-A101", Disposition::Change, format!("Operation {index} changes the declared name of {subject} while preserving its stable identity.")),
        ("rename", SectionKind::SecurityAuthority) => ("SRV-S101", Disposition::BoundedNoChange, format!("Operation {index} leaves exact effects, capabilities, and imports unchanged.")),
        ("rename", SectionKind::MemoryOwnership) => ("SRV-M101", Disposition::BoundedNoChange, format!("Operation {index} leaves checked ownership and Cleanup unchanged.")),
        ("rename", SectionKind::TargetArtifact) => ("SRV-T101", Disposition::Unknown, format!("Operation {index} requires rebuilding; target artifact byte identity is not established.")),
        ("rename", SectionKind::Migration) => ("SRV-G101", Disposition::Unknown, format!("Operation {index} may affect external name consumers, which are outside this review.")),
        ("rename", SectionKind::Unsafe) => ("SRV-U101", Disposition::Unknown, format!("Operation {index} introduces no source unsafe construct; generated, native, and external unsafe behavior is not analyzed.")),
        ("call_instance", SectionKind::Behavior) => ("SRV-B102", Disposition::Change, format!("Operation {index} changes the selected generic call instance for {subject}.")),
        ("call_instance", SectionKind::ApiIdentity) => ("SRV-A102", Disposition::BoundedNoChange, format!("Operation {index} leaves declaration identities and declared API names unchanged.")),
        ("call_instance", SectionKind::SecurityAuthority) => ("SRV-S102", Disposition::BoundedNoChange, format!("Operation {index} leaves exact effects, capabilities, and imports unchanged.")),
        ("call_instance", SectionKind::MemoryOwnership) => ("SRV-M102", Disposition::BoundedNoChange, format!("Operation {index} changes only admitted scalar Copy type arguments; ownership, Cleanup, and effects remain within the checked delta.")),
        ("call_instance", SectionKind::TargetArtifact) => ("SRV-T102", Disposition::Change, format!("Operation {index} changes the function instance and its backend symbol/artifact identity.")),
        ("call_instance", SectionKind::Migration) => ("SRV-G102", Disposition::Unknown, format!("Operation {index} may invalidate external caches or tests, which are not executed.")),
        ("call_instance", SectionKind::Unsafe) => ("SRV-U102", Disposition::Unknown, format!("Operation {index} introduces no source unsafe construct; generated, native, and external unsafe behavior is not analyzed.")),
        ("requirement", SectionKind::SecurityAuthority) => ("SRV-S103", Disposition::BoundedNoChange, format!("Operation {index} is a verified no-new-effects policy requirement and makes no source edit.")),
        ("requirement", SectionKind::Unsafe) => ("SRV-U103", Disposition::Unknown, format!("Operation {index} does not establish generated, native, or external unsafe behavior.")),
        ("requirement", _) => ("SRV-P103", Disposition::NotApplicable, format!("Operation {index} is a policy requirement and makes no change in this section.")),
        ("identity_rebase", SectionKind::Behavior) => ("SRV-B104", Disposition::BoundedNoChange, format!("Operation {index} preserves runtime behavior within the admitted monomorphic scalar Graph-v10 structural rebase proof.")),
        ("identity_rebase", SectionKind::ApiIdentity) => ("SRV-A104", Disposition::Change, format!("Operation {index} rebases automatic function identity {subject} to a supplied persistent identity.")),
        ("identity_rebase", SectionKind::SecurityAuthority) => ("SRV-S104", Disposition::BoundedNoChange, format!("Operation {index} leaves exact effects, capabilities, and imports unchanged.")),
        ("identity_rebase", SectionKind::MemoryOwnership) => ("SRV-M104", Disposition::Change, format!("Operation {index} rebases Graph, derived IDs, callee references, and identity-bearing Cleanup facts while preserving scalar runtime execution.")),
        ("identity_rebase", SectionKind::TargetArtifact) => ("SRV-T104", Disposition::Change, format!("Operation {index} changes backend symbol and artifact identity; artifact bytes are not compared.")),
        ("identity_rebase", SectionKind::Migration) => ("SRV-G104", Disposition::MigrationRequired, format!("Operation {index} requires consumers of the automatic identity to migrate to the persistent identity.")),
        ("identity_rebase", SectionKind::Unsafe) => ("SRV-U104", Disposition::Unknown, format!("Operation {index} introduces no source unsafe construct; generated, native, and external unsafe behavior is not analyzed.")),
        _ => unreachable!("closed review operation and section table"),
    };
    Finding {
        code,
        disposition,
        statement,
        operation_index: index,
    }
}

fn operation_index(operation: &PreflightOperation) -> usize {
    match operation {
        PreflightOperation::AssignFunctionId { index, .. }
        | PreflightOperation::Rename { index, .. }
        | PreflightOperation::RenameMember { index, .. }
        | PreflightOperation::RenameCase { index, .. }
        | PreflightOperation::ReplaceCallTypeArgument { index, .. }
        | PreflightOperation::RequireNoNewEffects { index } => *index,
    }
}

fn operation_subject(operation: &PreflightOperation) -> (&'static str, String) {
    match operation {
        PreflightOperation::AssignFunctionId { target, to, .. } => {
            ("identity_rebase", format!("`{target}` as `{to}`"))
        }
        PreflightOperation::Rename { target, to, .. } => {
            ("rename", format!("`{target}` to `{to}`"))
        }
        PreflightOperation::RenameMember {
            owner, member, to, ..
        } => (
            "rename",
            format!("member `{member}` of `{owner}` to `{to}`"),
        ),
        PreflightOperation::RenameCase {
            owner, case, to, ..
        } => ("rename", format!("case `{case}` of `{owner}` to `{to}`")),
        PreflightOperation::ReplaceCallTypeArgument { expression, .. } => {
            ("call_instance", format!("call `{expression}`"))
        }
        PreflightOperation::RequireNoNewEffects { .. } => {
            ("requirement", "`no-new-effects`".to_owned())
        }
    }
}

fn render_with_budget(
    input: &ReviewRender<'_>,
    max_output_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    let mut used_output_bytes = 0usize;
    for _ in 0..4 {
        let (output, overflowed) = crate::bounded_output::with_limit(max_output_bytes, || {
            render_report(input, used_output_bytes)
        });
        if overflowed || output.len() > max_output_bytes {
            return Err(vec![limit_error(
                "semantic review aggregate output budget is exhausted",
            )]);
        }
        if output.len() == used_output_bytes {
            if output.len() > MAX_OUTPUT_BYTES {
                return Err(vec![limit_error(format!(
                    "semantic review output exceeds {MAX_OUTPUT_BYTES} bytes"
                ))]);
            }
            return Ok(output);
        }
        used_output_bytes = output.len();
    }
    Err(vec![invariant_error(
        "semantic review output byte accounting did not converge",
    )])
}

struct ReviewRender<'a> {
    preflight: &'a PatchPreflight,
    source_graph_schema: &'a str,
    source_digest: &'a str,
    patch_digest: &'a str,
    evidence: &'a str,
    sections: &'a str,
    usage: ReviewUsage,
}

fn render_report(input: &ReviewRender<'_>, used_output_bytes: usize) -> String {
    let preflight = input.preflight;
    let usage = input.usage;
    crate::bounded_output::budgeted_format(format_args!(
        "{{\"schema\":\"{REVIEW_SCHEMA}\",\"source_graph_schema\":{},\"base_revision\":{},\"candidate_revision\":{},\"source\":{{\"digest\":{}}},\"patch\":{{\"schema\":{},\"digest\":{}}},\"limits\":{{\"max_source_bytes\":{MAX_SOURCE_BYTES},\"max_patch_bytes\":{MAX_PATCH_BYTES},\"max_operations\":{MAX_OPERATIONS},\"max_declarations\":{MAX_DECLARATIONS},\"max_callables\":{MAX_CALLABLES},\"max_call_sites\":{MAX_CALL_SITES},\"max_impact_depth\":{MAX_IMPACT_DEPTH},\"max_impact_nodes\":{MAX_IMPACT_NODES},\"max_impact_bytes\":{MAX_IMPACT_BYTES},\"max_output_bytes\":{MAX_OUTPUT_BYTES}}},\"budget\":{{\"used_source_bytes\":{},\"used_patch_bytes\":{},\"used_operations\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_impact_depth\":{},\"used_impact_nodes\":{},\"used_impact_bytes\":{},\"used_output_bytes\":{used_output_bytes}}},\"sections\":{{{}}},\"evidence\":{},\"nonclaims\":[\"not_proof_carrying_patch\",\"no_authenticated_provenance_or_signature\",\"no_human_approval_ui_or_policy\",\"no_public_verify_api_or_proof_artifact\",\"no_lock_stage_apply_or_commit_authority\",\"no_repository_or_multi_file_analysis\",\"no_agent_context_generation_or_embedding\",\"no_test_or_target_execution\",\"no_general_capability_security_unsafe_or_abi_analysis\",\"no_semantic_impact_v3\",\"no_persistence_or_incrementality\",\"no_external_consumer_compatibility\"]}}",
        quote_json(input.source_graph_schema),
        quote_json(preflight.base_revision()),
        quote_json(preflight.candidate_revision()),
        quote_json(input.source_digest),
        quote_json(preflight.schema_label()),
        quote_json(input.patch_digest),
        usage.source_bytes,
        usage.patch_bytes,
        usage.operations,
        usage.declarations,
        usage.callables,
        usage.call_sites,
        usage.impact_depth,
        usage.impact_nodes,
        usage.impact_bytes,
        input.sections,
        input.evidence,
    ))
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn limit_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G120", message)
}

fn invariant_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G121", message)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn fixture(
        source: &str,
        patch_source: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-review-unit-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        let patch_path = directory.join("change.spatch");
        std::fs::write(&source_path, source).unwrap();
        std::fs::write(&patch_path, patch_source).unwrap();
        (directory, source_path, patch_path)
    }

    #[test]
    fn final_source_drift_is_rejected() {
        let source = "module review.unit;\n@id(\"review.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let program = parse(source, Path::new("review.spx")).unwrap();
        let patch_source = format!(
            "base {}\nrename review.helper to renamed\n",
            graph::revision(&program)
        );
        let (directory, source_path, patch_path) = fixture(source, &patch_source);
        let error = preview_with_hook(&source_path, &patch_path, |canonical, _| {
            std::fs::write(canonical, source.replace("{1}", "{2}"))
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn oversized_patch_is_rejected_without_reading_past_bound() {
        let source = "module review.unit;\n@id(\"app.main\") fn main()->i64{1}\n";
        let (directory, source_path, patch_path) = fixture(source, "");
        std::fs::write(&patch_path, vec![b'x'; MAX_PATCH_BYTES + 1]).unwrap();
        let error = preview(&source_path, &patch_path).unwrap_err();
        assert_eq!(error[0].code, "SPX-G120");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn oversized_source_is_rejected_by_the_initial_bounded_snapshot() {
        let (directory, path, patch_path) = fixture("", "");
        std::fs::write(&path, vec![b'x'; MAX_SOURCE_BYTES + 1]).unwrap();
        let error = preview(&path, &patch_path).unwrap_err();
        assert_eq!(error[0].code, "SPX-G120");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn final_source_growth_is_rejected_by_the_same_hard_bound() {
        let source = "module review.unit;\n@id(\"review.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let program = parse(source, Path::new("review.spx")).unwrap();
        let patch_source = format!(
            "base {}\nrename review.helper to renamed\n",
            graph::revision(&program)
        );
        let (directory, path, patch_path) = fixture(source, &patch_source);
        let error = preview_with_hook(&path, &patch_path, |canonical, _| {
            std::fs::write(canonical, vec![b'x'; MAX_SOURCE_BYTES + 1])
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn late_workspace_v3_child_caps_many_caller_identity_serialization() {
        let mut source =
            String::from("module review.rebase_budget;\nfn helper(value:i64)->i64{value+1}\n");
        for index in 0..128 {
            source.push_str(&format!(
                "@id(\"review.rebase_budget.c{index}\") fn c{index}(value:i64)->i64{{helper(value)}}\n"
            ));
        }
        source.push_str("@id(\"app.main\") fn main()->i64{c0(41)}\n");
        let (directory, source_path, patch_path) = fixture(&source, "");
        let query = crate::repair::DiagnosticRepairQuery::assign_function_id(
            "auto:review.rebase_budget.helper",
        )
        .unwrap();
        let repairs: serde_json::Value =
            serde_json::from_str(&crate::repair::query(&source_path, &query).unwrap()).unwrap();
        let instantiated: serde_json::Value = serde_json::from_str(
            &crate::repair::instantiate(
                &source_path,
                repairs["repair"]["id"].as_str().unwrap(),
                &crate::repair::PersistentDeclarationId::new("review.rebase_budget.helper")
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let patch_source = instantiated["patch"]["source"].as_str().unwrap();
        std::fs::write(&patch_path, patch_source).unwrap();
        let preflight = patch::preflight_review_owned(
            source,
            patch_source.to_owned(),
            source_path,
            MAX_OPERATIONS,
        )
        .unwrap();
        let error = evidence_json(
            &preflight,
            Some(WorkspaceEvidenceLimits {
                max_impact_nodes: MAX_IMPACT_NODES,
                max_impact_bytes: MAX_IMPACT_BYTES,
                max_review_bytes: 128,
            }),
        )
        .err()
        .expect("late child identity evidence must respect remaining bytes");
        assert_eq!(error[0].code, "SPX-G120");
        std::fs::remove_dir_all(directory).unwrap();
    }
}
