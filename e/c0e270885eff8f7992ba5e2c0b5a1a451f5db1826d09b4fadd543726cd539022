//! Explicit boundary membership is independent of complete source-role custody.
//! The full predecessor helper remains byte-exact and independently compiled.
#[path = "new_boundary_module.rs"]
mod boundary;
// The independently hashed predecessor is a byte-exact evidence fixture.
#[rustfmt::skip]
#[path = "production_source_inventory_original.rs"]
mod original;

pub(super) fn production(raw: &str, manifest: &str) -> String {
    assert_current_accepted_batch_relocated_prefix(raw);
    // Authenticate ALL raw bytes, sources, registrations and production/test
    // roles first. No outside-boundary source is relabelled or left unaudited.
    let complete_authenticated_production = original::production(raw, manifest);
    boundary::production(raw, manifest, &complete_authenticated_production)
}

// Authenticate placement on the actual HIR/projection payload received by the
// full source owner, not by counting include tokens in the carrier's text.
fn relocated_prefix() -> &'static str {
    concat!(
        include_str!(
            "../../src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs"
        ),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests/before.rs"),
    )
}
fn assert_relocated_prefix(raw: &str) {
    let prefix = relocated_prefix();
    assert!(!prefix.is_empty());
    let rest = raw
        .strip_prefix(prefix)
        .expect("all relocated subjects must enter the actual raw inventory");
    assert!(
        !rest.starts_with(prefix),
        "duplicate relocated inventory prefix"
    );
}

#[test]
fn relocated_subjects_must_reach_the_actual_owner_payload() {
    let prefix = relocated_prefix();
    assert_relocated_prefix(&format!("{prefix}remaining original inventory"));
    for wrong in [
        String::new(),
        prefix[1..].to_owned(),
        format!("sentinel{prefix}"),
        format!("{prefix}{prefix}"),
    ] {
        assert!(std::panic::catch_unwind(|| assert_relocated_prefix(&wrong)).is_err());
    }
}

// New census generation: original three subjects remain, with two actual
// additions between first and second. The original guard/tests stay compiled.
fn current_relocated_prefix() -> &'static str {
    concat!(
        include_str!(
            "../../src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs"
        ),
        include_str!("../../src/loop_calls/byte_writer/tests/string_slots.rs"),
        include_str!("../../tests/wasm/scalar_browser_ci_contract/before_21abc.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests/before.rs"),
    )
}
fn assert_current_relocated_prefix(raw: &str) {
    let prefix = current_relocated_prefix();
    assert!(!prefix.is_empty());
    let rest = raw
        .strip_prefix(prefix)
        .expect("all five relocated subjects must enter the actual raw inventory");
    assert!(
        !rest.starts_with(prefix),
        "duplicate current relocated inventory prefix"
    );
}
#[test]
fn current_relocated_subjects_require_all_five_in_order_and_reject_old_or_duplicate_prefixes() {
    let prefix = current_relocated_prefix();
    assert_current_relocated_prefix(&format!("{prefix}remaining original inventory"));
    for wrong in [
        String::new(),
        relocated_prefix().to_owned(),
        prefix[1..].to_owned(),
        format!("sentinel{prefix}"),
        format!("{prefix}{prefix}"),
        prefix.replacen(
            include_str!("../../src/loop_calls/byte_writer/tests/string_slots.rs"),
            "",
            1,
        ),
        prefix.replacen(
            include_str!("../../tests/wasm/scalar_browser_ci_contract/before_21abc.rs"),
            "",
            1,
        ),
    ] {
        assert!(std::panic::catch_unwind(|| assert_current_relocated_prefix(&wrong)).is_err());
    }
}

// Distinct assembled generation; old three/five-subject guards stay exact.
fn semantic_batch_relocated_prefix() -> &'static str {
    concat!(
        include_str!(
            "../../src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs"
        ),
        include_str!("../../src/loop_calls/byte_writer/tests/string_slots.rs"),
        include_str!("../../tests/wasm/scalar_browser_ci_contract/before_21abc.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests/before.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/before_hir_compare.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/before.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/sequence_vector.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/before.rs"),
        include_str!(
            "../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/source_custody.rs"
        ),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs"),
    )
}
fn assert_semantic_batch_relocated_prefix_before162(raw: &str) {
    let prefix = semantic_batch_relocated_prefix();
    assert!(!prefix.is_empty());
    let rest = raw
        .strip_prefix(prefix)
        .expect("all seventeen relocated subjects must enter the actual raw inventory");
    assert!(
        !rest.starts_with(prefix),
        "duplicate semantic batch relocated inventory prefix"
    );
}
// The complete prefix is a declaration of the relocated slice, not a complete
// inventory on its own. The full source owner authenticates the remaining raw
// sources and role table after this placement check; do not project that tail.
fn assert_semantic_batch_relocated_prefix(raw: &str) {
    assert_semantic_batch_relocated_prefix_before162(raw);
    let rest = raw.strip_prefix(semantic_batch_relocated_prefix()).unwrap();
    assert!(
        !rest.is_empty(),
        "relocated prefix must be followed by the remaining raw inventory"
    );
}
#[test]
fn semantic_batch_relocated_subjects_require_all_seventeen_and_keep_prior_three_five_guards() {
    let prefix = semantic_batch_relocated_prefix();
    assert_semantic_batch_relocated_prefix(&format!("{prefix}remaining original inventory"));
    assert_relocated_prefix(&format!(
        "{}remaining original inventory",
        relocated_prefix()
    ));
    assert_current_relocated_prefix(&format!(
        "{}remaining original inventory",
        current_relocated_prefix()
    ));
    for wrong in [
        String::new(),
        current_relocated_prefix().to_owned(),
        prefix[1..].to_owned(),
        format!("sentinel{prefix}"),
        format!("{prefix}{prefix}"),
    ] {
        assert!(
            std::panic::catch_unwind(|| assert_semantic_batch_relocated_prefix(&wrong)).is_err()
        );
    }
    let added = [
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/before_hir_compare.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/before.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/sequence_vector.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/before.rs"),
        include_str!(
            "../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/source_custody.rs"
        ),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs"),
    ];
    for subject in added {
        assert!(!subject.is_empty());
        let missing = prefix.replacen(subject, "", 1);
        assert!(
            std::panic::catch_unwind(|| assert_semantic_batch_relocated_prefix(&missing)).is_err()
        );
        let moved = format!("{missing}{subject}");
        assert!(
            std::panic::catch_unwind(|| assert_semantic_batch_relocated_prefix(&moved)).is_err()
        );
    }
}

// Separate accepted generation; complete three/five/seventeen controls above
// remain independently compiled, including the nonempty raw-tail regression.
fn accepted_batch_relocated_prefix() -> &'static str {
    concat!(
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs"),
        include_str!("../../src/loop_calls/byte_writer/tests/string_slots.rs"),
        include_str!("../../tests/wasm/scalar_browser_ci_contract/before_21abc.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests/before.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/before_hir_compare.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/before.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/sequence_vector.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/source_custody.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs"),
        concat!(include_str!("../../src/bounded_output/stage_debits/route_debits.rs"), include_str!("../../src/bounded_output/stage_debits/route_debits/json_codec_probe.rs")),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/main_c883.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/before.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/tests.rs"),
        include_str!("../../src/source_reflows/main_c883_receivers.rs"),
        include_str!("../../src/source_reflows_merge/main_c883_inputs.rs"),
        include_str!("../../tests/workspace/semantic_graph/source_inventory_controls.rs"),
    )
}
fn formatted_accepted_batch_relocated_prefix() -> &'static str {
    concat!(
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs"),
        include_str!("../../src/loop_calls/byte_writer/tests/string_slots.rs"),
        include_str!("../../tests/wasm/scalar_browser_ci_contract/before_21abc.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests/before.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/before_hir_compare.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/before.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/sequence_vector.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/source_custody.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs"),
        concat!(include_str!("../../src/bounded_output/stage_debits/route_debits.rs"), include_str!("../../src/bounded_output/stage_debits/route_debits/json_codec_probe.rs")),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/main_c883.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/before.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/matched.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/current_tests.rs"),
        include_str!("../../src/source_reflows/main_c883_receivers.rs"),
        include_str!("../../src/source_reflows_merge/main_c883_inputs.rs"),
        include_str!("../../tests/workspace/semantic_graph/source_inventory_controls.rs"),
    )
}
fn assert_accepted_batch_relocated_prefix(raw: &str) {
    let prefix = accepted_batch_relocated_prefix();
    assert!(!prefix.is_empty());
    let rest = raw
        .strip_prefix(prefix)
        .expect("all twenty-eight accepted subjects enter the raw owner in order");
    assert!(
        !rest.is_empty(),
        "accepted relocated prefix requires the remaining raw inventory"
    );
    assert!(
        !rest.starts_with(prefix),
        "duplicate accepted relocated inventory prefix"
    );
}
#[test]
fn accepted_raw_prefix_keeps_all_twenty_eight_subjects_old_guards_and_nonempty_tail() {
    let prefix = accepted_batch_relocated_prefix();
    assert_accepted_batch_relocated_prefix(&format!("{prefix}remaining raw inventory"));
    assert_semantic_batch_relocated_prefix(&format!(
        "{}remaining raw inventory",
        semantic_batch_relocated_prefix()
    ));
    for bad in [
        String::new(),
        prefix.to_owned(),
        format!("sentinel{prefix}"),
        format!("{prefix}{prefix}"),
        format!(
            "{}remaining raw inventory",
            semantic_batch_relocated_prefix()
        ),
    ] {
        assert!(std::panic::catch_unwind(|| assert_accepted_batch_relocated_prefix(&bad)).is_err());
    }
    for subject in [
        concat!(include_str!("../../src/bounded_output/stage_debits/route_debits.rs"), include_str!("../../src/bounded_output/stage_debits/route_debits/json_codec_probe.rs")),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/main_c883.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/before.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/matched.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/current_tests.rs"),
        include_str!("../../src/source_reflows/main_c883_receivers.rs"),
        include_str!("../../src/source_reflows_merge/main_c883_inputs.rs"),
        include_str!("../../tests/workspace/semantic_graph/source_inventory_controls.rs"),
    ] {
        assert!(!subject.is_empty());
        let current = current_accepted_batch_relocated_prefix();
        let (owner, verify): (&str, fn(&str)) = if prefix.contains(subject) {
            (prefix, assert_accepted_batch_relocated_prefix)
        } else {
            assert!(
                [
                    include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/matched.rs"),
                    include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/current_tests.rs"),
                ].contains(&subject),
                "only the declared new lexical children use the current receiver"
            );
            (current.as_str(), assert_current_accepted_batch_relocated_prefix)
        };
        assert_eq!(owner.matches(subject).count(), 1);
        let missing = owner.replacen(subject, "", 1);
        let moved = format!("{missing}{subject}");
        let moved = if moved == owner { format!("{subject}{missing}") } else { moved };
        for changed in [missing, moved] {
            assert_ne!(changed, owner, "identity mutation cannot prove refusal");
            for bad in [changed.clone(), format!("{changed}remaining raw inventory")] {
                assert!(std::panic::catch_unwind(|| verify(&bad)).is_err());
            }
        }
    }
}

// The original twenty-eight-subject receiver and all of its controls above stay
// independently compiled. This current generation joins the two actual lexical
// children at their parent's existing raw slot; raw input is never rewritten.
fn before_format_current_accepted_batch_relocated_prefix() -> String {
    let previous = accepted_batch_relocated_prefix();
    let slot = include_str!(
        "../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/tests.rs"
    );
    let complete = concat!(
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/matched.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/current_tests.rs"),
    );
    assert!(!slot.is_empty());
    assert_eq!(
        previous.matches(slot).count(),
        1,
        "unique original lexical child slot"
    );
    let current = previous.replacen(slot, complete, 1);
    assert_eq!(current.matches(complete).count(), 1);
    assert_eq!(
        current.replacen(complete, slot, 1),
        previous,
        "complete original prefix inverse"
    );
    current
}
fn current_accepted_batch_relocated_prefix() -> String {
    concat!(
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/incoming_flat_targets.rs"),
        include_str!("../../src/loop_calls/byte_writer/tests/string_slots.rs"),
        include_str!("../../tests/wasm/scalar_browser_ci_contract/before_21abc.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/moderate_tests/before.rs"),
        include_str!("../../src/hir/resolve_program_methods.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/before_hir_compare.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/before.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/sequence_vector.rs"),
        include_str!("../../src/cleanup_plan/replay/owned_suffix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/source_custody.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs"),
        concat!(include_str!("../../src/bounded_output/stage_debits/route_debits.rs"), include_str!("../../src/bounded_output/stage_debits/route_debits/json_codec_probe.rs")),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/main_c883.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/before.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/matched.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/current_tests.rs"),
        include_str!("../../src/source_reflows/main_c883_receivers.rs"),
        include_str!("../../src/source_reflows_merge/main_c883_inputs.rs"),
        include_str!("../../tests/workspace/semantic_graph/source_inventory_controls.rs"),
    ).to_owned()
}
fn assert_before_format_accepted_prefix(raw: &str) {
    let prefix = before_format_current_accepted_batch_relocated_prefix();
    let rest = raw
        .strip_prefix(prefix.as_str())
        .expect("complete independent pre-format thirty-subject prefix");
    assert!(!rest.is_empty());
    assert!(!rest.starts_with(prefix.as_str()));
}
fn assert_current_accepted_batch_relocated_prefix(raw: &str) {
    let prefix = current_accepted_batch_relocated_prefix();
    let rest = raw.strip_prefix(prefix.as_str()).expect("all original twenty-eight subjects and both nested lexical children enter the raw owner in order");
    assert!(
        !rest.is_empty(),
        "current relocated prefix requires the remaining raw inventory"
    );
    assert!(
        !rest.starts_with(prefix.as_str()),
        "duplicate current relocated inventory prefix"
    );
}
#[test]
fn current_accepted_prefix_keeps_nested_lexical_children_and_all_original_subjects() {
    let prefix = current_accepted_batch_relocated_prefix();
    let original = accepted_batch_relocated_prefix();
    let before_format = before_format_current_accepted_batch_relocated_prefix();
    assert_eq!(before_format, formatted_accepted_batch_relocated_prefix());
    assert_before_format_accepted_prefix(&format!("{before_format}remaining raw inventory"));
    for bad in [
        String::new(),
        before_format.clone(),
        format!("sentinel{before_format}"),
        format!("{before_format}{before_format}"),
        format!("{original}remaining raw inventory"),
    ] {
        assert!(std::panic::catch_unwind(|| assert_before_format_accepted_prefix(&bad)).is_err());
    }
    let valid = format!("{prefix}remaining raw inventory");
    assert_current_accepted_batch_relocated_prefix(&valid);
    assert_accepted_batch_relocated_prefix(&format!("{original}remaining raw inventory"));
    for bad in [
        String::new(),
        prefix.clone(),
        format!("sentinel{prefix}"),
        format!("{prefix}{prefix}"),
        format!("{original}remaining raw inventory"),
    ] {
        assert_ne!(bad, valid, "identity mutation cannot prove refusal");
        assert!(
            std::panic::catch_unwind(|| assert_current_accepted_batch_relocated_prefix(&bad))
                .is_err()
        );
    }
    for subject in [
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/matched.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/current_tests.rs"),
        include_str!("../../src/hir/resolve_program_methods.rs"),
    ] {
        assert!(!subject.is_empty());
        assert_eq!(prefix.matches(subject).count(), 1);
        let missing = prefix.replacen(subject, "", 1);
        for bad in [format!("{missing}remaining raw inventory"),
            format!("{missing}{subject}remaining raw inventory"),
            format!("{prefix}{subject}remaining raw inventory")] {
            // A duplicate inside the current prefix must also be refused;
            // appending an unrelated raw tail remains allowed by this contract.
            let bad = if bad.starts_with(prefix.as_str()) {
                prefix.replacen(subject, &format!("{subject}{subject}"), 1) + "remaining raw inventory"
            } else { bad };
            assert_ne!(bad, valid);
            assert!(std::panic::catch_unwind(|| assert_current_accepted_batch_relocated_prefix(&bad)).is_err());
        }
    }
}
