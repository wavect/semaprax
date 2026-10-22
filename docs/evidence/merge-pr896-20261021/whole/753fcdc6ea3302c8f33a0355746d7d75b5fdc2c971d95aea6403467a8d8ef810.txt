//! Preserve the exact Take<Repeat<()>> test with a Clippy-clean constructor.
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
    previous_pair_tests_tail_stage(&super::paired_tails::previous_pair_tests(current))
}
fn previous_pair_tests_tail_stage(current: &str) -> String {
    restore(current, 50521, PAIR_TESTS)
}
#[rustfmt::skip]
const PAIR_TESTS: &[(usize, &str, &str)] = &[
    (50110, "            let repeating = std::iter::repeat(());\n            let mut driver: SharedUnitDriver = repeating.take(length);\n", "            let mut driver: SharedUnitDriver = std::iter::repeat(()).take(length);\n"),
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
    previous_controls_stage(&super::json_ledger::previous_controls(current))
}
fn previous_controls_stage(current: &str) -> String {
    restore(current, 24375, CONTROLS)
}
#[rustfmt::skip]
const CONTROLS: &[(usize, &str, &str)] = &[
    (22392, "            &super::constructor::previous_pair_tests(include_str!(\n                \"../../../../pair_scope_tests.rs\"\n            )),\n            previous_pair_tests_stage,\n", "            include_str!(\"../../../../pair_scope_tests.rs\"),\n            previous_pair_tests,\n"),
    (10201, "    previous_pair_tests_stage(&constructor::previous_pair_tests(current))\n}\nfn previous_pair_tests_stage(current: &str) -> String {\n", ""),
    (10123, "mod constructor;\n", ""),
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
    fn constructor_sources_restore_both_complete_predecessors() {
        assert_eq!(
            previous_pair_tests(include_str!("../../../../../pair_scope_tests.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/repeat-constructor-308-20261018/before-pair-tests.rs.txt"
            ))
        );
        assert_eq!(
            previous_controls(include_str!("../paired_links.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/repeat-constructor-308-20261018/before-controls.rs.txt"
            ))
        );
    }
    #[test]
    fn constructor_sources_refuse_span_extent_and_outside_drift() {
        hostile(
            &super::super::paired_tails::previous_pair_tests(include_str!(
                "../../../../../pair_scope_tests.rs"
            )),
            previous_pair_tests_tail_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/repeat-constructor-308-20261018/before-pair-tests.rs.txt"
            )),
            PAIR_TESTS,
        );
        hostile(
            &super::super::json_ledger::previous_controls(include_str!("../paired_links.rs")),
            previous_controls_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/repeat-constructor-308-20261018/before-controls.rs.txt"
            )),
            CONTROLS,
        );
    }
}
