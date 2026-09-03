use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{graph, parse};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

fn fixture(
    source: &str,
    patch_source: &str,
) -> (
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
    std::path::PathBuf,
) {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "semaprax-patch-evidence-unit-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let source_path = directory.join("module.spx");
    let patch_path = directory.join("change.spatch");
    let evidence_path = directory.join("evidence.json");
    std::fs::write(&source_path, source).unwrap();
    std::fs::write(&patch_path, patch_source).unwrap();
    (directory, source_path, patch_path, evidence_path)
}

fn assert_no_a0_artifacts(source_path: &Path) {
    assert!(std::fs::read_dir(source_path.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .all(|name| {
            !(name.ends_with(".semaprax-patch.lock")
                || name.contains(".semaprax-stage.") && name.ends_with(".tmp"))
        }));
}

#[test]
fn verification_rejects_oversize_before_source_semantics() {
    let (directory, source, patch, evidence) = fixture("not source", "not patch");
    std::fs::write(&evidence, vec![b'x'; MAX_EVIDENCE_BYTES + 1]).unwrap();
    let error = verify(&source, &patch, &evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G131");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn typed_assessment_count_is_an_invariant_not_an_indexing_panic() {
    let assessments = std::iter::repeat_n(("behavior", "unknown"), 8);
    let error = validated_assessments(assessments).unwrap_err();
    assert_eq!(error.code, "SPX-G133");
}

#[test]
fn workspace_child_renderer_respects_tiny_remaining_budget() {
    let source = "module evidence.child_bound;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let program = parse(source, Path::new("child-bound.spx")).unwrap();
    let patch_source = format!(
        "base {}\nrename evidence.helper to renamed\n",
        graph::revision(&program)
    );
    let preflight = crate::patch::preflight_review_owned(
        source.to_owned(),
        patch_source,
        Path::new("child-bound.spx").to_path_buf(),
        review::MAX_OPERATIONS,
    )
    .unwrap();
    let build = review::build_from_preflight(preflight).unwrap();
    let facts = facts_from_review(&build).unwrap();
    let exact = render_from_facts(&facts).unwrap();
    assert_eq!(
        render_from_facts_with_limit(&facts, MAX_EVIDENCE_BYTES)
            .unwrap()
            .artifact(),
        exact.artifact()
    );
    let Err(error) = render_from_facts_with_limit(&facts, 1) else {
        panic!("child rendering must stop at the remaining aggregate budget")
    };
    assert_eq!(error[0].code, "SPX-G131");
}

#[test]
fn evidence_v2_translates_target_bounds_and_invariants() {
    let translated = map_review_diagnostics(vec![
        Diagnostic::io("SPX-G140", "target bound"),
        Diagnostic::io("SPX-G141", "target invariant"),
    ]);
    assert_eq!(translated[0].code, "SPX-G131");
    assert_eq!(translated[1].code, "SPX-G133");
}

#[test]
fn parsed_ast_call_boundary_accepts_exact_and_rejects_limit_plus_one() {
    let source_with_calls = |count: usize| {
        let mut source =
            String::from("module evidence.call_bound;\n@id(\"app.main\") fn main()->i64{\n");
        for index in 0..count {
            source.push_str(&format!("let value{index}=missing();\n"));
        }
        source.push_str("0}\n");
        source
    };
    let exact_source = source_with_calls(review::MAX_CALL_SITES);
    let exact = parse(&exact_source, Path::new("exact.spx")).unwrap();
    assert_eq!(
        review::precheck_counts_for_test(&exact).unwrap().2,
        review::MAX_CALL_SITES
    );
    let over_source = source_with_calls(review::MAX_CALL_SITES + 1);
    let over = parse(&over_source, Path::new("over.spx")).unwrap();
    let error = review::precheck_counts_for_test(&over).unwrap_err();
    assert_eq!(error[0].code, "SPX-G120");
}

#[test]
fn parsed_ast_declaration_and_callable_boundaries_are_exact() {
    let declarations = |fields: usize| {
        let mut source = String::from("module evidence.declarations;\nrecord Row {\n");
        for index in 0..fields {
            source.push_str(&format!("field{index}: i64,\n"));
        }
        source.push_str("}\nfn main()->i64{0}\n");
        source
    };
    let exact = parse(
        &declarations(review::MAX_DECLARATIONS - 2),
        Path::new("declarations-exact.spx"),
    )
    .unwrap();
    assert_eq!(
        review::precheck_counts_for_test(&exact).unwrap().0,
        review::MAX_DECLARATIONS
    );
    let over = parse(
        &declarations(review::MAX_DECLARATIONS - 1),
        Path::new("declarations-over.spx"),
    )
    .unwrap();
    assert_eq!(
        review::precheck_counts_for_test(&over).unwrap_err()[0].code,
        "SPX-G120"
    );

    let callables = |count: usize| {
        let mut source = String::from("module evidence.callables;\n");
        for index in 0..count {
            source.push_str(&format!("fn callable{index}()->i64{{0}}\n"));
        }
        source
    };
    let exact = parse(
        &callables(review::MAX_CALLABLES),
        Path::new("callables-exact.spx"),
    )
    .unwrap();
    assert_eq!(
        review::precheck_counts_for_test(&exact).unwrap().1,
        review::MAX_CALLABLES
    );
    let over = parse(
        &callables(review::MAX_CALLABLES + 1),
        Path::new("callables-over.spx"),
    )
    .unwrap();
    assert_eq!(
        review::precheck_counts_for_test(&over).unwrap_err()[0].code,
        "SPX-G120"
    );
}

#[test]
fn owned_text_reads_accept_exact_limits_and_reject_one_more_byte() {
    let (directory, source, patch, evidence) = fixture("source", "patch");
    for (path, limit, evidence_input) in [
        (&source, review::MAX_SOURCE_BYTES, false),
        (&patch, review::MAX_PATCH_BYTES, false),
        (&evidence, MAX_EVIDENCE_BYTES, true),
    ] {
        std::fs::write(path, vec![b'x'; limit]).unwrap();
        assert_eq!(
            read_text_bounded(path, limit, "SPX-I208", evidence_input)
                .unwrap()
                .len(),
            limit
        );
        std::fs::write(path, vec![b'x'; limit + 1]).unwrap();
        assert_eq!(
            read_text_bounded(path, limit, "SPX-I208", evidence_input).unwrap_err()[0].code,
            "SPX-G131"
        );
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn generation_and_verification_reject_final_source_drift() {
    let source = "module evidence.unit;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let program = parse(source, Path::new("evidence.spx")).unwrap();
    let patch_source = format!(
        "base {}\nrename evidence.helper to renamed\n",
        graph::revision(&program)
    );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let error = generate_with_hook(&source_path, &patch_path, |phase, canonical, _| {
        if phase == ReadPhase::FinalCheck {
            std::fs::write(canonical, source.replace("{1}", "{2}"))?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    std::fs::write(&source_path, source).unwrap();
    let capsule = generate(&source_path, &patch_path).unwrap();
    std::fs::write(&evidence_path, capsule).unwrap();
    let error = verify_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, canonical, _| {
            if phase == ReadPhase::FinalCheck {
                std::fs::write(canonical, source.replace("{1}", "{2}"))?;
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn owned_patch_and_evidence_bytes_are_never_reread() {
    let source = "module evidence.owned;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
        "base {}\nrename evidence.helper to renamed\n",
        graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
    );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let expected_capsule = generate(&source_path, &patch_path).unwrap();
    let actual_capsule = generate_with_hook(&source_path, &patch_path, |phase, path, _| {
        if phase == ReadPhase::PatchRead {
            std::fs::write(path, "mutated after read\n")?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(actual_capsule, expected_capsule);

    std::fs::write(&patch_path, &patch_source).unwrap();
    std::fs::write(&evidence_path, &expected_capsule).unwrap();
    let expected_receipt = verify(&source_path, &patch_path, &evidence_path).unwrap();
    let actual_receipt = verify_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if phase == ReadPhase::EvidenceRead {
                std::fs::write(path, "mutated after read\n")?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(actual_receipt, expected_receipt);

    std::fs::write(&evidence_path, &expected_capsule).unwrap();
    let actual_receipt = verify_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if phase == ReadPhase::PatchRead {
                std::fs::write(path, "mutated after read\n")?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(actual_receipt, expected_receipt);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn evidence_apply_uses_owned_patch_and_evidence_bytes_exactly_once() {
    let source = "module evidence.apply_owned;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
        "base {}\nrename evidence.helper to renamed\n",
        graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
    );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let capsule = generate(&source_path, &patch_path).unwrap();
    let candidate = serde_json::from_str::<serde_json::Value>(&capsule).unwrap()
        ["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(&evidence_path, &capsule).unwrap();
    let revision = apply_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if phase == ApplyPhase::PatchRead || phase == ApplyPhase::EvidenceRead {
                std::fs::write(path, "mutated after owned read\n")?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(revision, candidate);
    assert!(std::fs::read_to_string(&source_path)
        .unwrap()
        .contains("fn renamed"));
    assert_no_a0_artifacts(&source_path);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn evidence_apply_rechecks_source_at_every_a0_boundary() {
    let source = "module evidence.apply_drift;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    for (label, selected) in [
        ("before-stage", ApplyPhase::BeforeStage),
        ("first-final", ApplyPhase::BeforeFinalCheck),
        ("second-final", ApplyPhase::BeforeRename),
    ] {
        let patch_source = format!(
            "base {}\nrename evidence.helper to renamed\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let changed = source.replace("{1}", "{2}");
        let error = apply_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == selected {
                    std::fs::write(path, &changed)?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207", "{label}");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), changed);
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn evidence_apply_rejects_same_bytes_with_replaced_source_identity() {
    let source = "module evidence.apply_identity;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
        "base {}\nrename evidence.helper to renamed\n",
        graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
    );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let capsule = generate(&source_path, &patch_path).unwrap();
    std::fs::write(&evidence_path, capsule).unwrap();
    let backup = source_path.with_extension("original.spx");
    let error = apply_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if phase == ApplyPhase::BeforeRename {
                std::fs::rename(path, &backup)?;
                std::fs::write(path, source)?;
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), source);
    assert_no_a0_artifacts(&source_path);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn evidence_apply_bounds_both_final_source_reads_and_cleans_stage() {
    let source = "module evidence.apply_growth;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    for selected in [ApplyPhase::BeforeFinalCheck, ApplyPhase::BeforeRename] {
        let patch_source = format!(
            "base {}\nrename evidence.helper to renamed\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let oversized = vec![b'x'; review::MAX_SOURCE_BYTES + 1];
        let error = apply_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == selected {
                    std::fs::write(path, &oversized)?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        assert_eq!(
            std::fs::metadata(&source_path).unwrap().len(),
            (review::MAX_SOURCE_BYTES + 1) as u64
        );
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn evidence_apply_rejects_stage_mutation_and_injected_rename_failure() {
    let source = "module evidence.apply_stage;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    for (label, selected, expected) in [
        ("stage", ApplyPhase::BeforeFinalCheck, "SPX-I203"),
        ("rename", ApplyPhase::BeforeRename, "SPX-I204"),
    ] {
        let patch_source = format!(
            "base {}\nrename evidence.helper to renamed\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let error = apply_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, _, staging| {
                if phase == selected {
                    if selected == ApplyPhase::BeforeFinalCheck {
                        std::fs::write(staging, "mutated stage")?;
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "injected rename rejection",
                        ));
                    }
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, expected, "{label}");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn evidence_apply_never_deletes_a_foreign_stage_path_replacement() {
    let source = "module evidence.apply_stage_identity;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
        "base {}\nrename evidence.helper to renamed\n",
        graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
    );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let capsule = generate(&source_path, &patch_path).unwrap();
    std::fs::write(&evidence_path, capsule).unwrap();
    let displaced = source_path.with_extension("owned-stage");
    let mut foreign = None;
    let error = apply_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, _, staging| {
            if phase == ApplyPhase::BeforeRename {
                std::fs::rename(staging, &displaced)?;
                std::fs::write(staging, "foreign path object")?;
                foreign = Some(staging.to_path_buf());
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I203");
    let foreign = foreign.unwrap();
    assert_eq!(
        std::fs::read_to_string(&foreign).unwrap(),
        "foreign path object"
    );
    assert!(displaced.exists());
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
    assert!(std::fs::read_dir(&directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .all(|name| !name.ends_with(".semaprax-patch.lock")));
    std::fs::remove_file(foreign).unwrap();
    std::fs::remove_file(displaced).unwrap();
    assert_no_a0_artifacts(&source_path);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn evidence_v2_apply_owns_inputs_and_replays_every_a0_boundary() {
    let source = "module evidence.v2_hooks;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let capsule = generate_v2(&source_path, &patch_path).unwrap();
    std::fs::write(&evidence_path, &capsule).unwrap();
    let revision = apply_v2_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if matches!(phase, ApplyPhase::PatchRead | ApplyPhase::EvidenceRead) {
                std::fs::write(path, "mutated after owned read\n")?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert!(revision.starts_with("sha256:"));
    assert!(std::fs::read_to_string(&source_path)
        .unwrap()
        .contains("fn renamed"));
    assert_no_a0_artifacts(&source_path);
    std::fs::remove_dir_all(directory).unwrap();

    for (label, selected, expected) in [
        ("before-stage", ApplyPhase::BeforeStage, "SPX-I207"),
        ("first-final", ApplyPhase::BeforeFinalCheck, "SPX-I207"),
        ("second-final", ApplyPhase::BeforeRename, "SPX-I207"),
    ] {
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate_v2(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let changed = source.replace("{1}", "{2}");
        let error = apply_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == selected {
                    std::fs::write(path, &changed)?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, expected, "{label}");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), changed);
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn evidence_v2_read_only_routes_own_inputs_and_reject_final_drift() {
    let source = "module evidence.v2_readonly;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let expected_capsule = generate_v2(&source_path, &patch_path).unwrap();
    let capsule = generate_v2_with_hook(&source_path, &patch_path, |phase, path, _| {
        if phase == ReadPhase::PatchRead {
            std::fs::write(path, "mutated after read\n")?;
        }
        Ok(())
    })
    .unwrap();
    assert_eq!(capsule, expected_capsule);

    std::fs::write(&patch_path, &patch_source).unwrap();
    std::fs::write(&evidence_path, &expected_capsule).unwrap();
    let expected_receipt = verify_v2(&source_path, &patch_path, &evidence_path).unwrap();
    let receipt = verify_v2_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if phase == ReadPhase::EvidenceRead {
                std::fs::write(path, "mutated after read\n")?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(receipt, expected_receipt);

    std::fs::write(&evidence_path, &expected_capsule).unwrap();
    let receipt = verify_v2_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if phase == ReadPhase::PatchRead {
                std::fs::write(path, "mutated after read\n")?;
            }
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(receipt, expected_receipt);

    std::fs::write(&patch_path, &patch_source).unwrap();
    std::fs::write(&evidence_path, &expected_capsule).unwrap();
    let error = generate_v2_with_hook(&source_path, &patch_path, |phase, path, _| {
        if phase == ReadPhase::FinalCheck {
            std::fs::write(path, source.replace("{1}", "{2}"))?;
        }
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");

    std::fs::write(&source_path, source).unwrap();
    let backup = source_path.with_extension("readonly-original.spx");
    let error = verify_v2_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if phase == ReadPhase::FinalCheck {
                std::fs::rename(path, &backup)?;
                std::fs::write(path, source)?;
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), source);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn evidence_v2_apply_rejects_stage_replacement_and_rename_failure() {
    let source = "module evidence.v2_stage;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
    for (selected, expected) in [
        (ApplyPhase::BeforeFinalCheck, "SPX-I203"),
        (ApplyPhase::BeforeRename, "SPX-I204"),
    ] {
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate_v2(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let error = apply_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, _, staging| {
                if phase == selected {
                    if selected == ApplyPhase::BeforeFinalCheck {
                        std::fs::write(staging, "mutated stage")?;
                    } else {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            "injected rename failure",
                        ));
                    }
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, expected);
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn evidence_v2_apply_rejects_same_bytes_with_replaced_source_identity() {
    let source = "module evidence.v2_identity;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let capsule = generate_v2(&source_path, &patch_path).unwrap();
    std::fs::write(&evidence_path, capsule).unwrap();
    let backup = source_path.with_extension("original.spx");
    let error = apply_v2_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if phase == ApplyPhase::BeforeRename {
                std::fs::rename(path, &backup)?;
                std::fs::write(path, source)?;
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I207");
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
    assert_eq!(std::fs::read_to_string(&backup).unwrap(), source);
    assert_no_a0_artifacts(&source_path);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn evidence_v2_apply_bounds_both_final_reads_and_preserves_foreign_stage() {
    let source = "module evidence.v2_growth;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
    for selected in [ApplyPhase::BeforeFinalCheck, ApplyPhase::BeforeRename] {
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate_v2(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let oversized = vec![b'x'; review::MAX_SOURCE_BYTES + 1];
        let error = apply_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == selected {
                    std::fs::write(path, &oversized)?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        assert_eq!(
            std::fs::metadata(&source_path).unwrap().len(),
            (review::MAX_SOURCE_BYTES + 1) as u64
        );
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }

    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let capsule = generate_v2(&source_path, &patch_path).unwrap();
    std::fs::write(&evidence_path, capsule).unwrap();
    let displaced = source_path.with_extension("owned-stage");
    let mut foreign = None;
    let error = apply_v2_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, _, staging| {
            if phase == ApplyPhase::BeforeRename {
                std::fs::rename(staging, &displaced)?;
                std::fs::write(staging, "foreign v2 stage")?;
                foreign = Some(staging.to_path_buf());
            }
            Ok(())
        },
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-I203");
    let foreign = foreign.unwrap();
    assert_eq!(
        std::fs::read_to_string(&foreign).unwrap(),
        "foreign v2 stage"
    );
    assert!(displaced.exists());
    assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
    std::fs::remove_file(foreign).unwrap();
    std::fs::remove_file(displaced).unwrap();
    assert_no_a0_artifacts(&source_path);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn evidence_v2_apply_acquires_lock_before_owned_reads() {
    let source = "module evidence.v2_lock;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    let capsule = generate_v2(&source_path, &patch_path).unwrap();
    std::fs::write(&evidence_path, capsule).unwrap();
    apply_v2_with_hook(
        &source_path,
        &patch_path,
        &evidence_path,
        |phase, path, _| {
            if phase == ApplyPhase::PatchRead {
                let names = std::fs::read_dir(path.parent().unwrap())?
                    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                    .collect::<Vec<_>>();
                assert!(names
                    .iter()
                    .any(|name| name.ends_with(".semaprax-patch.lock")));
            }
            Ok(())
        },
    )
    .unwrap();
    assert_no_a0_artifacts(&source_path);
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn evidence_v2_apply_preserves_source_permissions() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let source = "module evidence.v2_permissions;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
    let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
    std::fs::set_permissions(&source_path, std::fs::Permissions::from_mode(0o640)).unwrap();
    let before = std::fs::metadata(&source_path).unwrap();
    let capsule = generate_v2(&source_path, &patch_path).unwrap();
    std::fs::write(&evidence_path, capsule).unwrap();
    apply_v2(&source_path, &patch_path, &evidence_path).unwrap();
    let after = std::fs::metadata(&source_path).unwrap();
    assert_eq!(after.mode() & 0o777, before.mode() & 0o777);
    assert_eq!(after.uid(), before.uid());
    assert_eq!(after.gid(), before.gid());
    assert_no_a0_artifacts(&source_path);
    std::fs::remove_dir_all(directory).unwrap();
}
