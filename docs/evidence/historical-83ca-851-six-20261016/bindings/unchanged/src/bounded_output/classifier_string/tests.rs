//! Logical formatting oracle, independently of physical payer acquisition.
use super::*;
use crate::bounded_output::{set_active_floor, with_limit_usage_refusal, CappedString};
use std::cell::Cell;
use std::fmt::Write;

struct Chunks;
impl fmt::Display for Chunks {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("aaaaa")?;
        output.write_str("b")
    }
}
#[test]
fn original_reserved_floor_suppression_keeps_partial_bytes_without_physical_growth() {
    let old = with_limit_usage_refusal(10, || {
        assert!(set_active_floor(6));
        let mut output = CappedString::new();
        write!(&mut output, "{}", Chunks).unwrap();
        output.into_string()
    });
    let requests = Cell::new(0usize);
    let current = with_limit_usage_refusal(10, || {
        assert!(set_active_floor(6));
        // This callback observes physical requests independently; it grants no
        // production authority and this logical oracle claims no funded heap.
        let mut pay = |bytes| {
            requests.set(requests.get() + bytes);
            Ok::<_, ()>(())
        };
        let mut output = ClassifierString::acquire(&mut pay).unwrap();
        output.capped(format_args!("{}", Chunks), &mut pay).unwrap();
        output.into_string()
    });
    assert_eq!(old, current);
    assert_eq!(current.0, "b");
    assert!(!current.1);
    assert_eq!(requests.get(), size_of::<ClassifierString>() + 8);
    let old = with_limit_usage_refusal(10, || {
        assert!(set_active_floor(6));
        crate::bounded_output::budgeted_format(format_args!("{}", Chunks))
    });
    requests.set(0);
    let current = with_limit_usage_refusal(10, || {
        assert!(set_active_floor(6));
        let mut pay = |bytes| {
            requests.set(requests.get() + bytes);
            Ok::<_, ()>(())
        };
        let mut output = ClassifierString::acquire(&mut pay).unwrap();
        output.strict(format_args!("{}", Chunks), &mut pay).unwrap();
        output.into_string()
    });
    assert_eq!(old, current);
    assert_eq!(current.0, "");
    assert_eq!(requests.get(), size_of::<ClassifierString>());
}
#[test]
fn original_overflow_and_formatter_error_keep_exact_suppressed_output() {
    for limit in [0, 1, 5, 6, 17] {
        let old = with_limit_usage_refusal(limit, || {
            let mut output = CappedString::new();
            write!(&mut output, "{}", Chunks).unwrap();
            output.into_string()
        });
        let current = with_limit_usage_refusal(limit, || {
            let mut pay = |_| Ok::<_, ()>(());
            let mut output = ClassifierString::acquire(&mut pay).unwrap();
            output.capped(format_args!("{}", Chunks), &mut pay).unwrap();
            output.into_string()
        });
        assert_eq!(old, current);
    }
    struct Broken;
    impl fmt::Display for Broken {
        fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
            output.write_str("accepted")?;
            Err(fmt::Error)
        }
    }
    let old = with_limit_usage_refusal(17, || {
        crate::bounded_output::budgeted_format(format_args!("{}", Broken))
    });
    let current = with_limit_usage_refusal(17, || {
        let mut pay = |_| Ok::<_, ()>(());
        let mut output = ClassifierString::acquire(&mut pay).unwrap();
        output.strict(format_args!("{}", Broken), &mut pay).unwrap();
        output.into_string()
    });
    assert_eq!(old, current);
}
