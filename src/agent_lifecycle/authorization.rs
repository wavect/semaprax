//! The opaque, one-use authorization value, and the only place it is minted.
//!
//! [`Authorized`] has no public constructor, no `Clone`, no `Copy`, no
//! `Default`, and no `From`. Its fields are private to this module, so a
//! struct literal cannot name them from anywhere else in the crate, let alone
//! from a consumer. The only function that builds one is the private `mint`
//! below, and the only call to `mint` is inside `run_authorize_stage`.
//!
//! `run_authorize_stage` cannot be reached without an `AuthorizeStage`, which
//! only the lifecycle compiler's stage
//! binder constructs, and only after the authorize role has passed identity,
//! signature, ownership, effect and decision-shape validation. It additionally
//! requires the retained product it dispatches to name that exact validated
//! function, and it mints only when the evaluated stage returns the validated
//! grant case of the validated decision variant.
//!
//! Therefore `observe`, `reduce`, the model's proposal, and any other caller
//! have no route to an `Authorized`: a proposal is data, and a deterministic
//! stage that is not the authorize role never reaches the mint. The value is
//! consumed by move at the effect boundary, so one authorization admits at
//! most one effect.

use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::interpreter::retained_call::{
    evaluate_retained_call, RetainedCallOutcome, RetainedValue,
};

use super::stages::AuthorizeStage;
use super::{encode_value, StageRecord};

const BINDING_DOMAIN: &[u8] = b"semaprax.agent-lifecycle.authorization.v1\0";

/// One opaque, one-use authorization.
///
/// It carries the binding of the exact policy, state and proposal the
/// validated authorize stage granted against. It is not a boolean, and it is
/// not a hash a caller can hand back: the only way to obtain one is for the
/// validated authorize stage to return its grant case, and the only way to
/// spend one is to move it into the effect boundary, which consumes it.
pub struct Authorized {
    binding: String,
    budget: i64,
    seal: Vec<u8>,
}

impl Authorized {
    /// The domain-separated binding of the exact policy, state and proposal.
    #[must_use]
    pub fn binding(&self) -> &str {
        &self.binding
    }

    /// The budget the validated grant carried.
    ///
    /// Crate-internal and read-only: the durable journal records it as a fact
    /// about one intent. It is never an input to minting, and no API accepts
    /// it in place of an authorization.
    pub(in crate::agent_lifecycle) const fn granted_budget(&self) -> i64 {
        self.budget
    }

    /// Spends the authorization. The value is moved, so it authorizes at most
    /// one effect.
    #[must_use]
    pub fn consume(self) -> AuthorizedRequest {
        AuthorizedRequest {
            binding: self.binding,
            budget: self.budget,
            seal: self.seal,
        }
    }
}

/// The spent authorization handed to the single injected read operation.
pub struct AuthorizedRequest {
    binding: String,
    budget: i64,
    seal: Vec<u8>,
}

impl AuthorizedRequest {
    #[must_use]
    pub fn binding(&self) -> &str {
        &self.binding
    }

    /// The budget the authorize stage itself granted, read from the validated
    /// grant case rather than from the proposal.
    #[must_use]
    pub const fn budget(&self) -> i64 {
        self.budget
    }

    /// The owned seal the authorize stage constructed inside the program.
    #[must_use]
    pub fn seal(&self) -> &[u8] {
        &self.seal
    }
}

/// The closed result of running the authorizing transition.
pub(super) enum AuthorizationOutcome {
    Granted(Authorized),
    Refused(i64),
    /// The stage did not decide: a contract failure, a fuel or depth limit, or
    /// an impossible post-verify shape. No authorization exists.
    Undecided(&'static str),
}

/// The domain-separated binding of one authorization.
///
/// It is deterministic in its inputs, so the same policy, state, proposal and
/// seal replay to the same value. Reproducing the string grants nothing: no
/// API accepts one in place of an [`Authorized`].
pub(super) fn binding(
    policy_digest: &str,
    state: &RetainedValue,
    proposal_canonical: &str,
    grant_case: &hir::DeclarationId,
    seal: &[u8],
) -> String {
    let mut hash = Sha256::new();
    hash.update(BINDING_DOMAIN);
    hash.update(policy_digest.as_bytes());
    hash.update([0]);
    hash.update(encode_value(state).as_bytes());
    hash.update([0]);
    hash.update(proposal_canonical.as_bytes());
    hash.update([0]);
    hash.update(grant_case.as_str().as_bytes());
    hash.update([0]);
    hash.update(seal);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

/// The single mint site of the entire crate.
fn mint(binding: String, budget: i64, seal: Vec<u8>) -> Authorized {
    Authorized {
        binding,
        budget,
        seal,
    }
}

/// Runs the validated authorizing transition and, only on its validated grant
/// case, mints the authorization bound to that exact policy, state and
/// proposal.
pub(super) fn run_authorize_stage(
    program: &hir::ResolvedProgram,
    stage: &AuthorizeStage,
    arguments: &[RetainedValue],
    max_steps: usize,
    policy_digest: &str,
    state: &RetainedValue,
    proposal_canonical: &str,
) -> Result<(AuthorizationOutcome, StageRecord), Vec<Diagnostic>> {
    let prepared = stage.stage().prepared();
    if prepared.function_id() != stage.stage().function_id() {
        return Err(vec![super::stages::invariant(
            "authorize.retained_call.identity",
        )]);
    }
    let evaluation = evaluate_retained_call(program, prepared, arguments, max_steps)?;
    if evaluation.function_id.as_str() != stage.stage().function_id() {
        return Err(vec![super::stages::invariant(
            "authorize.retained_call.dispatch",
        )]);
    }
    let record = StageRecord::of(stage.stage(), &evaluation);
    let outcome = match evaluation.outcome {
        RetainedCallOutcome::Returned(RetainedValue::Variant(decision)) => {
            if decision.variant != *stage.decision_type() {
                AuthorizationOutcome::Undecided("decision_identity")
            } else if decision.case == *stage.grant_case() {
                let seal = decision
                    .fields
                    .iter()
                    .find(|field| field.field == *stage.grant_seal_field())
                    .and_then(|field| match &field.value {
                        RetainedValue::Bytes(bytes) => Some(bytes.clone()),
                        _ => None,
                    });
                let budget = decision
                    .fields
                    .iter()
                    .find(|field| field.field == *stage.grant_budget_field())
                    .and_then(|field| match field.value {
                        RetainedValue::I64(value) => Some(value),
                        _ => None,
                    });
                match (seal, budget) {
                    (Some(seal), Some(budget)) => {
                        let binding = binding(
                            policy_digest,
                            state,
                            proposal_canonical,
                            &decision.case,
                            &seal,
                        );
                        AuthorizationOutcome::Granted(mint(binding, budget, seal))
                    }
                    _ => AuthorizationOutcome::Undecided("grant_payload"),
                }
            } else if decision.case == *stage.refuse_case() {
                let code = decision
                    .fields
                    .iter()
                    .find(|field| field.field == *stage.refuse_code_field())
                    .and_then(|field| match field.value {
                        RetainedValue::I64(value) => Some(value),
                        _ => None,
                    });
                match code {
                    Some(code) => AuthorizationOutcome::Refused(code),
                    None => AuthorizationOutcome::Undecided("refusal_payload"),
                }
            } else {
                AuthorizationOutcome::Undecided("decision_case")
            }
        }
        RetainedCallOutcome::Returned(_) => AuthorizationOutcome::Undecided("decision_carrier"),
        RetainedCallOutcome::LanguageFailure(_) => AuthorizationOutcome::Undecided("contract"),
        RetainedCallOutcome::FuelExhausted => AuthorizationOutcome::Undecided("fuel"),
        RetainedCallOutcome::CallDepthExceeded => AuthorizationOutcome::Undecided("depth"),
        RetainedCallOutcome::GuardError(_) => AuthorizationOutcome::Undecided("guard"),
    };
    Ok((outcome, record))
}
