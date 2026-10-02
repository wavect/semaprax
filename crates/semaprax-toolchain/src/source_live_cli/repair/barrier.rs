//! One-shot, operator-selected post-settlement pause for repair V2 smoke work.
//!
//! The wrapper is deliberately local to the physical checkpoint host. It has
//! no source, provider, effect, candidate, or publication authority.

use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use serde_json::Value;

use super::super::checkpoint::CheckpointDir;
use crate::opencode_host::OpenCodeHostConfig;

const MARKER_SCHEMA: &str = "semaprax.source-live-cli.repair-post-settled-pause.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SettledAttemptMarker {
    pub(super) generation: u64,
    pub(super) turn: u32,
    pub(super) attempt: u32,
    pub(super) response_digest: String,
}

/// A transparent checkpoint host wrapper. When armed, it pauses only after a
/// successful commit whose final causal entry is `attempt_settled`.
pub(super) struct PostSettledBarrierStore<'a> {
    inner: &'a mut CheckpointDir,
    armed: bool,
    marker_host: Option<OpenCodeHostConfig>,
}

impl<'a> PostSettledBarrierStore<'a> {
    pub(super) fn new(
        inner: &'a mut CheckpointDir,
        marker_host: Option<OpenCodeHostConfig>,
    ) -> Self {
        Self {
            inner,
            armed: marker_host.is_some() || test_hook_armed(),
            marker_host,
        }
    }
}

impl CheckpointStore for PostSettledBarrierStore<'_> {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.inner.commit(generation, document)?;
        if !self.armed {
            return Ok(());
        }
        let Some(marker) = settled_attempt_marker(generation, document)? else {
            return Ok(());
        };
        // Consume before emitting or parking. A spurious thread unpark cannot
        // create a second pause later in the same invocation.
        self.armed = false;
        if let Some(host) = self.marker_host.as_ref() {
            host.write_repair_post_settled_marker(&marker_document(&marker))
                .unwrap_or_else(|_| {
                    panic!("repair post-settled pause marker could not be persisted")
                });
        }
        run_test_hook(marker);
        if self.marker_host.is_some() {
            std::thread::park();
        }
        Ok(())
    }
}

fn settled_attempt_marker(
    generation: u64,
    document: &str,
) -> Result<Option<SettledAttemptMarker>, CheckpointStoreError> {
    let document: Value = serde_json::from_str(document).map_err(|_| CheckpointStoreError)?;
    let entries = document
        .get("entries")
        .and_then(Value::as_array)
        .ok_or(CheckpointStoreError)?;
    let Some(entry) = entries.last() else {
        return Ok(None);
    };
    if entry.get("kind").and_then(Value::as_str) != Some("attempt_settled") {
        return Ok(None);
    }
    let turn = entry
        .get("turn")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(CheckpointStoreError)?;
    let attempt = entry
        .get("attempt")
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or(CheckpointStoreError)?;
    let response_digest = entry
        .get("response_digest")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or(CheckpointStoreError)?
        .to_owned();
    Ok(Some(SettledAttemptMarker {
        generation,
        turn,
        attempt,
        response_digest,
    }))
}

fn marker_document(marker: &SettledAttemptMarker) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({
        "schema": MARKER_SCHEMA,
        "checkpoint_generation": marker.generation,
        "turn": marker.turn,
        "attempt": marker.attempt,
        "response_digest": marker.response_digest,
    }))
    .expect("repair pause marker is bounded canonical JSON")
}

#[cfg(test)]
std::thread_local! {
    static TEST_POST_SETTLED_HOOK: std::cell::RefCell<Option<Box<dyn FnOnce(SettledAttemptMarker)>>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(super) fn set_test_post_settled_hook(hook: impl FnOnce(SettledAttemptMarker) + 'static) {
    TEST_POST_SETTLED_HOOK.with(|slot| {
        assert!(slot.borrow().is_none(), "post-settled hook is already set");
        *slot.borrow_mut() = Some(Box::new(hook));
    });
}

#[cfg(test)]
fn test_hook_armed() -> bool {
    TEST_POST_SETTLED_HOOK.with(|slot| slot.borrow().is_some())
}

#[cfg(not(test))]
fn test_hook_armed() -> bool {
    false
}

#[cfg(test)]
fn run_test_hook(marker: SettledAttemptMarker) {
    TEST_POST_SETTLED_HOOK.with(|slot| {
        if let Some(hook) = slot.borrow_mut().take() {
            hook(marker);
        }
    });
}

#[cfg(not(test))]
fn run_test_hook(_: SettledAttemptMarker) {}
