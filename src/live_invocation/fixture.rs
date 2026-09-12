//! Deterministic fixture implementations of every injected seam.
//!
//! These are the only implementations this crate ships. They exist so
//! ordinary compiler CI can exercise the full `model.invoke` boundary and
//! causal journal end to end with **no network, no provider credentials,
//! and no model spend** — every response is a byte string scripted ahead of
//! time by the test that constructs [`FixtureModelHandler`]. A real
//! provider transport, a real compiled proposal grammar, and a real
//! authorization mint are downstream integration work against the same
//! traits (`super::model_invoke`).

use std::collections::VecDeque;

use super::budget::InvocationClock;
use super::kernel::{TurnEffect, TurnObserver, TurnPolicy, TurnTransition};
use super::model_invoke::{
    AuthorizationContext, AuthorizationGate, AuthorizationGrant, AuthorizationRefusal,
    BudgetRefusal, InvocationBudgetHook, InvocationUsage, ModelHandler, ModelInvocationOutcome,
    ModelInvocationRequest, ModelInvokeCapability, ProposalDecoder, ProposalOutcome,
    ReservedBudget,
};

/// A scripted response (or failure) queue, one entry consumed per dispatched
/// `model.invoke` call. Calling [`ModelHandler::invoke`] past the end of the
/// script panics — a test that reaches this has asked for a call it did not
/// script, which is itself the point: a passing replay test proves it never
/// runs out.
pub struct FixtureModelHandler {
    script: VecDeque<ModelInvocationOutcome>,
    pub calls: usize,
}

impl FixtureModelHandler {
    #[must_use]
    pub fn scripted(script: Vec<ModelInvocationOutcome>) -> Self {
        Self {
            script: script.into(),
            calls: 0,
        }
    }

    /// A handler that panics if it is ever called — used to prove a replay
    /// makes zero dispatches.
    #[must_use]
    pub fn must_not_be_called() -> Self {
        Self {
            script: VecDeque::new(),
            calls: 0,
        }
    }
}

impl ModelHandler for FixtureModelHandler {
    fn invoke(
        &mut self,
        _capability: &ModelInvokeCapability,
        _request: &ModelInvocationRequest,
    ) -> ModelInvocationOutcome {
        self.calls += 1;
        self.script
            .pop_front()
            .unwrap_or_else(|| panic!("fixture model handler called past its scripted script"))
    }
}

/// Admits any response that round-trips as `{"turn":<n>,"answer":<bytes>}`
/// shaped JSON produced by [`fixture_response`]; refuses anything else. This
/// is a toy grammar, not the real compiler-derived proposal decode — the
/// point under test is *where* decode sits in the boundary, not what a real
/// grammar checks.
pub struct FixtureProposalDecoder {
    schema_digest: String,
}

impl FixtureProposalDecoder {
    #[must_use]
    pub fn new(schema_digest: impl Into<String>) -> Self {
        Self {
            schema_digest: schema_digest.into(),
        }
    }
}

impl ProposalDecoder for FixtureProposalDecoder {
    fn schema_digest(&self) -> &str {
        &self.schema_digest
    }

    fn decode(&mut self, turn: u32, response: &[u8]) -> ProposalOutcome {
        let expected_prefix = format!("{{\"turn\":{turn},");
        let text = match std::str::from_utf8(response) {
            Ok(text) => text,
            Err(_) => return ProposalOutcome::Refused("not_utf8".into()),
        };
        if text.starts_with(&expected_prefix) && text.ends_with('}') {
            ProposalOutcome::Admitted(response.to_vec())
        } else {
            ProposalOutcome::Refused("shape_mismatch".into())
        }
    }
}

/// Builds one scripted, admissible response for `turn`.
#[must_use]
pub fn fixture_response(turn: u32, answer: &str) -> Vec<u8> {
    format!("{{\"turn\":{turn},\"answer\":{answer:?}}}").into_bytes()
}

/// Always grants, within a fixed per-invocation call ceiling. Not a
/// reference authorization policy — the real mint is
/// `agent_lifecycle::authorization`; this fixture only proves the kernel
/// consumes exactly one grant per turn, in order.
pub struct FixtureAuthorizationGate {
    pub granted: usize,
    pub max_grants: usize,
}

impl FixtureAuthorizationGate {
    #[must_use]
    pub fn new(max_grants: usize) -> Self {
        Self {
            granted: 0,
            max_grants,
        }
    }
}

impl AuthorizationGate for FixtureAuthorizationGate {
    fn authorize(
        &mut self,
        context: &AuthorizationContext<'_>,
    ) -> Result<AuthorizationGrant, AuthorizationRefusal> {
        if self.granted >= self.max_grants {
            return Err(AuthorizationRefusal("grant_ceiling".into()));
        }
        self.granted += 1;
        Ok(AuthorizationGrant::new(super::identity::digest(
            b"semaprax.live-invocation.fixture-grant.v1\0",
            format!(
                "{}:{}:{}",
                context.turn, context.observation_digest, context.proposal_digest
            )
            .as_bytes(),
        )))
    }
}

/// A trivial per-invocation counter. Deliberately not a cumulative
/// cross-invocation budget policy: issues #113/#179 own that and attach it
/// behind [`InvocationBudgetHook`] instead of inventing a second seam.
pub struct FixtureBudgetHook {
    pub per_turn_amount: i64,
    pub reservations: usize,
    pub usage: Vec<InvocationUsage>,
}

impl FixtureBudgetHook {
    #[must_use]
    pub fn new(per_turn_amount: i64) -> Self {
        Self {
            per_turn_amount,
            reservations: 0,
            usage: Vec::new(),
        }
    }

    #[must_use]
    pub fn refusing() -> Self {
        Self {
            per_turn_amount: -1,
            reservations: 0,
            usage: Vec::new(),
        }
    }
}

impl InvocationBudgetHook for FixtureBudgetHook {
    fn reserve(
        &mut self,
        _request: &ModelInvocationRequest,
    ) -> Result<ReservedBudget, BudgetRefusal> {
        if self.per_turn_amount < 0 {
            return Err(BudgetRefusal("fixture_refuses_all".into()));
        }
        self.reservations += 1;
        Ok(ReservedBudget {
            amount: self.per_turn_amount,
        })
    }

    fn record(&mut self, usage: &InvocationUsage) {
        self.usage.push(*usage);
    }
}

/// A pure, offline observation: the turn number as a fixed-width byte
/// string. Real deployments derive this from the compiled Agent's `observe`
/// stage; this fixture only needs it to be deterministic and turn-scoped.
pub struct FixtureObserver;

impl TurnObserver for FixtureObserver {
    fn observe(&mut self, turn: u32) -> Vec<u8> {
        format!("observation:{turn}").into_bytes()
    }
}

/// Continues through `total_turns - 1`, then completes, echoing the last
/// admitted proposal as the completion payload.
pub struct FixturePolicy {
    pub total_turns: u32,
}

impl TurnPolicy for FixturePolicy {
    fn reduce(&mut self, turn: u32, proposal: &[u8]) -> TurnTransition {
        if turn.saturating_add(1) < self.total_turns {
            TurnTransition::Continue
        } else {
            TurnTransition::Complete(proposal.to_vec())
        }
    }
}

/// Echoes the grant digest back as the observed effect result. Stands in
/// for a real deployed tool call.
pub struct FixtureEffect {
    pub calls: usize,
}

impl TurnEffect for FixtureEffect {
    fn call(&mut self, _turn: u32, grant_digest: &str) -> Result<Vec<u8>, String> {
        self.calls += 1;
        Ok(grant_digest.as_bytes().to_vec())
    }
}

/// A deterministic, test-controlled [`InvocationClock`]: `now_millis` never
/// reads a real wall clock, only whatever this fixture was last told, so a
/// test can advance time by an exact amount and assert a deadline crosses at
/// that precise instant rather than racing a real clock.
pub struct StepClock {
    millis: i64,
}

impl StepClock {
    #[must_use]
    pub fn new(start_millis: i64) -> Self {
        Self {
            millis: start_millis,
        }
    }

    pub fn advance(&mut self, delta_millis: i64) {
        self.millis = self.millis.saturating_add(delta_millis);
    }
}

impl InvocationClock for StepClock {
    fn now_millis(&self) -> i64 {
        self.millis
    }
}
