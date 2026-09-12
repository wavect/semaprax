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
use super::migration::LiveStateMigration;
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

/// A deterministic checked "pure" state migration: appends a fixed suffix
/// to whatever state bytes it is given. Real deployments bind
/// `LiveStateMigration` to a compiler-checked pure function over retained
/// source (see `execution_revision::typed_migration::evaluate_migration`
/// for the real mechanism); this fixture only proves *where* the checked
/// boundary sits and that it is actually called (`calls`), never a claim
/// about real state-schema evolution.
pub struct FixtureStateMigration {
    pub suffix: Vec<u8>,
    pub calls: usize,
}

impl FixtureStateMigration {
    #[must_use]
    pub fn appending(suffix: impl Into<Vec<u8>>) -> Self {
        Self {
            suffix: suffix.into(),
            calls: 0,
        }
    }
}

impl LiveStateMigration for FixtureStateMigration {
    fn migrate(&mut self, previous_state: &[u8]) -> Result<Vec<u8>, String> {
        self.calls += 1;
        let mut migrated = previous_state.to_vec();
        migrated.extend_from_slice(&self.suffix);
        Ok(migrated)
    }
}

/// A migration that answers differently on every call — proves
/// `migration::migrate_live_invocation`'s double-evaluation check actually
/// rejects an impure or effectful migration rather than trusting whatever
/// the first call returns.
pub struct FixtureNondeterministicStateMigration {
    pub calls: usize,
}

impl FixtureNondeterministicStateMigration {
    #[must_use]
    pub fn new() -> Self {
        Self { calls: 0 }
    }
}

impl Default for FixtureNondeterministicStateMigration {
    fn default() -> Self {
        Self::new()
    }
}

impl LiveStateMigration for FixtureNondeterministicStateMigration {
    fn migrate(&mut self, previous_state: &[u8]) -> Result<Vec<u8>, String> {
        self.calls += 1;
        let mut migrated = previous_state.to_vec();
        migrated.push(u8::try_from(self.calls % 256).unwrap_or(0));
        Ok(migrated)
    }
}

/// A migration that always refuses, with a fixed closed reason. Proves a
/// migration function's own refusal reaches
/// `migration::LiveMigrationError::MigrationRefused` unchanged.
pub struct FixtureRefusingStateMigration;

impl LiveStateMigration for FixtureRefusingStateMigration {
    fn migrate(&mut self, _previous_state: &[u8]) -> Result<Vec<u8>, String> {
        Err("fixture_refuses_all_migrations".into())
    }
}

/// A deterministic checked "pure" state migration that, unlike
/// [`FixtureStateMigration`], declares exactly which
/// `(previous_schema, destination_schema)` pairs it is checked to
/// interpret ([`LiveStateMigration::known_schema_transitions`]) — the
/// rich-schema shape a real compiler-checked migration function actually
/// has (bound against one specific old schema and one specific new
/// schema, never "any bytes to any bytes"). Migrating against a pair
/// outside its declared set must be refused by
/// `migration::migrate_live_invocation` before `migrate` is ever called;
/// `calls` staying at zero after such a refusal is what proves that.
pub struct FixtureSchemaBoundStateMigration {
    pub suffix: Vec<u8>,
    pub known: Vec<(String, String)>,
    pub calls: usize,
}

impl FixtureSchemaBoundStateMigration {
    #[must_use]
    pub fn bound_to(known: Vec<(String, String)>, suffix: impl Into<Vec<u8>>) -> Self {
        Self {
            suffix: suffix.into(),
            known,
            calls: 0,
        }
    }
}

impl LiveStateMigration for FixtureSchemaBoundStateMigration {
    fn migrate(&mut self, previous_state: &[u8]) -> Result<Vec<u8>, String> {
        self.calls += 1;
        let mut migrated = previous_state.to_vec();
        migrated.extend_from_slice(&self.suffix);
        Ok(migrated)
    }

    fn known_schema_transitions(&self) -> Option<&[(String, String)]> {
        Some(&self.known)
    }
}
