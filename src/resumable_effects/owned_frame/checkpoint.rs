//! Authenticated inert checkpoint. Decoding cannot restore an owner.
use super::{codec, CheckedOwnedFramePlan, OwnedFrameError as Error};
use crate::interpreter::resumable::owned_frame::OwnedFrameInput;
use crate::interpreter::ArgumentValue;
use crate::resumable_effects::source_checkpoint::{SourceCheckpointKey, SourceCheckpointScope};
use serde_json::{json, Value};
const SCHEMA: &str = "semaprax.source-owned-frame-checkpoint.v1";
const MAC_DOMAIN: &[u8] = b"semaprax.source-owned-frame-checkpoint-authentication.v1\0";

pub(crate) struct InertOwnedFrameCheckpoint {
    pub(super) input: OwnedFrameInput,
    pub(super) request: ArgumentValue,
    pub(super) sequence: u64,
    pub(super) reserved_total: u64,
    pub(super) bytes: Vec<u8>,
}
#[allow(clippy::too_many_arguments)]
pub(super) fn encode(
    plan: &CheckedOwnedFramePlan,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    input: &OwnedFrameInput,
    argument_digest: &str,
    request: &ArgumentValue,
    generation: &str,
    sequence: u64,
    reserved_total: u64,
) -> Result<Vec<u8>, Error> {
    if !crate::interpreter::resumable::owned_frame::snapshot::request_valid(plan, request)
        || codec::fact_digest(
            b"semaprax.source-owned-frame-arguments.v1\0",
            &codec::input(plan, input)?,
        ) != argument_digest
        || !codec::is_digest(generation)
    {
        return Err(Error::Binding);
    }
    let mut value = json!({"schema":SCHEMA,"scope":codec::scope(scope)?,"function":plan.function().id.as_str(),"plan_digest":plan.binding(),"signature":codec::signature(plan),"argument_digest":argument_digest,"frame":codec::frame(plan,input)?,"site":plan.liveness().site.as_str(),"request":codec::scalar(request)?,"journal_generation":generation,"journal_sequence":sequence,"reserved_total":reserved_total});
    let mac = key.authenticate(MAC_DOMAIN, &codec::canonical(&value));
    value
        .as_object_mut()
        .expect("envelope")
        .insert("authentication".into(), json!(codec::hex(&mac)));
    let mut bytes = codec::canonical(&value);
    bytes.push(b'\n');
    if bytes.len() > codec::MAX_CHECKPOINT {
        return Err(Error::Capacity);
    }
    Ok(bytes)
}
#[allow(clippy::too_many_arguments)]
pub(super) fn decode(
    plan: &CheckedOwnedFramePlan,
    key: &SourceCheckpointKey,
    scope: &SourceCheckpointScope,
    argument_digest: &str,
    generation: &str,
    sequence: u64,
    reserved_total: u64,
    bytes: &[u8],
) -> Result<InertOwnedFrameCheckpoint, Error> {
    let mut value = codec::parse(bytes, codec::MAX_CHECKPOINT)?;
    codec::keys(
        &value,
        &[
            "schema",
            "scope",
            "function",
            "plan_digest",
            "signature",
            "argument_digest",
            "frame",
            "site",
            "request",
            "journal_generation",
            "journal_sequence",
            "reserved_total",
            "authentication",
        ],
    )?;
    let mac = codec::unhex(codec::text(&value["authentication"], 64)?, 32)?;
    if mac.len() != 32 {
        return Err(Error::Malformed);
    }
    value
        .as_object_mut()
        .expect("object")
        .remove("authentication");
    if !key.verify(MAC_DOMAIN, &codec::canonical(&value), &mac) {
        return Err(Error::Authentication);
    }
    if value["schema"] != SCHEMA
        || value["scope"] != codec::scope(scope)?
        || value["function"] != plan.function().id.as_str()
        || value["plan_digest"] != plan.binding()
        || value["signature"] != codec::signature(plan)
        || value["argument_digest"] != argument_digest
        || value["site"] != plan.liveness().site.as_str()
        || value["journal_generation"] != generation
        || value["journal_sequence"] != sequence
        || value["reserved_total"] != reserved_total
    {
        return Err(Error::Binding);
    }
    let input = codec::decode_frame(plan, &value["frame"])?;
    let request = codec::decode_scalar(&value["request"])?;
    if encode(
        plan,
        key,
        scope,
        &input,
        argument_digest,
        &request,
        generation,
        sequence,
        reserved_total,
    )? != bytes
    {
        return Err(Error::Malformed);
    }
    Ok(InertOwnedFrameCheckpoint {
        input,
        request,
        sequence,
        reserved_total,
        bytes: bytes.to_vec(),
    })
}
pub(super) fn digest(bytes: &[u8]) -> String {
    codec::digest(b"semaprax.source-owned-frame-checkpoint.v1\0", bytes)
}

#[cfg(test)]
mod tests;
