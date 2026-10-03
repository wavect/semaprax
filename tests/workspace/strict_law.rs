//! LAW-04's first strict Project/candidate evidence join; no external solver runs.
use super::*;
use semaprax::assurance_manifest::law_set::workflow;
use semaprax::assurance_manifest::{
    law_set::strict::{self, RequiredLawEvidence, StrictLawPolicy},
    model_checking as mc,
};
use semaprax::project::{ProjectCandidate, ProjectRevision};
use std::{collections::BTreeMap, sync::Arc};

fn revision(fixture: &Fixture) -> Arc<ProjectRevision> {
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap()
}
fn architecture_module() -> LawModule {
    let mut declared = module();
    declared.laws[0] = LawDefinition {
        law_id: "calculator.architecture".into(),
        selector: LawSelector::ForbidReaches {
            claim_id: "no-divide".into(),
            from: "calculator.is-negative".into(),
            to: "calculator.divide".into(),
        },
        assumption_ids: vec![],
        requires_laws: vec![],
        evidence: EvidenceRequirement::CompilerProved,
    };
    declared
}
fn policy(laws: &LawSet, id: &str, requirement: RequiredLawEvidence) -> StrictLawPolicy {
    StrictLawPolicy::new(laws.clone(), BTreeMap::from([(id.into(), requirement)])).unwrap()
}
fn lean_requirement() -> RequiredLawEvidence {
    RequiredLawEvidence::PinnedLeanSource {
        toolchain: semaprax::proof_export::PINNED_TOOLCHAIN.into(),
        accepted_assumptions: semaprax::proof_export::ASSUMPTIONS
            .iter()
            .map(|(id, _)| (*id).into())
            .collect(),
        accepted_axioms: semaprax::proof_export::kernel_report::STANDARD_AXIOMS
            .iter()
            .map(|id| (*id).into())
            .collect(),
    }
}

#[test]
fn strict_law_compiler_success_replays_exact_candidate_policy_and_report() {
    let fixture = Fixture::new("strict-law-compiler");
    let revision = revision(&fixture);
    let laws = LawSet::derive(&revision, "checked-v1", vec![architecture_module()]).unwrap();
    let policy = policy(
        &laws,
        "calculator.architecture",
        RequiredLawEvidence::CompilerStatic,
    );
    let report = strict::derive(&revision, &laws, &policy, &[]).unwrap();
    strict::require(&report, &revision, &laws, &policy, &[]).unwrap();
    let candidate = ProjectCandidate::open(revision.clone(), revision.project_revision()).unwrap();
    let report = candidate
        .strict_law_assurance(candidate.candidate_digest(), &laws, &policy, &[])
        .unwrap();
    candidate
        .require_strict_law_assurance(&report, &laws, &policy, &[])
        .unwrap();
    assert_code(
        candidate.require_strict_law_assurance(
            &report.replace(
                "\"publication_authority\":false",
                "\"publication_authority\":true",
            ),
            &laws,
            &policy,
            &[],
        ),
        "SPX-LW104",
    );
    assert!(report.contains("\"publication_authority\":false"));
}

#[test]
fn strict_law_missing_inventory_clause_or_requirement_cannot_pass_empty() {
    let fixture = Fixture::new("strict-law-omissions");
    let base = revision(&fixture);
    let laws = LawSet::derive(&base, "checked-v1", vec![module()]).unwrap();
    assert_code(
        StrictLawPolicy::new(laws.clone(), BTreeMap::new()),
        "SPX-LW101",
    );
    let policy = policy(
        &laws,
        "calculator.divide.nonzero",
        RequiredLawEvidence::CompilerStatic,
    );
    let empty = LawSet::derive(&base, "checked-v1", vec![]).unwrap();
    let missing = strict::derive(&base, &empty, &policy, &[]).unwrap();
    assert_eq!(wire(&missing)["counts"]["required"], 1);
    assert_code(
        strict::require(&missing, &base, &empty, &policy, &[]),
        "SPX-LW130",
    );
    patch_core(&fixture, "    requires right != 0\n", "");
    let changed = revision(&fixture);
    let laws = LawSet::derive(&changed, "checked-v1", vec![module()]).unwrap();
    let missing = strict::derive(&changed, &laws, &policy, &[]).unwrap();
    assert_code(
        strict::require(&missing, &changed, &laws, &policy, &[]),
        "SPX-LW130",
    );
    assert_eq!(wire(&missing)["counts"]["required"], 1);
}

#[test]
fn strict_workflow_keeps_failed_verdict_and_replays_exact_proof_inventory() {
    let fixture = Fixture::new("strict-law-workflow");
    let current = revision(&fixture);
    let laws = LawSet::derive(&current, "checked-v1", vec![module()]).unwrap();
    let policy = policy(
        &laws,
        "calculator.divide.nonzero",
        RequiredLawEvidence::CompilerStatic,
    );
    let report = strict::derive(&current, &laws, &policy, &[]).unwrap();
    let first = wire(
        &workflow::strict_summary(&report, &current, &laws, &policy, &[], &[], 0, 1, 8192).unwrap(),
    );
    assert_eq!(first["accepted"], false);
    assert_eq!(first["counts"]["required"], 1);
    assert_eq!(first["counts"]["satisfied"], 0);
    assert_eq!(first["laws"][0]["law_id"], "calculator.divide.nonzero");
    assert_eq!(first["laws"][0]["satisfied"], false);
    let empty_page = wire(
        &workflow::strict_summary(&report, &current, &laws, &policy, &[], &[], 1, 1, 8192).unwrap(),
    );
    assert_eq!(empty_page["accepted"], false);
    assert_eq!(empty_page["counts"], first["counts"]);
    let detail = wire(
        &workflow::strict_detail(
            &report,
            &current,
            &laws,
            &policy,
            &[],
            &[],
            "calculator.divide.nonzero",
            8192,
        )
        .unwrap(),
    );
    assert_eq!(detail["law"]["satisfied"], false);
    assert_eq!(detail["repair_target"], "implementation_or_proof");
    assert_code(
        workflow::strict_detail(
            &report.replace("\"accepted\":false", "\"accepted\":true"),
            &current,
            &laws,
            &policy,
            &[],
            &[],
            "calculator.divide.nonzero",
            8192,
        ),
        "SPX-LW104",
    );
}

#[test]
fn strict_law_runtime_and_unavailable_proofs_never_satisfy_static_or_lowering() {
    let fixture = Fixture::new("strict-law-unavailable");
    let revision = revision(&fixture);
    let laws = LawSet::derive(&revision, "checked-v1", vec![module()]).unwrap();
    for requirement in [
        RequiredLawEvidence::CompilerStatic,
        lean_requirement(),
        RequiredLawEvidence::SmtSource,
        RequiredLawEvidence::VerifiedLowering,
    ] {
        let policy = policy(&laws, "calculator.divide.nonzero", requirement);
        let report = strict::derive(&revision, &laws, &policy, &[]).unwrap();
        assert_code(
            strict::require(&report, &revision, &laws, &policy, &[]),
            "SPX-LW130",
        );
        let mut forged = wire(&report);
        forged["accepted"] = serde_json::json!(true);
        let forged = format!("{}\n", serde_json::to_string(&forged).unwrap());
        assert_code(
            strict::require(&forged, &revision, &laws, &policy, &[]),
            "SPX-LW104",
        );
    }
}

#[test]
fn strict_law_reference_model_bounds_domain_and_universal_scope_are_distinct() {
    let fixture = Fixture::new("strict-law-model");
    let revision = revision(&fixture);
    let mut declared = module();
    declared.laws[0].selector = LawSelector::ModelProperty {
        model: ModelKind::Authorization,
        property: "no_uncertain_redispatch".into(),
    };
    declared.laws[0].evidence = EvidenceRequirement::ModelChecked;
    let laws = LawSet::derive(&revision, "checked-v1", vec![declared]).unwrap();
    let requirement = RequiredLawEvidence::ReferenceModel {
        model_digest: mc::model_digest(
            &mc::authorization_model::DESCRIPTOR,
            mc::authorization_model::BOUNDS,
        ),
        minimum_states: 64,
        minimum_depth: 10,
        minimum_transitions: 128,
    };
    let accepted = policy(&laws, "calculator.divide.nonzero", requirement.clone());
    let report = strict::derive(&revision, &laws, &accepted, &[]).unwrap();
    strict::require(&report, &revision, &laws, &accepted, &[]).unwrap();
    let mut deeper = requirement.clone();
    if let RequiredLawEvidence::ReferenceModel { minimum_depth, .. } = &mut deeper {
        *minimum_depth = 100;
    }
    let mut wrong_domain = requirement;
    if let RequiredLawEvidence::ReferenceModel { model_digest, .. } = &mut wrong_domain {
        *model_digest = "sha256:wrong-domain".into();
    }
    for requirement in [
        deeper,
        wrong_domain,
        lean_requirement(),
        RequiredLawEvidence::VerifiedLowering,
    ] {
        let rejected = policy(&laws, "calculator.divide.nonzero", requirement);
        let report = strict::derive(&revision, &laws, &rejected, &[]).unwrap();
        assert_code(
            strict::require(&report, &revision, &laws, &rejected, &[]),
            "SPX-LW130",
        );
    }
}

#[test]
fn strict_law_source_drift_and_policy_substitution_refuse_prior_receipts() {
    let fixture = Fixture::new("strict-law-drift");
    let base = revision(&fixture);
    let laws = LawSet::derive(&base, "checked-v1", vec![architecture_module()]).unwrap();
    let selected = policy(
        &laws,
        "calculator.architecture",
        RequiredLawEvidence::CompilerStatic,
    );
    let report = strict::derive(&base, &laws, &selected, &[]).unwrap();
    let alternate = policy(
        &laws,
        "calculator.architecture",
        RequiredLawEvidence::VerifiedLowering,
    );
    assert_code(
        strict::require(&report, &base, &laws, &alternate, &[]),
        "SPX-LW104",
    );
    patch_core(&fixture, "value < 0", "value <= 0");
    let next = revision(&fixture);
    let next_laws = LawSet::derive(&next, "checked-v1", vec![architecture_module()]).unwrap();
    assert_code(
        strict::require(&report, &next, &next_laws, &selected, &[]),
        "SPX-LW104",
    );
    let candidate = ProjectCandidate::open(next.clone(), next.project_revision()).unwrap();
    assert_code(
        candidate.strict_law_assurance(candidate.candidate_digest(), &next_laws, &selected, &[]),
        "SPX-LW104",
    );
}

#[test]
fn strict_law_publication_binds_report_and_policy_before_active_pivot() {
    use semaprax::assurance_manifest::law_set::protected::{
        ProtectedLawBaseline, ProtectedLawReview, SpecificationChangeApproval,
        SpecificationChangeAuthority,
    };
    use semaprax::project::{
        apply_strict_law_publication, prepare_strict_law_publication, SemanticChange,
        StrictCandidateLawInputs,
    };
    struct Host;
    impl SpecificationChangeAuthority for Host {
        fn approve_specification_change(&mut self, _: &ProtectedLawReview) -> bool {
            true
        }
    }
    let fixture = Fixture::new("strict-law-publication");
    let paths = fixture.0.join("paths.json");
    std::fs::write(&paths,concat!(r#"{"schema":"semaprax.workspace-semantic-path-set.v1","files":[{"path":"src/app.spx"},{"path":"src/core.spx"},{"path":"src/tests.spx"}]}"#,"\n")).unwrap();
    semaprax::semantic_workspace::initialize(&fixture.0, &paths).unwrap();
    let base = revision(&fixture);
    let baseline = LawSet::derive(&base, "checked-v1", vec![architecture_module()]).unwrap();
    let selected = policy(
        &baseline,
        "calculator.architecture",
        RequiredLawEvidence::CompilerStatic,
    );
    let protection = ProtectedLawBaseline::new(&base, baseline, vec![]).unwrap();
    let root = ProjectCandidate::open(base.clone(), base.project_revision()).unwrap();
    let change=SemanticChange::new(base.project_revision(),&serde_json::json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]})).unwrap();
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    let laws = LawSet::derive(
        candidate.revision(),
        "checked-v1",
        vec![architecture_module()],
    )
    .unwrap();
    let intent = candidate.protected_law_review(&protection, &laws).unwrap();
    let approval = SpecificationChangeApproval::request(&intent, &mut Host).unwrap();
    let inputs = StrictCandidateLawInputs {
        protection: &protection,
        policy: &selected,
        laws: &laws,
        proofs: &[],
        native_proofs: &[],
        specification_approval: Some(&approval),
    };
    let workspace = semaprax::workspace_graph::snapshot(&fixture.0, "calculator.app")
        .unwrap()
        .workspace_revision()
        .to_owned();
    let active = fixture.0.join(".semaprax-workspace/ACTIVE");
    let before = std::fs::read(&active).unwrap();
    let proposal = prepare_strict_law_publication(
        &candidate,
        &inputs,
        candidate.candidate_digest(),
        &fixture.0,
        &fixture.manifest(),
        &workspace,
    )
    .unwrap();
    assert_eq!(std::fs::read(&active).unwrap(), before);
    let forged = proposal.to_json().replace(
        "\"publication_authority\":false",
        "\"publication_authority\":true",
    );
    assert_code(
        apply_strict_law_publication(
            &candidate,
            &inputs,
            candidate.candidate_digest(),
            &fixture.0,
            &fixture.manifest(),
            &workspace,
            forged.as_bytes(),
        ),
        "SPX-LW104",
    );
    assert_eq!(std::fs::read(&active).unwrap(), before);
    apply_strict_law_publication(
        &candidate,
        &inputs,
        candidate.candidate_digest(),
        &fixture.0,
        &fixture.manifest(),
        &workspace,
        proposal.to_json().as_bytes(),
    )
    .unwrap();
    assert_ne!(std::fs::read(&active).unwrap(), before);
}

#[test]
fn strict_law_false_static_claim_cannot_be_repaired_with_success_metadata() {
    let fixture = Fixture::new("strict-law-false-claim");
    let base = revision(&fixture);
    let laws = LawSet::derive(&base, "checked-v1", vec![architecture_module()]).unwrap();
    let selected = policy(
        &laws,
        "calculator.architecture",
        RequiredLawEvidence::CompilerStatic,
    );
    patch_core(&fixture, "value < 0", "divide(value, 1) < 0");
    let next = revision(&fixture);
    let laws = LawSet::derive(&next, "checked-v1", vec![architecture_module()]).unwrap();
    let report = strict::derive(&next, &laws, &selected, &[]).unwrap();
    assert_code(
        strict::require(&report, &next, &laws, &selected, &[]),
        "SPX-LW130",
    );
    assert_eq!(wire(&report)["accepted"], false);
}
