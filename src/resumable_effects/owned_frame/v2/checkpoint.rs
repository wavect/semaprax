//! Authenticated inert v2 checkpoint. This type cannot restore an owner.
use super::{CheckedOwnedAgentWaitBindingV8, CheckedOwnedWaitObservationV8};
use crate::resumable_effects::owned_frame::{codec, OwnedFrameError as Error};
use crate::resumable_effects::source_checkpoint::{SourceCheckpointKey, SourceCheckpointScope};
use serde_json::{json, Value};
const SCHEMA: &str = "semaprax.source-owned-frame-checkpoint.v2";
const AUTH: &[u8] = b"semaprax.source-owned-frame-checkpoint-authentication.v2\0";

pub(crate) struct OwnedWaitCheckpointExpectationV8<'a> {
    pub(crate) scope: &'a SourceCheckpointScope,
    pub(crate) argument_digest: &'a str,
    pub(crate) observation: &'a CheckedOwnedWaitObservationV8,
    /// True causal sequence of the Prepared row, never a local wait ordinal.
    pub(crate) sequence: u64,
    pub(crate) reserved_total: u64,
    /// Recorded consumption lower bound at this exact Prepared row.
    pub(crate) consumed_total: u64,
}
/// Sealed parsing/typing evidence only. Bytes and State here are inert data;
/// the held lease and live/recovery owner choreography remain separate.
pub(crate) struct CheckedOwnedWaitCheckpointV8 {
    payload: Value,
    checkpoint_digest: String,
    outer_digest: String,
}
impl CheckedOwnedWaitCheckpointV8 {
    pub(crate) fn payload(&self) -> &Value {
        &self.payload
    }
    pub(crate) fn checkpoint_digest(&self) -> &str {
        &self.checkpoint_digest
    }
    pub(crate) fn outer_digest(&self) -> &str {
        &self.outer_digest
    }
}
fn frame(
    binding: &CheckedOwnedAgentWaitBindingV8,
    argument: &Value,
    copy_arguments: &Value,
) -> Result<Value, Error> {
    super::validate_owned_wait_state_v8(binding, argument)?;
    let plan = binding.helper().liveness();
    let mut root = argument.clone();
    let root_map = root.as_object_mut().ok_or(Error::Malformed)?;
    root_map.insert("storage".into(), codec::storage(&plan.storage)?);
    root_map.insert("leaf_flags".into(),json!(plan.leaves.iter().map(|leaf|
        json!({"field":leaf.field.as_str(),"flag":leaf.flag.0,"live":true,"lifecycle":leaf.lifecycle.as_str()})).collect::<Vec<_>>()));
    // Preserve the frozen v1 structural root representation, including its
    // restricted renderer. New Decision/partial cleanup uses a separate codec.
    root_map.insert(
        "suspension_cleanup".into(),
        codec::operations(&plan.suspension_cleanup)?,
    );
    root_map.insert(
        "failure_cleanup".into(),
        codec::operations(&plan.failure_cleanup)?,
    );
    root_map.insert(
        "completion_cleanup".into(),
        codec::operations(&plan.completion_cleanup)?,
    );
    if codec::canonical(&root).len() > codec::MAX_CARRIER {
        return Err(Error::Capacity);
    }
    Ok(json!({"owned_root":root,"copy_arguments":copy_arguments}))
}
pub(crate) fn validate_owned_wait_checkpoint_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
    key: &SourceCheckpointKey,
    expected: &OwnedWaitCheckpointExpectationV8<'_>,
    bytes: &[u8],
) -> Result<CheckedOwnedWaitCheckpointV8, Error> {
    let envelope = codec::parse(bytes, 65536)?;
    codec::keys(&envelope, &["payload", "authentication"])?;
    let payload = &envelope["payload"];
    codec::keys(
        payload,
        &[
            "schema",
            "scope",
            "plan_digest",
            "cleanup_plan_digest",
            "signature",
            "argument_digest",
            "copy_arguments_digest",
            "frame",
            "frame_digest",
            "request",
            "request_digest",
            "reserved_total",
            "consumed_total",
            "sequence",
        ],
    )?;
    let authentication = codec::unhex(codec::text(&envelope["authentication"], 64)?, 32)?;
    if authentication.len() != 32 {
        return Err(Error::Malformed);
    }
    if !key.verify(AUTH, &codec::canonical(payload), &authentication) {
        return Err(Error::Authentication);
    }
    let mut canonical = codec::canonical(&envelope);
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(Error::Malformed);
    }
    let scope = codec::scope(expected.scope)?;
    if !expected.observation.matches(binding.binding(), &scope)
        || !codec::is_digest(expected.argument_digest)
        || expected.consumed_total > expected.reserved_total
        || payload["schema"] != SCHEMA
        || payload["scope"] != scope
        || payload["plan_digest"] != binding.binding()
        || payload["cleanup_plan_digest"] != binding.cleanup_digest()
        || payload["signature"] != *binding.signature()
        || payload["argument_digest"] != expected.argument_digest
        || payload["sequence"] != expected.sequence
        || payload["reserved_total"] != expected.reserved_total
        || payload["consumed_total"] != expected.consumed_total
    {
        return Err(Error::Binding);
    }
    let actual_frame = &payload["frame"];
    codec::keys(actual_frame, &["owned_root", "copy_arguments"])?;
    let root = &actual_frame["owned_root"];
    codec::keys(
        root,
        &[
            "declaration",
            "fields",
            "storage",
            "leaf_flags",
            "suspension_cleanup",
            "failure_cleanup",
            "completion_cleanup",
        ],
    )?;
    let argument = json!({"declaration":root["declaration"],"fields":root["fields"]});
    let copy = expected.observation.copy_arguments();
    if *actual_frame != frame(binding, &argument, copy)?
        || payload["argument_digest"]
            != codec::fact_digest(b"semaprax.source-owned-frame-args.v2\0", &argument)
        || payload["copy_arguments_digest"]
            != codec::fact_digest(b"semaprax.source-owned-frame-copy-args.v2\0", copy)
        || payload["frame_digest"]
            != codec::fact_digest(b"semaprax.source-owned-frame-frame.v2\0", actual_frame)
        || payload["request"] != copy[0]["value"]
        || payload["request_digest"] != expected.observation.request_digest()
    {
        return Err(Error::Binding);
    }
    Ok(CheckedOwnedWaitCheckpointV8 {
        checkpoint_digest: codec::fact_digest(
            b"semaprax.source-owned-frame-checkpoint.v2\0",
            payload,
        ),
        outer_digest: codec::digest(b"semaprax.source-agent-owned-wait.checkpoint.v1\0", bytes),
        payload: payload.clone(),
    })
}
#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn test_encode_owned_wait_checkpoint_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
    key: &SourceCheckpointKey,
    expected: &OwnedWaitCheckpointExpectationV8<'_>,
    argument: &Value,
) -> Vec<u8> {
    tests::sign(key, tests::payload(binding, expected, argument))
}
