//! Agent Stage Semantic Work v1: record each performed plan finalizer.
//!
//! The event is emitted inside the finalizer's own liveness guard, so only a
//! finalizer that actually runs is recorded, in execution order. Nothing is
//! emitted unless the private stage executor selected semantic metering.

use super::{ByteSlot, NativeBytesPlan};

impl NativeBytesPlan {
    /// Attach the metered function ordinal used to identify performed events.
    pub(in crate::codegen) fn with_semantic_function(mut self, ordinal: Option<u32>) -> Self {
        self.semantic_function = ordinal;
        self
    }

    pub(super) fn semantic_event(&self, slot: &ByteSlot) -> String {
        let (Some(function), Some(flag)) = (
            self.semantic_function,
            slot.flag.strip_prefix("spx_bytes_live_"),
        ) else {
            return String::new();
        };
        format!("spx_semantic_cleanup_event(UINT32_C({function}), UINT32_C({flag})); ")
    }
}
