//! Complete joined frozen carrier/helper and unchanged service source audit.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
#[test]
fn complete_frozen_receiver_and_shared_boundaries_match_static_body_provenance() {
    let proof: serde_json::Value = serde_json::from_str(ci_str!(
        "../../../docs/evidence/borrowed-root-bindings/source-provenance.json"
    ))
    .unwrap();
    let frozen = [
        ("src/hir/resolve_expr.rs", ci_str!("../borrowed_root_oracle/resolve_expr.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr.rs.txt")),
        ("src/hir/resolve_expr_frame.rs", ci_str!("../borrowed_root_oracle/resolve_expr_frame.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr_frame.rs.txt")),
        ("src/hir/resolve_for.rs", ci_str!("../borrowed_root_oracle/resolve_for.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for.rs.txt")),
        ("src/hir/resolve_for_own.rs", ci_str!("../borrowed_root_oracle/resolve_for_own.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for_own.rs.txt")),
        ("src/hir/resolve_box_call.rs", ci_str!("../borrowed_root_oracle/resolve_box_call.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_box_call.rs.txt")),
        ("src/hir/resolve_vec_call.rs", ci_str!("../borrowed_root_oracle/resolve_vec_call.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_vec_call.rs.txt")),
        ("src/hir/resolve_expr/sort_keys.rs", ci_str!("../borrowed_root_oracle/resolve_expr/sort_keys.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__sort_keys.rs.txt")),
        ("src/hir/resolve_expr/super_method.rs", ci_str!("../borrowed_root_oracle/resolve_expr/super_method.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__super_method.rs.txt")),
        ("src/hir/resolve_expr/capacity.rs", ci_str!("../borrowed_root_oracle/resolve_expr/capacity.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__capacity.rs.txt")),
        ("src/hir/resolve_expr/vec_field.rs", ci_str!("../borrowed_root_oracle/resolve_expr/vec_field.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__vec_field.rs.txt")),
        ("src/hir/resolve_expr/finish.rs", ci_str!("../borrowed_root_oracle/resolve_expr/finish.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__finish.rs.txt")),
        ("src/hir/resolve_expr/vec_constructor.rs", ci_str!("../borrowed_root_oracle/resolve_expr/vec_constructor.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__vec_constructor.rs.txt")),
        ("src/hir/resolve_expr/lexical_exit.rs", ci_str!("../borrowed_root_oracle/resolve_expr/lexical_exit.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__lexical_exit.rs.txt")),
        ("src/hir/resolve_expr/path_arena_regression_tests.rs", ci_str!("../borrowed_root_oracle/resolve_expr/path_arena_regression_tests.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__path_arena_regression_tests.rs.txt")),
        ("src/hir/resolve_expr/path_arena_tests.rs", ci_str!("../borrowed_root_oracle/resolve_expr/path_arena_tests.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__path_arena_tests.rs.txt")),
        ("src/hir/resolve_expr/literal_format.rs", ci_str!("../borrowed_root_oracle/resolve_expr/literal_format.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__literal_format.rs.txt")),
        ("src/hir/resolve_expr/variable.rs", ci_str!("../borrowed_root_oracle/resolve_expr/variable.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__variable.rs.txt")),
        ("src/hir/resolve_expr/path_arena.rs", ci_str!("../borrowed_root_oracle/resolve_expr/path_arena.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__path_arena.rs.txt")),
        ("src/hir/resolve_expr/byte_view.rs", ci_str!("../borrowed_root_oracle/resolve_expr/byte_view.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__byte_view.rs.txt")),
        ("src/hir/resolve_expr/capacity/tests.rs", ci_str!("../borrowed_root_oracle/resolve_expr/capacity/tests.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__capacity__tests.rs.txt")),
        ("src/hir/resolve_expr_frame/capacity_old.rs", ci_str!("../borrowed_root_oracle/resolve_expr_frame/capacity_old.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr_frame__capacity_old.rs.txt")),
        ("src/hir/resolve_expr_frame/fixed_results.rs", ci_str!("../borrowed_root_oracle/resolve_expr_frame/fixed_results.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr_frame__fixed_results.rs.txt")),
        ("src/hir/resolve_expr_frame/fixed_results/tests.rs", ci_str!("../borrowed_root_oracle/resolve_expr_frame/fixed_results/tests.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr_frame__fixed_results__tests.rs.txt")),
        ("src/hir/resolve_for/bytes.rs", ci_str!("../borrowed_root_oracle/resolve_for/bytes.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for__bytes.rs.txt")),
        ("src/hir/resolve_for/record.rs", ci_str!("../borrowed_root_oracle/resolve_for/record.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for__record.rs.txt")),
        ("src/hir/resolve_for/bytes/tests.rs", ci_str!("../borrowed_root_oracle/resolve_for/bytes/tests.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for__bytes__tests.rs.txt")),
        ("src/hir/resolve_box_call/list.rs", ci_str!("../borrowed_root_oracle/resolve_box_call/list.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_box_call__list.rs.txt")),
        ("src/hir/resolve_box_call/map.rs", ci_str!("../borrowed_root_oracle/resolve_box_call/map.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_box_call__map.rs.txt")),
        ("src/hir/resolve_box_call/iterator.rs", ci_str!("../borrowed_root_oracle/resolve_box_call/iterator.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_box_call__iterator.rs.txt")),
        ("src/hir/closure/resolve.rs", ci_str!("../borrowed_root_oracle/closure/resolve.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__closure__resolve.rs.txt")),
        ("src/hir/closure/once.rs", ci_str!("../borrowed_root_oracle/closure/once.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__closure__once.rs.txt")),
        ("src/hir/closure/mutable.rs", ci_str!("../borrowed_root_oracle/closure/mutable.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__closure__mutable.rs.txt")),
        ("src/hir/resolve_pattern.rs", ci_str!("../borrowed_root_oracle/resolve_pattern.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_pattern.rs.txt")),
        ("src/hir/resolve_statement.rs", ci_str!("../borrowed_root_oracle/resolve_statement.rs"), ci_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_statement.rs.txt")),
    ];
    assert_eq!(
        frozen.len(),
        proof["frozen_receivers"].as_array().unwrap().len()
    );
    for ((source, receiver, original), record) in frozen
        .iter()
        .zip(proof["frozen_receivers"].as_array().unwrap())
    {
        assert_eq!(*source, record["source"].as_str().unwrap());
        assert_eq!(
            digest(original),
            record["frozen_sha256"].as_str().unwrap(),
            "entire original source {source}"
        );
        let mut receiver = receiver.to_string();
        if *source == "src/hir/resolve_expr/vec_field.rs" {
            // The later source-form origin tag is carried through the frozen
            // resolver verbatim; it does not change the original field read.
            for hook in [
                "                origin: crate::hir::VecFieldReadOrigin::Explicit,\n",
                "            origin,\n",
                "                origin: *origin,\n",
            ] {
                assert_eq!(receiver.matches(hook).count(), 1);
                receiver = receiver.replacen(hook, "", 1);
            }
        }
        assert_eq!(
            digest(&receiver),
            record["receiver_sha256"].as_str().unwrap(),
            "entire frozen receiver source {source}"
        );
        assert!(record["complete_function_bodies"].as_u64().unwrap() > 0);
    }
    let original_closure = qualified_closure_identity_projection();
    let original_mutable = qualified_mutable_walk_projection();
    let unchanged = [
        (
            "src/hir/resolve_expr/path_arena.rs",
            ci_str!("../resolve_expr/path_arena.rs"),
        ),
        ("src/hir/closure.rs", ci_str!("../closure.rs")),
        (
            "src/hir/closure/resolve.rs",
            ci_str!("../closure/resolve.rs"),
        ),
        ("src/hir/closure/once.rs", ci_str!("../closure/once.rs")),
        (
            "src/hir/closure/mutable.rs",
            ci_str!("../closure/mutable.rs"),
        ),
        ("src/hir/closure/paid.rs", ci_str!("../closure/paid.rs")),
        (
            "src/hir/closure/paid/copy.rs",
            ci_str!("../closure/paid/copy.rs"),
        ),
        (
            "src/hir/closure/paid/tests.rs",
            ci_str!("../closure/paid/tests.rs"),
        ),
        (
            "src/hir/closure/paid/original_tests.rs",
            ci_str!("../closure/paid/original_tests.rs"),
        ),
        (
            "src/hir/resolve_expr_reference.rs",
            ci_str!("../resolve_expr_reference.rs"),
        ),
        ("src/hir/capacity_probe.rs", ci_str!("../capacity_probe.rs")),
    ];
    assert_eq!(
        unchanged.len(),
        proof["unchanged_full_sources"].as_array().unwrap().len()
    );
    for ((source, current), record) in unchanged
        .iter()
        .zip(proof["unchanged_full_sources"].as_array().unwrap())
    {
        assert_eq!(*source, record["path"].as_str().unwrap());
        // Keep the original full-source digest; the one migrated producer is
        // additionally bound to exact full-current source equality above.
        let publication_source = if *source == "src/hir/closure/resolve.rs" {
            original_closure
        } else if *source == "src/hir/closure/mutable.rs" {
            original_mutable
        } else {
            current
        };
        assert_eq!(
            digest(publication_source),
            record["sha256"].as_str().unwrap(),
            "unchanged full publication/service {source}"
        );
    }
    let carrier = concat!(
        include_str!("../resolver_scope.rs"),
        include_str!("../borrowed_root_oracle.rs"),
        include_str!("tests.rs"),
        include_str!("source_audit.rs")
    );
    assert!(carrier.contains("Rc<Cow<"));
    assert!(carrier.contains("Cow::Borrowed(bindings)"));
    let frozen_dispatch = ci_str!("../borrowed_root_oracle/resolve_expr.rs");
    assert!(frozen_dispatch.contains("Rc::new(bindings.clone())"));
    assert!(!frozen_dispatch.contains("Cow::"));
}

/// Preserve the full original and qualify precisely one additive producer hook.
/// Every other source byte, including all diagnostic literals, is unchanged.
fn qualified_closure_identity_projection() -> &'static str {
    let original = include_str!(
        "../../../docs/evidence/borrowed-root-bindings/src__hir__closure__resolve.rs.txt"
    );
    let current = include_str!("../closure/resolve.rs");
    let old = "&DeclarationId::new(source_function.stable_id.clone())";
    let held = "&self.declarations.held_identity(&source_function.stable_id)";
    assert_eq!(original.matches(old).count(), 1);
    assert_eq!(original.matches(held).count(), 0);
    assert_eq!(current.matches(old).count(), 0);
    assert_eq!(current.matches(held).count(), 1);
    assert_eq!(current, original.replacen(old, held, 1));
    original
}

#[test]
fn full_current_closure_source_is_exact_original_held_identity_projection() {
    let original = qualified_closure_identity_projection();
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/borrowed-root-bindings/source-provenance.json"
    ))
    .unwrap();
    let record = proof["unchanged_full_sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|record| record["path"] == "src/hir/closure/resolve.rs")
        .unwrap();
    assert_eq!(digest(original), record["sha256"].as_str().unwrap());
    assert_eq!(
        record["sha256"].as_str().unwrap(),
        "bd47e88b57a91b87c272da799f45834097b43dd069ff3c652d3af53a4754e9ea"
    );
}

// Exactly three fallible traversal hooks; every old source byte remains bound.
fn qualified_mutable_walk_projection() -> &'static str {
    let original = include_str!(
        "../../../docs/evidence/borrowed-root-bindings/src__hir__closure__mutable.rs.txt"
    );
    let retained = include_str!(
        "../../../docs/evidence/overlay-transitive-funding/mutable_closure_before_paid_walk.txt"
    );
    let current = include_str!("../closure/mutable.rs");
    assert_eq!(retained, original);
    let old = "function_value::walk(function,";
    let paid = "function_value::try_walk(function,";
    assert_eq!(original.matches(old).count(), 3);
    assert_eq!(original.matches(paid).count(), 0);
    assert_eq!(original.matches("\n        });").count(), 3);
    assert_eq!(current.matches(old).count(), 0);
    assert_eq!(current.matches(paid).count(), 3);
    assert_eq!(current.matches("\n        })?;").count(), 3);
    let expected = original
        .replace(old, paid)
        .replace("\n        });", "\n        })?;");
    assert_eq!(current, expected);
    original
}
#[test]
fn full_current_mutable_source_is_exact_original_three_paid_walk_hooks() {
    let original = qualified_mutable_walk_projection();
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/borrowed-root-bindings/source-provenance.json"
    ))
    .unwrap();
    let record = proof["unchanged_full_sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == "src/hir/closure/mutable.rs")
        .unwrap();
    assert_eq!(digest(original), record["sha256"].as_str().unwrap());
}
