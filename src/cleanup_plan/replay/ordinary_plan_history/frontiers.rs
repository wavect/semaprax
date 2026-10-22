//! Test-only fixed observation buffer. It cannot inject or authorize a debit.
use std::cell::RefCell;
#[derive(Clone, Copy, Debug)]
pub(super) struct Event {
    pub phase: &'static str,
    pub before: usize,
    pub after: Option<usize>,
    pub swaps: usize,
}
const EMPTY: Event = Event {
    phase: "",
    before: 0,
    after: None,
    swaps: 0,
};
struct Trace {
    enabled: bool,
    count: usize,
    pending: Option<usize>,
    swaps_only: bool,
    swaps: usize,
    events: [Event; 4096],
}
thread_local! {
    static TRACE: RefCell<Trace> = const { RefCell::new(Trace { enabled: false, count: 0, pending: None, swaps_only: false, swaps: 0, events: [EMPTY; 4096] }) };
}
// The fixed TLS buffer has no heap backing. Warm thread-local access before
// measurements when a caller already has an explicit preparation boundary.
pub(super) fn prepare() {
    TRACE.with(|_| {});
    super::comparison_owner::pair_scope_census::prepare();
}
fn consumed() -> usize {
    crate::bounded_output::active_limit().unwrap()
        - crate::bounded_output::active_remaining().unwrap()
}
pub(super) fn reset() {
    reset_mode(false);
}
// The cold/hot invoice corpus needs complete swap order, not every debit.
// Keep the same fixed buffer and fail closed if the selected stream fills it.
pub(super) fn reset_swaps() {
    reset_mode(true);
}
fn reset_mode(swaps_only: bool) {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        assert_eq!(
            t.events.len(),
            4096,
            "frontier instrumentation must be prepared"
        );
        t.enabled = true;
        t.count = 0;
        t.pending = None;
        t.swaps_only = swaps_only;
        t.swaps = 0;
    });
}
pub(super) fn before(phase: &'static str) {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        if !t.enabled {
            return;
        }
        t.pending = None;
        if t.swaps_only
            && phase != "ordinary plan retained drain swap receivers"
            && phase != "ordinary plan retained sift swap receivers"
        {
            return;
        }
        assert!(
            t.count < t.events.len(),
            "finite frontier observation capacity exhausted"
        );
        let index = t.count;
        t.events[index] = Event {
            phase,
            before: consumed(),
            after: None,
            swaps: t.swaps,
        };
        t.pending = Some(index);
        t.count += 1;
    });
}
pub(super) fn after() {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        if t.enabled {
            if let Some(index) = t.pending {
                t.events[index].after = Some(consumed());
            }
        }
    });
}
pub(super) fn swapped() {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        if t.enabled {
            t.swaps += 1;
        }
    });
}
pub(super) fn finish() -> Vec<Event> {
    TRACE.with(|trace| {
        let mut t = trace.borrow_mut();
        t.enabled = false;
        t.events[..t.count].to_vec()
    })
}

// Closed literal current-source acquisitions for the five direct whole-body oracles.
// Every original inverse and final old hash remains downstream unchanged.
pub(super) fn before_preparation(source: &str) -> String {
    use sha2::{Digest, Sha256};
    let (current, length, sha, self_source) = match source {
        "main001_pair_scope_tests.rs" => (
            include_str!("comparison_owner/main001_pair_scope_tests.rs"),
            50692,
            "b6f24502e16f6439d59c14ffe8700a84decc8c36876378e557e9245dc58e27bc",
            false,
        ),
        "main487_pair_scope_tests.rs" => (
            include_str!("comparison_owner/main487_pair_scope_tests.rs"),
            55524,
            "0a76af1059064eaeaef23136068d1a35cdc77e5ad931e28eb0ee304a5a8baaa6",
            false,
        ),
        "main_b692_pair_scope_tests.rs" => (
            include_str!("comparison_owner/main_b692_pair_scope_tests.rs"),
            50866,
            "0b92d66d8c5b903c6c2b949b2b74517bc5e45117bc9e6d6ef46036f2173c8d0e",
            false,
        ),
        "main7e_pair_scope_tests.rs" => (
            include_str!("comparison_owner/main7e_pair_scope_tests.rs"),
            50993,
            "287ce9abf3b7a74164c1f58008deb3a6b8be214afb1218e98a90e4c320d30c7c",
            true,
        ),
        "main99_pair_scope_tests.rs" => (
            include_str!("comparison_owner/main99_pair_scope_tests.rs"),
            43884,
            "cd73e40396090ee5fd3cb7a1c7f76e0dc3958e083e32611e4eef3e05432965e3",
            true,
        ),
        _ => panic!("unknown observer preparation source"),
    };
    let mut restored = current.to_owned();
    if source == "main99_pair_scope_tests.rs" {
        assert_eq!(restored.len(), 43965);
        assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))), "a176917b2f70a4ddff56230d92de0abd6575bfc3e6869b08612d5fff767499a1");
        assert_eq!(restored.matches("    let current = include_str!(\"lexical_cursor.rs\");\n    let admitted = super::super::reachable_stage(\n        \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs\",\n        \"e3278dc29dcce665dd9d3a67ce048e552254a65c5a939f769b2b7c1eb86a9ded\",\n        current,\n    );\n    assert_eq!(include_str!(\"lexical_cursor_shared_steps.rs\"), admitted);\n").count(), 1);
        restored = restored.replacen("    let current = include_str!(\"lexical_cursor.rs\");\n    let admitted = super::super::reachable_stage(\n        \"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs\",\n        \"e3278dc29dcce665dd9d3a67ce048e552254a65c5a939f769b2b7c1eb86a9ded\",\n        current,\n    );\n    assert_eq!(include_str!(\"lexical_cursor_shared_steps.rs\"), admitted);\n", "    let current = include_str!(\"lexical_cursor.rs\");\n    let admitted = super::super::reachable_stage(\"src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs\", \"e3278dc29dcce665dd9d3a67ce048e552254a65c5a939f769b2b7c1eb86a9ded\", current);\n    assert_eq!(include_str!(\"lexical_cursor_shared_steps.rs\"), admitted);\n", 1);
        assert_eq!(restored.matches("use super::*;\nuse crate::bounded_output::with_limit_usage_refusal;\n").count(), 1);
        restored = restored.replacen("use super::*;\nuse crate::bounded_output::with_limit_usage_refusal;\n", "use super::*;\nuse super::super::before_shared_steps::{compare, PAYMENT_HEADER, PATH_INDEX, PAIR_SCOPE, PAIR_PRIMITIVE};\nuse crate::bounded_output::with_limit_usage_refusal;\n", 1);
        assert_eq!(restored.matches("//! Pair-local successor: real complete producer+comparison boundaries.\nuse super::super::before_shared_steps::{\n    compare, PAIR_PRIMITIVE, PAIR_SCOPE, PATH_INDEX, PAYMENT_HEADER,\n};\nuse super::*;\n").count(), 1);
        restored = restored.replacen("//! Pair-local successor: real complete producer+comparison boundaries.\nuse super::super::before_shared_steps::{\n    compare, PAIR_PRIMITIVE, PAIR_SCOPE, PATH_INDEX, PAYMENT_HEADER,\n};\nuse super::*;\n", "//! Pair-local successor: real complete producer+comparison boundaries.\nuse super::*;\n", 1);
    }
    if self_source {
        let after = format!("frontiers::before_preparation(\"{source}\")");
        let before = format!("include_str!(\"{source}\")");
        assert_eq!(restored.matches(&after).count(), 1);
        restored = restored.replacen(&after, &before, 1);
    }
    let preparation = "    frontiers::prepare();\n";
    assert_eq!(restored.matches(preparation).count(), 1);
    restored = restored.replacen(preparation, "", 1);
    assert_eq!(restored.len(), length);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        sha
    );
    restored
}

#[test]
fn prepared_fixed_observers_keep_measured_resets_and_frontiers_allocation_free() {
    use crate::test_allocation_observer::{Metrics, measure};
    prepare();
    let (((), heap), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                prepare();
                super::comparison_owner::pair_scope_census::reset_for_frontier_test();
                reset();
                before("prepared observer test");
                after();
                swapped();
            })
        });
    assert_eq!(heap, Metrics::default());
    assert!(!refused);
    assert_eq!(paid, 0);
    assert!(receipt.is_none());
    assert_eq!(TRACE.with(|trace| trace.borrow().events.len()), 4096);
    let events = finish();
    assert_eq!(events.len(), 1);
    assert_eq!(
        (
            events[0].phase,
            events[0].before,
            events[0].after,
            events[0].swaps
        ),
        ("prepared observer test", 0, Some(0), 0)
    );
    let (empty, lost) = super::comparison_owner::pair_scope_census::finish_empty_for_frontier_test();
    assert!(empty);
    assert_eq!(lost, 0);
}

#[cfg(test)]
mod initialization_tests;
