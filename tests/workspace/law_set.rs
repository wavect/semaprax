//! LAW-01 protected-inventory and independent replay regressions.
use super::*;
use semaprax::assurance_manifest::law_set::{
    self as laws, ContractKind, EvidenceRequirement, LawDefinition, LawModule, LawPolicy,
    LawSelector, LawSet, ModelKind,
};

fn module() -> LawModule {
    LawModule {
        module_id: "calculator.laws".into(),
        source_path: "src/core.spx".into(),
        assumptions: vec![],
        laws: vec![LawDefinition {
            law_id: "calculator.divide.nonzero".into(),
            selector: LawSelector::Contract {
                declaration_id: "calculator.divide".into(),
                clause: ContractKind::Precondition,
                proposition: "right != 0".into(),
            },
            assumption_ids: vec![],
            requires_laws: vec![],
            evidence: EvidenceRequirement::RuntimeGuarded,
        }],
    }
}
fn inventory(fixture: &Fixture, modules: Vec<LawModule>) -> LawSet {
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        LawSet::derive(&snapshot.retain_revision(), "checked-v1", modules)
    })
    .unwrap()
}
fn report(fixture: &Fixture, candidate: &LawSet, policy: &LawPolicy) -> Value {
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let document = laws::derive_report(&revision, candidate, policy)?;
        laws::verify_report(&document, &revision, candidate, policy)?;
        Ok(wire(&document)["payload"].clone())
    })
    .unwrap()
}
fn patch_core(fixture: &Fixture, old: &str, new: &str) {
    let path = fixture.source("src/core.spx");
    let source = std::fs::read_to_string(&path).unwrap();
    assert!(source.contains(old));
    std::fs::write(path, source.replace(old, new)).unwrap();
}
#[test]
fn inventory_replays_and_existing_project_report_is_unchanged() {
    let fixture = Fixture::new("laws-roundtrip");
    let original = generate(&fixture.manifest(), &Default::default()).unwrap();
    let set = inventory(&fixture, vec![module()]);
    let policy = LawPolicy::strict(set.clone()).unwrap();
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        assert_eq!(
            LawSet::replay(&revision, "checked-v1", set.to_json())?.to_json(),
            set.to_json()
        );
        let document = laws::derive_report(&revision, &set, &policy)?;
        laws::require_satisfied(&document, &revision, &set, &policy)?;
        Ok(())
    })
    .unwrap();
    let result = report(&fixture, &set, &policy);
    assert_eq!(
        result["counts"],
        serde_json::json!({"required":1,"covered":1,"missing":0,"unsupported":0,"open":0})
    );
    assert_eq!(
        generate(&fixture.manifest(), &Default::default()).unwrap(),
        original
    );
}
#[test]
fn formatting_and_unrelated_display_rename_preserve_identity_and_semantics() {
    let fixture = Fixture::new("laws-format");
    let baseline = inventory(&fixture, vec![module()]);
    patch_core(&fixture, "right != 0", "right  !=  0");
    let core = fixture.source("src/core.spx");
    let parsed = semaprax::parse(&std::fs::read_to_string(&core).unwrap(), "src/core.spx").unwrap();
    std::fs::write(core, semaprax::format::canonical(&parsed)).unwrap();
    patch_core(&fixture, "fn is_negative", "fn negative_display");
    // Keep callers in the entry/test modules in sync with the display rename.
    for name in ["src/app.spx", "src/tests.spx"] {
        let path = fixture.source(name);
        let text = std::fs::read_to_string(&path)
            .unwrap()
            .replace("is_negative", "negative_display");
        std::fs::write(path, text).unwrap();
    }
    let mut declared = module();
    if let LawSelector::Contract { proposition, .. } = &mut declared.laws[0].selector {
        *proposition = "right  !=  0".into();
    }
    let candidate = inventory(&fixture, vec![declared]);
    assert_eq!(
        baseline.semantic_digest("calculator.divide.nonzero"),
        candidate.semantic_digest("calculator.divide.nonzero")
    );
    assert_eq!(
        report(&fixture, &candidate, &LawPolicy::strict(baseline).unwrap())["counts"]["covered"],
        1
    );
}
#[test]
fn clause_reordering_reassociates_exact_proposition_and_rejects_ambiguity() {
    let fixture = Fixture::new("laws-reorder");
    let baseline = inventory(&fixture, vec![module()]);
    let policy = LawPolicy::strict(baseline.clone()).unwrap();
    let old = report(&fixture, &baseline, &policy)["laws"][0]["obligation_id"].clone();
    patch_core(
        &fixture,
        "requires right != 0",
        "requires left >= 0\n    requires right != 0",
    );
    let candidate = inventory(&fixture, vec![module()]);
    let new = report(&fixture, &candidate, &policy);
    assert_eq!(new["counts"]["covered"], 1);
    assert_ne!(old, new["laws"][0]["obligation_id"]);
    patch_core(&fixture, "requires left >= 0", "requires right != 0");
    let candidate = inventory(&fixture, vec![module()]);
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        assert_code(
            laws::derive_report(&snapshot.retain_revision(), &candidate, &policy),
            "SPX-LW101",
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn law_clause_and_module_omissions_cannot_shrink_expected_inventory() {
    let fixture = Fixture::new("laws-delete");
    let baseline = inventory(&fixture, vec![module()]);
    let policy = LawPolicy::strict(baseline).unwrap();
    for modules in [
        vec![],
        vec![LawModule {
            laws: vec![],
            ..module()
        }],
    ] {
        let candidate = inventory(&fixture, modules);
        let result = report(&fixture, &candidate, &policy);
        assert_eq!(result["counts"]["required"], 1);
        assert_eq!(result["counts"]["missing"], 1);
        assert_eq!(result["accepted"], false);
        with_authenticated_project(&fixture.manifest(), |snapshot| {
            let revision = snapshot.retain_revision();
            let document = laws::derive_report(&revision, &candidate, &policy)?;
            assert_code(
                laws::require_satisfied(&document, &revision, &candidate, &policy),
                "SPX-LW106",
            );
            Ok(())
        })
        .unwrap();
    }
    patch_core(&fixture, "    requires right != 0\n", "");
    let candidate = inventory(&fixture, vec![module()]);
    assert_eq!(
        report(&fixture, &candidate, &policy)["counts"]["missing"],
        1
    );
}
#[test]
fn workflow_views_replay_full_inventory_and_keep_failure_verdict_across_pages() {
    let fixture = Fixture::new("laws-workflow-view");
    let baseline = inventory(&fixture, vec![module()]);
    let policy = LawPolicy::strict(baseline).unwrap();
    let candidate = inventory(&fixture, vec![]);
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let document = laws::derive_report(&revision, &candidate, &policy)?;
        let first = wire(&laws::workflow::summary(
            &document, &revision, &candidate, &policy, 0, 1, 8192,
        )?);
        assert_eq!(first["accepted"], false);
        assert_eq!(first["counts"]["required"], 1);
        assert_eq!(first["counts"]["missing"], 1);
        assert_eq!(first["laws"][0]["law_id"], "calculator.divide.nonzero");
        assert_eq!(first["laws"][0]["status"], "missing");
        let past_end = wire(&laws::workflow::summary(
            &document, &revision, &candidate, &policy, 1, 1, 8192,
        )?);
        assert_eq!(past_end["accepted"], false);
        assert_eq!(past_end["counts"], first["counts"]);
        assert_eq!(past_end["returned"], 0);
        let detail = wire(&laws::workflow::detail(
            &document,
            &revision,
            &candidate,
            &policy,
            "calculator.divide.nonzero",
            8192,
        )?);
        assert_eq!(detail["law"]["status"], "missing");
        assert_eq!(detail["repair_target"], "implementation_or_proof");
        assert_eq!(detail["specification_change_path"], "protected_law_review");
        assert_code(
            laws::workflow::summary(&document, &revision, &candidate, &policy, 0, 0, 8192),
            "SPX-LW130",
        );
        let forged = document.replace("law_definition_missing", "required_evidence_available");
        assert_code(
            laws::workflow::detail(
                &forged,
                &revision,
                &candidate,
                &policy,
                "calculator.divide.nonzero",
                8192,
            ),
            "SPX-LW101",
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn semantic_changes_and_alias_retargeting_are_refused_by_protected_baseline() {
    let fixture = Fixture::new("laws-retarget");
    let baseline = inventory(&fixture, vec![module()]);
    let policy = LawPolicy::strict(baseline.clone()).unwrap();
    for selector in [
        LawSelector::Contract {
            declaration_id: "calculator.divide".into(),
            clause: ContractKind::Precondition,
            proposition: "right > 0".into(),
        },
        LawSelector::Contract {
            declaration_id: "calculator.add".into(),
            clause: ContractKind::Precondition,
            proposition: "right != 0".into(),
        },
    ] {
        let mut changed = module();
        changed.laws[0].selector = selector;
        let candidate = inventory(&fixture, vec![changed]);
        assert_ne!(
            baseline.semantic_digest("calculator.divide.nonzero"),
            candidate.semantic_digest("calculator.divide.nonzero")
        );
        with_authenticated_project(&fixture.manifest(), |snapshot| {
            assert_code(
                laws::derive_report(&snapshot.retain_revision(), &candidate, &policy),
                "SPX-LW104",
            );
            Ok(())
        })
        .unwrap();
    }
}
#[test]
fn duplicates_dangling_dependencies_cycles_and_capacity_fail_closed() {
    let fixture = Fixture::new("laws-invalid");
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        let mut second = module();
        second.module_id = "other.laws".into();
        second.source_path = "src/app.spx".into();
        assert_code(
            LawSet::derive(&revision, "checked-v1", vec![module(), second]),
            "SPX-LW101",
        );
        for (dependencies, assumptions) in [
            (vec!["missing".into()], vec![]),
            (vec!["calculator.divide.nonzero".into()], vec![]),
            (vec![], vec!["undeclared".into()]),
        ] {
            let mut invalid = module();
            invalid.laws[0].requires_laws = dependencies;
            invalid.laws[0].assumption_ids = assumptions;
            assert_code(
                LawSet::derive(&revision, "checked-v1", vec![invalid]),
                "SPX-LW101",
            );
        }
        assert_code(
            LawSet::derive(
                &revision,
                "checked-v1",
                vec![module(); laws::MAX_MODULES + 1],
            ),
            "SPX-LW102",
        );
        assert_code(
            LawSet::replay(&revision, "checked-v1", &" ".repeat(laws::MAX_BYTES + 1)),
            "SPX-LW102",
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn malformed_unknown_duplicate_and_stale_wire_and_empty_success_are_refused() {
    let fixture = Fixture::new("laws-wire");
    let set = inventory(&fixture, vec![module()]);
    let policy = LawPolicy::strict(set.clone()).unwrap();
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        let revision = snapshot.retain_revision();
        for document in [
            set.to_json()
                .replace("\"kind\":\"contract\"", "\"kind\":\"assert_string\""),
            set.to_json().replacen("{", "{\"extra\":0,", 1),
            set.to_json()
                .replacen("\"schema\":", "\"schema\":\"duplicate\",\"schema\":", 1),
        ] {
            assert_code(
                LawSet::replay(&revision, "checked-v1", &document),
                "SPX-LW101",
            );
        }
        assert_code(
            LawSet::replay(&revision, "other-profile", set.to_json()),
            "SPX-LW104",
        );
        assert_code(
            laws::verify_report("{\"accepted\":true,\"laws\":[]}", &revision, &set, &policy),
            "SPX-LW101",
        );
        Ok(())
    })
    .unwrap();
    patch_core(&fixture, "left + right", "left - right");
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        assert_code(
            LawSet::replay(&snapshot.retain_revision(), "checked-v1", set.to_json()),
            "SPX-LW104",
        );
        Ok(())
    })
    .unwrap();
}
#[test]
fn strict_empty_policy_and_deliberate_empty_baseline_are_distinct() {
    let fixture = Fixture::new("laws-empty");
    let empty = inventory(&fixture, vec![]);
    assert_code(LawPolicy::strict(empty.clone()), "SPX-LW105");
    let policy = LawPolicy::deliberate_empty(empty.clone()).unwrap();
    assert_eq!(report(&fixture, &empty, &policy)["accepted"], true);
    assert_code(
        LawPolicy::deliberate_empty(inventory(&fixture, vec![module()])),
        "SPX-LW105",
    );
}
#[test]
fn architecture_and_existing_model_properties_use_existing_evidence_owners() {
    let fixture = Fixture::new("laws-kinds");
    let mut declared = module();
    declared.laws.push(LawDefinition {
        law_id: "calculator.architecture".into(),
        selector: LawSelector::ForbidReaches {
            claim_id: "no-divide".into(),
            from: "calculator.is-negative".into(),
            to: "calculator.divide".into(),
        },
        assumption_ids: vec![],
        requires_laws: vec![],
        evidence: EvidenceRequirement::CompilerProved,
    });
    declared.laws.push(LawDefinition {
        law_id: "model.handle".into(),
        selector: LawSelector::ModelProperty {
            model: ModelKind::Handle,
            property: "discharged_exactly_once".into(),
        },
        assumption_ids: vec![],
        requires_laws: vec![],
        evidence: EvidenceRequirement::ModelChecked,
    });
    declared.laws.push(LawDefinition {
        law_id: "model.authorization".into(),
        selector: LawSelector::ModelProperty {
            model: ModelKind::Authorization,
            property: "no_uncertain_redispatch".into(),
        },
        assumption_ids: vec![],
        requires_laws: vec![],
        evidence: EvidenceRequirement::ModelChecked,
    });
    let set = inventory(&fixture, vec![declared]);
    assert_eq!(
        report(&fixture, &set, &LawPolicy::strict(set.clone()).unwrap())["counts"]["covered"],
        4
    );
}
#[test]
fn missing_evidence_assumptions_and_dependencies_stay_open() {
    let fixture = Fixture::new("laws-open");
    let mut declared = module();
    declared.assumptions.push("environment.review".into());
    declared.laws[0]
        .assumption_ids
        .push("environment.review".into());
    let mut dependent = declared.laws[0].clone();
    dependent.law_id = "dependent".into();
    dependent.assumption_ids.clear();
    dependent
        .requires_laws
        .push("calculator.divide.nonzero".into());
    declared.laws.push(dependent);
    let mut proof = declared.laws[0].clone();
    proof.law_id = "proof".into();
    proof.assumption_ids.clear();
    proof.evidence = EvidenceRequirement::TheoremProved;
    declared.laws.push(proof);
    let set = inventory(&fixture, vec![declared]);
    let result = report(&fixture, &set, &LawPolicy::strict(set.clone()).unwrap());
    assert_eq!(result["counts"]["open"], 3);
    assert_eq!(result["counts"]["covered"], 0);
}

#[test]
fn removing_law_owner_from_project_sources_is_visible() {
    let fixture = Fixture::new("laws-source-removal");
    let mut declared = module();
    declared.source_path = "src/laws.spx".into();
    let manifest = fixture.manifest();
    let original = std::fs::read_to_string(&manifest).unwrap();
    let with_owner = original.replace("\"src/tests.spx\"", "\"src/laws.spx\", \"src/tests.spx\"");
    std::fs::write(&manifest, with_owner).unwrap();
    std::fs::write(fixture.source("src/laws.spx"), "module calculator.laws;\n\n@id(\"calculator.laws.marker\")\nfn marker() -> i64\n{\n    0\n}\n").unwrap();
    let baseline = inventory(&fixture, vec![declared.clone()]);
    let text = original;
    std::fs::write(manifest, text).unwrap();
    let candidate = inventory(&fixture, vec![declared]);
    let result = report(&fixture, &candidate, &LawPolicy::strict(baseline).unwrap());
    assert_eq!(result["counts"]["missing"], 1);
    assert_eq!(result["laws"][0]["reason"], "law_source_module_missing");
}

#[test]
fn unsupported_selectors_and_selector_injection_are_refused() {
    let fixture = Fixture::new("laws-selector-refusal");
    with_authenticated_project(&fixture.manifest(), |snapshot| {
        for expression in [
            "check(right)",
            "true\n ensures false",
            "true\n requires false",
        ] {
            let mut declared = module();
            if let LawSelector::Contract { proposition, .. } = &mut declared.laws[0].selector {
                *proposition = expression.into();
            }
            assert_code(
                LawSet::derive(&snapshot.retain_revision(), "checked-v1", vec![declared]),
                "SPX-LW101",
            );
        }
        let mut declared = module();
        declared.laws[0].selector = LawSelector::ModelProperty {
            model: ModelKind::Handle,
            property: "arbitrary-assertion".into(),
        };
        assert_code(
            LawSet::derive(&snapshot.retain_revision(), "checked-v1", vec![declared]),
            "SPX-LW101",
        );
        Ok(())
    })
    .unwrap();
}

#[path = "protected_law.rs"]
mod protected_law;

#[path = "strict_law.rs"]
mod strict_law;

#[path = "installed_law.rs"]
mod installed_law;
#[path = "structured_law.rs"]
mod structured_law;

#[path = "law14_adversarial.rs"]
mod law14_adversarial;
