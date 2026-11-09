//! Pure packet reuse after the complete fallback acquisition has run again.
use super::{
    actual_with_packet, normalize_context, read_view_with_packet, InvocationCall,
    ProjectionInvocation,
};
use super::{clippy_stage, digest, from_packet, from_packet_with_context};
use serde_json::Value;
use std::path::Path;

pub(super) fn value(authenticated: &Value, source: &str, expected: &str, supplied: &str) -> String {
    value_with_fallback_hook(authenticated, source, expected, supplied, || {})
}

pub(super) fn value_with_fallback_hook(
    authenticated: &serde_json::Value,
    source: &str,
    expected: &str,
    supplied: &str,
    before_fallback: impl FnOnce(),
) -> String {
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
pub(in super::super) fn test_original_context(
    authenticated: &Value,
    source: &str,
    expected: &str,
    supplied: &str,
) -> String {
    super::with_packet_context(authenticated, source, expected, supplied)
}

#[cfg(test)]
pub(in super::super) fn test_reacquired_value(
    authenticated: &Value,
    source: &str,
    expected: &str,
    supplied: &str,
    before: impl FnOnce(),
) -> String {
    value_with_fallback_hook(authenticated, source, expected, supplied, before)
}

// Preserve the original caller cap on both the full current read and its
// explicitly declared historical output before entering a source-only view.
pub(in super::super) fn declared_registered_file(
    source: &str,
    expected: &str,
    cap: usize,
) -> Option<Option<String>> {
    let (part, index) = super::super::declared_history_inputs::slot(source, expected)?;
    let packet = super::super::packet();
    let actual = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)?;
    let fits = |row: &Value| row["current"]["length"].as_u64().unwrap() <= cap as u64;
    if !fits(actual)
        || actual.get("fragments").is_some_and(|children| {
            children
                .as_array()
                .unwrap()
                .iter()
                .any(|child| !fits(child))
        })
    {
        return Some(None);
    }
    actual_with_packet(&packet, actual);
    let row = &super::super::declared_history_inputs::records(part)[index];
    let length = row["before_bytes"].as_u64().unwrap() as usize;
    Some(
        if length <= cap && super::super::declared_history_inputs::compiled(source).len() <= cap {
            super::super::declared_history_inputs::expected_file(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join(source),
                length,
                expected,
            )
        } else {
            None
        },
    )
}

// This named source epoch remains a complete authenticated historical input.
pub(in super::super) fn cursor_epoch_value(
    source: &str,
    expected: &str,
    supplied: &str,
) -> Option<String> {
    const SOURCE: &str = "src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs";
    const EPOCH: &str = "2c91db9d0da06846703b41d7221a5daf16b56aa73933f0a88b714d23d08552a2";
    const BEFORE: &str = "fdb672ec67714810ba990a306ec8b282ce7c628d627b272b322d21e51293da34";
    if source != SOURCE || expected != BEFORE || digest(supplied.as_bytes()) != EPOCH {
        return None;
    }
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(SOURCE);
    let epoch = super::super::declared_history_inputs::expected_file(&path, 8182, EPOCH)?;
    assert_eq!(
        supplied, epoch,
        "entire named cursor epoch on current channels"
    );
    super::super::declared_history_inputs::expected_file(&path, 8127, BEFORE)
}

impl<F: FnMut() -> serde_json::Value, S: FnMut() -> serde_json::Value> ProjectionInvocation<F, S> {
    pub(crate) fn combined_side_expected_value(
        &mut self,
        source: &str,
        expected: &str,
        supplied: &str,
    ) -> String {
        let mut call = InvocationCall::default();
        let result = self.combined_side_expected_value_inner(&mut call, source, expected, supplied);
        self.finish_call(call);
        result
    }
    fn combined_side_expected_value_inner(
        &mut self,
        call: &mut InvocationCall,
        source: &str,
        expected: &str,
        supplied: &str,
    ) -> String {
        let packet = super::super::combined_side_packet();
        let Some(row) = packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)
        else {
            return supplied.to_owned();
        };
        let Some(previous) = row["views"]
            .as_array()
            .unwrap()
            .iter()
            .find(|view| view["sha256"] == expected)
        else {
            return supplied.to_owned();
        };
        let old = read_view_with_packet(self.call_metadata(call), previous);
        let actual = self.combined_side_actual_inner(call, row);
        let (authenticated, stage) = self.call_direct_metadata(call, source);
        if normalize_context(authenticated, stage, source, supplied).as_ref() == actual
            || supplied == old
        {
            old
        } else {
            supplied.to_owned()
        }
    }
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
        let mut injected = ProjectionInvocation::with_loaders(
            || {
                calls.set(calls.get() + 1);
                if calls.get() == 1 {
                    std::panic::resume_unwind(Box::new("loader failed before publishing metadata"));
                }
                serde_json::json!({"arbitrary-injected-metadata": [1, 2, 3]})
            },
            || serde_json::json!({}),
        );
        injected.guard = super::super::super::overlay_v2::InvocationGuard::Current;
        assert!(!injected.ordinary_packet_loader);
        let mut call = InvocationCall::default();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = injected.call_metadata(&mut call);
        }));
        assert!(failure.is_err() && injected.packet.is_none() && !call.metadata);
        drop(failure);
        assert_eq!(
            injected.call_metadata(&mut call)["arbitrary-injected-metadata"],
            serde_json::json!([1, 2, 3])
        );
        assert_eq!(calls.get(), 2);
        assert!(!injected.ordinary_packet_loader);
    }
}
