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

    let reordered = source.replace(
        "    @id(\"law07.accounts.debit\") debit: Account,\n    @id(\"law07.accounts.credit\") credit: Account,",
        "    @id(\"law07.accounts.credit\") credit: Account,\n    @id(\"law07.accounts.debit\") debit: Account,",
    );
    assert_ne!(reordered, source);
    let reordered_project = Fixture::new("field-reordered", &reordered);
    reordered_project.initialize_workspace();
    assert!(
        replay_structured_postcondition(&certificate, &reordered_project.revision(), &tool)
            .is_err(),
        "source-bound proof cannot retarget after record field reordering"
    );

    let (branch_certificate, _) =
        prove_structured_postcondition(&revision, "src/app.spx", "law07.outcome-total", 0, &tool)
            .expect("installed Z3 covers the explicit two-case match");
    let branch = source
        .replace("@id(\"law07.outcome.failure\") Failure {", "@id(\"law07.outcome.other\") Other { @id(\"law07.outcome.other.code\") code: i64, },\n    @id(\"law07.outcome.failure\") Failure {")
        .replace("Outcome::Failure { code: observed } => observed - observed,", "Outcome::Failure { code: observed } => observed - observed,\n        Outcome::Other { code: extra } => extra - extra,");
    let changed = Fixture::new("branch-added", &branch);
    changed.initialize_workspace();
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
#[cfg(unix)]
#[ignore = "requires explicitly provisioned installed Z3 and private Unix cache root"]
fn installed_structured_cache_reuses_logical_work_and_rebinds_current_project() {
    use semaprax::assurance_manifest::modular_law::cache::ProofTaskCache;
    use semaprax::assurance_manifest::structured_law::installed::prove_installed_project_cached;
    use semaprax::semantic_cache_store::{initialize, load_modular_proofs, persist_modular_proofs};
    use std::os::unix::fs::PermissionsExt;

    let project = Fixture::new("cached-z3", &project_source());
    let revision = project.revision();
    let tool = project.tool(ToolKind::Z3);
    let mut cache = ProofTaskCache::for_project(&project.root).unwrap();
    let prove = |revision: &ProjectRevision, cache: &mut ProofTaskCache| {
        prove_installed_project_cached(
            &project.root,
            revision,
            "src/app.spx",
            "law07.total-after",
            0,
            &tool,
            cache,
        )
        .unwrap()
    };
    let (cold_certificate, cold_proof, cold_work) = prove(&revision, &mut cache);
    assert_eq!(cold_work.fresh, 1);
    let cold_report = derive_with_verified_proofs(
        &revision,
        &ProjectAssuranceOptions::default(),
        &[cold_proof.clone()],
    )
    .unwrap();

    let root = project.root.join("proof-cache");
    std::fs::create_dir(&root).unwrap();
    std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
    initialize(&root).unwrap();
    let receipt = persist_modular_proofs(&root, &cache).unwrap();
    let mut restored = load_modular_proofs(&root, receipt.entry_digest()).unwrap();
    let (warm_certificate, warm_proof, warm_work) = prove(&revision, &mut restored);
    assert_eq!(warm_work.reused, 1);
    assert_eq!(warm_work.fresh, 0);
    assert_eq!(warm_certificate, cold_certificate);
    let warm_report = derive_with_verified_proofs(
        &revision,
        &ProjectAssuranceOptions::default(),
        &[warm_proof],
    )
    .unwrap();
    assert_eq!(warm_report, cold_report);

    let source = project.root.join("src/app.spx");
    let before = std::fs::read_to_string(&source).unwrap();
    std::fs::write(&source, format!("// cache-only comment\n{before}")).unwrap();
    let formatted = project.revision();
    let (rebound_certificate, rebound, format_work) = prove(&formatted, &mut restored);
    assert_eq!(format_work.reused, 1);
    assert_ne!(rebound_certificate, cold_certificate);
    assert!(derive_with_verified_proofs(
        &formatted,
        &ProjectAssuranceOptions::default(),
        &[cold_proof],
    )
    .is_err());
    assert!(derive_with_verified_proofs(
        &formatted,
        &ProjectAssuranceOptions::default(),
        &[rebound],
    )
    .is_ok());

    let changed = before.replace(
        "after.debit.balance + after.credit.balance",
        "after.credit.balance + after.debit.balance",
    );
    assert_ne!(changed, before);
    let canonical = semaprax::format::canonical(&semaprax::parse(&changed, &source).unwrap());
    std::fs::write(&source, canonical).unwrap();
    let semantic_edit = project.revision();
    let (_, _, changed_work) = prove(&semantic_edit, &mut restored);
    assert_eq!(changed_work.fresh, 1);
    assert_eq!(changed_work.stale, 1);
}

#[test]
#[ignore = "requires explicitly provisioned pinned Lean"]
fn installed_structured_cache_reuses_kernel_checked_query_and_refuses_changed_law() {
    use semaprax::assurance_manifest::modular_law::cache::ProofTaskCache;
    use semaprax::assurance_manifest::structured_law::installed::prove_installed_project_cached;

    let project = Fixture::new("cached-lean", &project_source());
    let revision = project.revision();
    let tool = project.tool(ToolKind::Lean);
    let mut cache = ProofTaskCache::for_project(&project.root).unwrap();
    let prove = |revision: &ProjectRevision, cache: &mut ProofTaskCache| {
        prove_installed_project_cached(
            &project.root,
            revision,
            "src/app.spx",
            "law07.total-after",
            0,
            &tool,
            cache,
        )
    };
    let (cold, _, cold_work) = prove(&revision, &mut cache).unwrap();
    assert_eq!(cold_work.fresh, 1);
    let (warm, _, warm_work) = prove(&revision, &mut cache).unwrap();
    assert_eq!(warm_work.reused, 1);
    assert_eq!(cold, warm);

    let source = project.root.join("src/app.spx");
    let before = std::fs::read_to_string(&source).unwrap();
    let wrong = before.replace(
        "ensures result == debit + credit",
        "ensures result == debit - credit",
    );
    assert_ne!(wrong, before);
    let canonical = semaprax::format::canonical(&semaprax::parse(&wrong, &source).unwrap());
    std::fs::write(&source, canonical).unwrap();
    let revision = project.revision();
    assert!(
        prove(&revision, &mut cache).is_err(),
        "changed false theorem cannot reuse a prior kernel success"
    );
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

#[test]
#[ignore = "requires explicitly provisioned installed Z3 and selected Unix Project host"]
fn selected_multimodule_project_and_workspace_accept_private_structured_law() {
    use semaprax::assurance_manifest::law_set::{
        protected::{
            ProtectedLawBaseline, ProtectedLawReview, SpecificationChangeApproval,
            SpecificationChangeAuthority,
        },
        strict::{self, RequiredLawEvidence, StrictLawPolicy},
        LawSet,
    };
    use semaprax::project::{
        apply_strict_law_publication, install_host_strict_law_policy,
        prepare_strict_law_publication, with_strict_authenticated_project, ProjectCandidate,
        ProjectExecutionOptions, SemanticChange, StrictCandidateLawInputs,
    };
    use std::collections::BTreeMap;

    let app = r#"module law07.app;
@id("law07.public") fn public_value() -> i64 { 0 }
@id("law07.main") fn main() -> i64 { 0 }
"#;
    let project = Fixture::new("selected-multimodule", app);
    let types = SOURCE.split_once("@id(\"law07.sum\")").unwrap().0;
    let core = format!("{types}\n{TOTAL_AFTER}\n{OUTCOME_TOTAL}");
    assert_eq!(
        semaprax::parse(&core, "src/core.spx")
            .unwrap()
            .functions
            .len(),
        2
    );
    std::fs::write(
        project.root.join("src/core.spx"),
        semaprax::format::canonical(&semaprax::parse(&core, "src/core.spx").unwrap()),
    )
    .unwrap();
    let tests = r#"module law07.tests;
use function @id("law07.total-after") from law07.accounting as total_after;
@id("law07.tests.main") fn main() -> i64 { total_after(1, 2, 0) }
"#;
    std::fs::write(
        project.root.join("src/tests.spx"),
        semaprax::format::canonical(&semaprax::parse(tests, "src/tests.spx").unwrap()),
    )
    .unwrap();
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
    let manifest = project.root.join("semaprax.toml");
    std::fs::write(&manifest, "schema = \"semaprax.manifest.v2\"\n\n[package]\nname = \"law07-selected\"\nversion = \"1.0.0\"\n\n[modules]\nentry = \"law07.app\"\nsources = [\"src/app.spx\", \"src/contracts.spx\", \"src/core.spx\", \"src/tests.spx\"]\nlaw_sources = [\"src/contracts.spx\"]\ntests = [\"law07.tests\"]\n\n[exports]\nweb = [\"law07.public\"]\n").unwrap();
    let paths = project.root.join("paths.json");
    std::fs::write(&paths, "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"src/app.spx\"},{\"path\":\"src/contracts.spx\"},{\"path\":\"src/core.spx\"},{\"path\":\"src/tests.spx\"}]}\n").unwrap();
    let workspace_revision =
        semaprax::semantic_workspace::initialize(&project.root, &paths).unwrap();
    let revision = project.revision();
    assert_eq!(revision.workspace_revision(), workspace_revision);
    let tool = project.tool(ToolKind::Z3);
    let laws = LawSet::derive(
        &revision,
        "structured-proof-v1",
        revision.law_modules().to_vec(),
    )
    .unwrap();
    let policy = StrictLawPolicy::new(
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
    let (certificate, proof) =
        prove_structured_postcondition(&revision, "src/core.spx", "law07.total-after", 0, &tool)
            .expect("installed Z3 proves selected private aggregate-body law");
    let row: Certificate = serde_json::from_str(&certificate).unwrap();
    assert_eq!(
        row.program_root,
        revision.program_root().unwrap().program_root()
    );
    let report = strict::derive(&revision, &laws, &policy, &[proof.clone()]).unwrap();
    strict::require(&report, &revision, &laws, &policy, &[proof.clone()]).unwrap();
    let protection = ProtectedLawBaseline::new(&revision, laws, vec![]).unwrap();
    install_host_strict_law_policy(&manifest, &policy, vec![]).unwrap();
    with_strict_authenticated_project(&manifest, &[proof.clone()], &[], |session| {
        session.execute_entry(&ProjectExecutionOptions::default())?;
        Ok(())
    })
    .expect("selected Project executes with exact private structured proof");
    assert!(with_strict_authenticated_project(&manifest, &[], &[], |_session| Ok(())).is_err());

    struct Host;
    impl SpecificationChangeAuthority for Host {
        fn approve_specification_change(&mut self, _: &ProtectedLawReview) -> bool {
            true
        }
    }
    let start = ProjectCandidate::open(revision.clone(), revision.project_revision()).unwrap();
    let change = SemanticChange::new(
        revision.project_revision(),
        &serde_json::json!({
            "kind":"change_function_signature", "target":"law07.public",
            "append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]
        }),
    )
    .unwrap();
    let candidate = start.apply(start.candidate_digest(), &change).unwrap();
    let second = SemanticChange::new(
        candidate.revision().project_revision(),
        &serde_json::json!({
            "kind":"change_function_signature", "target":"law07.outcome-total",
            "append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]
        }),
    )
    .unwrap();
    let candidate = candidate
        .apply(candidate.candidate_digest(), &second)
        .unwrap();
    let candidate_laws = LawSet::derive(
        candidate.revision(),
        "structured-proof-v1",
        candidate.revision().law_modules().to_vec(),
    )
    .unwrap();
    let fresh = prove_structured_postcondition(
        candidate.revision(),
        "src/core.spx",
        "law07.total-after",
        0,
        &tool,
    )
    .expect("candidate Project requires a new exact structured proof")
    .1;
    let intent = candidate
        .protected_law_review(&protection, &candidate_laws)
        .unwrap();
    let approval = SpecificationChangeApproval::request(&intent, &mut Host).unwrap();
    let stale_proofs = [proof];
    let stale = StrictCandidateLawInputs {
        protection: &protection,
        policy: &policy,
        laws: &candidate_laws,
        proofs: &stale_proofs,
        native_proofs: &[],
        specification_approval: Some(&approval),
    };
    let fresh_proofs = [fresh];
    let inputs = StrictCandidateLawInputs {
        protection: &protection,
        policy: &policy,
        laws: &candidate_laws,
        proofs: &fresh_proofs,
        native_proofs: &[],
        specification_approval: Some(&approval),
    };
    let active = project.root.join(".semaprax-workspace/ACTIVE");
    let before = std::fs::read(&active).unwrap();
    assert!(prepare_strict_law_publication(
        &candidate,
        &stale,
        candidate.candidate_digest(),
        &project.root,
        &manifest,
        &workspace_revision,
    )
    .is_err());
    assert_eq!(std::fs::read(&active).unwrap(), before);
    let proposal = prepare_strict_law_publication(
        &candidate,
        &inputs,
        candidate.candidate_digest(),
        &project.root,
        &manifest,
        &workspace_revision,
    )
    .expect("selected Project stages exact structured law publication");
    assert_eq!(std::fs::read(&active).unwrap(), before);
    let missing = StrictCandidateLawInputs {
        protection: &protection,
        policy: &policy,
        laws: &candidate_laws,
        proofs: &[],
        native_proofs: &[],
        specification_approval: Some(&approval),
    };
    assert!(prepare_strict_law_publication(
        &candidate,
        &missing,
        candidate.candidate_digest(),
        &project.root,
        &manifest,
        &workspace_revision,
    )
    .is_err());
    assert_eq!(std::fs::read(&active).unwrap(), before);
    apply_strict_law_publication(
        &candidate,
        &inputs,
        candidate.candidate_digest(),
        &project.root,
        &manifest,
        &workspace_revision,
        proposal.to_json().as_bytes(),
    )
    .expect("fresh proof permits one managed-Workspace ACTIVE pivot");
    assert_ne!(std::fs::read(&active).unwrap(), before);
}
