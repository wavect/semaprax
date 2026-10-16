use super::*;
use crate::test_allocation_observer::measure;

#[test]
fn complete_field_and_partial_debits_match_original_formatter_at_each_byte() {
    const TEXT: &str = "é\0x";
    for floor in [0, 1, 3] {
        for limit in 0..=32 {
            let old = with_limit_usage_refusal(limit, || {
                assert!(set_active_floor(floor.min(limit)));
                let text = budgeted_format(format_args!("start:{TEXT}:{}:end", 97usize));
                let complete = !text.is_empty();
                drop(text);
                complete
            });
            let current = with_limit_usage_refusal(limit, || {
                assert!(set_active_floor(floor.min(limit)));
                let (complete, physical) =
                    measure(|| formatted(format_args!("start:{TEXT}:{}:end", 97usize)).complete);
                assert!(!physical.invalid && physical.allocations == 0 && physical.live == 0);
                complete
            });
            assert_eq!(old, current, "floor={floor} limit={limit}");
        }
    }
}

#[test]
fn joined_fields_keep_three_original_writes_and_complete_empty_is_not_failure() {
    for limit in 0..=16 {
        let old = with_limit_usage_refusal(limit, || {
            let a = "help";
            let b = "é";
            let result = budgeted_format(format_args!("{a} {b}"));
            let complete = !result.is_empty();
            drop(result);
            complete
        });
        let current = with_limit_usage_refusal(limit, || joined(4, 2).complete);
        assert_eq!(old, current);
    }
    let (field, refused, used, first) = with_limit_usage_refusal(0, || formatted(format_args!("")));
    assert!(field.complete && field.bytes == 0 && !refused && used == 0 && first.is_none());
    let (field, refused, _, first) = with_limit_usage_refusal(0, || {
        assert!(!reserve_active_required(1));
        formatted(format_args!("already sticky"))
    });
    assert!(!field.complete && refused && first.is_some());
}
