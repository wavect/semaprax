//! Exact whole-source boundary successor; every prior independent guard remains.
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
#[path = "shared_boundaries/shared_runs.rs"]
mod shared_runs;
pub(super) fn previous_macro(current: &str) -> String {
    previous_macro_stage(&shared_runs::previous_macro(current))
}
fn previous_macro_stage(current: &str) -> String {
    restore(current, 17308, MACRO)
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (10946, "\n// A loaded empty slice and a remaining validated link describe exactly one\n// exhausted-to-next-segment transition. Inspect only that next immutable\n// segment; empty or divergent successors use the complete original traversal.\n// Four payment and seven work sites have distinct full multipath headers.\nmacro_rules! shared_plan_boundary {\n    ($owner:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n        paid!($budget, $function, size_of::<bool>(), $phase)?;\n        #[cfg(test)]\n        $census.shared_empty_checks();\n        if let [] = $owner.right_items {\n            paid!(\n                $budget,\n                $function,\n                4 * size_of::<&Vec<usize>>()\n                    + 4 * size_of::<&[usize]>()\n                    + 4 * size_of::<usize>()\n                    + 3 * size_of::<bool>(),\n                $phase\n            )?;\n            #[cfg(test)]\n            $census.shared_links();\n            let left_links = $owner.left_cursor.links.as_slice();\n            let right_links = $owner.right_cursor.links.as_slice();\n            if let &[.., left_next] = left_links {\n                if let &[.., right_next] = right_links {\n                    if left_next == right_next {\n                        paid!(\n                            $budget,\n                            $function,\n                            2 * size_of::<&[Segment]>()\n                                + 2 * size_of::<usize>()\n                                + size_of::<&Segment>()\n                                + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()\n                                + 3 * size_of::<&[Rc<SkeletonObservation>]>()\n                                + size_of::<bool>(),\n                            $phase\n                        )?;\n                        #[cfg(test)]\n                        $census.shared_queries();\n                        let next_items = $owner.arena[left_next].items.as_slice();\n                        if let [_, rest @ ..] = next_items {\n                            paid!(\n                                $budget,\n                                $function,\n                                4 * size_of::<&mut Vec<usize>>()\n                                    + 2 * size_of::<Option<usize>>()\n                                    + 10 * size_of::<usize>()\n                                    + 6 * size_of::<bool>()\n                                    + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n                                    + size_of::<Ordering>(),\n                                $phase\n                            )?;\n                            #[cfg(test)]\n                            $census.shared_boundaries();\n                            $budget.charge(\n                                $function,\n                                1,\n                                \"ordinary plan history forward cursor step\",\n                            )?;\n                            #[cfg(test)]\n                            $census.step();\n                            $budget.charge(\n                                $function,\n                                1,\n                                \"ordinary plan history forward segment traversal\",\n                            )?;\n                            $owner.left_cursor.segment += 1;\n                            $owner.left_cursor.item = 0;\n                            $owner.left_loaded = false;\n                            #[cfg(test)]\n                            $census.step();\n                            $budget.charge(\n                                $function,\n                                1,\n                                \"ordinary plan history forward segment traversal\",\n                            )?;\n                            #[cfg(test)]\n                            $census.load();\n                            let Some(left_link) = $owner.left_cursor.links.pop() else {\n                                unreachable!(\"validated ordinary plan next segment\");\n                            };\n                            $owner.left_link = left_link;\n                            $owner.left_items = rest;\n                            $owner.left_loaded = true;\n                            $owner.left_cursor.item = 1;\n                            $budget.charge(\n                                $function,\n                                1,\n                                \"ordinary plan history forward cursor step\",\n                            )?;\n                            #[cfg(test)]\n                            $census.step();\n                            $budget.charge(\n                                $function,\n                                1,\n                                \"ordinary plan history forward segment traversal\",\n                            )?;\n                            $owner.right_cursor.segment += 1;\n                            $owner.right_cursor.item = 0;\n                            $owner.right_loaded = false;\n                            #[cfg(test)]\n                            $census.step();\n                            $budget.charge(\n                                $function,\n                                1,\n                                \"ordinary plan history forward segment traversal\",\n                            )?;\n                            #[cfg(test)]\n                            $census.load();\n                            let Some(right_link) = $owner.right_cursor.links.pop() else {\n                                unreachable!(\"validated ordinary plan next segment\");\n                            };\n                            $owner.right_link = right_link;\n                            $owner.right_items = rest;\n                            $owner.right_loaded = true;\n                            $owner.right_cursor.item = 1;\n                            $owner.ordering = Ordering::Equal;\n                            $budget.charge(\n                                $function,\n                                1,\n                                \"ordinary plan history observation comparison\",\n                            )?;\n                            #[cfg(test)]\n                            $census.round();\n                            continue;\n                        }\n                    }\n                }\n            }\n        }\n    };\n}\n", ""),
    (10808, "            } else {\n                shared_plan_boundary!($owner, $census, $function, $budget, $phase);\n", ""),
    (489, "pub(super) const PRIMITIVE_FRAMES: usize = 40 * PAYMENT_HEADER;\n", "pub(super) const PRIMITIVE_FRAMES: usize = 29 * PAYMENT_HEADER;\n"),
];
pub(super) fn previous_pair_tests(current: &str) -> String {
    previous_pair_tests_stage(&shared_runs::previous_pair_tests(current))
}
fn previous_pair_tests_stage(current: &str) -> String {
    restore(current, 47414, PAIR_TESTS)
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (45161, "\n#[test]\nfn shared_boundary_one_item_and_empty_segments_preserve_real_883_status_and_work_cuts() {\n    let program = super::super::super::tests::program();\n    let function = program\n        .functions\n        .iter()\n        .find(|f| f.id.as_str() == \"f\")\n        .unwrap();\n    let item = Rc::new(SkeletonObservation::Boolean {\n        expression: function.body.id.clone(),\n        value: true,\n    });\n    let expected = vec![\n        SkeletonPath {\n            observations: vec![item.clone(); 3].into(),\n            terminal: SkeletonTerminal::Success\n        };\n        2\n    ];\n    let actual = || HistorySet {\n        arena: (0usize..4)\n            .map(|at| Segment {\n                previous: at.checked_sub(1),\n                depth: at + 1,\n                items: if at == 1 {\n                    Vec::new()\n                } else {\n                    vec![item.clone()]\n                },\n            })\n            .collect(),\n        paths: (0..2)\n            .map(|_| PlanPath {\n                history: History {\n                    previous: Some(3),\n                    tail: Vec::new(),\n                },\n                terminal: SkeletonTerminal::Success,\n            })\n            .collect(),\n        max_depth: 4,\n    };\n    pair_scope_census::reset();\n    let (old, old_heap, _) =\n        observe_shaped_pair(function, &expected, actual(), true, MAX_REPLAY_WORK_UNITS);\n    pair_scope_census::reset();\n    let (new, new_heap, _) =\n        observe_shaped_pair(function, &expected, actual(), false, MAX_REPLAY_WORK_UNITS);\n    let row = complete_row(false);\n    assert!(\n        row.shared_boundaries > 0,\n        \"exercise the actual boundary branch\"\n    );\n    assert_eq!(new, old);\n    assert!(new.result.is_ok());\n    assert_eq!(new_heap, old_heap);\n    let spent = MAX_REPLAY_WORK_UNITS - old.remaining;\n    for cut in 0..=spent {\n        pair_scope_census::reset();\n        let (old, _, _) = observe_shaped_pair(function, &expected, actual(), true, cut);\n        pair_scope_census::reset();\n        let (new, _, _) = observe_shaped_pair(function, &expected, actual(), false, cut);\n        assert_eq!(\n            new, old,\n            \"all six diagnostic fields and exact remaining work at cut {cut}\"\n        );\n    }\n}\n", ""),
    (6836, "    let (boundary_removed, boundary_retained) = boundary_prices(paths, row);\n    (\n        (pair + 2 * step) * row.shared_steps + boundary_removed,\n        retained + boundary_retained,\n    )\n", "    ((pair + 2 * step) * row.shared_steps, retained)\n"),
    (3116, "// Independent additive invoice for the boundary branch. Original invoices\n// and every older physical/work/diagnostic assertion remain complete.\nfn boundary_prices(paths: usize, row: &pair_scope_census::Row) -> (usize, usize) {\n    let header = 2 * size_of::<&ResolvedFunction>()\n        + 2 * size_of::<&mut ReplayBudget>()\n        + 2 * size_of::<&str>()\n        + 2 * size_of::<usize>()\n        + 2 * size_of::<Result<(), Diagnostic>>();\n    let pair = size_of::<(Option<&SkeletonObservation>, Option<&SkeletonObservation>)>()\n        + 6 * size_of::<&SkeletonObservation>()\n        + 4 * size_of::<&SkeletonTerminal>()\n        + 2 * size_of::<Ordering>();\n    let step = 2 * size_of::<usize>()\n        + 3 * size_of::<bool>()\n        + 3 * size_of::<&Rc<SkeletonObservation>>()\n        + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n        + size_of::<&SkeletonObservation>()\n        + 2 * size_of::<Option<&SkeletonObservation>>()\n        + 3 * size_of::<usize>()\n        + size_of::<bool>();\n    let load = 2 * size_of::<&mut Vec<usize>>()\n        + size_of::<Option<usize>>()\n        + 4 * size_of::<usize>()\n        + 2 * size_of::<bool>()\n        + 2 * size_of::<&[Segment]>()\n        + size_of::<&Segment>()\n        + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()\n        + 2 * size_of::<&[Rc<SkeletonObservation>]>();\n    let links = 4 * size_of::<&Vec<usize>>()\n        + 4 * size_of::<&[usize]>()\n        + 4 * size_of::<usize>()\n        + 3 * size_of::<bool>();\n    let query = 2 * size_of::<&[Segment]>()\n        + 2 * size_of::<usize>()\n        + size_of::<&Segment>()\n        + 2 * size_of::<&Vec<Rc<SkeletonObservation>>>()\n        + 3 * size_of::<&[Rc<SkeletonObservation>]>()\n        + size_of::<bool>();\n    let advance = 4 * size_of::<&mut Vec<usize>>()\n        + 2 * size_of::<Option<usize>>()\n        + 10 * size_of::<usize>()\n        + 6 * size_of::<bool>()\n        + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n        + size_of::<Ordering>();\n    (\n        (pair + 4 * step + 2 * load) * row.shared_boundaries,\n        if paths >= 2 { 11 * header } else { 0 }\n            + size_of::<bool>() * row.shared_empty_checks\n            + links * row.shared_links\n            + query * row.shared_queries\n            + advance * row.shared_boundaries,\n    )\n}\n", ""),
];
pub(super) fn previous_census(current: &str) -> String {
    previous_census_stage(&shared_runs::previous_census(current))
}
fn previous_census_stage(current: &str) -> String {
    restore(current, 15306, CENSUS)
}
#[rustfmt::skip]
const CENSUS: &[(usize, &str, &str)] = &[
    (14035, "    #[test]\n    fn shared_boundary_counter_overflow_is_incomplete_without_spending_work() {\n        for counter in 0..4 {\n            let function = fixture();\n            let budget = ReplayBudget::unobserved();\n            let remaining = budget.remaining;\n            reset();\n            let guard = begin(&function, &budget, false);\n            TRACE.with(|trace| {\n                let mut trace = trace.borrow_mut();\n                match counter {\n                    0 => trace.rows[0].shared_empty_checks = usize::MAX,\n                    1 => trace.rows[0].shared_links = usize::MAX,\n                    2 => trace.rows[0].shared_queries = usize::MAX,\n                    _ => trace.rows[0].shared_boundaries = usize::MAX,\n                }\n            });\n            match counter {\n                0 => guard.shared_empty_checks(),\n                1 => guard.shared_links(),\n                2 => guard.shared_queries(),\n                _ => guard.shared_boundaries(),\n            }\n            guard.success();\n            drop(guard);\n            let (rows, lost) = finish();\n            assert_eq!(lost, 0);\n            assert_eq!(budget.remaining, remaining);\n            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);\n        }\n    }\n", ""),
    (6151, "            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_empty_checks(&self) {\n        self.update(|r| match r.shared_empty_checks.checked_add(1) {\n            Some(n) => r.shared_empty_checks = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_links(&self) {\n        self.update(|r| match r.shared_links.checked_add(1) {\n            Some(n) => r.shared_links = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_queries(&self) {\n        self.update(|r| match r.shared_queries.checked_add(1) {\n            Some(n) => r.shared_queries = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_boundaries(&self) {\n        self.update(|r| match r.shared_boundaries.checked_add(1) {\n            Some(n) => r.shared_boundaries = n,\n", ""),
    (1196, "    shared_empty_checks: 0,\n    shared_links: 0,\n    shared_queries: 0,\n    shared_boundaries: 0,\n", ""),
    (602, "    pub shared_empty_checks: usize,\n    pub shared_links: usize,\n    pub shared_queries: usize,\n    pub shared_boundaries: usize,\n", ""),
];
pub(super) fn previous_shared_steps(current: &str) -> String {
    let stage = if current
        == include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs"
        )) {
        crate::source_reflows_merge::main_e177_final_inputs::value_direct(
            "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/shared_steps.rs",
            "329c0b6c60e477128d26f30383223566cc0bca79e13063e97a7c79ef7fa03255",
            current,
        )
    } else {
        current.to_owned()
    };
    let stage = shared_runs::previous_shared_step_controls(&stage);
    let current = stage.as_str();
    restore(current, 18577, SHARED_STEPS)
}
#[rustfmt::skip]
const SHARED_STEPS: &[(usize, &str, &str)] = &[
    (17916, "            &shared_boundaries::previous_census(include_str!(\"../../../../pair_scope_census.rs\")),\n            previous_census_stage,\n", "            include_str!(\"../../../../pair_scope_census.rs\"),\n            previous_census,\n"),
    (17516, "            &shared_boundaries::previous_pair_tests(include_str!(\n                \"../../../../pair_scope_tests.rs\"\n            )),\n            previous_pair_tests_stage,\n", "            include_str!(\"../../../../pair_scope_tests.rs\"),\n            previous_pair_tests,\n"),
    (16857, "            &shared_boundaries::previous_macro(include_str!(\"../../../../lexical_cursor.rs\")),\n            previous_macro_stage,\n", "            include_str!(\"../../../../lexical_cursor.rs\"),\n            previous_macro,\n"),
    (9807, "    previous_census_stage(&shared_boundaries::previous_census(current))\n}\nfn previous_census_stage(current: &str) -> String {\n", ""),
    (6697, "    previous_pair_tests_stage(&shared_boundaries::previous_pair_tests(current))\n}\nfn previous_pair_tests_stage(current: &str) -> String {\n", ""),
    (797, "    previous_macro_stage(&shared_boundaries::previous_macro(current))\n}\nfn previous_macro_stage(current: &str) -> String {\n", ""),
    (718, "mod shared_boundaries;\n", ""),
];
pub(super) fn previous_route(current: &str) -> String {
    previous_route_stage(&shared_runs::previous_route(current))
}
fn previous_route_stage(current: &str) -> String {
    restore(current, 35044, ROUTE)
}
#[rustfmt::skip]
const ROUTE: &[(usize, &str, &str)] = &[
    (34032, "    assert_eq!(sites.len(), 11, \"audit every generic payment site\");\n    assert_eq!(lexical.matches(\"shared_plan_boundary!(\").count(), 1);\n    let compact: String = lexical.split_whitespace().collect();\n    assert_eq!(\n        compact\n            .matches(\"shared_plan_boundary!($owner,$census,$function,$budget,$phase);\")\n            .count(),\n        1\n    );\n", "    assert_eq!(sites.len(), 7, \"audit every generic payment site\");\n"),
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
    fn complete_boundary_sources_restore_all_whole_before_images() {
        assert_eq!(
            previous_macro(include_str!("../../../../../lexical_cursor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-macro.rs.txt"
            ))
        );
        assert_eq!(
            previous_pair_tests(include_str!("../../../../../pair_scope_tests.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-pair-tests.rs.txt"
            ))
        );
        assert_eq!(
            previous_census(include_str!("../../../../../pair_scope_census.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-census.rs.txt"
            ))
        );
        assert_eq!(
            previous_shared_steps(include_str!("../shared_steps.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-shared-steps.rs.txt"
            ))
        );
        assert_eq!(
            previous_route(include_str!(
                "../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-route.rs.txt"
            ))
        );
    }
    #[test]
    fn every_boundary_source_refuses_span_extent_and_outside_drift() {
        hostile(
            &shared_runs::previous_macro(include_str!("../../../../../lexical_cursor.rs")),
            previous_macro_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-macro.rs.txt"
            )),
            MACRO,
        );
        hostile(
            &shared_runs::previous_pair_tests(include_str!("../../../../../pair_scope_tests.rs")),
            previous_pair_tests_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-pair-tests.rs.txt"
            )),
            PAIR_TESTS,
        );
        hostile(
            &shared_runs::previous_census(include_str!("../../../../../pair_scope_census.rs")),
            previous_census_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-census.rs.txt"
            )),
            CENSUS,
        );
        hostile(
            include_str!("../shared_steps.rs"),
            previous_shared_steps,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-shared-steps.rs.txt"
            )),
            SHARED_STEPS,
        );
        hostile(
            &shared_runs::previous_route(include_str!(
                "../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            previous_route_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-boundaries-99b-20261018/before-route.rs.txt"
            )),
            ROUTE,
        );
    }
}
