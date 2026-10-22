#![allow(
    clippy::duplicate_mod,
    reason = "The complete fixture intentionally compiles independently against current and predecessor owner types."
)]
use super::*;
use crate::test_allocation_observer::{current_metrics, measure};
#[test]
fn held_first_six_field_diagnostic_moves_once_and_terminal_retry_keeps_empty_sentinel() {
    let mut expected = Diagnostic::io("SPX-H006", "paid first message".to_owned());
    expected.severity = crate::diagnostic::Severity::Warning;
    expected.path = Some("selected.spx".to_owned());
    expected.span = Some(crate::ast::Span {
        start: 2,
        end: 7,
        line: 3,
        column: 5,
    });
    expected.help = Some("paid first help".to_owned());
    let selected = expected.clone();
    let message = selected.message.as_ptr();
    let path = selected.path.as_ref().unwrap().as_ptr();
    let help = selected.help.as_ref().unwrap().as_ptr();
    let function = crate::cleanup_plan::replay::factored::lexical_plan_tests::fixture(
        crate::cleanup_plan::replay::factored::lexical_plan_tests::WHILE,
    )
    .1;
    let ((), refused, _, first) = crate::bounded_output::with_limit_usage_refusal(1 << 20, || {
        let mut budget = ReplayBudget::new();
        let mut memo = RetainedMemo::<usize>::new();
        memo.authenticate_retained(&function, &mut budget);
        assert!(memo.controls_paid && !memo.failed());
        // The complete preselected fixture is test input; activation above pays
        // the real held receiving owner before moving it into that slot.
        memo.memo.failed = true;
        memo.refusal = Some(selected);
        let ((), allocation) = measure(|| {
            let at = current_metrics().allocated;
            let outward = memo.failure();
            assert_eq!(outward, expected);
            assert_eq!(outward.message.as_ptr(), message);
            assert_eq!(outward.path.as_ref().unwrap().as_ptr(), path);
            assert_eq!(outward.help.as_ref().unwrap().as_ptr(), help);
            assert_eq!(current_metrics().allocated, at);
            assert_eq!(memo.failure(), Diagnostic::io("SPX-H006", String::new()));
            drop(outward);
        });
        assert!(!allocation.invalid && allocation.allocated == 0);
        drop(memo);
        drop(budget);
    });
    assert!(!refused && first.is_none());
}
