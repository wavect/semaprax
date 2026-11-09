//! Explicit boundary membership is independent of complete source-role custody.
//! The full predecessor helper remains byte-exact and independently compiled.
#[path = "new_boundary_module.rs"]
mod boundary;
// The independently hashed predecessor is a byte-exact evidence fixture.
#[rustfmt::skip]
#[path = "production_source_inventory_original.rs"]
mod original;

pub(super) fn production(raw: &str, manifest: &str) -> String {
    assert_current_full_raw_relocated_prefix(raw);
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
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before.rs"), include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs"),
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
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before.rs"), include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs"),
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
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before.rs"), include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs"),
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
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before.rs"), include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs"),
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
// Exact current leading acquisitions; the complete raw payload remains unchanged.
fn current_hir_leading_acquisitions() -> &'static str {
    concat!(
        include_str!("../../tests/wasm/scalar_browser_ci_contract/current_profiles573.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/compact_failure_before.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/compact_failure_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/contains_before.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/contains_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/selected/tests/fixture_scope.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_digests_v13.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_digests_v13/logical_v12.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_digests_v13/tests.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_digests_v13/controller_transport.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/capacity/type_cursor_tests.rs"),
        include_str!("../../src/source_verify/constructor_tests/corpus/operator_refusals.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/direct_one_before.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/direct_one_before/boxed_flow.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/direct_one_before/frontier.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/direct_one_tests.rs"),
        include_str!("../../src/bounded_output/exclusive_debits/path_census.rs"),
        include_str!("../../src/bounded_output/request_reservation.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/ordinary_receiving_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/ordinary_receiving_before.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/compact/ordinary_receiving_before.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/held_edge_before.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/held_edge_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/hir_last_use.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/hir_memo_release_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/shared_insertions_before.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/shared_insertions_before/boxed_flow.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/shared_insertions_before/boxed_states.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/shared_insertions_before/frontier.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/shared_insertions_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/plan_prefix.rs"),
        include_str!("../../src/cleanup_plan/replay/plan_prefix/before.rs"),
        include_str!("../../src/cleanup_plan/replay/plan_prefix/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/plan_prefix.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/query_proof.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/query_proof/before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/query_proof/tests.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_lookup.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs"),
        include_str!("../../src/cleanup_plan/replay/branch_clone/short_plan.rs"),
        include_str!("../../src/cleanup_plan/replay/nested_shape/scalar_tests.rs"),
        include_str!("../../src/test_allocation_observer/phase_peaks.rs"),
        include_str!("../../src/loan_plan/paid_plan/before_reachability_release.rs"),
        include_str!("../../src/loan_plan/paid_plan/reachability_release_tests.rs"),
        include_str!("../../src/loan_plan/funded_flat/before_reachability_release.rs"),
        include_str!("../../src/workspace_graph/loan_retention/overlap_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/immediate_before.rs"),
        include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/ordinary_labels.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before_frame_pricing.rs"),
        include_str!("../../src/cleanup_plan/replay/selected_owner/ordinary.rs"),
        include_str!("../../src/cleanup_plan/replay/selected_owner/ordinary/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/last_use.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/before.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/last_use/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/receiving_sort/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/before_frontier/boxed_states.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/before_frontier/branch_join.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/before_frontier/mod.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/before_frontier/storage.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/before_frontier/storage/atomic_controls.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/before_frontier/storage/empty_renewals.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/before_frontier/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/before_frontier/transition.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/boxed_flow.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/frontier.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/frontier_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/frontier_tests/custody.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/frontier_tests/whole.rs"),
        include_str!("../../src/call_index/validation_before.rs"),
        include_str!("../../src/call_index/validation_receipt_tests.rs"),
        include_str!("../../src/codegen/native_owned_data_provider/emit_provider_before.rs"),
        include_str!("../../src/codegen/native_owned_data_provider/validation_reuse_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/before.rs"),
        include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/flat_probe/receiving_tests.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/names/interface_census.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/names/interface_before.rs"),
        include_str!("../../src/workspace_graph/expected_projection/private_layout/names/interface_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/validation/tests.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/tests.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_labels_v12.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs"),
        include_str!("../../src/workspace_graph/expected_projection/call_carriers_directory/modular.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/modular_census.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/modular_pair.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/modular_pair_tests.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/modular_projection.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/modular_session.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/modular_tests.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/paired.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/paired_projection.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/paired_tests.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/record_before.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/record_signature_tests.rs"),
        include_str!("../../src/workspace_graph/expected_projection/shared_local_callees/record_signatures.rs"),
    )
}
fn current_projection_leading_acquisitions() -> &'static str {
    concat!(
        include_str!("../../src/cleanup_plan/replay/path_join/compact_failure_before.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/compact_failure_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/contains_before.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/contains_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/selected/tests/fixture_scope.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_digests_v13.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_digests_v13/logical_v12.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_digests_v13/tests.rs"),
        include_str!("../../src/source_reflows_merge/main_e177_overlay_digests_v13/controller_transport.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/capacity/cursor_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/expression_facts/capacity/type_cursor_tests.rs"),
        include_str!("../../src/source_verify/constructor_tests/corpus/operator_refusals.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/direct_one_before.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/direct_one_before/boxed_flow.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/direct_one_before/frontier.rs"),
        include_str!("../../src/cleanup_plan/replay/path_join/direct_one_tests.rs"),
        include_str!("../../src/bounded_output/exclusive_debits/path_census.rs"),
        include_str!("../../src/bounded_output/request_reservation.rs"),
        include_str!("../../src/test_allocation_observer/phase_peaks.rs"),
        include_str!("../../src/loan_plan/paid_plan/before_reachability_release.rs"),
        include_str!("../../src/loan_plan/paid_plan/reachability_release_tests.rs"),
        include_str!("../../src/loan_plan/funded_flat/before_reachability_release.rs"),
        include_str!("../../src/workspace_graph/loan_retention/overlap_tests.rs"),
        include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/shipping/flat_admission/immediate_before.rs"),
    )
}
fn current_hir_browser_leading_acquisitions() -> String {
    let first = include_str!("../../tests/wasm/scalar_browser_ci_contract/current_profiles573.rs");
    let browser = include_str!("../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs");
    let original = current_hir_leading_acquisitions();
    assert!(!first.is_empty() && !browser.is_empty());
    assert_eq!(original.matches(first).count(), 1);
    assert_eq!(original.matches(browser).count(), 0);
    let rest = original
        .strip_prefix(first)
        .expect("complete original HIR first acquisition before the browser child");
    format!("{first}{browser}{rest}")
}
fn assert_current_full_raw_relocated_prefix(raw: &str) {
    let moved_child = include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs");
    assert_eq!(raw.matches(moved_child).count(), 1, "the complete moved lexical child must remain present exactly once in actual raw");
    let next_hir = current_hir_browser_leading_acquisitions();
    for leading in [current_hir_leading_acquisitions(), current_projection_leading_acquisitions(), next_hir.as_str()] {
        if let Some(original_tail) = raw.strip_prefix(leading) {
            let original = current_accepted_batch_relocated_prefix();
            let moved_child = include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs");
            assert_eq!(original.matches(moved_child).count(), 1);
            let current_slot = original.replacen(moved_child, "", 1);
            let rest = original_tail.strip_prefix(current_slot.as_str()).expect("all current thirty-two source chunks retain exact order and multiplicity");
            assert!(!rest.is_empty());
            assert!(!rest.starts_with(current_slot.as_str()));
            // Only the historical guard view restores the child slot. The full
            // unmodified raw payload is still authenticated by original::production.
            assert_current_accepted_batch_relocated_prefix(&format!("{original}{rest}"));
            return;
        }
    }
    panic!("complete exact HIR or projection leading acquisition sequence required");
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
    let next_hir = current_hir_browser_leading_acquisitions();
    for (leading, first, second) in [
        (current_hir_leading_acquisitions(), include_str!("../../tests/wasm/scalar_browser_ci_contract/current_profiles573.rs"), include_str!("../../src/cleanup_plan/replay/path_join/compact_failure_before.rs")),
        (current_projection_leading_acquisitions(), include_str!("../../src/cleanup_plan/replay/path_join/compact_failure_before.rs"), include_str!("../../src/cleanup_plan/replay/path_join/compact_failure_tests.rs")),
        (next_hir.as_str(), include_str!("../../tests/wasm/scalar_browser_ci_contract/current_profiles573.rs"), include_str!("../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs")),
    ] {
        let first_two = format!("{first}{second}");
        let rest = leading.strip_prefix(first_two.as_str()).unwrap();
        let relocated_child = include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity/before_short.rs");
        assert_eq!(prefix.matches(relocated_child).count(), 1);
        let current_prefix = prefix.replacen(relocated_child, "", 1);
        let actual_tail_child = if leading.matches(relocated_child).count() == 1 { "" } else { relocated_child };
        let valid = format!("{leading}{current_prefix}{actual_tail_child}remaining raw inventory");
        assert_current_full_raw_relocated_prefix(&valid);
        for bad in [valid.replacen(relocated_child, "", 1), format!("{valid}{relocated_child}")] {
            assert!(std::panic::catch_unwind(|| assert_current_full_raw_relocated_prefix(&bad)).is_err());
        }
        for changed in [
            format!("{second}{rest}"),
            format!("{second}{first}{rest}"),
            format!("{first}{first}{second}{rest}"),
            format!("{first}{second}{second}{rest}"),
            format!("foreign{leading}"),
        ] {
            assert_ne!(changed, leading);
            let bad = format!("{changed}{current_prefix}{actual_tail_child}remaining raw inventory");
            assert!(std::panic::catch_unwind(|| assert_current_full_raw_relocated_prefix(&bad)).is_err());
        }
        let missing_nested = current_prefix.replacen(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/matched.rs"), "", 1);
        assert!(std::panic::catch_unwind(|| assert_current_full_raw_relocated_prefix(&format!("{leading}{missing_nested}{actual_tail_child}remaining raw inventory"))).is_err());
    }
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
