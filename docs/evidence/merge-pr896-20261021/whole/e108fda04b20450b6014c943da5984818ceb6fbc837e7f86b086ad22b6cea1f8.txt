//! Exact immutable-run successor; complete older guards remain independently executable.
// The complete incoming stage is authenticated before its unchanged finite
// inverse. Mutated historical inputs stay mutated and must fail that inverse.
fn incoming_stage(current: &str, path: &str, sha: &str, live: &str) -> String {
    if current == live {
        crate::source_reflows_merge::main_e177_final_inputs::value_direct(path, sha, current)
    } else {
        current.to_owned()
    }
}
fn restore(current: &str, length: usize, edits: &[(usize, &str, &str)]) -> String {
    assert_eq!(current.len(), length, "unknown shared-step subject extent");
    let mut result = current.to_owned();
    let mut last = length;
    for &(at, after, before) in edits {
        let end = at
            .checked_add(after.len())
            .expect("shared step inverse overflow");
        assert!(end <= last && result.is_char_boundary(at) && result.is_char_boundary(end));
        assert_eq!(&result[at..end], after, "unknown shared step source span");
        result.replace_range(at..end, before);
        last = at;
    }
    result
}
pub(super) fn previous_macro(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs",
        "1278774f715445f4cee3784ef21bc2618c8be3e088a188785e2d8fd3649df0b9",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 17993, MACRO)
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (15443, "\n// Both retained views name the same immutable segment at the same offset.\n// Replay every original work request, but form only numeric loop carriers.\n// Private borrowed views/item positions publish after the whole run. An error\n// returns immediately: these non-owning fields cannot affect a Diagnostic,\n// owned Drop, Rc count or any externally visible state before that return.\nmacro_rules! shared_plan_segment_run {\n    ($owner:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n        paid!(\n            $budget,\n            $function,\n            2 * size_of::<&[Rc<SkeletonObservation>]>() + 3 * size_of::<usize>(),\n            $phase\n        )?;\n        #[cfg(test)]\n        $census.shared_runs();\n        let shared_length = $owner.left_items.len();\n        let mut shared_remaining = shared_length;\n        loop {\n            // Full condition/decrement envelope, including the zero sentinel.\n            paid!(\n                $budget,\n                $function,\n                4 * size_of::<usize>() + size_of::<bool>(),\n                $phase\n            )?;\n            #[cfg(test)]\n            $census.shared_turns();\n            if shared_remaining == 0 {\n                break;\n            }\n            #[cfg(test)]\n            $census.shared_steps();\n            $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n            #[cfg(test)]\n            $census.step();\n            $budget.charge(\n                $function,\n                1,\n                \"ordinary plan history forward segment traversal\",\n            )?;\n            $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n            #[cfg(test)]\n            $census.step();\n            $budget.charge(\n                $function,\n                1,\n                \"ordinary plan history forward segment traversal\",\n            )?;\n            $budget.charge($function, 1, \"ordinary plan history observation comparison\")?;\n            #[cfg(test)]\n            $census.round();\n            shared_remaining -= 1;\n        }\n        paid!(\n            $budget,\n            $function,\n            4 * size_of::<&[Rc<SkeletonObservation>]>()\n                + 4 * size_of::<usize>()\n                + size_of::<Ordering>(),\n            $phase\n        )?;\n        $owner.left_items = &[];\n        $owner.right_items = &[];\n        $owner.left_cursor.item += shared_length;\n        $owner.right_cursor.item += shared_length;\n        $owner.ordering = Ordering::Equal;\n        continue;\n    };\n}\n", ""),
    (8834, "                    shared_plan_segment_run!($owner, $census, $function, $budget, $phase);\n", "                    paid!(\n                        $budget,\n                        $function,\n                        3 * size_of::<&[Rc<SkeletonObservation>]>()\n                            + 2 * size_of::<usize>()\n                            + size_of::<Ordering>()\n                            + size_of::<bool>(),\n                        $phase\n                    )?;\n                    #[cfg(test)]\n                    $census.shared_steps();\n                    let [_, right_rest @ ..] = $owner.right_items else {\n                        unreachable!(\"identical immutable ordinary plan segment and offset\");\n                    };\n                    $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n                    #[cfg(test)]\n                    $census.step();\n                    $budget.charge(\n                        $function,\n                        1,\n                        \"ordinary plan history forward segment traversal\",\n                    )?;\n                    $owner.left_items = left_rest;\n                    $owner.left_cursor.item += 1;\n                    $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n                    #[cfg(test)]\n                    $census.step();\n                    $budget.charge(\n                        $function,\n                        1,\n                        \"ordinary plan history forward segment traversal\",\n                    )?;\n                    $owner.right_items = right_rest;\n                    $owner.right_cursor.item += 1;\n                    $owner.ordering = Ordering::Equal;\n                    $budget.charge($function, 1, \"ordinary plan history observation comparison\")?;\n                    #[cfg(test)]\n                    $census.round();\n                    continue;\n"),
    (8365, "            if let [_, ..] = $owner.left_items {\n", "            if let [_, left_rest @ ..] = $owner.left_items {\n"),
    (8234, "            paid!($budget, $function, size_of::<bool>(), $phase)?;\n", "            paid!(\n                $budget,\n                $function,\n                size_of::<&[Rc<SkeletonObservation>]>() + size_of::<bool>(),\n                $phase\n            )?;\n"),
    (7623, "// Six payment and five work sites have distinct complete primitive frames in\n", "// Four payment and five work sites have distinct complete primitive frames in\n"),
    (488, "pub(super) const PRIMITIVE_FRAMES: usize = 42 * PAYMENT_HEADER;\n", "pub(super) const PRIMITIVE_FRAMES: usize = 40 * PAYMENT_HEADER;\n"),
    (339, "// The shared immutable-step branch adds six payment and five work sites.\n", "// The shared immutable-step branch adds four payment and five work sites.\n"),
];
pub(super) fn previous_pair_tests(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs",
        "63c14f327486e06d5d953aa494c5a9c9d530cb172d6dec03114892ef1b8055fa",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 49614, PAIR_TESTS)
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (47511, "\n#[test]\nfn shared_run_replays_every_real_883_work_cut_before_private_cursor_publication() {\n    let program = super::super::super::tests::program();\n    let function = program\n        .functions\n        .iter()\n        .find(|f| f.id.as_str() == \"f\")\n        .unwrap();\n    let item = Rc::new(SkeletonObservation::Boolean {\n        expression: function.body.id.clone(),\n        value: true,\n    });\n    let expected = vec![\n        SkeletonPath {\n            observations: vec![item.clone(); 4].into(),\n            terminal: SkeletonTerminal::Success\n        };\n        2\n    ];\n    let actual = || HistorySet {\n        arena: vec![Segment {\n            previous: None,\n            depth: 1,\n            items: vec![item.clone(); 4],\n        }],\n        paths: (0..2)\n            .map(|_| PlanPath {\n                history: History {\n                    previous: Some(0),\n                    tail: Vec::new(),\n                },\n                terminal: SkeletonTerminal::Success,\n            })\n            .collect(),\n        max_depth: 1,\n    };\n    pair_scope_census::reset();\n    let (old, old_heap, _) =\n        observe_shaped_pair(function, &expected, actual(), true, MAX_REPLAY_WORK_UNITS);\n    pair_scope_census::reset();\n    let (new, new_heap, _) =\n        observe_shaped_pair(function, &expected, actual(), false, MAX_REPLAY_WORK_UNITS);\n    let row = complete_row(false);\n    assert!(row.shared_runs > 0 && row.shared_steps > row.shared_runs);\n    assert_eq!(row.shared_turns, row.shared_steps + row.shared_runs);\n    assert_eq!(new, old);\n    assert!(new.result.is_ok());\n    assert_eq!(new_heap, old_heap);\n    let spent = MAX_REPLAY_WORK_UNITS - old.remaining;\n    for cut in 0..=spent {\n        pair_scope_census::reset();\n        let (old, _, _) = observe_shaped_pair(function, &expected, actual(), true, cut);\n        pair_scope_census::reset();\n        let (new, _, _) = observe_shaped_pair(function, &expected, actual(), false, cut);\n        assert_eq!(\n            new, old,\n            \"complete six-field diagnostics and exact remaining work at cut {cut}\"\n        );\n    }\n}\n", ""),
    (6600, "        + (2 * size_of::<&[Rc<SkeletonObservation>]>() + 3 * size_of::<usize>()) * row.shared_runs\n        + (4 * size_of::<usize>() + size_of::<bool>()) * row.shared_turns\n        + (4 * size_of::<&[Rc<SkeletonObservation>]>()\n            + 4 * size_of::<usize>()\n            + size_of::<Ordering>())\n            * row.shared_runs;\n", "        + (3 * size_of::<&[Rc<SkeletonObservation>]>()\n            + 2 * size_of::<usize>()\n            + size_of::<Ordering>()\n            + size_of::<bool>())\n            * row.shared_steps;\n"),
    (6469, "        + size_of::<bool>() * row.shared_patterns\n", "        + (size_of::<&[Rc<SkeletonObservation>]>() + size_of::<bool>()) * row.shared_patterns\n"),
    (6325, "        + if paths >= 2 { 11 * header } else { 0 }\n", "        + if paths >= 2 { 9 * header } else { 0 }\n"),
];
pub(super) fn previous_census(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs",
        "b0caf3a16b37f3eb69418ae93a7e405bb06388b78795c4fa34a491ef3ee66ef5",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 16853, CENSUS)
}
#[rustfmt::skip]
const CENSUS: &[(usize, &str, &str)] = &[
    (15803, "    #[test]\n    fn shared_run_counter_overflow_is_incomplete_without_work_or_identity_aliasing() {\n        for turn in [false, true] {\n            let function = fixture();\n            let budget = ReplayBudget::unobserved();\n            let remaining = budget.remaining;\n            reset();\n            let guard = begin(&function, &budget, false);\n            TRACE.with(|trace| {\n                let mut trace = trace.borrow_mut();\n                if turn {\n                    trace.rows[0].shared_turns = usize::MAX;\n                } else {\n                    trace.rows[0].shared_runs = usize::MAX;\n                }\n            });\n            if turn {\n                guard.shared_turns();\n            } else {\n                guard.shared_runs();\n            }\n            guard.success();\n            drop(guard);\n            let (rows, lost) = finish();\n            assert_eq!(lost, 0);\n            assert_eq!(budget.remaining, remaining);\n            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);\n        }\n    }\n", ""),
    (7099, "            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_runs(&self) {\n        self.update(|r| match r.shared_runs.checked_add(1) {\n            Some(n) => r.shared_runs = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_turns(&self) {\n        self.update(|r| match r.shared_turns.checked_add(1) {\n            Some(n) => r.shared_turns = n,\n", ""),
    (1351, "    shared_runs: 0,\n    shared_turns: 0,\n", ""),
    (732, "    pub shared_runs: usize,\n    pub shared_turns: usize,\n", ""),
];
pub(super) fn previous_boundary_controls(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs",
        "f3c7bd5f0c297d24432f4893c1e3ef9b0dd12b45080efc4f851c6e335c841a16",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs"
        )),
    );
    let stage = if current
        == include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries.rs"
        ))
    {
        use sha2::{Digest, Sha256};
        assert_eq!(current.len(), 23784);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            "921932de646b2be17d4f64a8a2f8e499d6ba0098a4e145d326ed0e88f66f90ca",
        );
        let glue = "    let stage = shared_runs::previous_shared_step_controls(&stage);\n";
        assert_eq!(current.matches(glue).count(), 1);
        // Keep the original whole current input for the entry-stage guard.
        // This intermediate is only an operand of the exact finite inverse.
        let without_glue = current.replacen(glue, "", 1);
        assert_eq!(without_glue.len(), 23716);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(without_glue.as_bytes()))
            ),
            "6c805d362018fbb4a84f47bba80ca9481ab1b2439d70e90374399e7265ba815a",
        );
        let registration = "#[path = \"shared_boundaries/shared_runs.rs\"]\n";
        assert_eq!(without_glue.matches(registration).count(), 1);
        let before = without_glue.replacen(registration, "", 1);
        assert_eq!(before.len(), 23671);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
            ),
            "3a1eeb16c1dba344d7835ff7c9bf23078205645aedf8fe2a54364f519f0059b3"
        );
        let old_acquisition = "    let stage = if current\n        == include_str!(concat!(\n            env!(\"CARGO_MANIFEST_DIR\"),\n            \"/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs\"\n        )) {\n        crate::source_reflows_merge::main_e177_final_inputs::value_direct(\n            \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs\",\n            \"329c0b6c60e477128d26f30383223566cc0bca79e13063e97a7c79ef7fa03255\",\n            current,\n        )\n    } else {\n        current.to_owned()\n    };\n    let current = stage.as_str();\n";
        assert_eq!(before.matches(old_acquisition).count(), 1);
        let before = before.replacen(old_acquisition, "", 1);
        assert_eq!(before.len(), 22995);
        assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))),
            "f3c7bd5f0c297d24432f4893c1e3ef9b0dd12b45080efc4f851c6e335c841a16");
        assert!(stage == current || stage == before, "unexpected complete boundary entry stage");
        before
    } else {
        stage
    };
    let current = stage.as_str();
    restore(current, 22995, BOUNDARY_CONTROLS)
}
#[rustfmt::skip]
const BOUNDARY_CONTROLS: &[(usize, &str, &str)] = &[
    (22585, "            &shared_runs::previous_route(include_str!(\n                \"../../../../../../../../../bounded_output/stage_debits/route_debits.rs\"\n            )),\n            previous_route_stage,\n", "            include_str!(\"../../../../../../../../../bounded_output/stage_debits/route_debits.rs\"),\n            previous_route,\n"),
    (21905, "            &shared_runs::previous_census(include_str!(\"../../../../../pair_scope_census.rs\")),\n            previous_census_stage,\n", "            include_str!(\"../../../../../pair_scope_census.rs\"),\n            previous_census,\n"),
    (21532, "            &shared_runs::previous_pair_tests(include_str!(\"../../../../../pair_scope_tests.rs\")),\n            previous_pair_tests_stage,\n", "            include_str!(\"../../../../../pair_scope_tests.rs\"),\n            previous_pair_tests,\n"),
    (21181, "            &shared_runs::previous_macro(include_str!(\"../../../../../lexical_cursor.rs\")),\n            previous_macro_stage,\n", "            include_str!(\"../../../../../lexical_cursor.rs\"),\n            previous_macro,\n"),
    (17387, "    previous_route_stage(&shared_runs::previous_route(current))\n}\nfn previous_route_stage(current: &str) -> String {\n", ""),
    (13153, "    previous_census_stage(&shared_runs::previous_census(current))\n}\nfn previous_census_stage(current: &str) -> String {\n", ""),
    (7881, "    previous_pair_tests_stage(&shared_runs::previous_pair_tests(current))\n}\nfn previous_pair_tests_stage(current: &str) -> String {\n", ""),
    (794, "    previous_macro_stage(&shared_runs::previous_macro(current))\n}\nfn previous_macro_stage(current: &str) -> String {\n", ""),
    (721, "mod shared_runs;\n", ""),
];
pub(super) fn previous_route(current: &str) -> String {
    let stage = incoming_stage(
        current,
        "src/bounded_output/stage_debits/route_debits.rs",
        "4fbbb4241187bdc85e3687bffd1829ff6c16c63e5a198a6e1ef767e84c273c9d",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/bounded_output/stage_debits/route_debits.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 35279, ROUTE)
}
#[rustfmt::skip]
const ROUTE: &[(usize, &str, &str)] = &[
    (34428, "            .count(),\n        1\n    );\n    assert_eq!(\n        compact\n            .matches(\"shared_plan_segment_run!($owner,$census,$function,$budget,$phase);\")\n", ""),
    (34171, "    assert_eq!(lexical.matches(\"shared_plan_segment_run!(\").count(), 1);\n", ""),
    (34032, "    assert_eq!(sites.len(), 13, \"audit every generic payment site\");\n", "    assert_eq!(sites.len(), 11, \"audit every generic payment site\");\n"),
];
#[cfg(test)]
mod tests {
    use super::*;
    fn hostile(
        current: &str,
        inverse: fn(&str) -> String,
        before: &str,
        edits: &[(usize, &str, &str)],
        expected_stage: (usize, &str),
    ) {
        // Authenticate the complete physical input first, including its EOF.
        assert_eq!(inverse(current), before);
        assert!(std::panic::catch_unwind(|| inverse(&format!("{current} "))).is_err());
        let physical_at = current
            .char_indices()
            .find(|(_, c)| c.is_ascii_alphanumeric())
            .unwrap()
            .0;
        let mut physical_drift = current.to_owned();
        let replacement = if &physical_drift[physical_at..physical_at + 1] == "x" {
            "y"
        } else {
            "x"
        };
        physical_drift.replace_range(physical_at..physical_at + 1, replacement);
        let physical_result = std::panic::catch_unwind(|| inverse(&physical_drift));
        assert!(physical_result.is_err() || physical_result.unwrap() != before);
        // Restore the exact complete historical input BEFORE applying its
        // original offset controls. Keep all old span/extent/outside checks.
        let mut stage = before.to_owned();
        for &(at, after, prior) in edits.iter().rev() {
            let end = at.checked_add(prior.len()).expect("shared stage overflow");
            assert!(
                end <= stage.len() && stage.is_char_boundary(at) && stage.is_char_boundary(end)
            );
            assert_eq!(&stage[at..end], prior);
            stage.replace_range(at..end, after);
        }
        assert_eq!(stage.len(), expected_stage.0);
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(stage.as_bytes()))
            ),
            expected_stage.1
        );
        let current = stage.as_str();
        assert_eq!(inverse(current), before);
        let &(at, after, _) = edits
            .iter()
            .find(|(_, after, _)| !after.is_empty())
            .unwrap();
        let mut changed = current.to_owned();
        let end = at + after.chars().next().unwrap().len_utf8();
        let replacement = if &changed[at..end] == "x" { "y" } else { "x" };
        changed.replace_range(at..end, replacement);
        assert!(std::panic::catch_unwind(|| inverse(&changed)).is_err());
        assert!(std::panic::catch_unwind(|| inverse(&format!("{current} "))).is_err());
        let at = current
            .char_indices()
            .find(|(i, c)| {
                c.is_ascii_alphanumeric()
                    && !edits
                        .iter()
                        .any(|(at, after, _)| *i >= *at && *i < at + after.len())
            })
            .unwrap()
            .0;
        let mut outside = current.to_owned();
        let replacement = if &outside[at..at + 1] == "x" {
            "y"
        } else {
            "x"
        };
        outside.replace_range(at..at + 1, replacement);
        let restored = std::panic::catch_unwind(|| inverse(&outside));
        assert!(restored.is_err() || restored.unwrap() != before);
    }
    #[test]
    fn whole_run_sources_restore_every_complete_predecessor() {
        assert_eq!(
            previous_macro(include_str!("../../../../../../lexical_cursor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-macro.rs.txt"
            ))
        );
        assert_eq!(
            previous_pair_tests(include_str!("../../../../../../pair_scope_tests.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-pair-tests.rs.txt"
            ))
        );
        assert_eq!(
            previous_census(include_str!("../../../../../../pair_scope_census.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-census.rs.txt"
            ))
        );
        assert_eq!(
            previous_boundary_controls(include_str!("../shared_boundaries.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-boundary-controls.rs.txt"
            ))
        );
        assert_eq!(
            previous_route(include_str!(
                "../../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-route.rs.txt"
            ))
        );
    }
    #[test]
    fn run_sources_refuse_span_extent_and_outside_drift_without_masking() {
        hostile(
            include_str!("../../../../../../lexical_cursor.rs"),
            previous_macro,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-macro.rs.txt"
            )),
            MACRO,
            (
                17993,
                "1278774f715445f4cee3784ef21bc2618c8be3e088a188785e2d8fd3649df0b9",
            ),
        );
        hostile(
            include_str!("../../../../../../pair_scope_tests.rs"),
            previous_pair_tests,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-pair-tests.rs.txt"
            )),
            PAIR_TESTS,
            (
                49614,
                "63c14f327486e06d5d953aa494c5a9c9d530cb172d6dec03114892ef1b8055fa",
            ),
        );
        hostile(
            include_str!("../../../../../../pair_scope_census.rs"),
            previous_census,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-census.rs.txt"
            )),
            CENSUS,
            (
                16853,
                "b0caf3a16b37f3eb69418ae93a7e405bb06388b78795c4fa34a491ef3ee66ef5",
            ),
        );
        hostile(
            include_str!("../shared_boundaries.rs"),
            previous_boundary_controls,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-boundary-controls.rs.txt"
            )),
            BOUNDARY_CONTROLS,
            (
                22995,
                "f3c7bd5f0c297d24432f4893c1e3ef9b0dd12b45080efc4f851c6e335c841a16",
            ),
        );
        hostile(
            include_str!(
                "../../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            ),
            previous_route,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-runs-b66-20261018/before-route.rs.txt"
            )),
            ROUTE,
            (
                35279,
                "4fbbb4241187bdc85e3687bffd1829ff6c16c63e5a198a6e1ef767e84c273c9d",
            ),
        );
    }
}

// Preserve the complete pure7e control before its unchanged offset inverse.
// Only the literal compiled current subject may enter this finite glue inverse.
pub(super) fn previous_shared_step_controls(current: &str) -> String {
    let compiled = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs"
    ));
    if current != compiled {
        return current.to_owned();
    }
    use sha2::{Digest, Sha256};
    let acquired = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs",
        "651542d472bf505cc7f9c5eacebaf31718b83f8043ab40ead8e90a706e0be3f4",
        current,
    );
    assert_eq!(acquired.len(), 21023);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(acquired.as_bytes()))
        ),
        "651542d472bf505cc7f9c5eacebaf31718b83f8043ab40ead8e90a706e0be3f4"
    );
    let before = restore(&acquired, 21023, SHARED_STEP_CONTROLS);
    assert_eq!(before.len(), 18577);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        "329c0b6c60e477128d26f30383223566cc0bca79e13063e97a7c79ef7fa03255"
    );
    before
}
#[rustfmt::skip]
const SHARED_STEP_CONTROLS: &[(usize, &str, &str)] = &[
    (20792, "            (\"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs\", \"fc15bb2a80a9821f3616075f730b8e1dc9e958427c7f42a55e12d7d9d894c921\", 11132),\n", ""),
    (20301, "            (\"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs\", \"ced9cc53bd363653bcc182b443bcbc41b2be69c005d441e3c4a3599f20e90ef5\", 12959),\n", ""),
    (20017, "            include_str!(\"../../../../pair_scope_census.rs\"),\n            previous_census,\n", "            &shared_boundaries::previous_census(include_str!(\"../../../../pair_scope_census.rs\")),\n            previous_census_stage,\n"),
    (19814, "            (\"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs\", \"10ecb901fd3ce0805b1fbc81f68ad221d9a4fc3ddf3f707ff05fc38fff5b13e5\", 42740),\n", ""),
    (19519, "            include_str!(\"../../../../pair_scope_tests.rs\"),\n            previous_pair_tests,\n", "            &shared_boundaries::previous_pair_tests(include_str!(\n                \"../../../../pair_scope_tests.rs\"\n            )),\n            previous_pair_tests_stage,\n"),
    (19333, "            (\"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs\", \"a9b5037a2522c8e1bd7d24363894176aef29983900d48028b30fa86e6dabe5c8\", 23539),\n", ""),
    (18849, "            (\"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs\", \"e3278dc29dcce665dd9d3a67ce048e552254a65c5a939f769b2b7c1eb86a9ded\", 10841),\n", ""),
    (18571, "            include_str!(\"../../../../lexical_cursor.rs\"),\n            previous_macro,\n", "            &shared_boundaries::previous_macro(include_str!(\"../../../../lexical_cursor.rs\")),\n            previous_macro_stage,\n"),
    (15045, "        // Public current-first acquisition must still reject physical drift.\n        assert_eq!(inverse(current), before);\n        assert!(std::panic::catch_unwind(|| inverse(&format!(\"{current} \"))).is_err());\n        let acquired = super::super::super::super::super::super::reachable_stage(\n            stage.0, stage.1, current);\n        assert_eq!(acquired.len(), stage.2);\n        let current = acquired.as_str();\n        // Archived inverse offsets apply to this authenticated complete stage,\n        // not to the later physical source which legitimately has another size.\n        let inverse = |current: &str| restore(current, stage.2, edits);\n        assert_eq!(inverse(current), before);\n", ""),
    (15001, "        stage: (&str, &str, usize),\n", ""),
    (13199, "    let stage = super::super::super::super::super::reachable_stage(\n        \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs\", \"fc15bb2a80a9821f3616075f730b8e1dc9e958427c7f42a55e12d7d9d894c921\", current);\n    let current = stage.as_str();\n", ""),
    (10326, "    let stage = super::super::super::super::super::reachable_stage(\n        \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs\", \"ced9cc53bd363653bcc182b443bcbc41b2be69c005d441e3c4a3599f20e90ef5\", current);\n    let current = stage.as_str();\n", "    previous_census_stage(&shared_boundaries::previous_census(current))\n}\nfn previous_census_stage(current: &str) -> String {\n"),
    (7080, "    let stage = super::super::super::super::super::reachable_stage(\n        \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs\", \"10ecb901fd3ce0805b1fbc81f68ad221d9a4fc3ddf3f707ff05fc38fff5b13e5\", current);\n    let current = stage.as_str();\n", "    previous_pair_tests_stage(&shared_boundaries::previous_pair_tests(current))\n}\nfn previous_pair_tests_stage(current: &str) -> String {\n"),
    (6119, "    let stage = super::super::super::super::super::reachable_stage(\n        \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs\", \"a9b5037a2522c8e1bd7d24363894176aef29983900d48028b30fa86e6dabe5c8\", current);\n    let current = stage.as_str();\n", ""),
    (774, "    let stage = super::super::super::super::super::reachable_stage(\n        \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs\", \"e3278dc29dcce665dd9d3a67ce048e552254a65c5a939f769b2b7c1eb86a9ded\", current);\n    let current = stage.as_str();\n", "    previous_macro_stage(&shared_boundaries::previous_macro(current))\n}\nfn previous_macro_stage(current: &str) -> String {\n"),
    (718, "", "mod shared_boundaries;\n"),
];
