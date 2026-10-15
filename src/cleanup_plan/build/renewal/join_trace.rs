//! Bounded test-build-only explanation; no liveness/history mutation or authority.
use super::super::{FlowState, PlanBuilder};
use std::fmt::Write;
const MAX_BYTES: usize = 4096;
struct Bounded(String);
impl std::fmt::Write for Bounded {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let available = MAX_BYTES - self.0.len();
        let mut end = text.len().min(available);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        self.0.push_str(&text[..end]);
        Ok(())
    }
}
pub(super) fn describe(builder: &PlanBuilder<'_>, left: &FlowState, right: &FlowState) -> String {
    let mut text = Bounded(String::with_capacity(MAX_BYTES));
    let _ = writeln!(text, "cleanup join function={}", builder.function.id);
    for (name, state) in [("left", left), ("right", right)] {
        let _ = writeln!(text, "{name} live_order={:?}", state.live_order);
        for flag in state.live_order.iter().take(32) {
            if let Some(leaf) = builder.leaves.get(flag) {
                let _ = writeln!(text, "{name} flag={} place={:?}", flag.0, leaf.place);
            }
        }
    }
    text.0
}
#[test]
fn debug_writer_keeps_utf8_and_exact_byte_envelope_without_heap_per_fragment() {
    let mut text = Bounded(String::with_capacity(MAX_BYTES));
    let capacity = text.0.capacity();
    text.write_str(&"a".repeat(MAX_BYTES - 1)).unwrap();
    text.write_str("é").unwrap();
    text.write_str("z").unwrap();
    assert_eq!(text.0.len(), MAX_BYTES);
    assert!(text.0.ends_with('z'));
    assert_eq!(text.0.capacity(), capacity);
    text.write_str("late").unwrap();
    assert_eq!(text.0.len(), MAX_BYTES);
}
