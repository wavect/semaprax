use super::super::{cached_exact, dependency_identity_max};
use super::{input_cost, limit_error, retained_upper, StructuralCost};
use crate::prelude::base_schema::{BASE, OPTION, RESULT};

struct RestoreLimit(usize);
impl Drop for RestoreLimit {
    fn drop(&mut self) {
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
    }
}

#[test]
fn borrowed_metadata_matches_independent_ast_inputs_without_allocating() {
    // The complete original constructors, independently retained in prelude,
    // are checked against the producer's metadata by its own physical gate.
    let held = crate::prelude::declarations();
    for (selected, declarations) in [
        (&[][..], &held[0..0]),
        (&[&OPTION][..], &held[0..1]),
        (&[&RESULT][..], &held[1..2]),
        (&BASE[..], &held[0..2]),
    ] {
        let ((actual, upper), allocation) = crate::test_allocation_observer::measure(|| {
            (
                input_cost(selected).unwrap(),
                retained_upper(selected, 20).unwrap(),
            )
        });
        assert_eq!(upper == 0, selected.is_empty());
        assert_eq!(allocation.allocations, 0);
        assert_eq!(allocation.reallocations, 0);
        assert_eq!(allocation.live, 0);
        assert!(!allocation.invalid);
        let mut expected = StructuralCost::new();
        let mut slots = 0;
        for declaration in declarations {
            super::super::ast_type_declaration_cost(declaration, &mut expected).unwrap();
            slots += super::super::ast_type_declaration_identity_slots(declaration).unwrap();
        }
        assert_eq!(
            (actual.0.total, actual.0.string_bytes, actual.1),
            (expected.total, expected.string_bytes, slots)
        );
    }
}

#[test]
fn complete_saved_forecast_differs_only_by_base_prelude_retention() {
    for source in [
        "module prelude.forecast; @id(\"entry\") fn main() -> i64 { 0 }",
        // A parser-only shadow control: resolution admission remains separate.
        "module prelude.forecast; @id(\"shadow.option\") record Option { } @id(\"entry\") fn main() -> i64 { 0 }",
        "module prelude.forecast; @id(\"shadow.result\") record Result { } @id(\"entry\") fn main() -> i64 { 0 }",
    ] {
        let programs = [crate::parse(source, "forecast.spx").unwrap()];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let maximum = dependency_identity_max(&programs[0], &authored, &programs).unwrap();
        let delta = retained_upper(&BASE, maximum).unwrap();
        let proof = super::super::call_carriers::count(&programs[0], &authored, true);
        assert_eq!(proof.roots, 0);
        for mode in 0..=7 {
            for retention in [cached_exact::Retention::Parsed, cached_exact::Retention::Semantic] {
                let current = super::super::held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
                    &programs[0], &authored, &programs, Some(maximum), true, (mode, retention), None).unwrap();
                let saved = super::super::prelude_forecast_original::synthetic_builder_bytes_scoped_with_retention_with_registry(
                    &programs[0], &authored, &programs, Some(maximum), true, (mode, retention), None).unwrap();
                let invoice = if mode >= 5 { proof.bytes } else { 0 };
                assert_eq!(current.retained_hir, saved.retained_hir + delta + invoice);
                assert_eq!(current.retained_clone_and_hir, saved.retained_clone_and_hir + delta + invoice);
                assert_eq!(current.raw_clone_and_hir, saved.raw_clone_and_hir + delta + invoice);
                assert_eq!(current.runtime, saved.runtime);
                assert_eq!(current.synthetic_ast, saved.synthetic_ast);
                assert_eq!(current.transient_import_clone, saved.transient_import_clone);
            }
        }
    }
}

#[test]
fn independent_prelude_term_has_exact_short_zero_and_sticky_refusal_controls() {
    let expected = retained_upper(&BASE, 20).unwrap();
    let old = crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(expected));
    let _restore = RestoreLimit(old);
    assert_eq!(retained_upper(&BASE, 20).unwrap(), expected);
    for limit in [expected - 1, 0] {
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|active| active.set(limit));
        let error = retained_upper(&BASE, 20).unwrap_err();
        assert_eq!(error, vec![limit_error("builder_bytes", limit)]);
        // Empty selection remains an allocation-free census at the same bound.
        let (empty, allocation) =
            crate::test_allocation_observer::measure(|| retained_upper(&[], 0).unwrap());
        assert_eq!(empty, 0);
        assert_eq!(allocation.allocated, 0);
        use crate::bounded_output::{reserve_active_required, with_limit_usage_refusal};
        let (sticky_error, refused, paid, first) = with_limit_usage_refusal(0, || {
            assert!(!reserve_active_required(1));
            let selected_error = retained_upper(&BASE, 20).unwrap_err();
            assert_eq!(retained_upper(&[], 0).unwrap(), 0);
            assert!(!reserve_active_required(0));
            selected_error
        });
        // The unfunded private boundary cannot render public text after the
        // first sticky refusal. Preserve the complete raw six-field receipt;
        // the rich boundary above still compares its complete public receipt.
        assert_eq!(
            sticky_error.as_slice(),
            &[crate::diagnostic::Diagnostic::io_static("SPX-G171", "")]
        );
        assert_eq!(sticky_error[0].message_owned_capacity(), 0);
        assert!(refused);
        assert_eq!(paid, 0);
        assert_eq!(
            first,
            Some(crate::bounded_output::ReservationRefusal {
                requested: 1,
                remaining: 0,
                floor: 0,
                stage: None,
            })
        );
    }
}

#[test]
fn complete_forecast_exact_short_zero_receipts_and_resolver_drop() {
    let source = "module prelude.boundary; @id(\"entry\") fn main() -> i64 { 0 }";
    let programs = [crate::parse(source, "boundary.spx").unwrap()];
    let authored = crate::workspace_graph::index_authored(&programs).unwrap();
    let maximum = dependency_identity_max(&programs[0], &authored, &programs).unwrap();
    let forecast = || {
        super::super::held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
            &programs[0],
            &authored,
            &programs,
            Some(maximum),
            true,
            (5, cached_exact::Retention::Parsed),
            None,
        )
    };
    let exact = forecast().unwrap().raw_clone_and_hir;
    let old = crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(exact));
    let _restore = RestoreLimit(old);
    assert_eq!(forecast().unwrap().raw_clone_and_hir, exact);
    for limit in [exact - 1, 0] {
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|active| active.set(limit));
        let error = forecast().err().unwrap();
        assert_eq!(error, vec![limit_error("builder_bytes", limit)]);
        assert_eq!(forecast().err().unwrap(), error);
    }
    crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(old));
    let checked = crate::check(source, "boundary.spx").unwrap();
    let (_, allocation) = crate::test_allocation_observer::measure(|| {
        let resolved = crate::hir::resolve(&checked).unwrap();
        crate::hir::validate(&resolved).unwrap();
        assert!(!resolved.types.is_empty());
        drop(resolved);
    });
    assert!(!allocation.invalid);
    assert!(allocation.highwater <= exact);
    assert_eq!(allocation.live, 0);
    assert_eq!(allocation.allocated, allocation.freed);
}

// Closed acquisition for this older five-owner proof; every original inverse follows.
fn receiving_pre286_source(path: &str, current: &str) -> String {
    let entry = crate::source_reflows::next_custody_before("prelude286", path, current);
    let current = entry.as_str();
    use sha2::{Digest, Sha256};
    let hash = |source: &str| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes())));
    let (length, digest, before_length, before_digest) = match path {
        "src/prelude.rs" => (46154, "e7ad3dacb0541f2c4558a6826aaef49f05a3cbdec2ae358061f6e387017f6a51", 46130, "32a5461db85f2832bb788ed32efd1df82eb8f48ffe44c51c533ceb20a596b478"),
        "src/workspace_graph/expected_projection.rs" => (59903, "6c6e07a5bc02662dbc273eac0bb9fa2cb3f580134dd208f58212fc6f64528f5e", 59698, "cdd35c1e31d513032b5f4943ee86efdb7cce9dd4a8b77e152b4640d2ba094ab4"),
        "src/workspace_graph/expected_projection/held_projection.rs" => (22889, "8443e4086088b9a623df9f7b7cd9755a0e69664fc4a60066bf5e8da2e6797710", 21826, "6c53a3d48bbd7bd7d269c18d0214fead0908c582a3dcad46a24ce14c9cdd28ed"),
        "src/source_verify/constructor_tests.rs" => (17238, "3bb066087c62d99a1169a6eabb6a24af2c786afb3561a18e6fd0641ac33d944d", 17125, "79fc6385f511d98cdae7404b9100abc21dbd1e793dec7e004509f5f545def0e3"),
        "tests/workspace/semantic_graph/source_inventory.rs" => (174167, "d80ad38594194381610e5d99c491883678e7acdb8209dd913c23a04b45d6a6fa", 45102, "4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14"),
        _ => panic!("foreign prelude-forecast source owner"),
    };
    assert_eq!((current.len(), hash(current)), (length, digest.to_owned()));
    let mut restored = current.to_owned();
    match path {
        "src/prelude.rs" => {
            assert_eq!(restored.matches(r########"program.functions().profile_iter().any(function_uses_box)"########).count(), 1);
            restored = restored.replacen(r########"program.functions().profile_iter().any(function_uses_box)"########, r########"program.functions().iter().any(function_uses_box)"########, 1);
            assert_eq!(restored.matches(r########"program.functions().profile_iter().any(function_uses_vec_v3)"########).count(), 1);
            restored = restored.replacen(r########"program.functions().profile_iter().any(function_uses_vec_v3)"########, r########"program.functions().iter().any(function_uses_vec_v3)"########, 1);
            assert_eq!(restored.matches(r########"program.functions().profile_iter().any(function_uses_vec)"########).count(), 1);
            restored = restored.replacen(r########"program.functions().profile_iter().any(function_uses_vec)"########, r########"program.functions().iter().any(function_uses_vec)"########, 1);
        }
        "src/workspace_graph/expected_projection.rs" => {
            assert_eq!(restored.matches(r########"#[path = "expected_projection/shared_local_callees.rs"]
mod shared_local_callees;
#[cfg(test)]
#[path = "expected_projection/shared_local_callees/before_projection.rs"]
mod before_shared_local_projection;
"########).count(), 1);
            restored = restored.replacen(r########"#[path = "expected_projection/shared_local_callees.rs"]
mod shared_local_callees;
#[cfg(test)]
#[path = "expected_projection/shared_local_callees/before_projection.rs"]
mod before_shared_local_projection;
"########, r########""########, 1);
        }
        "src/workspace_graph/expected_projection/held_projection.rs" => {
            restored = super::super::call_carriers::inverse::pre916_held_dc45(current);
            assert_eq!(restored.matches(r########"    #[cfg(test)]
    super::super::forecast_observation::record_components(
        &program.path,
        layout_mode,
        retention == cached_exact::Retention::Semantic,
        include_imports,
        [
            raw.total,
            raw.string_bytes,
            raw.exact_expressions,
            raw.literal_fixed_discount,
            runtime.total,
            runtime.string_bytes,
            runtime.exact_expressions,
            structure_input,
            string_input,
            prior_fixed_hir_upper,
            fixed_hir_upper,
            identity_occurrence_upper,
            identity_carrier_bytes,
            retained_clone_and_hir,
            generic_instances.bytes,
            transient_import_clone,
            original_mode7_fixed_upper,
            prelude_hir_upper,
            hir_upper,
        ],
    );
"########).count(), 1);
            restored = restored.replacen(r########"    #[cfg(test)]
    super::super::forecast_observation::record_components(
        &program.path,
        layout_mode,
        retention == cached_exact::Retention::Semantic,
        include_imports,
        [
            raw.total,
            raw.string_bytes,
            raw.exact_expressions,
            raw.literal_fixed_discount,
            runtime.total,
            runtime.string_bytes,
            runtime.exact_expressions,
            structure_input,
            string_input,
            prior_fixed_hir_upper,
            fixed_hir_upper,
            identity_occurrence_upper,
            identity_carrier_bytes,
            retained_clone_and_hir,
            generic_instances.bytes,
            transient_import_clone,
            original_mode7_fixed_upper,
            prelude_hir_upper,
            hir_upper,
        ],
    );
"########, r########"    #[cfg(test)]
    super::super::forecast_observation::record(
        &program.path,
        layout_mode,
        retention == cached_exact::Retention::Semantic,
        [
            raw.total,
            raw.string_bytes,
            raw.exact_expressions,
            raw.literal_fixed_discount,
            runtime.total,
            runtime.string_bytes,
            runtime.exact_expressions,
            structure_input,
            string_input,
            prior_fixed_hir_upper,
            fixed_hir_upper,
            identity_occurrence_upper,
            identity_carrier_bytes,
            retained_clone_and_hir,
            generic_instances.bytes,
            transient_import_clone,
            original_mode7_fixed_upper,
        ],
    );
"########, 1);
        }
        "src/source_verify/constructor_tests.rs" => {
            assert_eq!(restored.matches(r########"        include_str!("declaration/record_shape.rs"),
        include_str!("declaration/record_shape_before.rs"),
"########).count(), 1);
            restored = restored.replacen(r########"        include_str!("declaration/record_shape.rs"),
        include_str!("declaration/record_shape_before.rs"),
"########, r########""########, 1);
        }
        "tests/workspace/semantic_graph/source_inventory.rs" => {
            restored = crate::source_reflows_merge::main_e177_final_inputs::value_direct(path, "4a1920f6abdff90036fe9afabe687c091b08a38d4f26a28e2e7c7feb678cbd14", current);
        }
        _ => unreachable!(),
    }
    assert_eq!((restored.len(), hash(&restored)), (before_length, before_digest.to_owned()));
    restored
}

#[test]
fn complete_receiving_sources_reconstruct_pinned_originals() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/prelude-forecast-286/exact-source-inverses.json"
    )))
    .unwrap();
    let currents = [
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/prelude.rs")),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/workspace_graph/expected_projection.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/workspace_graph/expected_projection/held_projection.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/source_verify/constructor_tests.rs"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/workspace/semantic_graph/source_inventory.rs"
        )),
    ];
    let originals = [
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prelude-forecast-286/src__prelude.rs.before.txt")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prelude-forecast-286/src__workspace_graph__expected_projection.rs.before.txt")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prelude-forecast-286/src__workspace_graph__expected_projection__held_projection.rs.before.txt")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prelude-forecast-286/src__source_verify__constructor_tests.rs.before.txt")),
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/prelude-forecast-286/tests__workspace__semantic_graph__source_inventory.rs.before.txt")),
    ];
    let hashes = [
        "acfbd839aee3ce120bb05677374efb0005e7c3f324cd0dbd4976c5a09942fd80",
        "65533cf9ba31f97cbc6618222b5a99bbb6b89dda14c34b686e541a8b77ce3a4c",
        "78952cede1ba2ad17dddc43fb22a3e3c423154755721f34f3e58c6bcad62f187",
        "a47d52960691d9ca39896dc938a4c8ad320eddaf37d406da7e67eddaeeac2cc0",
        "b3e5276b6bd016d1632ed6349ce34cd2e25fce762627b12c4416790ac02842be",
    ];
    assert_eq!(records.as_array().unwrap().len(), currents.len());
    for (((record, current), original), digest) in records
        .as_array()
        .unwrap()
        .iter()
        .zip(currents)
        .zip(originals)
        .zip(hashes)
    {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            digest
        );
        assert_eq!(record["sha256"].as_str().unwrap(), digest);
        let current_stage = receiving_pre286_source(record["path"].as_str().unwrap(), current);
        let current = current_stage.as_str();
        let mut restored = super::super::call_carriers::inverse::pre898(current);
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original);
    }
    assert_eq!(include_str!("prelude_forecast_original.rs"), originals[2]);
}

#[test]
fn historical_pre898_complete_saved_forecast_differs_only_by_base_prelude_retention() {
    for source in [
        "module prelude.forecast; @id(\"entry\") fn main() -> i64 { 0 }",
        // A parser-only shadow control: resolution admission remains separate.
        "module prelude.forecast; @id(\"shadow.option\") record Option { } @id(\"entry\") fn main() -> i64 { 0 }",
        "module prelude.forecast; @id(\"shadow.result\") record Result { } @id(\"entry\") fn main() -> i64 { 0 }",
    ] {
        let programs = [crate::parse(source, "forecast.spx").unwrap()];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let maximum = dependency_identity_max(&programs[0], &authored, &programs).unwrap();
        let delta = retained_upper(&BASE, maximum).unwrap();
        for mode in 0..=7 {
            for retention in [cached_exact::Retention::Parsed, cached_exact::Retention::Semantic] {
                let current = super::super::call_carriers_original::synthetic_builder_bytes_scoped_with_retention_with_registry(
                    &programs[0], &authored, &programs, Some(maximum), true, (mode, retention), None).unwrap();
                let saved = super::super::prelude_forecast_original::synthetic_builder_bytes_scoped_with_retention_with_registry(
                    &programs[0], &authored, &programs, Some(maximum), true, (mode, retention), None).unwrap();
                assert_eq!(current.retained_hir, saved.retained_hir + delta);
                assert_eq!(current.retained_clone_and_hir, saved.retained_clone_and_hir + delta);
                assert_eq!(current.raw_clone_and_hir, saved.raw_clone_and_hir + delta);
                assert_eq!(current.runtime, saved.runtime);
                assert_eq!(current.synthetic_ast, saved.synthetic_ast);
                assert_eq!(current.transient_import_clone, saved.transient_import_clone);
            }
        }
    }
}
