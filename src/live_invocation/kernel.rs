//! The one runtime kernel `run_live_invocation` drives every live
//! invocation through — a fresh start, a resumed run, and a full replay are
//! all the same call with a different starting journal.
//!
//! Passing an empty journal starts turn 0 fresh. Passing a journal that
//! ends cleanly after a `continue` transition resumes at the next turn,
//! consuming the recorded prefix without redispatching it. Passing an
//! already-terminal journal replays it: [`journal::validate`] recognises the
//! trailing `TerminalOutcome`, the kernel returns its outcome immediately,
//! and [`LiveKernelRun::dispatched`] is `0` because no turn loop ever runs.
//! A journal ending immediately after a `RequestIntent` with no recorded
//! response is uncertain delivery, and is refused before any stage, host
//! call, or new journal write — never silently redispatched.

use crate::agent_runtime::AgentCancellation;

use super::identity::{digest, LiveInvocationId};
use super::journal::{self, JournalEntry};
use super::model_invoke::{
    AuthorizationContext, AuthorizationGate, InvocationBudgetHook, InvocationUsage, ModelFailure,
    ModelHandler, ModelInvocationOutcome, ModelInvocationRequest, ModelInvokeCapability,
    ProposalDecoder, ProposalOutcome,
};

const TRANSITION_DOMAIN: &[u8] = b"semaprax.live-invocation.transition-carrier.v1\0";
const OBSERVATION_DOMAIN: &[u8] = b"semaprax.live-invocation.observation.v1\0";
const RESPONSE_DOMAIN: &[u8] = b"semaprax.live-invocation.response.v1\0";
const PROPOSAL_DOMAIN: &[u8] = b"semaprax.live-invocation.proposal.v1\0";
const EFFECT_REQUEST_DOMAIN: &[u8] = b"semaprax.live-invocation.effect-request.v1\0";
const EFFECT_OBSERVATION_DOMAIN: &[u8] = b"semaprax.live-invocation.effect-observation.v1\0";

/// Produces one turn's deterministic observation/context projection. Pure
/// and offline: no model or effect call may happen here.
pub trait TurnObserver {
    fn observe(&mut self, turn: u32) -> Vec<u8>;
}

/// The deterministic transition one turn's admitted proposal selects. A
/// stand-in for the real reducer this kernel is deliberately agnostic to;
/// downstream integration binds `reduce` to the compiled Agent function the
/// same way [`crate::agent_lifecycle::iterative`] does today.
pub enum TurnTransition {
    Continue,
    Complete(Vec<u8>),
    Suspend(Vec<u8>),
    Fail(Vec<u8>),
}

/// Selects the next transition from an admitted proposal.
pub trait TurnPolicy {
    fn reduce(&mut self, turn: u32, proposal: &[u8]) -> TurnTransition;
}

/// One optional further external effect within a turn, after the model step
/// authorizes it. A real deployment binds this to
/// [`crate::agent_lifecycle::iterative::effects::TypedEffectHandler`]; this
/// kernel only needs a byte-shaped seam to record the same
/// `EffectIntent`/`EffectObserved` journal pair a tool call would produce.
pub trait TurnEffect {
    fn call(&mut self, turn: u32, grant_digest: &str) -> Result<Vec<u8>, String>;
}

/// The invocation-level result once the journal stops advancing.
///
/// The journal itself never stores a terminal carrier's raw bytes, only its
/// digest — the same "digest, without exposing payloads" discipline
/// [`crate::agent_lifecycle::iterative`]'s evidence already uses. A *fresh*
/// run therefore returns the real payload bytes here (the kernel has them
/// in hand), but replaying an already-terminal journal returns this same
/// enum with an empty payload: replay reproduces the terminal *case*
/// (and, via the journal's `carrier_digest`, a checkable commitment to the
/// original bytes) without re-deriving bytes the journal was never asked to
/// keep.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveInvocationOutcome {
    Complete(Vec<u8>),
    Suspend(Vec<u8>),
    Fail(Vec<u8>),
    /// Cancellation was observed before a turn's boundary was crossed. The
    /// journal is left non-terminal; nothing about this turn was committed.
    Cancelled,
}

/// A hard kernel refusal: something the kernel will not do regardless of
/// what a handler, decoder, or policy would otherwise decide.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LiveKernelError {
    /// The supplied journal does not causally validate against `identity`.
    InvalidJournal(journal::JournalError),
    /// The journal ends with an uncertain, undelivered request. Refused
    /// before any stage, store write, or host call.
    UncertainIntent,
    /// The journal ends mid-turn in a state this bounded kernel does not
    /// resume from (decode/authorize/effect pending). A real deployment
    /// reconciles this out of band before calling in again; this kernel
    /// makes no attempt to guess the missing entries.
    UnresolvedPrefix,
    /// The decoder's bound schema digest does not match this invocation's
    /// interaction schema. Refused before any dispatch.
    SchemaDrift,
}

/// One kernel run's result.
pub struct LiveKernelRun {
    pub journal: Vec<JournalEntry>,
    pub outcome: LiveInvocationOutcome,
    /// The number of `ModelHandler::invoke` calls *this call* made. `0` on
    /// a full replay of an already-terminal journal, or when the run stops
    /// at cancellation before any turn's request intent is committed.
    pub dispatched: usize,
}

fn carrier_digest(bytes: &[u8]) -> String {
    digest(TRANSITION_DOMAIN, bytes)
}

/// Exact length up to `cap`; anything larger charges the bounded sentinel
/// `cap + 1` rather than the true size, matching the existing typed-effects
/// convention of never letting an oversized carrier drive unbounded work.
fn measure(bytes: &[u8], cap: usize) -> usize {
    bytes.len().min(cap.saturating_add(1))
}

/// Every non-response-shaped input a turn's `model.invoke` request needs,
/// grouped so `run_live_invocation`'s own signature stays readable.
pub struct LiveInvocationConfig<'a> {
    pub identity: &'a LiveInvocationId,
    pub task: &'a [u8],
    pub deployment_binding: &'a str,
    pub interaction_schema_digest: &'a str,
    pub max_turns: u32,
    pub max_response_bytes: usize,
    pub requested_budget_per_turn: i64,
}

/// Every injected seam one call needs. Grouped for the same reason as
/// [`LiveInvocationConfig`]: this function has many independent
/// responsibilities (model, decode, authorize, budget, observe, reduce, and
/// an optional further effect), each deliberately its own trait so a real
/// deployment can bind each to its own real implementation.
pub struct LiveInvocationHandlers<'a> {
    pub capability: &'a ModelInvokeCapability,
    pub handler: &'a mut dyn ModelHandler,
    pub decoder: &'a mut dyn ProposalDecoder,
    pub gate: &'a mut dyn AuthorizationGate,
    pub budget: &'a mut dyn InvocationBudgetHook,
    pub observer: &'a mut dyn TurnObserver,
    pub policy: &'a mut dyn TurnPolicy,
    pub effect: Option<&'a mut dyn TurnEffect>,
}

pub fn run_live_invocation(
    config: &LiveInvocationConfig<'_>,
    mut journal: Vec<JournalEntry>,
    handlers: &mut LiveInvocationHandlers<'_>,
    cancellation: &AgentCancellation,
) -> Result<LiveKernelRun, LiveKernelError> {
    if handlers.decoder.schema_digest() != config.interaction_schema_digest {
        return Err(LiveKernelError::SchemaDrift);
    }

    let mut turn = if journal.is_empty() {
        0u32
    } else {
        let validated = journal::validate(&journal, config.identity.digest())
            .map_err(LiveKernelError::InvalidJournal)?;
        if validated.terminal {
            let outcome = terminal_outcome_of(&journal);
            return Ok(LiveKernelRun {
                journal,
                outcome,
                dispatched: 0,
            });
        }
        if validated.uncertain_intent {
            return Err(LiveKernelError::UncertainIntent);
        }
        validated
            .resumable_turn
            .ok_or(LiveKernelError::UnresolvedPrefix)?
    };

    let mut dispatched = 0usize;
    loop {
        if cancellation.is_cancelled() {
            return Ok(LiveKernelRun {
                journal,
                outcome: LiveInvocationOutcome::Cancelled,
                dispatched,
            });
        }
        let observation = handlers.observer.observe(turn);
        let observation_digest = digest(OBSERVATION_DOMAIN, &observation);
        journal.push(JournalEntry::TurnOpened {
            turn,
            invocation: config.identity.digest().to_owned(),
            observation_digest: observation_digest.clone(),
        });

        if cancellation.is_cancelled() {
            return Ok(LiveKernelRun {
                journal,
                outcome: LiveInvocationOutcome::Cancelled,
                dispatched,
            });
        }
        let mut request = ModelInvocationRequest {
            turn,
            task: config.task.to_vec(),
            observation,
            proposal_grammar_digest: config.interaction_schema_digest.to_owned(),
            deployment_binding: config.deployment_binding.to_owned(),
            max_response_bytes: config.max_response_bytes,
            effective_budget: config.requested_budget_per_turn,
        };
        let reserved = handlers.budget.reserve(&request);
        let reserved_budget = match reserved {
            Ok(reserved) => reserved.amount,
            Err(_refusal) => {
                journal.push(JournalEntry::RequestIntent {
                    turn,
                    request_digest: request.digest(),
                    reserved_budget: 0,
                });
                journal.push(JournalEntry::ResponseFailed {
                    turn,
                    failure: ModelFailure::CapacityExceeded.as_str().to_owned(),
                    attempted_bytes: 0,
                });
                let mut run = finish(
                    journal,
                    turn,
                    TurnTransition::Fail(b"budget_refused".to_vec()),
                );
                run.dispatched = dispatched;
                return Ok(run);
            }
        };
        request.effective_budget = reserved_budget;
        let request_digest = request.digest();
        journal.push(JournalEntry::RequestIntent {
            turn,
            request_digest,
            reserved_budget,
        });

        let mut outcome = if cancellation.is_cancelled() {
            ModelInvocationOutcome::Failed {
                failure: ModelFailure::Cancelled,
                attempted_bytes: 0,
            }
        } else {
            dispatched += 1;
            handlers.handler.invoke(handlers.capability, &request)
        };
        if let ModelInvocationOutcome::Settled(response) = &outcome {
            if response.len() > config.max_response_bytes {
                outcome = ModelInvocationOutcome::Failed {
                    failure: ModelFailure::MalformedResponse,
                    attempted_bytes: measure(response, config.max_response_bytes),
                };
            }
        }

        let response = match outcome {
            ModelInvocationOutcome::Settled(response) => {
                let response_digest = digest(RESPONSE_DOMAIN, &response);
                journal.push(JournalEntry::ResponseRecorded {
                    turn,
                    response_digest,
                    response: response.clone(),
                });
                handlers.budget.record(&InvocationUsage {
                    turn,
                    request_bytes: request.observation.len(),
                    response_bytes: response.len(),
                    failed: false,
                });
                response
            }
            ModelInvocationOutcome::Failed {
                failure,
                attempted_bytes,
            } => {
                journal.push(JournalEntry::ResponseFailed {
                    turn,
                    failure: failure.as_str().to_owned(),
                    attempted_bytes,
                });
                handlers.budget.record(&InvocationUsage {
                    turn,
                    request_bytes: request.observation.len(),
                    response_bytes: 0,
                    failed: true,
                });
                let mut run = finish(
                    journal,
                    turn,
                    TurnTransition::Fail(b"model_call_failed".to_vec()),
                );
                run.dispatched = dispatched;
                return Ok(run);
            }
        };

        let proposal = match handlers.decoder.decode(turn, &response) {
            ProposalOutcome::Admitted(bytes) => {
                let proposal_digest = digest(PROPOSAL_DOMAIN, &bytes);
                journal.push(JournalEntry::ProposalAdmitted {
                    turn,
                    proposal_digest,
                });
                bytes
            }
            ProposalOutcome::Refused(reason) => {
                journal.push(JournalEntry::ProposalRefused { turn, reason });
                let mut run = finish(
                    journal,
                    turn,
                    TurnTransition::Fail(b"proposal_refused".to_vec()),
                );
                run.dispatched = dispatched;
                return Ok(run);
            }
        };
        let proposal_digest = digest(PROPOSAL_DOMAIN, &proposal);

        let grant = match handlers.gate.authorize(&AuthorizationContext {
            turn,
            observation_digest: &observation_digest,
            proposal_digest: &proposal_digest,
        }) {
            Ok(grant) => {
                journal.push(JournalEntry::AuthorizationConsumed {
                    turn,
                    grant_digest: grant.digest().to_owned(),
                });
                grant
            }
            Err(_refusal) => {
                let mut run = finish(
                    journal,
                    turn,
                    TurnTransition::Fail(b"authorization_refused".to_vec()),
                );
                run.dispatched = dispatched;
                return Ok(run);
            }
        };

        if let Some(effect) = handlers.effect.as_mut() {
            let operation = "live-invocation.turn-effect".to_owned();
            let request_digest = digest(EFFECT_REQUEST_DOMAIN, grant.digest().as_bytes());
            journal.push(JournalEntry::EffectIntent {
                turn,
                operation: operation.clone(),
                request_digest,
            });
            match effect.call(turn, grant.digest()) {
                Ok(observed) => {
                    let observation_digest = digest(EFFECT_OBSERVATION_DOMAIN, &observed);
                    journal.push(JournalEntry::EffectObserved {
                        turn,
                        operation,
                        observation_digest,
                    });
                }
                Err(_) => {
                    let mut run = finish(
                        journal,
                        turn,
                        TurnTransition::Fail(b"effect_failed".to_vec()),
                    );
                    run.dispatched = dispatched;
                    return Ok(run);
                }
            }
        }

        let transition = match handlers.policy.reduce(turn, &proposal) {
            TurnTransition::Continue if turn.saturating_add(1) >= config.max_turns => {
                TurnTransition::Fail(b"turn_budget_exhausted".to_vec())
            }
            other => other,
        };
        match transition {
            TurnTransition::Continue => {
                journal.push(JournalEntry::Transition {
                    turn,
                    case: "continue".to_owned(),
                    carrier_digest: carrier_digest(b""),
                });
                turn = turn.saturating_add(1);
            }
            terminal => {
                let mut run = finish(journal, turn, terminal);
                run.dispatched = dispatched;
                return Ok(run);
            }
        }
    }
}

fn finish(mut journal: Vec<JournalEntry>, turn: u32, transition: TurnTransition) -> LiveKernelRun {
    let (case, bytes): (&'static str, Vec<u8>) = match transition {
        TurnTransition::Continue => unreachable!("continue is handled by the caller"),
        TurnTransition::Complete(bytes) => ("complete", bytes),
        TurnTransition::Suspend(bytes) => ("suspend", bytes),
        TurnTransition::Fail(bytes) => ("fail", bytes),
    };
    let carrier = carrier_digest(&bytes);
    journal.push(JournalEntry::Transition {
        turn,
        case: case.to_owned(),
        carrier_digest: carrier.clone(),
    });
    journal.push(JournalEntry::TerminalOutcome {
        turn,
        case: case.to_owned(),
        carrier_digest: carrier,
    });
    let outcome = match case {
        "complete" => LiveInvocationOutcome::Complete(bytes),
        "suspend" => LiveInvocationOutcome::Suspend(bytes),
        _ => LiveInvocationOutcome::Fail(bytes),
    };
    LiveKernelRun {
        journal,
        outcome,
        dispatched: 0, // callers of `finish` overwrite this with the real count
    }
}

/// Test-only escape hatch onto the private proposal-digest domain, so a test
/// can independently recompute the exact digest the kernel would bind into
/// `AuthorizationContext::proposal_digest` for given decoded bytes, and
/// thereby prove which bytes (decoded, not raw response) actually reached
/// that digest — without widening `PROPOSAL_DOMAIN` into non-test-visible
/// API surface.
#[cfg(test)]
pub(crate) fn proposal_digest_for_test(decoded_proposal: &[u8]) -> String {
    digest(PROPOSAL_DOMAIN, decoded_proposal)
}

fn terminal_outcome_of(journal: &[JournalEntry]) -> LiveInvocationOutcome {
    match journal.last() {
        Some(JournalEntry::TerminalOutcome { case, .. }) if case == "complete" => {
            LiveInvocationOutcome::Complete(Vec::new())
        }
        Some(JournalEntry::TerminalOutcome { case, .. }) if case == "suspend" => {
            LiveInvocationOutcome::Suspend(Vec::new())
        }
        _ => LiveInvocationOutcome::Fail(Vec::new()),
    }
}
