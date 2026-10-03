//! LAW-03 exercises actual retained Projects and independently held baselines.
use super::*;
use semaprax::assurance_manifest::law_set::protected::{
    ProtectedLawBaseline, ProtectedLawReview, SpecificationChangeApproval,
    SpecificationChangeAuthority,
};
use semaprax::project::{ProjectCandidate, ProjectRevision};
use std::sync::Arc;

fn revision(fixture: &Fixture) -> Arc<ProjectRevision> {
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap()
}
fn set(revision: &ProjectRevision, modules: Vec<LawModule>) -> LawSet {
    LawSet::derive(revision, "checked-v1", modules).unwrap()
}
fn baseline(base: &ProjectRevision) -> ProtectedLawBaseline {
    ProtectedLawBaseline::new(
        base,
        set(base, vec![module()]),
        vec!["calculator.add".into()],
    )
    .unwrap()
}
fn review(
    baseline: &ProtectedLawBaseline,
    base: &Arc<ProjectRevision>,
    next: &Arc<ProjectRevision>,
    modules: Vec<LawModule>,
) -> ProtectedLawReview {
    baseline
        .review(base, next, &set(next, modules), next.project_revision())
        .unwrap()
}
struct Host(bool);
impl SpecificationChangeAuthority for Host {
    fn approve_specification_change(&mut self, _: &ProtectedLawReview) -> bool {
        self.0
    }
}

#[test]
fn protected_law_implementation_repair_preserves_intent_and_invalidates_proof() {
    let fixture = Fixture::new("protected-repair");
    let base = revision(&fixture);
    let protected = baseline(&base);
    patch_core(&fixture, "left + right", "left - right");
    let next = revision(&fixture);
    let checked = review(&protected, &base, &next, vec![module()]);
    checked.require(None).unwrap();
    assert_eq!(wire(checked.to_json())["proof_work_invalidated"], true);
    assert_eq!(
        wire(checked.to_json())["non_weakening"],
        "canonical_equivalence"
    );
    let candidate = ProjectCandidate::open(base.clone(), base.project_revision()).unwrap();
    candidate
        .protected_law_review(&protected, &set(&base, vec![module()]))
        .unwrap()
        .require(None)
        .unwrap();
}

#[test]
fn protected_law_helper_change_is_unknown_even_with_untouched_law_module() {
    let fixture = Fixture::new("protected-helper");
    let base = revision(&fixture);
    let protected = baseline(&base);
    patch_core(&fixture, "value < 0", "true");
    let next = revision(&fixture);
    let changed = review(&protected, &base, &next, vec![module()]);
    assert_code(changed.require(None), "SPX-LW120");
    assert_eq!(wire(changed.to_json())["non_weakening"], "unknown");
    assert!(changed
        .to_json()
        .contains("protected_specification_closure_changed"));
}

#[test]
fn protected_law_deletion_rename_move_false_domain_assumptions_and_policy_require_review() {
    let fixture = Fixture::new("protected-hostiles");
    let base = revision(&fixture);
    let protected = baseline(&base);
    let mut cases = vec![vec![]];
    let mut renamed = module();
    renamed.laws[0].law_id = "renamed.law".into();
    cases.push(vec![renamed]);
    let mut moved = module();
    moved.source_path = "src/app.spx".into();
    cases.push(vec![moved]);
    let mut false_pre = module();
    if let LawSelector::Contract { proposition, .. } = &mut false_pre.laws[0].selector {
        *proposition = "false".into();
    }
    cases.push(vec![false_pre]);
    let mut assumption = module();
    assumption.assumptions.push("trusted.conclusion".into());
    assumption.laws[0]
        .assumption_ids
        .push("trusted.conclusion".into());
    cases.push(vec![assumption]);
    let mut evidence = module();
    evidence.laws[0].evidence = EvidenceRequirement::CompilerProved;
    cases.push(vec![evidence]);
    for modules in cases {
        let changed = review(&protected, &base, &base, modules);
        assert_code(changed.require(None), "SPX-LW120");
    }
    let changed_profile = LawSet::derive(&base, "weaker-target", vec![module()]).unwrap();
    assert_code(
        protected
            .review(&base, &base, &changed_profile, base.project_revision())
            .unwrap()
            .require(None),
        "SPX-LW120",
    );
}

#[test]
fn protected_law_approval_is_exact_and_host_authority_is_required() {
    let fixture = Fixture::new("protected-approval");
    let base = revision(&fixture);
    let protected = baseline(&base);
    let proposal = review(&protected, &base, &base, vec![]);
    assert_code(
        SpecificationChangeApproval::request(&proposal, &mut Host(false)),
        "SPX-LW121",
    );
    let approval = SpecificationChangeApproval::request(&proposal, &mut Host(true)).unwrap();
    proposal.require(Some(&approval)).unwrap();
    let mut changed = module();
    changed.laws[0].law_id = "different.law".into();
    assert_code(
        review(&protected, &base, &base, vec![changed]).require(Some(&approval)),
        "SPX-LW104",
    );
    let unchanged = review(&protected, &base, &base, vec![module()]);
    assert_code(unchanged.require(Some(&approval)), "SPX-LW104");
    patch_core(&fixture, "left + right", "left - right");
    let next = revision(&fixture);
    assert_code(
        review(&protected, &base, &next, vec![]).require(Some(&approval)),
        "SPX-LW104",
    );
    assert_code(
        protected.review(
            &next,
            &next,
            &set(&next, vec![module()]),
            next.project_revision(),
        ),
        "SPX-LW104",
    );
}

#[test]
fn protected_law_canonical_equivalence_needs_no_approval() {
    let fixture = Fixture::new("protected-equivalence");
    let base = revision(&fixture);
    let protected = baseline(&base);
    let mut same = module();
    if let LawSelector::Contract { proposition, .. } = &mut same.laws[0].selector {
        *proposition = "(right  != 0)".into();
    }
    review(&protected, &base, &base, vec![same])
        .require(None)
        .unwrap();
}

#[test]
fn protected_law_contracts_of_editable_implementation_are_still_specification() {
    let fixture = Fixture::new("protected-contract");
    let base = revision(&fixture);
    let protected = ProtectedLawBaseline::new(
        &base,
        set(&base, vec![module()]),
        vec!["calculator.divide".into()],
    )
    .unwrap();
    patch_core(&fixture, "requires right != 0", "requires right > 0");
    let next = revision(&fixture);
    assert_code(
        review(&protected, &base, &next, vec![module()]).require(None),
        "SPX-LW120",
    );
}

#[test]
fn protected_law_unknown_editable_identity_is_rejected() {
    let fixture = Fixture::new("protected-unknown-editable");
    let base = revision(&fixture);
    assert_code(
        ProtectedLawBaseline::new(
            &base,
            set(&base, vec![module()]),
            vec!["missing.function".into()],
        ),
        "SPX-LW101",
    );
}

#[test]
fn protected_law_publication_rechecks_approval_and_preserves_active_on_refusal() {
    use semaprax::project::{
        apply_protected_law_publication, prepare_candidate_publication, SemanticChange,
    };
    let fixture = Fixture::new("protected-publication");
    let paths = fixture.0.join("paths.json");
    std::fs::write(&paths, concat!(r#"{"schema":"semaprax.workspace-semantic-path-set.v1","files":[{"path":"src/app.spx"},{"path":"src/core.spx"},{"path":"src/tests.spx"}]}"#, "\n")).unwrap();
    semaprax::semantic_workspace::initialize(&fixture.0, &paths).unwrap();
    let base = revision(&fixture);
    let protected = baseline(&base);
    let root = ProjectCandidate::open(base.clone(), base.project_revision()).unwrap();
    let change = SemanticChange::new(base.project_revision(), &serde_json::json!({"kind":"change_function_signature","target":"calculator.add","append_parameters":[{"name":"unused","type":"i64","argument":{"kind":"i64","value":0}}]})).unwrap();
    let candidate = root.apply(root.candidate_digest(), &change).unwrap();
    let laws = set(candidate.revision(), vec![module()]);
    let workspace = semaprax::workspace_graph::snapshot(&fixture.0, "calculator.app")
        .unwrap()
        .workspace_revision()
        .to_owned();
    let proof = prepare_candidate_publication(
        &candidate,
        candidate.candidate_digest(),
        &fixture.0,
        &fixture.manifest(),
        &workspace,
    )
    .unwrap();
    let active = fixture.0.join(".semaprax-workspace/ACTIVE");
    let before = std::fs::read(&active).unwrap();
    assert_code(
        apply_protected_law_publication(
            &candidate,
            &protected,
            &laws,
            None,
            candidate.candidate_digest(),
            &fixture.0,
            &fixture.manifest(),
            &workspace,
            proof.to_json().as_bytes(),
        ),
        "SPX-LW120",
    );
    assert_eq!(std::fs::read(&active).unwrap(), before);
    let proposal = candidate.protected_law_review(&protected, &laws).unwrap();
    let approval = SpecificationChangeApproval::request(&proposal, &mut Host(true)).unwrap();
    apply_protected_law_publication(
        &candidate,
        &protected,
        &laws,
        Some(&approval),
        candidate.candidate_digest(),
        &fixture.0,
        &fixture.manifest(),
        &workspace,
        proof.to_json().as_bytes(),
    )
    .unwrap();
    assert_ne!(std::fs::read(&active).unwrap(), before);
}
