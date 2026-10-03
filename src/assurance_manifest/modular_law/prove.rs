//! Real Z3 discharge of the bounded call-free proof subjects.
use crate::assurance_manifest::smt_discharge::{
    self as smt, DischargeOutcome, Model, Provisioning, ReplayOutcome, RunLimits, Verdict,
};
use crate::project::ProjectRevision;

use super::{inline::inline_subject, plan, Plan, Refusal};

#[derive(Clone, Debug)]
pub struct ProvedClause {
    pub declaration_id: String,
    pub summary_digest: String,
    pub ensures_index: usize,
    pub script_digest: String,
    pub solver_identity: &'static str,
    pub solver_version: String,
}

#[derive(Clone, Debug)]
pub struct Proof {
    pub plan: Plan,
    pub clauses: Vec<ProvedClause>,
}

#[derive(Clone, Debug)]
pub enum ProofFailure {
    Refused(Refusal),
    NoPostcondition {
        declaration_id: String,
    },
    Counterexample {
        declaration_id: String,
        ensures_index: usize,
        model: Model,
        replay: ReplayOutcome,
    },
    Unknown {
        declaration_id: String,
        reason: String,
    },
}

/// Prove every transitive callee before its caller, using exact linked HIR
/// and checked inlining. No callee postcondition is assumed by this fallback.
/// Only a real solver `unsat` produces a `ProvedClause`.
pub fn prove_postconditions(
    revision: &ProjectRevision,
    target: &str,
    provisioning: Option<&Provisioning>,
    limits: &RunLimits,
) -> Result<Proof, ProofFailure> {
    let plan = plan(revision, target).map_err(ProofFailure::Refused)?;
    let mut clauses = Vec::new();
    for summary in &plan.summaries {
        let function =
            inline_subject(revision, &summary.declaration_id).map_err(ProofFailure::Refused)?;
        if function.ensures.is_empty() {
            return Err(ProofFailure::NoPostcondition {
                declaration_id: summary.declaration_id.clone(),
            });
        }
        for ensures_index in 0..function.ensures.len() {
            match smt::discharge_postcondition(&function, ensures_index, provisioning, limits) {
                DischargeOutcome::Proved {
                    script_digest,
                    solver_identity,
                    solver_version,
                } => {
                    clauses.push(ProvedClause {
                        declaration_id: summary.declaration_id.clone(),
                        summary_digest: summary.digest.clone(),
                        ensures_index,
                        script_digest,
                        solver_identity,
                        solver_version,
                    });
                }
                DischargeOutcome::Inconclusive { reason } => {
                    return Err(ProofFailure::Unknown {
                        declaration_id: summary.declaration_id.clone(),
                        reason,
                    })
                }
                DischargeOutcome::Refuted { replay, .. } => {
                    // The discharge path independently replayed a SAT model.
                    // Re-query solely to return the concrete witness to the
                    // caller; never promote if it does not replay again.
                    let Some(provisioning) = provisioning else {
                        unreachable!("refutation requires solver")
                    };
                    let encoding = smt::translate_function(&function).map_err(|reason| {
                        ProofFailure::Unknown {
                            declaration_id: summary.declaration_id.clone(),
                            reason: reason.detail(),
                        }
                    })?;
                    let timeout_ms = u64::try_from(limits.timeout.as_millis()).unwrap_or(u64::MAX);
                    let script =
                        smt::render_postcondition_script(&encoding, ensures_index, timeout_ms);
                    let Verdict::Sat(raw) = smt::run(provisioning, &script, limits) else {
                        return Err(ProofFailure::Unknown {
                            declaration_id: summary.declaration_id.clone(),
                            reason: "counterexample model could not be reproduced".into(),
                        });
                    };
                    let model = smt::parse_model(&raw).map_err(|reason| ProofFailure::Unknown {
                        declaration_id: summary.declaration_id.clone(),
                        reason,
                    })?;
                    let again = smt::replay_function(&function, &model).map_err(|reason| {
                        ProofFailure::Unknown {
                            declaration_id: summary.declaration_id.clone(),
                            reason,
                        }
                    })?;
                    if !matches!(
                        again,
                        ReplayOutcome::Trapped { .. } | ReplayOutcome::EnsuresViolated { .. }
                    ) {
                        return Err(ProofFailure::Unknown {
                            declaration_id: summary.declaration_id.clone(),
                            reason: "counterexample no longer validates".into(),
                        });
                    }
                    debug_assert!(matches!(
                        replay,
                        ReplayOutcome::Trapped { .. } | ReplayOutcome::EnsuresViolated { .. }
                    ));
                    return Err(ProofFailure::Counterexample {
                        declaration_id: summary.declaration_id.clone(),
                        ensures_index,
                        model,
                        replay: again,
                    });
                }
            }
        }
    }
    Ok(Proof { plan, clauses })
}
