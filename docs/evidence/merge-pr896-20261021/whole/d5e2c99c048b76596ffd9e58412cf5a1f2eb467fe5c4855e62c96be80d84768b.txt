//! Exact observational JSON-profile continuation, preserving all older controls.
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
    previous_route_tail_stage(&super::paired_tails::previous_route(current))
}
fn previous_route_tail_stage(current: &str) -> String {
    restore(current, 35723, ROUTE)
}
#[rustfmt::skip]
const ROUTE: &[(usize, &str, &str)] = &[
    (201, "mod json_codec_probe;\n", ""),
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
    previous_controls_tail_stage(&super::paired_tails::previous_controls(current))
}
fn previous_controls_tail_stage(current: &str) -> String {
    restore(current, 24582, CONTROLS)
}
#[rustfmt::skip]
const CONTROLS: &[(usize, &str, &str)] = &[
    (22925, "            &super::json_ledger::previous_route(include_str!(\n                \"../../../../../../../../bounded_output/stage_debits/route_debits.rs\"\n            )),\n            previous_route_stage,\n", "            include_str!(\"../../../../../../../../bounded_output/stage_debits/route_debits.rs\"),\n            previous_route,\n"),
    (12603, "    previous_route_stage(&json_ledger::previous_route(current))\n}\nfn previous_route_stage(current: &str) -> String {\n", ""),
    (12530, "mod json_ledger;\n", ""),
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
    previous_constructor_tail_stage(&super::paired_tails::previous_constructor(current))
}
fn previous_constructor_tail_stage(current: &str) -> String {
    restore(current, 4758, CONSTRUCTOR)
}
#[rustfmt::skip]
const CONSTRUCTOR: &[(usize, &str, &str)] = &[
    (4403, "            &super::super::json_ledger::previous_controls(include_str!(\"../paired_links.rs\")),\n            previous_controls_stage,\n", "            include_str!(\"../paired_links.rs\"),\n            previous_controls,\n"),
    (1174, "    previous_controls_stage(&super::json_ledger::previous_controls(current))\n}\nfn previous_controls_stage(current: &str) -> String {\n", ""),
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
    fn json_profile_sources_restore_all_three_complete_predecessors() {
        assert_eq!(
            previous_route(include_str!(
                "../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/json-ledger-bc294-20261018/before-route.rs.txt"
            ))
        );
        assert_eq!(
            previous_controls(include_str!("../paired_links.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/json-ledger-bc294-20261018/before-controls.rs.txt"
            ))
        );
        assert_eq!(
            previous_constructor(include_str!("constructor.rs")),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/json-ledger-bc294-20261018/before-constructor.rs.txt"
            ))
        );
    }
    #[test]
    fn json_profile_sources_refuse_span_extent_and_outside_drift() {
        hostile(
            &super::super::paired_tails::previous_route(include_str!(
                "../../../../../../../../../bounded_output/stage_debits/route_debits.rs"
            )),
            previous_route_tail_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/json-ledger-bc294-20261018/before-route.rs.txt"
            )),
            ROUTE,
        );
        hostile(
            &super::super::paired_tails::previous_controls(include_str!("../paired_links.rs")),
            previous_controls_tail_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/json-ledger-bc294-20261018/before-controls.rs.txt"
            )),
            CONTROLS,
        );
        hostile(
            &super::super::paired_tails::previous_constructor(include_str!("constructor.rs")),
            previous_constructor_tail_stage,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/json-ledger-bc294-20261018/before-constructor.rs.txt"
            )),
            CONSTRUCTOR,
        );
    }
}
