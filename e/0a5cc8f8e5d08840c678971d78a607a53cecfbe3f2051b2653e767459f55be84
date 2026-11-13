use super::*;

/// Exact additive source composition for the original whole-schema assertion.
/// Each current substitution is unique; the unchanged assertion still compares
/// every reconstructed byte with its immutable pre-adoption source archive.
pub(super) fn before_summary(current: &str) -> String {
    let mut source = current.to_owned();
    for (after, before) in [
        ("let inventory = crate::cleanup::schema_summary::collect(program, function)?;\n    let has_nested_owned_bytes = inventory.has_nested_owned_bytes;", "let inventory = crate::cleanup::build_inventory(program, function)?;\n    let has_nested_owned_bytes = inventory.slots.iter().try_fold(false, |nested, slot| {\n        crate::cleanup::cleanup_shape_profile(&slot.shape)\n            .map(|profile| nested || profile.has_nested_owned_bytes)\n    })?;"),
        ("inventory.has_iterator", "inventory\n        .flags\n        .iter()\n        .any(|flag| flag.lifecycle.as_str() == crate::cleanup::ITER_DROP_LIFECYCLE_ID)"),
        ("inventory.has_variant", "inventory.schema == crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2"),
        ("\n#[cfg(test)]\npub(super) mod original_materializer;\n\n#[cfg(test)]\nmod ordinary_tests;\n", ""),
    ] {
        assert_eq!(source.matches(after).count(), 1);
        source = source.replacen(after, before, 1);
    }
    source
}

#[test]
fn ordinary_summary_matches_complete_original_schema_and_raw_diagnostics() {
    let mut saw_original_error = false;
    for program in paid_type_facts::tests::fixtures() {
        for function in &program.functions {
            assert_eq!(
                selected_schema_with_index(&program, function, None),
                original_materializer::selected_schema_with_index(&program, function, None)
            );
            let mut forged = function.clone();
            forged.cleanup_plan.schema = "forged.summary.schema";
            forged.cleanup = crate::cleanup::CleanupInventory::unresolved();
            assert_eq!(
                selected_schema_with_index(&program, &forged, None),
                original_materializer::selected_schema_with_index(&program, &forged, None)
            );
            for ty in [
                ResolvedType::Nominal {
                    declaration: crate::hir::DeclarationId::new("missing.λ\0.schema"),
                    arguments: Vec::new(),
                },
                ResolvedType::Function {
                    parameters: vec![ResolvedType::Bytes],
                    result: Box::new(ResolvedType::Bytes),
                },
            ] {
                let mut subject = function.clone();
                subject.return_type = ty;
                // Eq compares every diagnostic field, including raw UTF8 text.
                let expected =
                    original_materializer::selected_schema_with_index(&program, &subject, None);
                saw_original_error |= expected.is_err();
                assert_eq!(
                    selected_schema_with_index(&program, &subject, None),
                    expected
                );
            }
        }
    }
    assert!(saw_original_error);
}

#[test]
fn ordinary_summary_retains_original_logical_sink_failure_selection() {
    for program in paid_type_facts::tests::fixtures() {
        for function in &program.functions {
            for limit in [0, 1, usize::MAX] {
                let (expected, old_refused) = crate::bounded_output::with_limit(limit, || {
                    original_materializer::selected_schema_with_index(&program, function, None)
                });
                let (actual, current_refused) = crate::bounded_output::with_limit(limit, || {
                    selected_schema_with_index(&program, function, None)
                });
                assert_eq!(actual, expected);
                assert_eq!(current_refused, old_refused);
            }
        }
    }
}

/// The old receiving body remains identical; the explicit alias holds it on
/// the independently compiled original materializer after ordinary adoption.
pub(super) fn before_original_receiver(current: &str) -> String {
    let alias = "use super::super::original_materializer as schema;\n";
    assert_eq!(current.matches(alias).count(), 1);
    current.replacen(alias, "", 1)
}

#[test]
fn whole_schema_composition_refuses_missing_duplicate_and_foreign_source() {
    let raw = include_str!("../schema.rs");
    let immutable = include_str!("../../../../docs/evidence/schema-summary/schema.rs.before");
    for hostile in [
        raw.replacen("inventory.has_iterator", "false", 1),
        format!("{raw}inventory.has_iterator"),
        format!("{raw}// foreign current source\n"),
    ] {
        assert!(std::panic::catch_unwind(|| {
            let restored = before_summary(&hostile);
            assert_eq!(
                restored
                    .strip_suffix("\npub(super) mod paid_summary;\n")
                    .unwrap(),
                immutable
            );
        })
        .is_err());
    }
}
