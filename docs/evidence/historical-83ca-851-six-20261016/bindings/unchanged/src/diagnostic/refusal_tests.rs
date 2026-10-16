//! Message representation only; output buffers/report vectors have other owners.
use super::*;
use crate::bounded_output::with_limit_usage;
use crate::test_allocation_observer::measure;

#[test]
fn static_message_creation_clone_and_drop_need_no_zero_allowance_backing() {
    const MESSAGE: &str = "fixed refusal: quoted \"text\", slash \\, newline\n and UTF-8 é";
    let (((), observed), refused, paid) = with_limit_usage(0, || {
        measure(|| {
            let diagnostic = Diagnostic::io_static("SPX-H006", MESSAGE);
            assert!(matches!(diagnostic.message, Cow::Borrowed(_)));
            assert_eq!(diagnostic.message.as_ptr(), MESSAGE.as_ptr());
            assert_eq!(diagnostic.message_owned_capacity(), 0);
            assert_eq!(diagnostic.code, "SPX-H006");
            assert_eq!(diagnostic.severity, Severity::Error);
            assert!(
                diagnostic.path.is_none() && diagnostic.span.is_none() && diagnostic.help.is_none()
            );
            let copy = diagnostic.clone();
            assert_eq!(copy, diagnostic);
            assert_eq!(copy.message.as_ptr(), MESSAGE.as_ptr());
            drop(copy);
            drop(diagnostic);
        })
    });
    assert!(!refused);
    assert_eq!(paid, 0);
    assert_eq!(observed.allocations, 0);
    assert_eq!(observed.reallocations, 0);
    assert_eq!(observed.allocated, 0);
    assert_eq!(observed.freed, 0);
    assert_eq!(observed.live, 0);
    assert!(!observed.invalid);
}

#[test]
fn dynamic_constructors_keep_owned_nonstatic_slices_and_prepaid_text_pointer_capacity() {
    let caller = String::from("caller owned é");
    for constructor in 0..3 {
        let span = Span {
            line: 7,
            column: 3,
            start: 42,
            end: 51,
        };
        let diagnostic = match constructor {
            0 => Diagnostic::io("SPX-H006", caller.as_str()),
            1 => Diagnostic::error("SPX-H006", caller.as_str(), span),
            _ => Diagnostic::warning("SPX-H006", caller.as_str(), span),
        };
        assert!(matches!(diagnostic.message, Cow::Owned(_)));
        assert_eq!(diagnostic.message.as_ref(), caller.as_str());
        assert_ne!(diagnostic.message.as_ptr(), caller.as_ptr());
        let (_, copies) = measure(|| drop(diagnostic.clone()));
        assert_eq!(copies.allocations, 1);
        assert_eq!(copies.allocated, caller.len());
        assert_eq!(copies.freed, copies.allocated);
        assert_eq!(copies.live, 0);
        assert!(!copies.invalid);
    }
    let mut text = String::with_capacity(128);
    text.push_str("prepaid current report");
    let pointer = text.as_ptr();
    let capacity = text.capacity();
    // This window receives existing backing; that backing is disposed below,
    // outside the receiving-only observer. No production payer is invented.
    let (diagnostic, received) = measure(|| Diagnostic::io("SPX-H006", text));
    assert!(matches!(diagnostic.message, Cow::Owned(_)));
    assert_eq!(diagnostic.message.as_ptr(), pointer);
    assert_eq!(diagnostic.message_owned_capacity(), capacity);
    assert_eq!(received.allocations, 0);
    assert_eq!(received.reallocations, 0);
    assert_eq!(received.live, 0);
    assert!(!received.invalid);
    drop(diagnostic);
}

#[test]
fn full_wire_display_and_debug_match_original_owned_messages_for_every_location_shape() {
    const MESSAGE: &str = "refused \"byte\" \\ \t\n é";
    for location in 0..4 {
        let mut old = Diagnostic::io("SPX-H006", MESSAGE);
        let mut current = Diagnostic::io_static("SPX-H006", MESSAGE);
        if location & 1 != 0 {
            old.path = Some("source\npath.spx".to_owned());
            current.path = old.path.clone();
        }
        if location & 2 != 0 {
            old.span = Some(Span {
                line: 7,
                column: 3,
                start: 42,
                end: 51,
            });
            current.span = old.span;
        }
        old.help = Some("same help\né".to_owned());
        current.help = old.help.clone();
        assert_eq!(current, old);
        // Output storage is intentionally outside the zero-message observer.
        assert_eq!(current.json(), old.json());
        assert_eq!(current.to_string(), old.to_string());
        assert_eq!(format!("{current:?}"), format!("{old:?}"));
        assert_eq!(current.message.into_owned(), old.message.into_owned());
    }
}
