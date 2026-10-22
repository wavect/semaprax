//! Exact complete successor inverse, preserving all prior independent guards.
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
    let stage = super::super::super::super::super::reachable_stage(
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs", "e3278dc29dcce665dd9d3a67ce048e552254a65c5a939f769b2b7c1eb86a9ded", current);
    let current = stage.as_str();
    restore(current, 10841, MACRO)
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (7171, "\n// The arena and its observations are immutable during comparison. A loaded\n// view is exactly the remembered validated segment starting at cursor.item.\n// Same segment and item therefore prove equality of the next observation.\nmacro_rules! remember_plan_segment {\n    ($owner:ident, left_cursor, $link:ident) => {\n        $owner.left_link = $link;\n    };\n    ($owner:ident, right_cursor, $link:ident) => {\n        $owner.right_link = $link;\n    };\n}\n\n// Four payment and five work sites have distinct complete primitive frames in\n// the multipath bank. Every actual selector, slice receiver and scalar copy\n// remains separately paid before formation. No observation Option/tuple/cmp\n// payload is formed on this branch. Original left-before-right work is exact.\nmacro_rules! shared_plan_pair_step {\n    ($owner:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n        paid!($budget, $function, 3 * size_of::<bool>(), $phase)?;\n        #[cfg(test)]\n        $census.shared_checks();\n        if $owner.left_loaded && $owner.right_loaded {\n            paid!(\n                $budget,\n                $function,\n                size_of::<&[Rc<SkeletonObservation>]>() + size_of::<bool>(),\n                $phase\n            )?;\n            #[cfg(test)]\n            $census.shared_patterns();\n            if let [_, left_rest @ ..] = $owner.left_items {\n                paid!(\n                    $budget,\n                    $function,\n                    4 * size_of::<usize>() + 3 * size_of::<bool>(),\n                    $phase\n                )?;\n                #[cfg(test)]\n                $census.shared_metadata();\n                if $owner.left_link == $owner.right_link\n                    && $owner.left_cursor.item == $owner.right_cursor.item\n                {\n                    paid!(\n                        $budget,\n                        $function,\n                        3 * size_of::<&[Rc<SkeletonObservation>]>()\n                            + 2 * size_of::<usize>()\n                            + size_of::<Ordering>()\n                            + size_of::<bool>(),\n                        $phase\n                    )?;\n                    #[cfg(test)]\n                    $census.shared_steps();\n                    let [_, right_rest @ ..] = $owner.right_items else {\n                        unreachable!(\"identical immutable ordinary plan segment and offset\");\n                    };\n                    $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n                    #[cfg(test)]\n                    $census.step();\n                    $budget.charge(\n                        $function,\n                        1,\n                        \"ordinary plan history forward segment traversal\",\n                    )?;\n                    $owner.left_items = left_rest;\n                    $owner.left_cursor.item += 1;\n                    $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n                    #[cfg(test)]\n                    $census.step();\n                    $budget.charge(\n                        $function,\n                        1,\n                        \"ordinary plan history forward segment traversal\",\n                    )?;\n                    $owner.right_items = right_rest;\n                    $owner.right_cursor.item += 1;\n                    $owner.ordering = Ordering::Equal;\n                    $budget.charge($function, 1, \"ordinary plan history observation comparison\")?;\n                    #[cfg(test)]\n                    $census.round();\n                    continue;\n                }\n            }\n        }\n    };\n}\n", ""),
    (6040, "                    remember_plan_segment!($owner, $cursor, link);\n", ""),
    (2742, "    + size_of::<bool>() // loaded=true\n    + size_of::<usize>(); // remember exact validated segment identity\n                          // The original exhausted-segment update remains above. Each tail scope forms\n                          // one immutable Vec slice; each yielded head/rest, assignment, as_ref output,\n                          // Option transport and original cursor item increment is still paid below.\n", "    + size_of::<bool>(); // loaded=true\n                         // The original exhausted-segment update remains above. Each tail scope forms\n                         // one immutable Vec slice; each yielded head/rest, assignment, as_ref output,\n                         // Option transport and original cursor item increment is still paid below.\n"),
    (489, "pub(super) const PRIMITIVE_FRAMES: usize = 29 * PAYMENT_HEADER;\n", "pub(super) const PRIMITIVE_FRAMES: usize = 20 * PAYMENT_HEADER;\n"),
    (339, "// The shared immutable-step branch adds four payment and five work sites.\n", ""),
];
pub(super) fn previous_owner(current: &str) -> String {
    let stage = super::super::super::super::super::reachable_stage(
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs", "a9b5037a2522c8e1bd7d24363894176aef29983900d48028b30fa86e6dabe5c8", current);
    let current = stage.as_str();
    restore(current, 23539, OWNER)
}
#[rustfmt::skip]
const OWNER: &[(usize, &str, &str)] = &[
    (14766, "                        shared_plan_pair_step!(\n                            owner,\n                            census,\n                            function,\n                            budget,\n                            \"ordinary plan history paired cursor next\"\n                        );\n", ""),
    (9017, "        left_link: 0,\n        right_link: 0,\n", ""),
    (7551, "    + 2 * size_of::<usize>() // initial remembered segment identities\n", ""),
    (640, "    left_link: usize,\n    right_link: usize,\n", ""),
];
pub(super) fn previous_pair_tests(current: &str) -> String {
    let stage = super::super::super::super::super::reachable_stage(
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs", "10ecb901fd3ce0805b1fbc81f68ad221d9a4fc3ddf3f707ff05fc38fff5b13e5", current);
    let current = stage.as_str();
    restore(current, 42740, PAIR_TESTS)
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (42333, "                        + shared_retained\n", ""),
    (41957, "                        + all_primitives\n                        + shared_removed,\n", "                        + all_primitives,\n"),
    (41567, "                let (shared_removed, shared_retained) = shared_step_prices(1, &rows[0]);\n", ""),
    (31914, "            + reachable_primitive_prices(0).1\n            + shared_step_prices(0, &row).1,\n", "            + reachable_primitive_prices(0).1,\n"),
    (29319, "        + reachable_primitives\n        + shared_retained;\n", "        + reachable_primitives;\n"),
    (29013, "        + all_primitives\n        + shared_removed;\n", "        + all_primitives;\n"),
    (28750, "    let (shared_removed, shared_retained) = shared_step_prices(expected.len(), &row);\n", ""),
    (11884, "                + shared_retained\n", ""),
    (11459, "                + all_primitives\n                + shared_removed,\n", "                + all_primitives,\n"),
    (11139, "        let (shared_removed, shared_retained) = shared_step_prices(expected.len(), &row);\n", ""),
    (2956, "}\n// Independent source-type invoice for the actual shared-step cells. The old\n// complete883 invoice stays additive; no measured debit delta is a coefficient.\nfn shared_step_prices(paths: usize, row: &pair_scope_census::Row) -> (usize, usize) {\n    let header = 2 * size_of::<&ResolvedFunction>()\n        + 2 * size_of::<&mut ReplayBudget>()\n        + 2 * size_of::<&str>()\n        + 2 * size_of::<usize>()\n        + 2 * size_of::<Result<(), Diagnostic>>();\n    let pair = size_of::<(Option<&SkeletonObservation>, Option<&SkeletonObservation>)>()\n        + 6 * size_of::<&SkeletonObservation>()\n        + 4 * size_of::<&SkeletonTerminal>()\n        + 2 * size_of::<Ordering>();\n    let step = 2 * size_of::<usize>()\n        + 3 * size_of::<bool>()\n        + 3 * size_of::<&Rc<SkeletonObservation>>()\n        + 2 * size_of::<&[Rc<SkeletonObservation>]>()\n        + size_of::<&SkeletonObservation>()\n        + 2 * size_of::<Option<&SkeletonObservation>>()\n        + size_of::<usize>()\n        + 2 * size_of::<usize>()\n        + size_of::<bool>();\n    let retained = 4 * size_of::<usize>()\n        + if paths >= 2 { 9 * header } else { 0 }\n        + size_of::<usize>() * row.loads\n        + 3 * size_of::<bool>() * row.shared_checks\n        + (size_of::<&[Rc<SkeletonObservation>]>() + size_of::<bool>()) * row.shared_patterns\n        + (4 * size_of::<usize>() + 3 * size_of::<bool>()) * row.shared_metadata\n        + (3 * size_of::<&[Rc<SkeletonObservation>]>()\n            + 2 * size_of::<usize>()\n            + size_of::<Ordering>()\n            + size_of::<bool>())\n            * row.shared_steps;\n    ((pair + 2 * step) * row.shared_steps, retained)\n", ""),
];
pub(super) fn previous_census(current: &str) -> String {
    let stage = super::super::super::super::super::reachable_stage(
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs", "ced9cc53bd363653bcc182b443bcbc41b2be69c005d441e3c4a3599f20e90ef5", current);
    let current = stage.as_str();
    restore(current, 12959, CENSUS)
}
#[rustfmt::skip]
const CENSUS: &[(usize, &str, &str)] = &[
    (11704, "    #[test]\n    fn shared_step_counter_overflow_is_incomplete_and_does_not_spend_work() {\n        for counter in 0..4 {\n            let function = fixture();\n            let budget = ReplayBudget::unobserved();\n            let remaining = budget.remaining;\n            reset();\n            let guard = begin(&function, &budget, false);\n            TRACE.with(|trace| {\n                let mut trace = trace.borrow_mut();\n                match counter {\n                    0 => trace.rows[0].shared_checks = usize::MAX,\n                    1 => trace.rows[0].shared_patterns = usize::MAX,\n                    2 => trace.rows[0].shared_metadata = usize::MAX,\n                    _ => trace.rows[0].shared_steps = usize::MAX,\n                }\n            });\n            match counter {\n                0 => guard.shared_checks(),\n                1 => guard.shared_patterns(),\n                2 => guard.shared_metadata(),\n                _ => guard.shared_steps(),\n            }\n            guard.success();\n            drop(guard);\n            let (rows, lost) = finish();\n            assert_eq!(lost, 0);\n            assert_eq!(budget.remaining, remaining);\n            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);\n        }\n    }\n", ""),
    (5094, "            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_checks(&self) {\n        self.update(|r| match r.shared_checks.checked_add(1) {\n            Some(n) => r.shared_checks = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_patterns(&self) {\n        self.update(|r| match r.shared_patterns.checked_add(1) {\n            Some(n) => r.shared_patterns = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_metadata(&self) {\n        self.update(|r| match r.shared_metadata.checked_add(1) {\n            Some(n) => r.shared_metadata = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn shared_steps(&self) {\n        self.update(|r| match r.shared_steps.checked_add(1) {\n            Some(n) => r.shared_steps = n,\n", ""),
    (975, "    shared_checks: 0,\n    shared_patterns: 0,\n    shared_metadata: 0,\n    shared_steps: 0,\n", ""),
    (479, "    pub shared_checks: usize,\n    pub shared_patterns: usize,\n    pub shared_metadata: usize,\n    pub shared_steps: usize,\n", ""),
];
pub(super) fn previous_reachable(current: &str) -> String {
    let stage = super::super::super::super::super::reachable_stage(
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs", "fc15bb2a80a9821f3616075f730b8e1dc9e958427c7f42a55e12d7d9d894c921", current);
    let current = stage.as_str();
    restore(current, 11132, REACHABLE)
}
#[rustfmt::skip]
const REACHABLE: &[(usize, &str, &str)] = &[
    (10426, "            &shared_steps::previous_pair_tests(include_str!(\"../../../pair_scope_tests.rs\")),\n            previous_pair_tests_stage,\n", "            include_str!(\"../../../pair_scope_tests.rs\"),\n            previous_pair_tests,\n"),
    (10071, "            &shared_steps::previous_owner(include_str!(\"../../../../comparison_owner.rs\")),\n            previous_owner_stage,\n", "            include_str!(\"../../../../comparison_owner.rs\"),\n            previous_owner,\n"),
    (9721, "            &shared_steps::previous_macro(include_str!(\"../../../lexical_cursor.rs\")),\n            previous_macro_stage,\n", "            include_str!(\"../../../lexical_cursor.rs\"),\n            previous_macro,\n"),
    (3112, "    previous_pair_tests_stage(&shared_steps::previous_pair_tests(current))\n}\nfn previous_pair_tests_stage(current: &str) -> String {\n", ""),
    (1988, "    previous_owner_stage(&shared_steps::previous_owner(current))\n}\nfn previous_owner_stage(current: &str) -> String {\n", ""),
    (973, "    previous_macro_stage(&shared_steps::previous_macro(current))\n}\nfn previous_macro_stage(current: &str) -> String {\n", ""),
    (899, "mod shared_steps;\n", ""),
];
#[cfg(test)]
mod tests {
    use super::*;
    fn hostile(
        current: &str,
        inverse: fn(&str) -> String,
        before: &str,
        edits: &[(usize, &str, &str)],
        stage: (&str, &str, usize),
    ) {
        // Public current-first acquisition must still reject physical drift.
        assert_eq!(inverse(current), before);
        assert!(std::panic::catch_unwind(|| inverse(&format!("{current} "))).is_err());
        let acquired = super::super::super::super::super::super::reachable_stage(
            stage.0, stage.1, current);
        assert_eq!(acquired.len(), stage.2);
        let current = acquired.as_str();
        // Archived inverse offsets apply to this authenticated complete stage,
        // not to the later physical source which legitimately has another size.
        let inverse = |current: &str| restore(current, stage.2, edits);
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
    fn complete_sources_and_all_old_controls_restore_exact_before_bytes() {
        assert_eq!(
            previous_macro(include_str!("../../../../lexical_cursor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-macro.rs.txt"
            ))
        );
        assert_eq!(
            previous_owner(include_str!("../../../../../comparison_owner.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-owner.rs.txt"
            ))
        );
        assert_eq!(
            previous_pair_tests(include_str!("../../../../pair_scope_tests.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-pair-tests.rs.txt"
            ))
        );
        assert_eq!(
            previous_census(include_str!("../../../../pair_scope_census.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-census.rs.txt"
            ))
        );
        assert_eq!(
            previous_reachable(include_str!("../reachable_primitives.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-reachable.rs.txt"
            ))
        );
    }
    #[test]
    fn all_new_source_subjects_refuse_span_extent_and_outside_drift() {
        hostile(
            include_str!("../../../../lexical_cursor.rs"),
            previous_macro,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-macro.rs.txt"
            )),
            MACRO,
            ("src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs", "e3278dc29dcce665dd9d3a67ce048e552254a65c5a939f769b2b7c1eb86a9ded", 10841),
        );
        hostile(
            include_str!("../../../../../comparison_owner.rs"),
            previous_owner,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-owner.rs.txt"
            )),
            OWNER,
            ("src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs", "a9b5037a2522c8e1bd7d24363894176aef29983900d48028b30fa86e6dabe5c8", 23539),
        );
        hostile(
            include_str!("../../../../pair_scope_tests.rs"),
            previous_pair_tests,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-pair-tests.rs.txt"
            )),
            PAIR_TESTS,
            ("src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs", "10ecb901fd3ce0805b1fbc81f68ad221d9a4fc3ddf3f707ff05fc38fff5b13e5", 42740),
        );
        hostile(
            include_str!("../../../../pair_scope_census.rs"),
            previous_census,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-census.rs.txt"
            )),
            CENSUS,
            ("src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs", "ced9cc53bd363653bcc182b443bcbc41b2be69c005d441e3c4a3599f20e90ef5", 12959),
        );
        hostile(
            include_str!("../reachable_primitives.rs"),
            previous_reachable,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/shared-pair-c79-20261018/before-reachable.rs.txt"
            )),
            REACHABLE,
            ("src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives.rs", "fc15bb2a80a9821f3616075f730b8e1dc9e958427c7f42a55e12d7d9d894c921", 11132),
        );
    }
}
