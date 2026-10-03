//! LAW-14's fast protected-route mutation corpus.
//!
//! Every case names its boundary.  These routes deliberately receive no proof
//! tool capability, so a refusal must occur with no process, source-write, or
//! publication authority.  The ignored real-tool companion lives with the
//! installed-law fixture where it can require explicit Lean and Z3 pins.
use super::*;
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

fn static_module() -> LawModule {
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

fn policy(laws: &LawSet) -> StrictLawPolicy {
    StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            "calculator.architecture".into(),
            RequiredLawEvidence::CompilerStatic,
        )]),
    )
    .unwrap()
}

fn snapshot(fixture: &Fixture) -> Vec<(String, Vec<u8>)> {
    [
        "semaprax.toml",
        "src/app.spx",
        "src/core.spx",
        "src/tests.spx",
    ]
    .into_iter()
    .map(|path| (path.into(), std::fs::read(fixture.source(path)).unwrap()))
    .collect()
}

fn assert_inert(fixture: &Fixture, before: &[(String, Vec<u8>)]) {
    for (path, bytes) in before {
        assert_eq!(
            std::fs::read(fixture.source(path)).unwrap(),
            bytes.clone(),
            "{path}"
        );
    }
    assert!(
        !fixture.0.join(".semaprax-workspace/ACTIVE").exists(),
        "a read-only law refusal must not publish ACTIVE"
    );
    assert!(
        !fixture.0.join(".git").exists(),
        "a read-only law refusal must not create a Git side effect"
    );
}

fn require_candidate_refusal(
    fixture: &Fixture,
    baseline: &LawSet,
    selected: &StrictLawPolicy,
    code: &str,
) {
    let next = revision(fixture);
    let current = LawSet::derive(&next, "law14-fast-v1", vec![static_module()]).unwrap();
    let candidate = ProjectCandidate::open(next.clone(), next.project_revision()).unwrap();
    let before = snapshot(fixture);
    assert_code(
        candidate.strict_law_assurance(candidate.candidate_digest(), &current, selected, &[]),
        code,
    );
    assert_inert(fixture, &before);
    assert_ne!(baseline.digest(), current.digest());
}

#[test]
fn law14_fast_mutation_corpus_rejects_named_weakening_before_authority() {
    // false requires, narrowed domain, weaker ensures, and a stale dependency
    // are separate source mutations.  The retained policy/report cannot be
    // reused by candidate acceptance after any of them.
    for (label, path, old, new) in [
        ("false_requires", "src/core.spx", "right != 0", "right == 0"),
        ("narrowed_domain", "src/core.spx", "right != 0", "right > 0"),
        (
            "weaker_ensures",
            "src/core.spx",
            "result == left + right",
            "result >= 0",
        ),
        (
            "stale_dependency",
            "src/tests.spx",
            "add(19, 23)",
            "add(18, 24)",
        ),
    ] {
        let fixture = Fixture::new(&format!("law14-{label}"));
        if label == "weaker_ensures" {
            let core = fixture.source("src/core.spx");
            let source = std::fs::read_to_string(&core).unwrap();
            let strengthened = source.replace(
                "fn add(left: i64, right: i64) -> i64\n{",
                "fn add(left: i64, right: i64) -> i64\n    ensures result == left + right\n{",
            );
            assert_ne!(strengthened, source, "weaker_ensures fixture setup drifted");
            std::fs::write(core, strengthened).unwrap();
        }
        let base = revision(&fixture);
        let baseline = LawSet::derive(&base, "law14-fast-v1", vec![static_module()]).unwrap();
        let selected = policy(&baseline);
        let report = strict::derive(&base, &baseline, &selected, &[]).unwrap();
        strict::require(&report, &base, &baseline, &selected, &[]).unwrap();
        let source = std::fs::read_to_string(fixture.source(path)).unwrap();
        assert!(source.contains(old), "{label} fixture drifted");
        std::fs::write(fixture.source(path), source.replacen(old, new, 1)).unwrap();
        let before = snapshot(&fixture);
        let next = revision(&fixture);
        let current = LawSet::derive(&next, "law14-fast-v1", vec![static_module()]).unwrap();
        assert_code(
            strict::require(&report, &next, &current, &selected, &[]),
            "SPX-LW104",
        );
        assert_inert(&fixture, &before);
        require_candidate_refusal(&fixture, &baseline, &selected, "SPX-LW104");
    }

    // Deleted law and module inventory removal must leave the independently
    // selected row required, rather than manufacturing an empty success.
    for (label, current_modules) in [
        (
            "deleted_law",
            vec![LawModule {
                laws: vec![],
                ..static_module()
            }],
        ),
        ("removed_module", vec![]),
    ] {
        let fixture = Fixture::new(&format!("law14-{label}"));
        let base = revision(&fixture);
        let baseline = LawSet::derive(&base, "law14-fast-v1", vec![static_module()]).unwrap();
        let selected = policy(&baseline);
        let current = LawSet::derive(&base, "law14-fast-v1", current_modules).unwrap();
        let before = snapshot(&fixture);
        let report = strict::derive(&base, &current, &selected, &[]).unwrap();
        assert_code(
            strict::require(&report, &base, &current, &selected, &[]),
            "SPX-LW130",
        );
        let candidate = ProjectCandidate::open(base.clone(), base.project_revision()).unwrap();
        let receipt = candidate
            .strict_law_assurance(candidate.candidate_digest(), &current, &selected, &[])
            .unwrap();
        assert_code(
            candidate.require_strict_law_assurance(&receipt, &current, &selected, &[]),
            "SPX-LW130",
        );
        assert_inert(&fixture, &before);
    }

    // Added assumptions and retargeted identities are changed law semantics,
    // not evidence that may weaken a protected baseline.
    let mutations: [(&str, fn(&mut LawModule)); 2] = [
        ("added_assumption", |module: &mut LawModule| {
            module.assumptions.push("unreviewed.assumption".into());
            module.laws[0]
                .assumption_ids
                .push("unreviewed.assumption".into());
        }),
        ("retargeted_id", |module: &mut LawModule| {
            module.laws[0].selector = LawSelector::ForbidReaches {
                claim_id: "retarget".into(),
                from: "calculator.app.main".into(),
                to: "calculator.add".into(),
            };
        }),
    ];
    for (label, mutate) in mutations {
        let fixture = Fixture::new(&format!("law14-{label}"));
        let base = revision(&fixture);
        let baseline = LawSet::derive(&base, "law14-fast-v1", vec![static_module()]).unwrap();
        let selected = policy(&baseline);
        let mut changed = static_module();
        mutate(&mut changed);
        let current = LawSet::derive(&base, "law14-fast-v1", vec![changed]).unwrap();
        let before = snapshot(&fixture);
        assert_code(strict::derive(&base, &current, &selected, &[]), "SPX-LW104");
        assert_inert(&fixture, &before);
    }

    // Duplicate identities and unsupported expressions fail during typed
    // inventory derivation, before report construction or an outer success.
    let fixture = Fixture::new("law14-invalid-inventory");
    let base = revision(&fixture);
    let mut duplicate = static_module();
    duplicate.module_id = "calculator.duplicate".into();
    assert_code(
        LawSet::derive(&base, "law14-fast-v1", vec![static_module(), duplicate]),
        "SPX-LW101",
    );
    let mut unsupported = module();
    if let LawSelector::Contract { proposition, .. } = &mut unsupported.laws[0].selector {
        *proposition = "check(right)".into();
    }
    assert_code(
        LawSet::derive(&base, "law14-fast-v1", vec![unsupported]),
        "SPX-LW101",
    );
    assert_inert(&fixture, &snapshot(&fixture));

    // A forged receipt or changed success bit cannot substitute for opaque
    // proof evidence; smaller reference-model bounds also stay insufficient.
    let fixture = Fixture::new("law14-forged-receipt");
    let base = revision(&fixture);
    let baseline = LawSet::derive(&base, "law14-fast-v1", vec![static_module()]).unwrap();
    let selected = policy(&baseline);
    let report = strict::derive(&base, &baseline, &selected, &[]).unwrap();
    let forged = report.replace(
        "\"publication_authority\":false",
        "\"publication_authority\":true",
    );
    let before = snapshot(&fixture);
    assert_code(
        strict::require(&forged, &base, &baseline, &selected, &[]),
        "SPX-LW104",
    );
    assert_inert(&fixture, &before);

    // A bounded model result cannot be promoted to a stronger requested
    // depth.  The report is still inspectable, but strict admission refuses.
    let fixture = Fixture::new("law14-smaller-model-bound");
    let base = revision(&fixture);
    let mut declared = module();
    declared.laws[0].selector = LawSelector::ModelProperty {
        model: ModelKind::Authorization,
        property: "no_uncertain_redispatch".into(),
    };
    declared.laws[0].evidence = EvidenceRequirement::ModelChecked;
    let laws = LawSet::derive(&base, "law14-fast-v1", vec![declared]).unwrap();
    let selected = StrictLawPolicy::new(
        laws.clone(),
        BTreeMap::from([(
            "calculator.divide.nonzero".into(),
            RequiredLawEvidence::ReferenceModel {
                model_digest: mc::model_digest(
                    &mc::authorization_model::DESCRIPTOR,
                    mc::authorization_model::BOUNDS,
                ),
                minimum_states: 64,
                minimum_depth: 100,
                minimum_transitions: 128,
            },
        )]),
    )
    .unwrap();
    let before = snapshot(&fixture);
    let report = strict::derive(&base, &laws, &selected, &[]).unwrap();
    assert_code(
        strict::require(&report, &base, &laws, &selected, &[]),
        "SPX-LW130",
    );
    assert_inert(&fixture, &before);
}
