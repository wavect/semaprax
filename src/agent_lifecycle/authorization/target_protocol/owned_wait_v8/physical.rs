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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_failure_stays_selected_when_handler_cancels() {
        struct FailingHandler<'a>(&'a AgentCancellation);
        impl TargetHostHandler for FailingHandler<'_> {
            fn dispatch(
                &mut self,
                _: &TargetHostRequest,
                _: &mut TargetResponseSink,
            ) -> Result<(), TargetHostError> {
                self.0.cancel();
                Err(TargetHostError::Failed)
            }
        }
        let argument = TypedCarrier::new("fixture.Argument", b"request".to_vec()).unwrap();
        let operation = TargetOperation::new(
            "fixture.agent.effect.read",
            "read",
            "fixture.Argument",
            "fixture.Result",
        )
        .unwrap();
        let grant = TargetGrant::bind_request(
            AuthorizedRequest {
                binding: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                    .into(),
                budget: 10,
                seal: b"seal".to_vec(),
            },
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            None,
            3,
            operation,
            &argument,
        );
        let limits = TargetLimits {
            max_calls: 1,
            max_request_bytes: 4096,
            max_result_bytes: 1024,
            max_total_bytes: 5120,
            max_fuel: 20,
        };
        let cancellation = AgentCancellation::new();
        let mut accounting = TargetAccounting::default();
        let run = crate::agent_lifecycle::authorization::target_protocol::dispatch(
            grant,
            argument,
            4,
            limits,
            &mut accounting,
            &cancellation,
            &mut FailingHandler(&cancellation),
        );
        assert_eq!(run.evidence().settlement(), Settlement::HostFailed);
        assert!(run.evidence().dispatched());
        assert_eq!(run.evidence().accounting(), accounting);
    }
}
