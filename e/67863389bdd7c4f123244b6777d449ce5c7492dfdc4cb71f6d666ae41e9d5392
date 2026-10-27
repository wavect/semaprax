//! Pure packet reuse after the complete fallback acquisition has run again.
use super::{clippy_stage, digest, from_packet, from_packet_with_context};
use serde_json::Value;

pub(super) fn value(authenticated: &Value, source: &str, expected: &str, supplied: &str) -> String {
    value_with_fallback_hook(authenticated, source, expected, supplied, || {})
}

pub(super) fn value_with_fallback_hook(authenticated: &serde_json::Value, source: &str, expected: &str, supplied: &str, before_fallback: impl FnOnce()) -> String {
    // Current and prior logical-table views retain their complete scope before
    // the separately authenticated historical Clippy-stage acquisition.
    let direct = from_packet_with_context(authenticated, source, expected, supplied);
    if digest(direct.as_bytes()) == expected {
        return direct;
    }
    if let Some(value) = clippy_stage::requested(source, expected, supplied) {
        let _ = authenticated;
        return value;
    }
    let normalized = clippy_stage::normalize(source, supplied);
    before_fallback();
    super::super::overlay_v2::packet_reacquire::run();
    let result = from_packet(authenticated, source, expected, normalized.as_ref());
    if normalized.as_ref() != supplied
        && result == normalized.as_ref()
        && digest(result.as_bytes()) != expected
    {
        supplied.to_owned()
    } else {
        result
    }
}

#[cfg(test)]
pub(in super::super) fn test_original_context(authenticated: &Value, source: &str, expected: &str, supplied: &str) -> String {
    super::with_packet_context(authenticated, source, expected, supplied)
}

#[cfg(test)]
pub(in super::super) fn test_reacquired_value(authenticated: &Value, source: &str, expected: &str, supplied: &str, before: impl FnOnce()) -> String {
    value_with_fallback_hook(authenticated, source, expected, supplied, before)
}

#[cfg(test)]
mod tests {
    use super::super::{InvocationCall, ProjectionInvocation};
    use std::cell::Cell;

    #[test]
    fn ordinary_origin_cannot_be_obtained_from_an_injected_current_guard() {
        let ordinary = ProjectionInvocation::new();
        assert!(ordinary.ordinary_packet_loader && ordinary.packet.is_none());
        let calls = Cell::new(0);
        let mut injected = ProjectionInvocation::with_loaders(|| {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                std::panic::resume_unwind(Box::new("loader failed before publishing metadata"));
            }
            serde_json::json!({"arbitrary-injected-metadata": [1, 2, 3]})
        }, || serde_json::json!({}));
        injected.guard = super::super::super::overlay_v2::InvocationGuard::Current;
        assert!(!injected.ordinary_packet_loader);
        let mut call = InvocationCall::default();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = injected.call_metadata(&mut call);
        }));
        assert!(failure.is_err() && injected.packet.is_none() && !call.metadata);
        drop(failure);
        assert_eq!(injected.call_metadata(&mut call)["arbitrary-injected-metadata"], serde_json::json!([1, 2, 3]));
        assert_eq!(calls.get(), 2);
        assert!(!injected.ordinary_packet_loader);
    }
}
