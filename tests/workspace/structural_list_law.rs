//! LAW-08: exact selected Project/LawSet list theorem attachment.
use semaprax::{
    agent_runtime::AgentCancellation,
    assurance_manifest::law_set::{
        native_proof::{prove_list_induction_law, replay_list_induction_law},
        strict::{self, RequiredLawEvidence, StrictLawPolicy},
        EvidenceRequirement, LawDefinition, LawModule, LawSelector, LawSet,
    },
    project::{with_authenticated_project, ProjectRevision},
    proof_export::{
        installed::{HostProfile, InstalledProofTool, Limits, ToolKind},
        list_induction::{Certificate, ProofModule},
    },
};
use std::{collections::BTreeMap, path::PathBuf, sync::Arc};

const SOURCE: &str = include_str!("../fixtures/law08-structural-list.spx");
const PROOFS: &str = include_str!("../../proofs/law08/list-lemmas.json");
const IMMUTABLE_SOURCE: &str = include_str!("../fixtures/law08-immutable-list.spx");
const IMMUTABLE_PROOFS: &str = include_str!("../../proofs/law08/immutable-list-lemmas.json");
const LAW_ID: &str = "law08.reverse.involution";

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new(label: &str, source: &str) -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-law08-selected-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let canonical =
            semaprax::format::canonical(&semaprax::parse(source, "src/app.spx").unwrap());
        std::fs::write(root.join("src/app.spx"), canonical).unwrap();
        let tests = "module law08.tests; @id(\"law08.tests.main\") fn main() -> i64 { 0 }";
        std::fs::write(
            root.join("src/tests.spx"),
            semaprax::format::canonical(&semaprax::parse(tests, "src/tests.spx").unwrap()),
        )
        .unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            "schema = \"semaprax.project.v8\"\nname = \"law08-list\"\nversion = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\nentry = \"test.structural_list\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = []\ntests = [\"law08.tests\"]\n",
        )
        .unwrap();
        Self { root }
    }
    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.root.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }
    fn tool(&self) -> InstalledProofTool {
        let path = PathBuf::from(std::env::var("SEMAPRAX_LAW_LEAN").expect("pinned Lean path"));
        let version = std::env::var("SEMAPRAX_LAW_LEAN_VERSION").expect("exact Lean version");
        InstalledProofTool::open(
            &path,
            &self.root,
            ToolKind::Lean,
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

fn law_module() -> LawModule {
    LawModule {
        module_id: "law08.list-laws".into(),
        source_path: "src/app.spx".into(),
        assumptions: vec![],
        laws: vec![LawDefinition {
            law_id: LAW_ID.into(),
            selector: LawSelector::ListInduction {
                declaration_id: "list.reverse".into(),
                theorem: "reverse_involution".into(),
            },
            assumption_ids: vec![],
            requires_laws: vec![],
            evidence: EvidenceRequirement::TheoremProved,
        }],
    }
}

#[test]
#[ignore = "requires explicitly provisioned pinned Lean 4.34.0 executable"]
fn installed_immutable_list_source_proves_and_replays_selected_project_law() {
    let fixture = Fixture::new("immutable", IMMUTABLE_SOURCE);
    let revision = fixture.revision();
    let tool = fixture.tool();
    let proofs: ProofModule = serde_json::from_str(IMMUTABLE_PROOFS).unwrap();
    let laws = LawSet::derive(&revision, "law08-immutable-list-v1", vec![law_module()]).unwrap();
    let (document, proof) =
        prove_list_induction_law(&revision, &laws, LAW_ID, &proofs, &tool).unwrap();
    let certificate: Certificate = serde_json::from_str(&document).unwrap();
    assert_eq!(
        certificate.profile,
        semaprax::proof_export::list_induction::IMMUTABLE_PROFILE
    );
    let policy = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            LAW_ID.into(),
            RequiredLawEvidence::PinnedListInductionLean {
                toolchain: semaprax::proof_export::PINNED_TOOLCHAIN.into(),
                proof_module_sha256: certificate.proof_module_sha256.clone(),
                accepted_axioms: semaprax::proof_export::kernel_report::STANDARD_AXIOMS
                    .iter()
                    .map(|name| (*name).into())
                    .collect(),
            },
        )]),
    )
    .unwrap();
    let report =
        strict::derive_with_native_proofs(&revision, &laws, &policy, &[], &[proof]).unwrap();
    let replay =
        replay_list_induction_law(&document, &revision, &laws, LAW_ID, &proofs, &tool).unwrap();
    strict::require_with_native_proofs(&report, &revision, &laws, &policy, &[], &[replay]).unwrap();

    let wrong = IMMUTABLE_SOURCE.replace(
        "append(reverse(tail), list_cons(head, list_nil()))",
        "list_cons(head, reverse(tail))",
    );
    let wrong_program = semaprax::check(&wrong, "wrong-immutable-reverse.spx").unwrap();
    struct NoKernel;
    impl semaprax::proof_export::LeanKernel for NoKernel {
        fn check(
            &self,
            _: &str,
        ) -> Result<semaprax::proof_export::KernelRun, semaprax::diagnostic::Diagnostic> {
            panic!("wrong source must refuse before the kernel")
        }
    }
    assert_eq!(
        semaprax::proof_export::list_induction::prove(&wrong_program, &proofs, &NoKernel)
            .unwrap_err()
            .code,
        "SPX-LI001"
    );
    assert!(replay_list_induction_law(
        &document,
        &revision,
        &laws,
        LAW_ID,
        &{
            let mut stale = proofs.clone();
            stale.reverse_eq.push_str("\n  simp");
            stale
        },
        &tool
    )
    .is_err());
}

#[test]
#[ignore = "requires explicitly provisioned pinned Lean 4.34.0 executable"]
fn installed_list_induction_selected_project_replay_and_strict_refusals() {
    let fixture = Fixture::new("baseline", SOURCE);
    let revision = fixture.revision();
    let tool = fixture.tool();
    let proofs: ProofModule = serde_json::from_str(PROOFS).unwrap();
    let laws = LawSet::derive(&revision, "law08-list-v1", vec![law_module()]).unwrap();
    let (document, proof) =
        prove_list_induction_law(&revision, &laws, LAW_ID, &proofs, &tool).unwrap();
    let certificate: Certificate = serde_json::from_str(&document).unwrap();
    assert_eq!(certificate.coverage.len(), 3);
    let accepted = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            LAW_ID.into(),
            RequiredLawEvidence::PinnedListInductionLean {
                toolchain: semaprax::proof_export::PINNED_TOOLCHAIN.into(),
                proof_module_sha256: certificate.proof_module_sha256.clone(),
                accepted_axioms: semaprax::proof_export::kernel_report::STANDARD_AXIOMS
                    .iter()
                    .map(|name| (*name).into())
                    .collect(),
            },
        )]),
    )
    .unwrap();
    let report =
        strict::derive_with_native_proofs(&revision, &laws, &accepted, &[], &[proof.clone()])
            .unwrap();
    strict::require_with_native_proofs(&report, &revision, &laws, &accepted, &[], &[proof])
        .unwrap();
    let replayed =
        replay_list_induction_law(&document, &revision, &laws, LAW_ID, &proofs, &tool).unwrap();
    strict::require_with_native_proofs(
        &report,
        &revision,
        &laws,
        &accepted,
        &[],
        &[replayed.clone()],
    )
    .unwrap();
    assert!(
        strict::require_with_native_proofs(&report, &revision, &laws, &accepted, &[], &[]).is_err()
    );
    let wrong_module_policy = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            LAW_ID.into(),
            RequiredLawEvidence::PinnedListInductionLean {
                toolchain: semaprax::proof_export::PINNED_TOOLCHAIN.into(),
                proof_module_sha256: "sha256:stale".into(),
                accepted_axioms: semaprax::proof_export::kernel_report::STANDARD_AXIOMS
                    .iter()
                    .map(|name| (*name).into())
                    .collect(),
            },
        )]),
    )
    .unwrap();
    let wrong_report = strict::derive_with_native_proofs(
        &revision,
        &laws,
        &wrong_module_policy,
        &[],
        &[replayed.clone()],
    )
    .unwrap();
    assert!(strict::require_with_native_proofs(
        &wrong_report,
        &revision,
        &laws,
        &wrong_module_policy,
        &[],
        &[replayed],
    )
    .is_err());

    let mut stale_proofs = proofs.clone();
    stale_proofs.append_eq.push_str("\n  simp");
    assert!(
        replay_list_induction_law(&document, &revision, &laws, LAW_ID, &stale_proofs, &tool,)
            .is_err()
    );
    let mut tampered: Certificate = serde_json::from_str(&document).unwrap();
    tampered.theorem_law_ids[4].1 = "list.append".into();
    assert!(replay_list_induction_law(
        &serde_json::to_string(&tampered).unwrap(),
        &revision,
        &laws,
        LAW_ID,
        &proofs,
        &tool,
    )
    .is_err());
    let mut wrong_selector = law_module();
    wrong_selector.laws[0].selector = LawSelector::ListInduction {
        declaration_id: "list.append".into(),
        theorem: "reverse_involution".into(),
    };
    assert!(LawSet::derive(&revision, "law08-list-v1", vec![wrong_selector]).is_err());
    let wrong_source = SOURCE.replace(
        "vec_push<i64>(reverse(rest), item)",
        "vec_push<i64>(reverse(rest), item + 1)",
    );
    let drift = Fixture::new("drift", &wrong_source).revision();
    assert!(replay_list_induction_law(&document, &drift, &laws, LAW_ID, &proofs, &tool).is_err());
}
