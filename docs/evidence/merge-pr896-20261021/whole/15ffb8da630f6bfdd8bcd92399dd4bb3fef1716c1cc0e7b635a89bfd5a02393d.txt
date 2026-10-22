//! Exact whole-source inverse for funding only reachable primitive-site groups.
//! Dynamic payloads, work events and independent older controls are retained.
fn restore(current: &str, length: usize, edits: &[(usize, &str, &str)]) -> String {
    assert_eq!(
        current.len(),
        length,
        "unknown reachable-primitive subject extent"
    );
    let mut result = current.to_owned();
    let mut last = length;
    for &(at, after, before) in edits {
        let end = at
            .checked_add(after.len())
            .expect("reachable primitive inverse overflow");
        assert!(end <= last && result.is_char_boundary(at) && result.is_char_boundary(end));
        assert_eq!(
            &result[at..end],
            after,
            "unknown reachable primitive source span"
        );
        result.replace_range(at..end, before);
        last = at;
    }
    result
}
mod paired_links;
mod shared_steps;
pub(super) fn previous_macro(current: &str) -> String {
    previous_macro_stage(&shared_steps::previous_macro(current))
}
fn previous_macro_stage(current: &str) -> String {
    restore(current, 6956, MACRO)
}
#[rustfmt::skip]
const MACRO: &[(usize, &str, &str)] = &[
    (4490, "        > OWNER_CONTROLS\n            + PRIMITIVE_FRAMES\n            + 2 * LOAD\n            + 2 * RESET\n            + 4 * (STEP + EMPTY)\n            + 2 * TAIL\n            + 4 * TAIL_VIEW\n", "        > OWNER_CONTROLS + 2 * LOAD + 2 * RESET + 4 * (STEP + EMPTY) + 2 * TAIL + 4 * TAIL_VIEW\n"),
    (478, "pub(super) const SINGLE_PRIMITIVE_FRAMES: usize = 5 * PAYMENT_HEADER;\n// Before inspecting the already-funded owner length, fund the new payment\n// site's complete primitive frame, match/local usize receivers and both\n// numeric pattern decisions. No dynamic payload uses this primitive bank.\npub(super) const PRIMITIVE_SELECTION: usize =\n    PAYMENT_HEADER + 3 * size_of::<usize>() + 2 * size_of::<bool>();\n", ""),
];
pub(super) fn previous_owner(current: &str) -> String {
    previous_owner_stage(&shared_steps::previous_owner(current))
}
fn previous_owner_stage(current: &str) -> String {
    restore(current, 23088, OWNER)
}
#[rustfmt::skip]
const OWNER: &[(usize, &str, &str)] = &[
    (9243, "    // Zero paths enter no cursor expansion. One path skips sorting and enters\n    // exactly one of the mutually exclusive flat/shared final expansions.\n    // Larger sets retain all four independent primitive-site banks. Every\n    // selected bank is acquired before any cursor expansion can execute.\n    let cursor_primitives = match owner.len {\n        0 => 0,\n        1 => lexical_cursor::SINGLE_PRIMITIVE_FRAMES,\n        _ => lexical_cursor::PRIMITIVE_FRAMES,\n    };\n    paid!(\n        budget,\n        function,\n        cursor_primitives,\n        \"ordinary plan reachable cursor primitive frames\"\n    )?;\n", ""),
    (6960, "    + lexical_cursor::PRIMITIVE_SELECTION + lexical_cursor::INITIALIZERS + PAIR_PRIMITIVE\n", "    + lexical_cursor::PRIMITIVE_FRAMES + lexical_cursor::INITIALIZERS + PAIR_PRIMITIVE\n"),
];
pub(super) fn previous_pair_tests(current: &str) -> String {
    previous_pair_tests_stage(&shared_steps::previous_pair_tests(current))
}
fn previous_pair_tests_stage(current: &str) -> String {
    restore(current, 40579, PAIR_TESTS)
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (40167, "                        + reachable_primitives\n", ""),
    (39832, "                        + load * rows[0].loads\n                        + all_primitives,\n", "                        + load * rows[0].loads,\n"),
    (39486, "                let (all_primitives, reachable_primitives) = reachable_primitive_prices(1);\n", ""),
    (29807, "        paid + reachable_primitive_prices(0).0,\n        old_paid\n            + PAIR_PRIMITIVE\n            + PAYMENT_HEADER\n            + prepared_link_prices().2\n            + reachable_primitive_prices(0).1,\n        \"every reachable primitive site and the complete new selector are funded\"\n", "        paid,\n        old_paid + PAIR_PRIMITIVE + PAYMENT_HEADER + prepared_link_prices().2,\n        \"all retained primitive frames are funded even on this unused path\"\n"),
    (27349, "        + limit * (2 * row.pairs + expected.len())\n        + reachable_primitives;\n", "        + limit * (2 * row.pairs + expected.len());\n"),
    (27092, "        + load * row.loads\n        + all_primitives;\n", "        + load * row.loads;\n"),
    (26849, "    let (all_primitives, reachable_primitives) = reachable_primitive_prices(expected.len());\n", ""),
    (10071, "                + reachable_primitives\n", ""),
    (9683, "                + load * row.loads\n                + all_primitives,\n", "                + load * row.loads,\n"),
    (9391, "        let (all_primitives, reachable_primitives) = reachable_primitive_prices(expected.len());\n", ""),
    (2321, "}\n// Independent source-type invoice for the reachable primitive-site bank.\n// No measured debit difference is used as a coefficient.\nfn reachable_primitive_prices(paths: usize) -> (usize, usize) {\n    let header = 2 * size_of::<&ResolvedFunction>()\n        + 2 * size_of::<&mut ReplayBudget>()\n        + 2 * size_of::<&str>()\n        + 2 * size_of::<usize>()\n        + 2 * size_of::<Result<(), Diagnostic>>();\n    let selector = header + 3 * size_of::<usize>() + 2 * size_of::<bool>();\n    let reachable = match paths {\n        0 => 0,\n        1 => 5 * header,\n        _ => 20 * header,\n    };\n    (20 * header, selector + reachable)\n", ""),
];
pub(super) fn previous_prepared_controls(current: &str) -> String {
    restore(current, 9965, PREPARED_CONTROLS)
}
#[rustfmt::skip]
const PREPARED_CONTROLS: &[(usize, &str, &str)] = &[
    (9373, "            &owner_stage,\n            previous_owner_stage,\n", "            include_str!(\"../../../comparison_owner.rs\"),\n            previous_owner,\n"),
    (9234, "        let owner_stage =\n            reachable_primitives::previous_owner(include_str!(\"../../../comparison_owner.rs\"));\n", ""),
    (8967, "            &macro_stage,\n            previous_macro_stage,\n", "            include_str!(\"../../lexical_cursor.rs\"),\n            previous_macro,\n"),
    (8833, "        let macro_stage =\n            reachable_primitives::previous_macro(include_str!(\"../../lexical_cursor.rs\"));\n", ""),
    (5040, "pub(super) fn previous_owner(current: &str) -> String {\n    previous_owner_stage(&reachable_primitives::previous_owner(current))\n}\n", ""),
    (3373, "pub(super) fn previous_macro(current: &str) -> String {\n    previous_macro_stage(&reachable_primitives::previous_macro(current))\n}\nfn previous_owner_stage(current: &str) -> String {\n", "pub(super) fn previous_owner(current: &str) -> String {\n"),
    (825, "mod reachable_primitives;\nfn previous_macro_stage(current: &str) -> String {\n", "pub(super) fn previous_macro(current: &str) -> String {\n"),
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
    fn whole_macro_owner_invoices_and_old_controls_match_exact_before_bytes() {
        assert_eq!(
            previous_macro(include_str!("../../../lexical_cursor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/reachable-primitives-8390-20261018/before-macro.rs.txt"
            ))
        );
        assert_eq!(
            previous_owner(include_str!("../../../../comparison_owner.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/reachable-primitives-8390-20261018/before-owner.rs.txt"
            ))
        );
        assert_eq!(
            previous_pair_tests(include_str!("../../../pair_scope_tests.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/reachable-primitives-8390-20261018/before-pair-tests.rs.txt"
            ))
        );
        assert_eq!(
            previous_prepared_controls(include_str!("../prepared_links.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/reachable-primitives-8390-20261018/before-prepared-controls.rs.txt"
            ))
        );
    }
    #[test]
    fn span_extent_and_outside_body_drift_never_match_whole_before_sources() {
        hostile(
            &shared_steps::previous_macro(include_str!("../../../lexical_cursor.rs")),
            previous_macro_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/reachable-primitives-8390-20261018/before-macro.rs.txt"
            )),
            MACRO,
        );
        hostile(
            &shared_steps::previous_owner(include_str!("../../../../comparison_owner.rs")),
            previous_owner_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/reachable-primitives-8390-20261018/before-owner.rs.txt"
            )),
            OWNER,
        );
        hostile(
            &shared_steps::previous_pair_tests(include_str!("../../../pair_scope_tests.rs")),
            previous_pair_tests_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/reachable-primitives-8390-20261018/before-pair-tests.rs.txt"
            )),
            PAIR_TESTS,
        );
        hostile(
            include_str!("../prepared_links.rs"),
            previous_prepared_controls,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/reachable-primitives-8390-20261018/before-prepared-controls.rs.txt"
            )),
            PREPARED_CONTROLS,
        );
    }
}
