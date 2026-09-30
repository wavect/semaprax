//! One checked SDK Proposal projection and its two immutable commitments.
//! Copy data never grants owner restoration, authorization or dispatch.
use super::CheckedOwnedAgentWaitBindingV8;
use crate::agent_proposal::DecodedProposal;
use crate::diagnostic::Diagnostic;
use crate::interpreter::resumable::{channel, checkpoint, ResumableChannelValue};
use crate::resumable_effects::owned_frame::codec;
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;
use serde_json::{json, Value};
#[derive(Clone)]
pub(crate) struct CheckedOwnedWaitProposalV8 {
    binding: String,
    scope: Value,
    ordinary_digest: String,
    answer_digest: String,
    value: Value,
    carrier: ResumableChannelValue,
    canonical_proposal: String,
}
impl CheckedOwnedWaitProposalV8 {
    pub(crate) fn matches(&self, binding: &str, scope: &Value) -> bool {
        self.binding == binding && &self.scope == scope
    }
    pub(crate) fn ordinary_digest(&self) -> &str {
        &self.ordinary_digest
    }
    pub(crate) fn answer_digest(&self) -> &str {
        &self.answer_digest
    }
    pub(crate) fn value(&self) -> &Value {
        &self.value
    }
    pub(crate) fn carrier(&self) -> &ResumableChannelValue {
        &self.carrier
    }
    pub(crate) fn canonical_proposal(&self) -> &str {
        &self.canonical_proposal
    }
    pub(crate) fn result_digest(&self, argument_digest: &str) -> Result<String, Diagnostic> {
        if !codec::is_digest(argument_digest) {
            return Err(refused());
        }
        Ok(codec::fact_digest(
            b"semaprax.source-owned-frame-result.v2\0",
            &json!({"argument_digest":argument_digest,"answer_digest":self.answer_digest}),
        ))
    }
}
fn refused() -> Diagnostic {
    Diagnostic::io("SPX-G583", "source owned Agent wait Proposal refused")
}
pub(crate) fn bind_owned_wait_proposal_v8(
    binding: &CheckedOwnedAgentWaitBindingV8,
    scope: &SourceCheckpointScope,
    decoded: &DecodedProposal,
) -> Result<CheckedOwnedWaitProposalV8, Diagnostic> {
    let carrier = binding
        .lifecycle()
        .proposal_schema()
        .model_wait_carrier(decoded)
        .ok_or_else(refused)?;
    let helper = binding.helper();
    if !channel::valid_copy_channel_response(
        helper.program(),
        helper.function().id.as_str(),
        &carrier,
    ) {
        return Err(refused());
    }
    let ResumableChannelValue::Record { fields, .. } = &carrier else {
        return Err(refused());
    };
    for field in fields {
        codec::scalar(field).map_err(|_| refused())?;
    }
    let value = checkpoint::channel_json(&carrier);
    if codec::canonical(&value).len() > codec::MAX_CARRIER {
        return Err(refused());
    }
    let scope = codec::scope(scope).map_err(|_| refused())?;
    let ordinary_digest = crate::live_invocation::identity::digest(
        b"semaprax.source-proposal.v2\0",
        decoded.canonical_json().as_bytes(),
    );
    let answer_digest = codec::fact_digest(
        b"semaprax.source-owned-frame-answer.v2\0",
        &json!({"scope":scope,"plan_digest":binding.binding(),"value":value}),
    );
    Ok(CheckedOwnedWaitProposalV8 {
        binding: binding.binding().into(),
        scope,
        ordinary_digest,
        answer_digest,
        value,
        carrier,
        canonical_proposal: decoded.canonical_json().into(),
    })
}
#[cfg(test)]
mod tests;
