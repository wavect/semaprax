//! LAW-07 exact Project attachment and real installed backend replay.
use semaprax::{
    agent_runtime::AgentCancellation,
    assurance_manifest::{
        project::{derive_with_verified_proofs, ProjectAssuranceOptions},
        smt_discharge::{replay_function, Model, ModelValue, ReplayOutcome},
        structured_law::installed::{Backend, Certificate, LEAN_PROFILE, SMT_PROFILE},
        structured_law::{lower, lower_aggregate_clause, Refusal},
    },
    project::{with_authenticated_project, ProjectRevision},
    proof_export::{
        installed::{HostProfile, InstalledProofTool, Limits, ToolKind},
        installed_project::{prove_structured_postcondition, replay_structured_postcondition},
    },
};
use std::{path::PathBuf, sync::Arc};

const SOURCE: &str = include_str!("../fixtures/law07-accounting.spx");
const TOTAL_AFTER: &str = r#"
@id("law07.total-after")
fn total_after(debit: i64, credit: i64, amount: i64) -> i64
    requires debit >= 0
    requires debit <= 1000
    requires credit >= 0
    requires credit <= 1000
    requires amount >= 0
    requires amount <= debit
    ensures result == debit + credit
{
    let before = Accounts {
        debit: Account { balance: debit },
        credit: Account { balance: credit },
    };
    let after = Accounts {
        debit: Account { balance: before.debit.balance - amount },
        credit: Account { balance: before.credit.balance + amount },
    };
    after.debit.balance + after.credit.balance
}
"#;
const OUTCOME_TOTAL: &str = r#"
@id("law07.outcome-total")
fn outcome_total(code: i64) -> i64
    requires code >= 0
    requires code <= 10
    ensures result == 0
{
    let outcome = Outcome::Failure { code: code };
    match outcome {
        Outcome::Success { credited } => credited - credited,
        Outcome::Failure { code: observed } => observed - observed,
    }
}
"#;

fn project_source() -> String {
    let types = SOURCE.split_once("@id(\"law07.sum\")").unwrap().0;
    format!(
        "{types}\n{TOTAL_AFTER}\n{OUTCOME_TOTAL}\n@id(\"law07.main\") fn main() -> i64 {{ total_after(1, 2, 0) + outcome_total(0) }}\n"
    )
}

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new(label: &str, source: &str) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-structured-law-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let canonical =
            semaprax::format::canonical(&semaprax::parse(source, "src/app.spx").unwrap());
        std::fs::write(root.join("src/app.spx"), canonical).unwrap();
        let tests = "module law07.tests; @id(\"law07.tests.main\") fn main() -> i64 { 0 }";
        std::fs::write(
            root.join("src/tests.spx"),
            semaprax::format::canonical(&semaprax::parse(tests, "src/tests.spx").unwrap()),
        )
        .unwrap();
        std::fs::write(root.join("semaprax.toml"), "schema = \"semaprax.project.v8\"\nname = \"law07-structured\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"law07.accounting\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = []\ntests = [\"law07.tests\"]\n").unwrap();
        Self { root }
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
    fn initialize_workspace(&self) -> String {
        let paths = self.root.join("paths.json");
        std::fs::write(&paths, "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"src/app.spx\"},{\"path\":\"src/tests.spx\"}]}\n").unwrap();
        semaprax::semantic_workspace::initialize(&self.root, &paths).unwrap()
    }
    fn tool(&self, kind: ToolKind) -> InstalledProofTool {
        let prefix = if kind == ToolKind::Lean {
            "SEMAPRAX_LAW_LEAN"
        } else {
            "SEMAPRAX_LAW_Z3"
        };
        let path = PathBuf::from(std::env::var(prefix).expect("explicit installed tool path"));
        let version =
            std::env::var(format!("{prefix}_VERSION")).expect("exact installed tool version");
        InstalledProofTool::open(
            &path,
            &self.root,
            kind,
            &version,
            HostProfile::TrustedLocal,
            Limits::default(),
            AgentCancellation::new(),
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn tiny_transfer_reference_model_matches_every_scalarized_clause() {
    let program = semaprax::check(SOURCE, "law07-reference.spx").unwrap();
    let transfer = program
        .functions
        .iter()
        .find(|row| row.stable_id == "law07.transfer")
        .unwrap();
    let lowered = (0..transfer.ensures.len())
        .map(|index| lower_aggregate_clause(&program, transfer, index).unwrap())
        .collect::<Vec<_>>();
    let mut states = 0;
    for debit in 0..=6_i128 {
        for credit in 0..=6_i128 {
            for amount in 0..=debit {
                // Independent finite reference semantics for the source
                // constructor, not an SMT encoding or solver transcript.
                let after_debit = debit - amount;
                let after_credit = credit + amount;
                assert_eq!(after_debit + after_credit, debit + credit);
                assert!(after_debit >= 0 && after_credit >= credit);
                for clause in &lowered {
                    let model = clause
                        .leaves
                        .iter()
                        .map(|leaf| {
                            let value = if leaf.field_path.contains(&"law07.accounts.debit".into())
                            {
                                debit
                            } else if leaf.field_path.contains(&"law07.accounts.credit".into()) {
                                credit
                            } else {
                                amount
                            };
                            (leaf.parameter.clone(), ModelValue::Int(value))
                        })
                        .collect::<Model>();
                    let result = replay_function(&clause.scalar, &model).unwrap();
                    assert!(
                        matches!(result, ReplayOutcome::Inconsistent { .. }),
                        "{result:?}"
                    );
                }
                states += 1;
            }
        }
    }
    assert_eq!(states, 196);
}

#[test]
fn recursive_mutable_and_borrowed_aggregate_subjects_refuse_before_proof() {
    use semaprax::ast::{Expr, ExprKind, ParamMode, Statement, Type, TypeDeclarationKind};
    let mut program = semaprax::check(SOURCE, "law07-refusal.spx").unwrap();
    let sum = program
        .functions
        .iter()
        .find(|row| row.stable_id == "law07.sum")
        .unwrap()
        .clone();

    let account = program
        .types
        .iter_mut()
        .find(|row| row.stable_id == "law07.account")
        .unwrap();
    let TypeDeclarationKind::Record { fields } = &mut account.kind else {
        panic!("record")
    };
    fields[0].ty = Type::Named {
        name: "Account".into(),
        arguments: vec![],
    };
    assert!(matches!(
        lower(&program, &sum),
        Err(Refusal::RecursiveType(_))
    ));

    let program = semaprax::check(SOURCE, "law07-refusal.spx").unwrap();
    let mut borrowed = sum.clone();
    borrowed.params[0].mode = ParamMode::Borrow;
    assert_eq!(lower(&program, &borrowed).unwrap_err(), Refusal::Ownership);

    let mut mutable = sum;
    let span = mutable.body.span;
    let tail = mutable.body.clone();
    mutable.body = Expr {
        span,
        kind: ExprKind::Block {
            statements: vec![Statement::Let {
                name: "scratch".into(),
                name_span: span,
                mutable: true,
                declared: Some(Type::I64),
                value: Expr {
                    span,
                    kind: ExprKind::Int(0),
                },
                span,
            }],
            tail: Box::new(tail),
        },
    };
    assert!(matches!(
        lower(&program, &mutable),
        Err(Refusal::UnsupportedExpression(
            "mutable or non-let statement"
        ))
    ));
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn installed_structured_project_z3_certificate_replays_and_refuses_branch_or_field_drift() {
    use semaprax::assurance_manifest::law_set::{
        strict::{self, RequiredLawEvidence, StrictLawPolicy},
        ContractKind, EvidenceRequirement, LawDefinition, LawModule, LawSelector, LawSet,
    };
    use std::collections::BTreeMap;
    let source = project_source();
    let project = Fixture::new("z3", &source);
    let workspace_revision = project.initialize_workspace();
    let revision = project.revision();
    assert_eq!(revision.workspace_revision(), workspace_revision);
    let tool = project.tool(ToolKind::Z3);
    let (certificate, proof) =
        prove_structured_postcondition(&revision, "src/app.spx", "law07.total-after", 0, &tool)
            .expect("installed Z3 proves aggregate-body conservation");
    let row: Certificate = serde_json::from_str(&certificate).unwrap();
    assert_eq!(row.backend, Backend::Z3);
    assert_eq!(row.profile, SMT_PROFILE);
    assert!(row
        .dependency_ids
        .iter()
        .any(|id| id == "law07.accounts.debit"));
    assert!(row
        .backend_coverage
        .iter()
        .any(|kind| kind == "checked_arithmetic"));
    let report = derive_with_verified_proofs(
        &revision,
        &ProjectAssuranceOptions::default(),
        &[proof.clone()],
    )
    .unwrap();
    assert!(report.contains(SMT_PROFILE));
    let laws = LawSet::derive(
        &revision,
        "structured-v1",
        vec![LawModule {
            module_id: "law07.laws".into(),
            source_path: "src/app.spx".into(),
            assumptions: vec![],
            laws: vec![LawDefinition {
                law_id: "law07.total-after.law".into(),
                selector: LawSelector::Contract {
                    declaration_id: "law07.total-after".into(),
                    clause: ContractKind::Postcondition,
                    proposition: "result == debit + credit".into(),
                },
                assumption_ids: vec![],
                requires_laws: vec![],
                evidence: EvidenceRequirement::SmtProved,
            }],
        }],
    )
    .unwrap();
    let structured = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            "law07.total-after.law".into(),
            RequiredLawEvidence::PinnedStructuredSmtSource {
                toolchain: tool.expected_version().into(),
                accepted_translation: SMT_PROFILE.into(),
            },
        )]),
    )
    .unwrap();
    let accepted = strict::derive(&revision, &laws, &structured, &[proof.clone()]).unwrap();
    strict::require(&accepted, &revision, &laws, &structured, &[proof.clone()]).unwrap();
    let scalar_only = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            "law07.total-after.law".into(),
            RequiredLawEvidence::PinnedSmtSource {
                toolchain: tool.expected_version().into(),
                accepted_translation: semaprax::assurance_manifest::smt_discharge::BOUNDS_V1.into(),
            },
        )]),
    )
    .unwrap();
    let refused = strict::derive(&revision, &laws, &scalar_only, &[proof.clone()]).unwrap();
    assert!(strict::require(&refused, &revision, &laws, &scalar_only, &[proof]).is_err());
    replay_structured_postcondition(&certificate, &revision, &tool)
        .expect("same exact Project and installed Z3 replay");

    let mut modified: Certificate = serde_json::from_str(&certificate).unwrap();
    modified.field_bindings[0]
        .field_path
        .push("law07.wrong-field".into());
    let altered = serde_json::to_string(&modified).unwrap();
    assert!(replay_structured_postcondition(&altered, &revision, &tool).is_err());

    let (branch_certificate, _) =
        prove_structured_postcondition(&revision, "src/app.spx", "law07.outcome-total", 0, &tool)
            .expect("installed Z3 covers the explicit two-case match");
    let branch = source
        .replace("@id(\"law07.outcome.failure\") Failure {", "@id(\"law07.outcome.other\") Other { @id(\"law07.outcome.other.code\") code: i64, },\n    @id(\"law07.outcome.failure\") Failure {")
        .replace("Outcome::Failure { code: observed } => observed - observed,", "Outcome::Failure { code: observed } => observed - observed,\n        Outcome::Other { code: extra } => extra - extra,");
    let changed = Fixture::new("branch-added", &branch);
    let changed_revision = changed.revision();
    assert!(
        replay_structured_postcondition(&branch_certificate, &changed_revision, &tool).is_err(),
        "cached proof cannot cover a new variant branch"
    );
}

#[test]
#[ignore = "requires explicitly provisioned installed Lean"]
fn installed_structured_project_lean_covers_record_and_refuses_wrong_source() {
    let source = project_source();
    let project = Fixture::new("lean", &source);
    let revision = project.revision();
    let tool = project.tool(ToolKind::Lean);
    let (certificate, proof) =
        prove_structured_postcondition(&revision, "src/app.spx", "law07.total-after", 0, &tool)
            .expect("installed pinned Lean proves exact record conservation");
    let row: Certificate = serde_json::from_str(&certificate).unwrap();
    assert_eq!(row.backend, Backend::Lean);
    assert_eq!(row.profile, LEAN_PROFILE);
    assert!(row
        .backend_coverage
        .iter()
        .any(|kind| kind == "all_declaration_range_obligations"));
    let report =
        derive_with_verified_proofs(&revision, &ProjectAssuranceOptions::default(), &[proof])
            .unwrap();
    assert!(report.contains(LEAN_PROFILE));
    replay_structured_postcondition(&certificate, &revision, &tool)
        .expect("same exact Project and installed Lean replay");

    let changed = Fixture::new(
        "lean-drift",
        &source.replace("result == debit + credit", "result == debit - credit"),
    );
    let changed_revision = changed.revision();
    assert!(replay_structured_postcondition(&certificate, &changed_revision, &tool).is_err());
}

#[test]
fn selected_project_public_scalar_export_refuses_authored_aggregate_inventory() {
    let project = Fixture::new("selected-refusal", &project_source());
    let native = r#"module law07.laws;
@id("law07.total-after.law")
law contract "law07.total-after" ensures (debit: i64, credit: i64, amount: i64, result: i64)
    result == debit + credit
    evidence smt_proved;
"#;
    let law = semaprax::native_law_source::parse(native, "src/contracts.spx").unwrap();
    std::fs::write(
        project.root.join("src/contracts.spx"),
        semaprax::native_law_source::canonical(&law),
    )
    .unwrap();
    std::fs::write(project.root.join("semaprax.toml"), "schema = \"semaprax.manifest.v2\"\n\n[package]\nname = \"law07-selected\"\nversion = \"1.0.0\"\n\n[modules]\nentry = \"law07.accounting\"\nsources = [\"src/app.spx\", \"src/contracts.spx\", \"src/tests.spx\"]\nlaw_sources = [\"src/contracts.spx\"]\ntests = [\"law07.tests\"]\n\n[exports]\nweb = [\"law07.total-after\"]\n").unwrap();
    let error = with_authenticated_project(&project.root.join("semaprax.toml"), |_snapshot| Ok(()))
        .expect_err("selected public export profile has no authored aggregate carrier");
    assert_eq!(error[0].code, "SPX-W115");
}
