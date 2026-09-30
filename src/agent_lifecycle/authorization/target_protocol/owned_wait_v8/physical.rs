//! Metadata grant construction consumes the sealed physical owner's one-use
//! dispatch permit. Inert commitments/request digests grant no dispatch.
use super::super::*;
use crate::interpreter::resumable::owned_frame::registered_stage::effect::{
    OwnedEffectDispatchPermitV8, OwnedEffectTargetRequestV8,
};

/// Exact existing request v2 bytes with one host-call work unit. Pure data.
pub(crate) fn request_digest(data: &OwnedEffectTargetRequestV8) -> String {
    request_digest_fields(
        data.grant(),
        data.authorization(),
        data.operation(),
        data.argument(),
        data.turn(),
    )
}
/// Descriptive frozen recipe for the journal validator; raw strings cannot
/// create a request, grant, dispatch permit or physical owner through this API.
pub(crate) fn request_digest_fields(
    grant: &str,
    authorization: &str,
    operation: &TargetOperation,
    argument: &TypedCarrier,
    turn: u64,
) -> String {
    let request = TargetHostRequest {
        grant_id: grant.into(),
        authorization_binding: authorization.into(),
        operation: operation.clone(),
        turn,
        argument: argument.clone(),
        fuel: 1,
    };
    digest(REQUEST_DOMAIN, &request.canonical_wire())
}
pub(crate) fn dispatch(
    permit: OwnedEffectDispatchPermitV8,
    accounting: &mut TargetAccounting,
    cancellation: &AgentCancellation,
    handler: &mut dyn TargetHostHandler,
) -> TargetDispatch {
    let (data, argument_digest, budget) = permit.consume();
    let grant = TargetGrant {
        grant_id: data.grant().into(),
        authorization_binding: data.authorization().into(),
        operation: data.operation().clone(),
        argument_digest,
        turn: data.turn(),
        granted_budget: budget,
    };
    // The permit has no Clone or public/raw constructor, and is consumed here.
    // Actual State and Decision stay in the calling engine's opaque holder.
    super::super::dispatch(
        grant,
        data.argument().clone(),
        1,
        data.limits(),
        accounting,
        cancellation,
        handler,
    )
}
