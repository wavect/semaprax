//! Complete independent-segment source inverses, followed by every earlier audit.
fn restore(current: &str, before: &str, length: usize, edits: &[(usize, &str, &str)]) -> String {
    assert_eq!(
        current.len(),
        length,
        "unknown independent-segment subject extent"
    );
    let mut result = current.to_owned();
    let mut last = length;
    for &(at, after, previous) in edits {
        let end = at
            .checked_add(after.len())
            .expect("independent-segment inverse overflow");
        assert!(end <= last && result.is_char_boundary(at) && result.is_char_boundary(end));
        assert_eq!(
            &result[at..end],
            after,
            "unknown independent-segment source span"
        );
        result.replace_range(at..end, previous);
        last = at;
    }
    assert_eq!(
        result, before,
        "whole predecessor must match, including outside spans"
    );
    result
}
pub(super) fn previous_owner(current: &str) -> String {
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/paired-segments-5d46-20261019/before-owner.rs.txt"
        )),
        22498,
        OWNER,
    )
}
#[rustfmt::skip]
const OWNER: &[(usize, &str, &str)] = &[
    (15715, "                            'paired_observations,\n", ""),
    (243, "#[cfg(test)]\npub(super) mod paired_segments_before;\n", ""),
];
pub(super) fn previous_macro(current: &str) -> String {
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/paired-segments-5d46-20261019/before-macro.rs.txt"
        )),
        33725,
        MACRO,
    )
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (13570, "        }\n    };\n}\n\n// Compare already-loaded immutable segments without rebuilding the normal\n// cursor transport or sharing decisions for each interior observation. Every\n// original semantic work request remains in its original order.\npub(super) const INDEPENDENT_RUN_SETUP: usize =\n    RUN_SETUP + 2 * size_of::<&[Rc<SkeletonObservation>]>() + size_of::<[usize; 8]>();\npub(super) const INDEPENDENT_RUN_PAYLOAD: usize = 2 * size_of::<&Rc<SkeletonObservation>>()\n    + 4 * size_of::<&[Rc<SkeletonObservation>]>()\n    + 4 * size_of::<&Rc<SkeletonObservation>>()\n    + 2 * size_of::<&SkeletonObservation>()\n    + 4 * size_of::<&SkeletonObservation>()\n    + 2 * size_of::<usize>()\n    + 4 * size_of::<Ordering>()\n    + 3 * size_of::<bool>();\nmacro_rules! independent_plan_segment_run {\n    ($owner:ident, $census:ident, $function:ident, $budget:ident, $pair_loop:lifetime, $phase:literal) => {\n        paid!($budget, $function, 3 * size_of::<bool>(), $phase)?;\n        #[cfg(test)]\n        $census.independent_checks();\n        if matches!($owner.left_items, [_, _, _, _, ..])\n            && matches!($owner.right_items, [_, _, _, _, ..])\n        {\n            paid!(\n                $budget,\n                $function,\n                lexical_cursor::INDEPENDENT_RUN_SETUP,\n                $phase\n            )?;\n            #[cfg(test)]\n            $census.independent_runs();\n            let paired_length = $owner.left_items.len().min($owner.right_items.len());\n            let paired_work_driver: lexical_cursor::SharedUnitDriver =\n                std::iter::repeat(()).take(paired_length);\n            for () in paired_work_driver {\n                paid!(\n                    $budget,\n                    $function,\n                    lexical_cursor::INDEPENDENT_RUN_PAYLOAD,\n                    $phase\n                )?;\n                #[cfg(test)]\n                $census.independent_turns();\n                $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n                #[cfg(test)]\n                $census.step();\n                $budget.charge(\n                    $function,\n                    1,\n                    \"ordinary plan history forward segment traversal\",\n                )?;\n                let [left, left_rest @ ..] = $owner.left_items else {\n                    unreachable!(\"immutable prepared left segment length\")\n                };\n                $owner.left_items = left_rest;\n                $owner.left_cursor.item += 1;\n                $budget.charge($function, 1, \"ordinary plan history forward cursor step\")?;\n                #[cfg(test)]\n                $census.step();\n                $budget.charge(\n                    $function,\n                    1,\n                    \"ordinary plan history forward segment traversal\",\n                )?;\n                let [right, right_rest @ ..] = $owner.right_items else {\n                    unreachable!(\"immutable prepared right segment length\")\n                };\n                $owner.right_items = right_rest;\n                $owner.right_cursor.item += 1;\n                $owner.ordering = left.as_ref().cmp(right.as_ref());\n                if $owner.ordering != Ordering::Equal {\n                    break $pair_loop;\n                }\n                $budget.charge($function, 1, \"ordinary plan history observation comparison\")?;\n                #[cfg(test)]\n                $census.round();\n            }\n            continue;\n", ""),
    (13124, "                independent_plan_segment_run!(\n                    $owner, $census, $function, $budget, $pair_loop, $phase\n                );\n", ""),
    (11569, "    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident, $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident, $pair_loop:lifetime, $phase:literal) => {\n", "    ($owner:ident, $left_path:ident, $right_path:ident, $left_tail:ident, $right_tail:ident, $left_links:ident, $right_links:ident, $census:ident, $function:ident, $budget:ident, $phase:literal) => {\n"),
    (890, "pub(super) const PRIMITIVE_FRAMES: usize = 59 * PAYMENT_HEADER;\n", "pub(super) const PRIMITIVE_FRAMES: usize = 51 * PAYMENT_HEADER;\n"),
];
pub(super) fn previous_census(current: &str) -> String {
    let mut current = current.to_owned();
    assert_eq!(current.matches("rows: Vec<Row>").count(), 1);
    current = current.replacen("rows: Vec<Row>", "rows: [Row; 128]", 1);
    assert_eq!(current.matches("rows: Vec::new()").count(), 1);
    current = current.replacen("rows: Vec::new()", "rows: [EMPTY; 128]", 1);
    assert_eq!(
        current
            .matches("        t.rows.resize(128, EMPTY);\n")
            .count(),
        1
    );
    current = current.replacen("        t.rows.resize(128, EMPTY);\n", "", 1);
    assert_eq!(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/tls-traces-cf72-20261019/before-census.rs.txt"
        )),
        "whole pre-TLS census, including all observation counters"
    );
    restore(
        &current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/paired-segments-5d46-20261019/before-census.rs.txt"
        )),
        22263,
        CENSUS,
    )
}
#[rustfmt::skip]
const CENSUS: &[(usize, &str, &str)] = &[
    (9355, "            None => r.overflow = true,\n        });\n    }\n    pub(super) fn independent_checks(&self) {\n        self.update(|r| match r.independent_checks.checked_add(1) {\n            Some(n) => r.independent_checks = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn independent_runs(&self) {\n        self.update(|r| match r.independent_runs.checked_add(1) {\n            Some(n) => r.independent_runs = n,\n            None => r.overflow = true,\n        });\n    }\n    pub(super) fn independent_turns(&self) {\n        self.update(|r| match r.independent_turns.checked_add(1) {\n            Some(n) => r.independent_turns = n,\n", ""),
    (1877, "    independent_checks: 0,\n    independent_runs: 0,\n    independent_turns: 0,\n", ""),
    (976, "    pub independent_checks: usize,\n    pub independent_runs: usize,\n    pub independent_turns: usize,\n", ""),
];
pub(super) fn previous_pair_tests(current: &str) -> String {
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/paired-segments-5d46-20261019/before-pair_tests.rs.txt"
        )),
        55874,
        PAIR_TESTS,
    )
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (5539, "    // The original equation quotes the preceding physical receiver counts.\n    // Actual diagnostic rows stay untouched; this copy is only an invoice\n    // operand, checked against the complete compiled immediate predecessor.\n    let previous = paired_segments::predecessor_price_counts(row);\n    let (removed, retained) = shared_step_prices_before(paths, &previous);\n    let (next_removed, next_retained) = paired_segments::prices(paths, row);\n    (removed + next_removed, retained + next_retained)\n}\nfn shared_step_prices_before(paths: usize, row: &pair_scope_census::Row) -> (usize, usize) {\n", ""),
    (86, "#[path = \"pair_scope_tests/paired_segments.rs\"]\nmod paired_segments;\n", ""),
];
pub(super) fn previous_route(current: &str) -> String {
    restore(
        current,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/paired-segments-5d46-20261019/before-route.rs.txt"
        )),
        36241,
        ROUTE,
    )
}
#[rustfmt::skip]
const ROUTE: &[(usize, &str, &str)] = &[
 (35456, "    assert_eq!(compact.matches(\"independent_plan_segment_run!($owner,$census,$function,$budget,$pair_loop,$phase);\").count(), 1);\n", ""),
 (34577, "    assert_eq!(lexical.matches(\"independent_plan_segment_run!(\").count(), 1);\n", ""),
 (34365, "    assert_eq!(sites.len(), 27, \"audit every generic payment site\");\n", "    assert_eq!(sites.len(), 24, \"audit every generic payment site\");\n"),
 (33693, "    let shared = phases(owner, \"shared_plan_pair_step!(\", &[12]);\n", "    let shared = phases(owner, \"shared_plan_pair_step!(\", &[11]);\n"),
];
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn whole_independent_segment_subjects_restore_and_refuse_source_drift() {
        {
            let current = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs"
            ));
            assert_eq!(
                previous_owner(current),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/paired-segments-5d46-20261019/before-owner.rs.txt"
                ))
            );
            for &(at, _, _) in OWNER {
                if at < current.len() {
                    let mut changed = current.to_owned();
                    let end = at + current[at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_owner(&changed)).is_err());
                }
            }
            assert!(std::panic::catch_unwind(|| previous_owner(&format!("{current} "))).is_err());
            let changed = current.replacen("//!", "//?", 1);
            assert_ne!(changed, current);
            assert!(std::panic::catch_unwind(|| previous_owner(&changed)).is_err());
        }
        {
            let current = include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs"
            ));
            assert_eq!(
                previous_macro(current),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/paired-segments-5d46-20261019/before-macro.rs.txt"
                ))
            );
            for &(at, _, _) in MACRO {
                if at < current.len() {
                    let mut changed = current.to_owned();
                    let end = at + current[at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_macro(&changed)).is_err());
                }
            }
            assert!(std::panic::catch_unwind(|| previous_macro(&format!("{current} "))).is_err());
            let changed = current.replacen("//!", "//?", 1);
            assert_ne!(changed, current);
            assert!(std::panic::catch_unwind(|| previous_macro(&changed)).is_err());
        }
        {
            let current = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs"));
            assert_eq!(
                previous_census(current),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/paired-segments-5d46-20261019/before-census.rs.txt"
                ))
            );
            for &(at, _, _) in CENSUS {
                if at < current.len() {
                    let mut changed = current.to_owned();
                    let end = at + current[at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_census(&changed)).is_err());
                }
            }
            assert!(std::panic::catch_unwind(|| previous_census(&format!("{current} "))).is_err());
            let changed = current.replacen("//!", "//?", 1);
            assert_ne!(changed, current);
            assert!(std::panic::catch_unwind(|| previous_census(&changed)).is_err());
        }
        {
            let current = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs"));
            assert_eq!(
                previous_pair_tests(current),
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/docs/evidence/paired-segments-5d46-20261019/before-pair_tests.rs.txt"
                ))
            );
            for &(at, _, _) in PAIR_TESTS {
                if at < current.len() {
                    let mut changed = current.to_owned();
                    let end = at + current[at..].chars().next().unwrap().len_utf8();
                    let replacement = if &changed[at..end] == "x" { "y" } else { "x" };
                    changed.replace_range(at..end, replacement);
                    assert!(std::panic::catch_unwind(|| previous_pair_tests(&changed)).is_err());
                }
            }
            assert!(
                std::panic::catch_unwind(|| previous_pair_tests(&format!("{current} "))).is_err()
            );
            let changed = current.replacen("//!", "//?", 1);
            assert_ne!(changed, current);
            assert!(std::panic::catch_unwind(|| previous_pair_tests(&changed)).is_err());
        }
    }
}
