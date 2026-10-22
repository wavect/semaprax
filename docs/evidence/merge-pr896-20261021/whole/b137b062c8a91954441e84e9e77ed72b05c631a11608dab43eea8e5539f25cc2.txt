//! Whole paired-tail successor inverses; every prior assertion remains.
#[path = "fused_readers.rs"]
mod fused_readers;
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
pub(super) fn previous_owner(current: &str) -> String {
    let stage = super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs",
        "23469f3a0ec012a0975f50e627811aa4874fdf4215b6a1c734b1c93a7a043955",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs"
        )),
    );
    let current = stage.as_str();
    previous_owner_before_fused(&fused_readers::previous_owner(current))
}
fn previous_owner_before_fused(current: &str) -> String {
    restore(current, 24302, OWNER)
}
#[rustfmt::skip]
const OWNER: &[(usize, &str, &str)] = &[
    (15292, "                            left_path,\n                            right_path,\n                            left_tail,\n                            right_tail,\n", ""),
    (14705, "                    owner.tail_mask = 0;\n", ""),
    (14415, "                    PAIR_SCOPE\n                        + 2 * lexical_cursor::TAIL_VIEW\n                        + 2 * lexical_cursor::PAIR_LINK_VIEW\n                        + size_of::<u8>(),\n", "                    PAIR_SCOPE + 2 * lexical_cursor::TAIL_VIEW + 2 * lexical_cursor::PAIR_LINK_VIEW,\n"),
    (8879, "        tail_mask: 0,\n", ""),
    (7705, "    + size_of::<u8>() // mask initializer; retained field is in owner storage\n", ""),
    (589, "    tail_mask: u8,\n", ""),
];
pub(super) fn previous_macro(current: &str) -> String {
    let stage = super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs",
        "b66e1c1f47583c8c687e352be56bcb01e49da9456b81c3e36862a359416e5583",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs"
        )),
    );
    let current = stage.as_str();
    previous_macro_before_fused(&fused_readers::previous_macro(current))
}
fn previous_macro_before_fused(current: &str) -> String {
    restore(current, 25469, MACRO)
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (22250, "\n// Only non-owning tails, after both original prepared chains finish.\nmacro_rules! paired_plan_tails {\n    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n        loop {\n            // All pattern, slice, comparison and scalar payloads, including\n            // mutually exclusive terminal cases, are paid on each turn.\n            paid!(\n                $budget,\n                $function,\n                2 * size_of::<&Rc<SkeletonObservation>>()\n                    + 4 * size_of::<&[Rc<SkeletonObservation>]>()\n                    + 4 * size_of::<&Rc<SkeletonObservation>>()\n                    + 2 * size_of::<&SkeletonObservation>()\n                    + 4 * size_of::<&SkeletonObservation>()\n                    + 4 * size_of::<&SkeletonTerminal>()\n                    + 2 * size_of::<usize>()\n                    + 4 * size_of::<Ordering>()\n                    + 3 * size_of::<bool>(),\n                $phase\n            )?;\n            #[cfg(test)]\n            $census.fused_turns();\n            $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n            #[cfg(test)]\n            $census.step();\n            $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n            #[cfg(test)]\n            $census.step();\n            #[cfg(test)]\n            {\n                $census.tail();\n                $census.tail();\n            }\n            match $left_tail {\n                [left, left_rest @ ..] => {\n                    $left_tail = left_rest;\n                    $owner.left_cursor.item += 1;\n                    match $right_tail {\n                        [right, right_rest @ ..] => {\n                            $right_tail = right_rest;\n                            $owner.right_cursor.item += 1;\n                            $owner.ordering = left.as_ref().cmp(right.as_ref());\n                            if $owner.ordering == Ordering::Equal {\n                                $budget.charge(\n                                    $function,\n                                    1,\n                                    \"ordinary plan history observation comparison\",\n                                )?;\n                                #[cfg(test)]\n                                $census.round();\n                                continue;\n                            }\n                        }\n                        [] => $owner.ordering = Ordering::Greater,\n                    }\n                }\n                [] => match $right_tail {\n                    [_, right_rest @ ..] => {\n                        $right_tail = right_rest;\n                        $owner.right_cursor.item += 1;\n                        $owner.ordering = Ordering::Less;\n                    }\n                    [] => $owner.ordering = $left_path.terminal.cmp(&$right_path.terminal),\n                },\n            }\n            break;\n        }\n        break;\n    };\n}\n\nmacro_rules! remember_plan_tail {\n    ($owner:ident, left_cursor) => {\n        $owner.tail_mask |= 1;\n    };\n    ($owner:ident, right_cursor) => {\n        $owner.tail_mask |= 2;\n    };\n}\n", ""),
    (12251, "    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident, $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n        paid!($budget, $function, 3 * size_of::<bool>(), $phase)?;\n        #[cfg(test)]\n        $census.mask_checks();\n        if $owner.tail_mask == 3 {\n            paired_plan_tails!(\n                $owner,\n                $left_path,\n                $right_path,\n                $left_tail,\n                $right_tail,\n                $census,\n                $function,\n                $budget,\n                $phase\n            );\n        }\n", "    ($owner:ident, $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n"),
    (10828, "            paid!(\n                $budget,\n                $function,\n                lexical_cursor::TAIL + 3 * size_of::<u8>(),\n                $phase\n            )?;\n            #[cfg(test)]\n            $census.tail_marks();\n            remember_plan_tail!($owner, $cursor);\n", "            paid!($budget, $function, lexical_cursor::TAIL, $phase)?;\n"),
    (489, "// Paired-tail selection adds one payment site; its bulk loop adds one\n// payload payment and three work sites, each with a complete primitive frame.\npub(super) const PRIMITIVE_FRAMES: usize = 46 * PAYMENT_HEADER;\n", "pub(super) const PRIMITIVE_FRAMES: usize = 41 * PAYMENT_HEADER;\n"),
];
pub(super) fn previous_census(current: &str) -> String {
    let stage = super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs",
        "e0ff7f5bd4f8a4fef1cc91114440e0819fde58098f5c64e1ba3bf28546537bbc",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs"
        )),
    );
    let current = stage.as_str();
    previous_census_before_fused(&fused_readers::previous_census(current))
}
fn previous_census_before_fused(current: &str) -> String {
    restore(current, 18711, CENSUS)
}
#[rustfmt::skip]
const CENSUS: &[(usize, &str, &str)] = &[
    (17587, "    #[test]\n    fn paired_tail_counter_overflow_is_incomplete_and_does_not_spend_work() {\n        for counter in 0..3 {\n            let function = fixture();\n            let budget = ReplayBudget::unobserved();\n            let remaining = budget.remaining;\n            reset();\n            let guard = begin(&function, &budget, false);\n            TRACE.with(|trace| {\n                let mut trace = trace.borrow_mut();\n                match counter {\n                    0 => trace.rows[0].mask_checks = usize::MAX,\n                    1 => trace.rows[0].tail_marks = usize::MAX,\n                    _ => trace.rows[0].fused_turns = usize::MAX,\n                }\n            });\n            match counter {\n                0 => guard.mask_checks(),\n                1 => guard.tail_marks(),\n                _ => guard.fused_turns(),\n            }\n            guard.success();\n            drop(guard);\n            let (rows, lost) = finish();\n            assert_eq!(lost, 0);\n            assert_eq!(budget.remaining, remaining);\n            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);\n        }\n    }\n", ""),
    (5562, "            None => r.overflow = true,\n        });\n    }\n    pub(super) fn mask_checks(&self) {\n        self.update(|r| match r.mask_checks.checked_add(1) {\n            Some(n) => r.mask_checks = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn tail_marks(&self) {\n        self.update(|r| match r.tail_marks.checked_add(1) {\n            Some(n) => r.tail_marks = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn fused_turns(&self) {\n        self.update(|r| match r.fused_turns.checked_add(1) {\n            Some(n) => r.fused_turns = n,\n", ""),
    (1267, "    mask_checks: 0,\n    tail_marks: 0,\n    fused_turns: 0,\n", ""),
    (509, "    pub mask_checks: usize,\n    pub tail_marks: usize,\n    pub fused_turns: usize,\n", ""),
];
pub(super) fn previous_pair_tests(current: &str) -> String {
    let stage = super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs",
        "b46e7bfec30cd08b716e6070975651a38d235b0a8ad3ffd851d7534fc0b94d52",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs"
        )),
    );
    let current = stage.as_str();
    previous_pair_tests_before_fused(&fused_readers::previous_pair_tests(current))
}
fn previous_pair_tests_before_fused(current: &str) -> String {
    restore(current, 50721, PAIR_TESTS)
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (34579, "        paid + reachable_primitive_prices(0).0 + shared_step_prices(0, &row).0,\n", "        paid + reachable_primitive_prices(0).0,\n"),
    (7294, "        (pair + 2 * step) * row.shared_steps + boundary_removed + tail_removed,\n        retained + boundary_retained + tail_retained,\n", "        (pair + 2 * step) * row.shared_steps + boundary_removed,\n        retained + boundary_retained,\n"),
    (7214, "    let (tail_removed, tail_retained) = paired_tails::prices(paths, row);\n", ""),
    (86, "#[path = \"pair_scope_tests/paired_tails.rs\"]\nmod paired_tails;\n", ""),
];
pub(super) fn previous_route(current: &str) -> String {
    let stage = super::incoming_stage(
        current,
        "src/bounded_output/stage_debits/route_debits.rs",
        "250e351274cf1e0f745a0b8326f24fa6addbab59a4923313229bb9f8394c8c2a",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/bounded_output/stage_debits/route_debits.rs"
        )),
    );
    let current = stage.as_str();
    previous_route_before_fused(&fused_readers::previous_route(current))
}
fn previous_route_before_fused(current: &str) -> String {
    restore(current, 35859, ROUTE)
}
#[rustfmt::skip]
const ROUTE: &[(usize, &str, &str)] = &[
    (34504, "    assert_eq!(lexical.matches(\"paired_plan_tails!(\").count(), 1);\n    assert_eq!(lexical.matches(\"remember_plan_tail!(\").count(), 1);\n", ""),
    (34292, "    assert_eq!(sites.len(), 17, \"audit every generic payment site\");\n", "    assert_eq!(sites.len(), 15, \"audit every generic payment site\");\n"),
    (33690, "    let shared = phases(owner, \"shared_plan_pair_step!(\", &[11]);\n", "    let shared = phases(owner, \"shared_plan_pair_step!(\", &[7]);\n"),
];
pub(super) fn previous_controls(current: &str) -> String {
    let stage = super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs",
        "0c85f1f992d5b831e601ea927e832e4f1df0cf5b0bcc27688eea9aa944a3b6f2",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 25298, CONTROLS)
}
#[rustfmt::skip]
const CONTROLS: &[(usize, &str, &str)] = &[
    (24584, "            &super::paired_tails::previous_runs(include_str!(\n                \"shared_steps/shared_boundaries/shared_runs.rs\"\n            )),\n            previous_stage3_tail_stage,\n", "            include_str!(\"shared_steps/shared_boundaries/shared_runs.rs\"),\n            previous_stage3,\n"),
    (22811, "            &super::paired_tails::previous_macro(include_str!(\"../../../../lexical_cursor.rs\")),\n            previous_macro_tail_stage,\n", "            include_str!(\"../../../../lexical_cursor.rs\"),\n            previous_macro,\n"),
    (22423, "            &super::paired_tails::previous_owner(include_str!(\n                \"../../../../../comparison_owner.rs\"\n            )),\n            previous_owner_tail_stage,\n", "            include_str!(\"../../../../../comparison_owner.rs\"),\n            previous_owner,\n"),
    (16181, "    previous_stage3_tail_stage(&paired_tails::previous_runs(current))\n}\nfn previous_stage3_tail_stage(current: &str) -> String {\n", ""),
    (12803, "mod paired_tails;\npub(super) fn previous_tail_census(current: &str) -> String {\n    paired_tails::previous_census(current)\n}\n", ""),
    (1633, "    previous_macro_tail_stage(&paired_tails::previous_macro(current))\n}\nfn previous_macro_tail_stage(current: &str) -> String {\n", ""),
    (755, "    previous_owner_tail_stage(&paired_tails::previous_owner(current))\n}\nfn previous_owner_tail_stage(current: &str) -> String {\n", ""),
];
pub(super) fn previous_constructor(current: &str) -> String {
    let stage = super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs",
        "9dc41df52f983bad1552fd556c0af3db2622a8e7eb1328356db230db906520ae",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/constructor.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 4999, CONSTRUCTOR)
}
#[rustfmt::skip]
const CONSTRUCTOR: &[(usize, &str, &str)] = &[
    (4220, "            &super::super::paired_tails::previous_pair_tests(include_str!(\n                \"../../../../../pair_scope_tests.rs\"\n            )),\n            previous_pair_tests_tail_stage,\n", "            include_str!(\"../../../../../pair_scope_tests.rs\"),\n            previous_pair_tests,\n"),
    (778, "    previous_pair_tests_tail_stage(&super::paired_tails::previous_pair_tests(current))\n}\nfn previous_pair_tests_tail_stage(current: &str) -> String {\n", ""),
];
pub(super) fn previous_json(current: &str) -> String {
    let stage = super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs",
        "4e01eab3ec8c582b781958702d3a82e044e959c93a49ba94e02c4be65ce8fa63",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/paired_links/json_ledger.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 6272, JSON)
}
#[rustfmt::skip]
const JSON: &[(usize, &str, &str)] = &[
    (5908, "            &super::super::paired_tails::previous_constructor(include_str!(\"constructor.rs\")),\n            previous_constructor_tail_stage,\n", "            include_str!(\"constructor.rs\"),\n            previous_constructor,\n"),
    (5543, "            &super::super::paired_tails::previous_controls(include_str!(\"../paired_links.rs\")),\n            previous_controls_tail_stage,\n", "            include_str!(\"../paired_links.rs\"),\n            previous_controls,\n"),
    (5108, "            &super::super::paired_tails::previous_route(include_str!(\n                \"../../../../../../../../../bounded_output/stage_debits/route_debits.rs\"\n            )),\n            previous_route_tail_stage,\n", "            include_str!(\"../../../../../../../../../bounded_output/stage_debits/route_debits.rs\"),\n            previous_route,\n"),
    (1955, "    previous_constructor_tail_stage(&super::paired_tails::previous_constructor(current))\n}\nfn previous_constructor_tail_stage(current: &str) -> String {\n", ""),
    (1111, "    previous_controls_tail_stage(&super::paired_tails::previous_controls(current))\n}\nfn previous_controls_tail_stage(current: &str) -> String {\n", ""),
    (777, "    previous_route_tail_stage(&super::paired_tails::previous_route(current))\n}\nfn previous_route_tail_stage(current: &str) -> String {\n", ""),
];
pub(super) fn previous_runs(current: &str) -> String {
    let stage = super::incoming_stage(
        current,
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs",
        "084e3c34169f471e5cc7a2f9b2b1593da9ce0b447a17178799877762f4c8e92d",
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps/shared_boundaries/shared_runs.rs"
        )),
    );
    let current = stage.as_str();
    restore(current, 19859, RUNS)
}
#[rustfmt::skip]
const RUNS: &[(usize, &str, &str)] = &[
    (18568, "            &super::super::super::super::paired_links::previous_tail_census(include_str!(\n                \"../../../../../../pair_scope_census.rs\"\n            )),\n            previous_census_tail_stage,\n", "            include_str!(\"../../../../../../pair_scope_census.rs\"),\n            previous_census,\n"),
    (9924, "    previous_census_tail_stage(&super::super::super::paired_links::previous_tail_census(\n        current,\n    ))\n}\nfn previous_census_tail_stage(current: &str) -> String {\n", ""),
];
#[cfg(test)]
mod tests {
    use super::*;
    fn hostile(
        current: &str,
        inverse: fn(&str) -> String,
        before: &str,
        edits: &[(usize, &str, &str)],
    ) {
        let stage = super::super::incoming_current_stage(current);
        let current = stage.as_str();
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
    fn paired_tail_sources_restore_every_complete_immediate_subject() {
        assert_eq!(
            previous_owner(include_str!("../../../../../../comparison_owner.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-owner.rs.txt"
            ))
        );
        assert_eq!(
            previous_macro(include_str!("../../../../../lexical_cursor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-macro.rs.txt"
            ))
        );
        assert_eq!(
            previous_census(include_str!("../../../../../pair_scope_census.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-census.rs.txt"
            ))
        );
        assert_eq!(
            previous_pair_tests(include_str!("../../../../../pair_scope_tests.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-pair_tests.rs.txt"
            ))
        );
        assert_eq!(
            previous_route(include_str!(
                "../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-route.rs.txt"
            ))
        );
        assert_eq!(
            previous_controls(include_str!("../paired_links.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-controls.rs.txt"
            ))
        );
        assert_eq!(
            previous_constructor(include_str!("constructor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-constructor.rs.txt"
            ))
        );
        assert_eq!(
            previous_json(include_str!("json_ledger.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-json.rs.txt"
            ))
        );
        assert_eq!(
            previous_runs(include_str!(
                "../shared_steps/shared_boundaries/shared_runs.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-runs.rs.txt"
            ))
        );
    }
    #[test]
    fn paired_tail_sources_refuse_span_extent_and_outside_drift() {
        hostile(
            &fused_readers::previous_owner(include_str!("../../../../../../comparison_owner.rs")),
            previous_owner_before_fused,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-owner.rs.txt"
            )),
            OWNER,
        );
        hostile(
            &fused_readers::previous_macro(include_str!("../../../../../lexical_cursor.rs")),
            previous_macro_before_fused,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-macro.rs.txt"
            )),
            MACRO,
        );
        hostile(
            &fused_readers::previous_census(include_str!("../../../../../pair_scope_census.rs")),
            previous_census_before_fused,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-census.rs.txt"
            )),
            CENSUS,
        );
        hostile(
            &fused_readers::previous_pair_tests(include_str!("../../../../../pair_scope_tests.rs")),
            previous_pair_tests_before_fused,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-pair_tests.rs.txt"
            )),
            PAIR_TESTS,
        );
        hostile(
            &fused_readers::previous_route(include_str!(
                "../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            previous_route_before_fused,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-route.rs.txt"
            )),
            ROUTE,
        );
        hostile(
            include_str!("../paired_links.rs"),
            previous_controls,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-controls.rs.txt"
            )),
            CONTROLS,
        );
        hostile(
            include_str!("constructor.rs"),
            previous_constructor,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-constructor.rs.txt"
            )),
            CONSTRUCTOR,
        );
        hostile(
            include_str!("json_ledger.rs"),
            previous_json,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-json.rs.txt"
            )),
            JSON,
        );
        hostile(
            include_str!("../shared_steps/shared_boundaries/shared_runs.rs"),
            previous_runs,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/paired-tails-001-20261018/before-runs.rs.txt"
            )),
            RUNS,
        );
    }
}
