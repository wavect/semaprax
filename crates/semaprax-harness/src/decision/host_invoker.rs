//! [`DecisionInvoker`] over an adapter handle. The request's own deadline
//! (derived by the router from its latency ceiling) is enforced by the host,
//! which kills the adapter's process group when it passes; this type adds no
//! second clock of its own beyond measuring elapsed time.

use super::call::CallMetadata;
use super::provider::{DecisionCall, DecisionInvoker};
use crate::contract::{CapabilityKind, RequestEnvelope};
use crate::host::{AdapterHandle, CancelToken, InvocationClass, Outcome};
use std::sync::Arc;
use std::time::Instant;

pub struct HostDecisionInvoker {
    handle: Arc<AdapterHandle>,
    cancel: CancelToken,
    /// Invocations sent to the adapter.
    pub calls: u32,
}

impl HostDecisionInvoker {
    pub fn new(handle: Arc<AdapterHandle>) -> Self {
        Self {
            handle,
            cancel: CancelToken::new(),
            calls: 0,
        }
    }

    /// Token that cancels an in-flight evaluation.
    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }
}

impl DecisionInvoker for HostDecisionInvoker {
    fn evaluate(&mut self, request: &RequestEnvelope) -> DecisionCall {
        self.calls += 1;
        let started = Instant::now();
        match self
            .handle
            .invoke(request, InvocationClass::Decision, &self.cancel)
        {
            Outcome::Completed(r) => match r.payload {
                Some(result) => DecisionCall::Answered {
                    // Typed identity/usage from the payload's `call` member
                    // (MR-03); malformed metadata is refused by the router.
                    call: result
                        .get("call")
                        .and_then(|c| CallMetadata::from_json(c).ok()),
                    result,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                },
                None => DecisionCall::Unavailable,
            },
            Outcome::Unavailable { reason, .. } if reason.code == "SPX-HPC008" => {
                DecisionCall::Timeout
            }
            _ => DecisionCall::Unavailable,
        }
    }

    fn decision_versions(&self) -> Vec<u32> {
        self.handle
            .negotiated_versions(CapabilityKind::DecisionEvaluate)
    }
}
