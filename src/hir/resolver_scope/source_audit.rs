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
        let physical = format!(
            "src/hir/borrowed_root_oracle/{}",
            source.strip_prefix("src/hir/").unwrap()
        );
        let mut receiver = receiver_before(&physical, receiver);
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
    let original_capacity = qualified_residual_capacity_projection();
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
        let unchanged_before = if matches!(
            *source,
            "src/hir/closure/resolve.rs"
                | "src/hir/closure/mutable.rs"
                | "src/hir/capacity_probe.rs"
        ) {
            None
        } else {
            Some(receiver_before(source, current))
        };
        let publication_source = if *source == "src/hir/closure/resolve.rs" {
            original_closure
        } else if *source == "src/hir/closure/mutable.rs" {
            original_mutable
        } else if *source == "src/hir/capacity_probe.rs" {
            original_capacity
        } else {
            unchanged_before.as_deref().unwrap()
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
    let current = receiver_before("src/hir/closure/resolve.rs", current);
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
    let current = receiver_before("src/hir/closure/mutable.rs", current);
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

// Keep the complete publication source while admitting only the two funded
// residual-box capacity additions. Neither old field traversal is replaced.
fn qualified_residual_capacity_projection() -> &'static str {
    let original = include_str!(
        "../../../docs/evidence/borrowed-root-bindings/src__hir__capacity_probe.rs.txt"
    );
    let current = include_str!("../capacity_probe.rs");
    let current = receiver_before("src/hir/capacity_probe.rs", current);
    let old = "                + resolved_type_owned_capacity(residual_type);";
    let funded = "                + std::mem::size_of::<ResolvedType>()\n                + resolved_type_owned_capacity(residual_type);";
    assert_eq!(original.matches(old).count(), 2);
    assert_eq!(current.matches(funded).count(), 2);
    assert_eq!(current, original.replace(old, funded));
    original
}

// These are complete named historical inputs for the old source audit only.
// Each invocation authenticates its exact supplied current/predecessor view
// and physical current owner before opening the sealed predecessor. It grants no
// semantic acceptance authority to the archived source.
pub(crate) fn receiver_before(source: &str, current: &str) -> String {
    type View = (
        &'static str,
        &'static str,
        usize,
        &'static str,
        &'static str,
        usize,
        &'static str,
    );
    const VIEWS: &[View] = &[
        ("src/hir/borrowed_root_oracle/resolve_expr.rs", "src/hir/borrowed_root_oracle/resolve_expr.rs", 140922, "2b1db091099c6367a2b13a20e628e7b6e09a5ba88e33d80b38b63b337c0523c3", "docs/evidence/resolver-source-reader-20261022/before/8b9bc136d4bedd83e218f4f7cd1e6b316426a76063c794b1679c4056039ed40a.txt", 140920, "8b9bc136d4bedd83e218f4f7cd1e6b316426a76063c794b1679c4056039ed40a"),
        ("src/hir/borrowed_root_oracle/resolve_expr_frame.rs", "src/hir/borrowed_root_oracle/resolve_expr_frame.rs", 25000, "73c6ef03b383a43ac7482d3752c1db4041516c743ed57405dfa01d6b3a75bb30", "docs/evidence/resolver-source-reader-20261022/before/73c6ef03b383a43ac7482d3752c1db4041516c743ed57405dfa01d6b3a75bb30.txt", 25000, "73c6ef03b383a43ac7482d3752c1db4041516c743ed57405dfa01d6b3a75bb30"),
        ("src/hir/borrowed_root_oracle/resolve_for.rs", "src/hir/borrowed_root_oracle/resolve_for.rs", 19279, "fafb9b336ec940edef4165e162aec7d5ded109a3e6a2c74410515691006a61c5", "docs/evidence/resolver-source-reader-20261022/before/fafb9b336ec940edef4165e162aec7d5ded109a3e6a2c74410515691006a61c5.txt", 19279, "fafb9b336ec940edef4165e162aec7d5ded109a3e6a2c74410515691006a61c5"),
        ("src/hir/borrowed_root_oracle/resolve_for_own.rs", "src/hir/borrowed_root_oracle/resolve_for_own.rs", 17367, "f73529c156b1539c0110d103843611c9a04d90a277ef1173e78e1bacb369fbb9", "docs/evidence/resolver-source-reader-20261022/before/f73529c156b1539c0110d103843611c9a04d90a277ef1173e78e1bacb369fbb9.txt", 17367, "f73529c156b1539c0110d103843611c9a04d90a277ef1173e78e1bacb369fbb9"),
        ("src/hir/borrowed_root_oracle/resolve_box_call.rs", "src/hir/borrowed_root_oracle/resolve_box_call.rs", 10184, "6f7cbc1f4ca60f0aac9118c643d82e8be3a97c2c7e7ea23940631c7265e8ee50", "docs/evidence/resolver-source-reader-20261022/before/6f7cbc1f4ca60f0aac9118c643d82e8be3a97c2c7e7ea23940631c7265e8ee50.txt", 10184, "6f7cbc1f4ca60f0aac9118c643d82e8be3a97c2c7e7ea23940631c7265e8ee50"),
        ("src/hir/borrowed_root_oracle/resolve_vec_call.rs", "src/hir/borrowed_root_oracle/resolve_vec_call.rs", 14872, "4666678e3aace39a7e26d4f2a7a6793658dba0d2f28284b6a78b3e193ba9e9e2", "docs/evidence/resolver-source-reader-20261022/before/4666678e3aace39a7e26d4f2a7a6793658dba0d2f28284b6a78b3e193ba9e9e2.txt", 14872, "4666678e3aace39a7e26d4f2a7a6793658dba0d2f28284b6a78b3e193ba9e9e2"),
        ("src/hir/borrowed_root_oracle/resolve_expr/sort_keys.rs", "src/hir/borrowed_root_oracle/resolve_expr/sort_keys.rs", 4608, "dec5f94b0875fd92c5401b1f2133694bf9e1d1860e0138ad69c3f65e426b6053", "docs/evidence/resolver-source-reader-20261022/before/dec5f94b0875fd92c5401b1f2133694bf9e1d1860e0138ad69c3f65e426b6053.txt", 4608, "dec5f94b0875fd92c5401b1f2133694bf9e1d1860e0138ad69c3f65e426b6053"),
        ("src/hir/borrowed_root_oracle/resolve_expr/super_method.rs", "src/hir/borrowed_root_oracle/resolve_expr/super_method.rs", 5071, "4591badc2f10f712e8e14a228eee46e73fbcb3ebf863e75aa28a73fb8af5e0ce", "docs/evidence/resolver-source-reader-20261022/before/4591badc2f10f712e8e14a228eee46e73fbcb3ebf863e75aa28a73fb8af5e0ce.txt", 5071, "4591badc2f10f712e8e14a228eee46e73fbcb3ebf863e75aa28a73fb8af5e0ce"),
        ("src/hir/borrowed_root_oracle/resolve_expr/capacity.rs", "src/hir/borrowed_root_oracle/resolve_expr/capacity.rs", 2288, "9158bb0e1d9256332aca30fe821818b2105b5c785eead28d5643192b20bce504", "docs/evidence/resolver-source-reader-20261022/before/76af4d03db05df8bab03d71a627673080843adf36f482189972adec34c04cbca.txt", 2281, "76af4d03db05df8bab03d71a627673080843adf36f482189972adec34c04cbca"),
        ("src/hir/borrowed_root_oracle/resolve_expr/vec_field.rs", "src/hir/borrowed_root_oracle/resolve_expr/vec_field.rs", 5274, "e9b35c919172c364216e2efa9986408311fcb9b437ad8a52f28b901f78fa682a", "docs/evidence/resolver-source-reader-20261022/before/e9b35c919172c364216e2efa9986408311fcb9b437ad8a52f28b901f78fa682a.txt", 5274, "e9b35c919172c364216e2efa9986408311fcb9b437ad8a52f28b901f78fa682a"),
        ("src/hir/borrowed_root_oracle/resolve_expr/finish.rs", "src/hir/borrowed_root_oracle/resolve_expr/finish.rs", 5102, "3e70aa00d1ffcb5d712727e35f4c06839f04f8d7a85de6684b48b915f452538c", "docs/evidence/resolver-source-reader-20261022/before/3e70aa00d1ffcb5d712727e35f4c06839f04f8d7a85de6684b48b915f452538c.txt", 5102, "3e70aa00d1ffcb5d712727e35f4c06839f04f8d7a85de6684b48b915f452538c"),
        ("src/hir/borrowed_root_oracle/resolve_expr/vec_constructor.rs", "src/hir/borrowed_root_oracle/resolve_expr/vec_constructor.rs", 2623, "7ae41a7e5d393e6d76d897a14be504003391917415a97988451ebfa171a5fecc", "docs/evidence/resolver-source-reader-20261022/before/70ccbee7c7f9630cd61976e24ff5baa7ed51de308ad24a71f83ef1649f0888f6.txt", 2630, "70ccbee7c7f9630cd61976e24ff5baa7ed51de308ad24a71f83ef1649f0888f6"),
        ("src/hir/borrowed_root_oracle/resolve_expr/lexical_exit.rs", "src/hir/borrowed_root_oracle/resolve_expr/lexical_exit.rs", 3176, "1ebbfbdf91f9588105f016e9e947b47bd5e43e5fbdff07bdce7aace987eecd24", "docs/evidence/resolver-source-reader-20261022/before/1ebbfbdf91f9588105f016e9e947b47bd5e43e5fbdff07bdce7aace987eecd24.txt", 3176, "1ebbfbdf91f9588105f016e9e947b47bd5e43e5fbdff07bdce7aace987eecd24"),
        ("src/hir/borrowed_root_oracle/resolve_expr/path_arena_regression_tests.rs", "src/hir/borrowed_root_oracle/resolve_expr/path_arena_regression_tests.rs", 7994, "02dd6444c7872f0730b88893cd94ec516a731d2354972a7dcdb1025c6d9dcbc3", "docs/evidence/resolver-source-reader-20261022/before/d74cdc3bf7ba9110abe91e879eff26402dead68118f2f595d1a5ae03d31274fd.txt", 7898, "d74cdc3bf7ba9110abe91e879eff26402dead68118f2f595d1a5ae03d31274fd"),
        ("src/hir/borrowed_root_oracle/resolve_expr/path_arena_tests.rs", "src/hir/borrowed_root_oracle/resolve_expr/path_arena_tests.rs", 13658, "566b0ed62ca322b59a3bda3e0da205bf45021ae63476596ebfddcbdd698eaacd", "docs/evidence/resolver-source-reader-20261022/before/566b0ed62ca322b59a3bda3e0da205bf45021ae63476596ebfddcbdd698eaacd.txt", 13658, "566b0ed62ca322b59a3bda3e0da205bf45021ae63476596ebfddcbdd698eaacd"),
        ("src/hir/borrowed_root_oracle/resolve_expr/literal_format.rs", "src/hir/borrowed_root_oracle/resolve_expr/literal_format.rs", 4367, "7b0d37229d2282ad3095c4a389ee15b4815e67e552d7574d92cea6bb7e57110b", "docs/evidence/resolver-source-reader-20261022/before/7b0d37229d2282ad3095c4a389ee15b4815e67e552d7574d92cea6bb7e57110b.txt", 4367, "7b0d37229d2282ad3095c4a389ee15b4815e67e552d7574d92cea6bb7e57110b"),
        ("src/hir/borrowed_root_oracle/resolve_expr/variable.rs", "src/hir/borrowed_root_oracle/resolve_expr/variable.rs", 1286, "421e9edd81c65b89cef9dcaf7fcdb7c251afcb6f6f27e8bf5b6e824bf9258edd", "docs/evidence/resolver-source-reader-20261022/before/421e9edd81c65b89cef9dcaf7fcdb7c251afcb6f6f27e8bf5b6e824bf9258edd.txt", 1286, "421e9edd81c65b89cef9dcaf7fcdb7c251afcb6f6f27e8bf5b6e824bf9258edd"),
        ("src/hir/borrowed_root_oracle/resolve_expr/path_arena.rs", "src/hir/borrowed_root_oracle/resolve_expr/path_arena.rs", 20176, "d3dcc7f0381204188023623fd1ab9cf90774f79cb22ae00a14ff81ebb695b210", "docs/evidence/resolver-source-reader-20261022/before/d3dcc7f0381204188023623fd1ab9cf90774f79cb22ae00a14ff81ebb695b210.txt", 20176, "d3dcc7f0381204188023623fd1ab9cf90774f79cb22ae00a14ff81ebb695b210"),
        ("src/hir/borrowed_root_oracle/resolve_expr/byte_view.rs", "src/hir/borrowed_root_oracle/resolve_expr/byte_view.rs", 2083, "dbf07dc264949d7e18ce759ee9f053206600fe0e170f7db99893383c15693866", "docs/evidence/resolver-source-reader-20261022/before/dbf07dc264949d7e18ce759ee9f053206600fe0e170f7db99893383c15693866.txt", 2083, "dbf07dc264949d7e18ce759ee9f053206600fe0e170f7db99893383c15693866"),
        ("src/hir/borrowed_root_oracle/resolve_expr/capacity/tests.rs", "src/hir/borrowed_root_oracle/resolve_expr/capacity/tests.rs", 3975, "cf0e81914c6980afdba10c4e0f84e1e4b68cafd00d730aac53f54278bae5403f", "docs/evidence/resolver-source-reader-20261022/before/15c9c83a71b52a20e16f5aef7a15b35db125b3537edd90f6aceebdfa0ec3caad.txt", 3975, "15c9c83a71b52a20e16f5aef7a15b35db125b3537edd90f6aceebdfa0ec3caad"),
        ("src/hir/borrowed_root_oracle/resolve_expr_frame/capacity_old.rs", "src/hir/borrowed_root_oracle/resolve_expr_frame/capacity_old.rs", 9258, "612e6806a9833f2fa50bc6025014d02a3340015756c6678765efad10213ced4c", "docs/evidence/resolver-source-reader-20261022/before/612e6806a9833f2fa50bc6025014d02a3340015756c6678765efad10213ced4c.txt", 9258, "612e6806a9833f2fa50bc6025014d02a3340015756c6678765efad10213ced4c"),
        ("src/hir/borrowed_root_oracle/resolve_expr_frame/fixed_results.rs", "src/hir/borrowed_root_oracle/resolve_expr_frame/fixed_results.rs", 2415, "fb5b1f780a108e91a483d33959610de169d9273e9c177b33505272a3cb688715", "docs/evidence/resolver-source-reader-20261022/before/fb5b1f780a108e91a483d33959610de169d9273e9c177b33505272a3cb688715.txt", 2415, "fb5b1f780a108e91a483d33959610de169d9273e9c177b33505272a3cb688715"),
        ("src/hir/borrowed_root_oracle/resolve_expr_frame/fixed_results/tests.rs", "src/hir/borrowed_root_oracle/resolve_expr_frame/fixed_results/tests.rs", 5522, "192c298e9634e8fc814c54faccfd27c15ab55c2b0298877671fadae700756b61", "docs/evidence/resolver-source-reader-20261022/before/192c298e9634e8fc814c54faccfd27c15ab55c2b0298877671fadae700756b61.txt", 5522, "192c298e9634e8fc814c54faccfd27c15ab55c2b0298877671fadae700756b61"),
        ("src/hir/borrowed_root_oracle/resolve_for/bytes.rs", "src/hir/borrowed_root_oracle/resolve_for/bytes.rs", 8415, "dd89658af0380e2475fe977ac89c581536de244e4d7320b3684f7b7952ac785e", "docs/evidence/resolver-source-reader-20261022/before/dd89658af0380e2475fe977ac89c581536de244e4d7320b3684f7b7952ac785e.txt", 8415, "dd89658af0380e2475fe977ac89c581536de244e4d7320b3684f7b7952ac785e"),
        ("src/hir/borrowed_root_oracle/resolve_for/record.rs", "src/hir/borrowed_root_oracle/resolve_for/record.rs", 12786, "bc1f4634d5a3dc59a2aa873ad5e7300a0166c515bd1e95aa7a618bce4b065a23", "docs/evidence/resolver-source-reader-20261022/before/50fbd5e1baeb912eab4663d4b3d3fe99885524a88fa80d7756b06b556114584d.txt", 12786, "50fbd5e1baeb912eab4663d4b3d3fe99885524a88fa80d7756b06b556114584d"),
        ("src/hir/borrowed_root_oracle/resolve_for/bytes/tests.rs", "src/hir/borrowed_root_oracle/resolve_for/bytes/tests.rs", 10165, "68fdaed4cb7bf4075e4effef7ed7ed2c5ff2f326f252775dc25004f37550f4a5", "docs/evidence/resolver-source-reader-20261022/before/68fdaed4cb7bf4075e4effef7ed7ed2c5ff2f326f252775dc25004f37550f4a5.txt", 10165, "68fdaed4cb7bf4075e4effef7ed7ed2c5ff2f326f252775dc25004f37550f4a5"),
        ("src/hir/borrowed_root_oracle/resolve_box_call/list.rs", "src/hir/borrowed_root_oracle/resolve_box_call/list.rs", 2861, "319cd8e850d9b9aee67563b0ec03a4c9f73aeb5ae6be0e5fb47d36f37f453da4", "docs/evidence/resolver-source-reader-20261022/before/319cd8e850d9b9aee67563b0ec03a4c9f73aeb5ae6be0e5fb47d36f37f453da4.txt", 2861, "319cd8e850d9b9aee67563b0ec03a4c9f73aeb5ae6be0e5fb47d36f37f453da4"),
        ("src/hir/borrowed_root_oracle/resolve_box_call/map.rs", "src/hir/borrowed_root_oracle/resolve_box_call/map.rs", 3332, "7dabdee1de4b028dc878fa25d22a9f2b21e72a44326dadcf1f8a82210761adf8", "docs/evidence/resolver-source-reader-20261022/before/7dabdee1de4b028dc878fa25d22a9f2b21e72a44326dadcf1f8a82210761adf8.txt", 3332, "7dabdee1de4b028dc878fa25d22a9f2b21e72a44326dadcf1f8a82210761adf8"),
        ("src/hir/borrowed_root_oracle/resolve_box_call/iterator.rs", "src/hir/borrowed_root_oracle/resolve_box_call/iterator.rs", 3873, "1749dd3c7c3ecfc22d6a89d53b1222f8622dcfaa242ecf4d04f7897e7d2c4ce6", "docs/evidence/resolver-source-reader-20261022/before/1749dd3c7c3ecfc22d6a89d53b1222f8622dcfaa242ecf4d04f7897e7d2c4ce6.txt", 3873, "1749dd3c7c3ecfc22d6a89d53b1222f8622dcfaa242ecf4d04f7897e7d2c4ce6"),
        ("src/hir/borrowed_root_oracle/closure/resolve.rs", "src/hir/borrowed_root_oracle/closure/resolve.rs", 9515, "a8e6e3184e7b19c5bbdff7e1b17e5328db31f056c18e492133f5d01ac5d62df5", "docs/evidence/resolver-source-reader-20261022/before/a8e6e3184e7b19c5bbdff7e1b17e5328db31f056c18e492133f5d01ac5d62df5.txt", 9515, "a8e6e3184e7b19c5bbdff7e1b17e5328db31f056c18e492133f5d01ac5d62df5"),
        ("src/hir/borrowed_root_oracle/closure/once.rs", "src/hir/borrowed_root_oracle/closure/once.rs", 17214, "09129edd97cdb27b4f4452b1d0119ead39a062cd432ff978041b044002b58561", "docs/evidence/resolver-source-reader-20261022/before/09129edd97cdb27b4f4452b1d0119ead39a062cd432ff978041b044002b58561.txt", 17214, "09129edd97cdb27b4f4452b1d0119ead39a062cd432ff978041b044002b58561"),
        ("src/hir/borrowed_root_oracle/closure/mutable.rs", "src/hir/borrowed_root_oracle/closure/mutable.rs", 14882, "149e97047e27cbeac6d5edf6a62dc388a6943be3c238e6f67c252fb80d313fed", "docs/evidence/resolver-source-reader-20261022/before/149e97047e27cbeac6d5edf6a62dc388a6943be3c238e6f67c252fb80d313fed.txt", 14882, "149e97047e27cbeac6d5edf6a62dc388a6943be3c238e6f67c252fb80d313fed"),
        ("src/hir/borrowed_root_oracle/resolve_pattern.rs", "src/hir/borrowed_root_oracle/resolve_pattern.rs", 22918, "46c1410155fa47ebf5699de32eb1b38e15f996b657b4057e1dd169a79f6b7bbb", "docs/evidence/resolver-source-reader-20261022/before/46c1410155fa47ebf5699de32eb1b38e15f996b657b4057e1dd169a79f6b7bbb.txt", 22918, "46c1410155fa47ebf5699de32eb1b38e15f996b657b4057e1dd169a79f6b7bbb"),
        ("src/hir/borrowed_root_oracle/resolve_statement.rs", "src/hir/borrowed_root_oracle/resolve_statement.rs", 24567, "23a4ce456e3db8001e3255788dad2e75c0b9dd7ef23a0eb29f54bedb6d7c934a", "docs/evidence/resolver-source-reader-20261022/before/23a4ce456e3db8001e3255788dad2e75c0b9dd7ef23a0eb29f54bedb6d7c934a.txt", 24567, "23a4ce456e3db8001e3255788dad2e75c0b9dd7ef23a0eb29f54bedb6d7c934a"),
        ("src/hir/closure/resolve.rs", "src/hir/closure/resolve.rs", 9765, "1351b8470da0d625b47df68b1c62555b1e6f9eb9722a1099ec86ee32b5f429fc", "docs/evidence/resolver-source-reader-20261022/before/b80c0ff64b54e8bfdd33809efc5f32b7936cf6541dea9e0190d45dddb5789d67.txt", 9513, "b80c0ff64b54e8bfdd33809efc5f32b7936cf6541dea9e0190d45dddb5789d67"),
        ("src/hir/closure/mutable.rs", "src/hir/closure/mutable.rs", 14910, "237cbd8d91f9331a47e9c90402dcf435d548bbfa09007a4826fd4e9a1d738895", "docs/evidence/resolver-source-reader-20261022/before/78a6a814591758354edd22e20317dde244894387e38e040cd27a2994e27d6b96.txt", 14893, "78a6a814591758354edd22e20317dde244894387e38e040cd27a2994e27d6b96"),
        ("src/hir/capacity_probe.rs", "src/hir/capacity_probe.rs", 22762, "6dd857d183753d61e86b4dce5b7879152abf0e2e398ea6ee60783974337e47ff", "docs/evidence/resolver-source-reader-20261022/before/6dd857d183753d61e86b4dce5b7879152abf0e2e398ea6ee60783974337e47ff.txt", 22762, "6dd857d183753d61e86b4dce5b7879152abf0e2e398ea6ee60783974337e47ff"),
        ("src/hir/resolve_expr/path_arena.rs", "src/hir/resolve_expr/path_arena.rs", 20174, "38a7e79c5d5dea9bb1469fd8fbb9ba856851c11c51671ab6f12f5b1160b28be1", "docs/evidence/resolver-source-reader-20261022/before/38a7e79c5d5dea9bb1469fd8fbb9ba856851c11c51671ab6f12f5b1160b28be1.txt", 20174, "38a7e79c5d5dea9bb1469fd8fbb9ba856851c11c51671ab6f12f5b1160b28be1"),
        ("src/hir/closure.rs", "src/hir/closure.rs", 5168, "d4a800c9c6118df5846c2432ede4b4f111a020b6e03da038ca1d5c9d1110fdfb", "docs/evidence/resolver-source-reader-20261022/before/d4a800c9c6118df5846c2432ede4b4f111a020b6e03da038ca1d5c9d1110fdfb.txt", 5168, "d4a800c9c6118df5846c2432ede4b4f111a020b6e03da038ca1d5c9d1110fdfb"),
        ("src/hir/closure/once.rs", "src/hir/closure/once.rs", 17260, "048928cd79ce68e7ebd71d66d10cd69e08c6ab2f7769753f9eb637b9a62b8527", "docs/evidence/resolver-source-reader-20261022/before/f45f7909124dcca1ccf75b09b53e1af115f929852ddab7f2dbc2564f6f68d6c7.txt", 17210, "f45f7909124dcca1ccf75b09b53e1af115f929852ddab7f2dbc2564f6f68d6c7"),
        ("src/hir/closure/paid.rs", "src/hir/closure/paid.rs", 5179, "d4f57dc46ffdba409de122e332eeea91927e1f08065cd66d3eca3b5935443486", "docs/evidence/resolver-source-reader-20261022/before/d4f57dc46ffdba409de122e332eeea91927e1f08065cd66d3eca3b5935443486.txt", 5179, "d4f57dc46ffdba409de122e332eeea91927e1f08065cd66d3eca3b5935443486"),
        ("src/hir/closure/paid/copy.rs", "src/hir/closure/paid/copy.rs", 5054, "8154773c948031810bc14dc2b5303c8a2bf4674576a18ba17efe936e7470e15d", "docs/evidence/resolver-source-reader-20261022/before/8154773c948031810bc14dc2b5303c8a2bf4674576a18ba17efe936e7470e15d.txt", 5054, "8154773c948031810bc14dc2b5303c8a2bf4674576a18ba17efe936e7470e15d"),
        ("src/hir/closure/paid/tests.rs", "src/hir/closure/paid/tests.rs", 10767, "44b8b68569988d384d87eeed680969a3495df3927e4dab2fe514348d405ba556", "docs/evidence/resolver-source-reader-20261022/before/44b8b68569988d384d87eeed680969a3495df3927e4dab2fe514348d405ba556.txt", 10767, "44b8b68569988d384d87eeed680969a3495df3927e4dab2fe514348d405ba556"),
        ("src/hir/closure/paid/original_tests.rs", "src/hir/closure/paid/original_tests.rs", 7279, "fd590277449b30c104a735f2c5b54854481684bf1a31be0137cd2b2a5ff2e1f5", "docs/evidence/resolver-source-reader-20261022/before/fd590277449b30c104a735f2c5b54854481684bf1a31be0137cd2b2a5ff2e1f5.txt", 7279, "fd590277449b30c104a735f2c5b54854481684bf1a31be0137cd2b2a5ff2e1f5"),
        ("src/hir/resolve_expr_reference.rs", "src/hir/resolve_expr_reference.rs", 76946, "052f56b46c90134152b60762bbe925d798e5b62615449365fc2dfaac41c18192", "docs/evidence/resolver-source-reader-20261022/before/23612278e943d9eaecffab56af1db8b8296bea2a97743638b3ea8697a959a420.txt", 76558, "23612278e943d9eaecffab56af1db8b8296bea2a97743638b3ea8697a959a420"),
    ];
    let matches: Vec<_> = VIEWS.iter().filter(|row| row.0 == source).collect();
    assert_eq!(matches.len(), 1, "closed historical resolver source");
    let row = matches[0];
    let supplied = digest(current);
    assert!(
        (current.len() == row.2 && supplied == row.3)
            || (current.len() == row.5 && supplied == row.6),
        "exact current or already-authenticated predecessor view: {source}: {supplied}"
    );
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = current_file(&root.join(row.1), row.2, row.3)
        .expect("complete no-follow physical resolver owner");
    assert_eq!(digest(&physical), row.3, "exact physical resolver source");
    current_file(&root.join(row.4), row.5, row.6)
        .expect("complete sealed original resolver audit input")
}

fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev()
            || a.ino() != b.ino()
            || a.mode() != b.mode()
            || a.mtime() != b.mtime()
            || a.mtime_nsec() != b.mtime_nsec()
            || a.ctime() != b.ctime()
            || a.ctime_nsec() != b.ctime_nsec()
        {
            return false;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if a.file_attributes() & 0x400 != 0
            || b.file_attributes() & 0x400 != 0
            || a.creation_time() != b.creation_time()
            || a.last_write_time() != b.last_write_time()
        {
            return false;
        }
    }
    true
}
fn current_file(
    path: &std::path::Path,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    use std::io::Read;
    if length > 1_048_576 {
        return Err("bound");
    }
    let before = std::fs::symlink_metadata(path).map_err(|_| "missing")?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err("nonregular");
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(unix, windows)))]
    {
        return Err("platform");
    }
    let mut file = options.open(path).map_err(|_| "open")?;
    let opened = file.metadata().map_err(|_| "metadata")?;
    if !same_file(&before, &opened) || opened.len() != u64::try_from(length).map_err(|_| "bound")? {
        return Err("length-or-identity");
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    let mut limited = (&mut file).take(length as u64 + 1);
    limited.read_to_end(&mut bytes).map_err(|_| "read")?;
    if bytes.len() != length
        || format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(&bytes))) != expected
    {
        return Err("length-or-digest");
    }
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink()
        || !same_file(&opened, &after)
        || !same_file(&after, &path_after)
    {
        return Err("identity-drift");
    }
    String::from_utf8(bytes).map_err(|_| "utf8")
}
