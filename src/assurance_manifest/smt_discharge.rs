//! Bounded SMT discharge for a deliberately small pure subset of SEMAPRAX
//! contracts (issue #184).
//!
//! See [`docs/SMT-DISCHARGE-V1.md`](../../docs/SMT-DISCHARGE-V1.md) for the
//! full specification: the exact supported grammar, the `QF_LIA` numeric
//! encoding and why it is semantics-preserving for SEMAPRAX's checked
//! (trapping, never-wrapping) arithmetic, solver provisioning and process
//! bounds, model validation, the cache key, and — most importantly — the
//! "Integration status" section documenting exactly what is and is not
//! wired into `generate()` today.
//!
//! **This module never runs on its own.** It is invoked explicitly by a
//! caller that already has an explicitly provisioned solver path (never
//! discovered on `PATH`); with no solver provisioned it returns
//! [`DischargeOutcome::Inconclusive`] and touches no process. A `unsat`
//! verdict is a genuine proof; a `sat` verdict is independently replayed
//! against real checked-arithmetic semantics
//! ([`replay::replay_function`]) before it is ever reported as a
//! [`DischargeOutcome::Refuted`] concrete counterexample — an unvalidated
//! `sat` model is reported as [`DischargeOutcome::Inconclusive`], never as
//! either proved or refuted.

mod cache;
mod model;
mod replay;
mod solver;
mod subset;
mod translate;

pub use cache::{cache_key, CacheKeyInput, DischargeCache};
pub use model::{parse_model, Model, ModelValue};
pub use replay::{replay_function, ReplayOutcome};
pub use solver::{
    provision_from_env, run, solver_version, Provisioning, RunLimits, Verdict, ENV_Z3_PATH,
};
pub use subset::{check_declaration_supported, NumericMode, Sort, UnsupportedReason};
pub use translate::{translate_function, FunctionEncoding, SideObligation};

use std::time::Duration;

use crate::ast::Function;

use super::lattice::AssuranceClass;
use super::obligation::{obligation_id, MethodRecord, ObligationKind};

/// The `bounds` text every method record this module produces carries,
/// naming the exact admitted subset so a reader never has to guess how
/// far a `smt_proved`/`attempt_inconclusive` record actually reaches.
pub const BOUNDS_V1: &str = "semaprax-smt-discharge-bounded-subset-v1: i64/i32/u8/usize/bool, \
arithmetic add/sub/mul, comparisons, and/or/not, if, immutable let, no calls/loops/division; \
see docs/SMT-DISCHARGE-V1.md";

/// This tranche's tool identity for `MethodRecord::tool` on non-solver
/// (unsupported/inconclusive-without-a-run) attempts. Solver-backed
/// attempts instead use [`Provisioning::identity`] (currently always
/// `"z3"`).
pub const DISCHARGE_TOOL: &str = "semaprax-smt-discharge";

/// The outcome of attempting to discharge one `ensures` clause (or, via
/// [`discharge_precondition_consistency`], one function's whole `requires`
/// conjunction).
#[derive(Clone, Debug)]
pub enum DischargeOutcome {
    /// `unsat`: the property holds for every input satisfying the declared
    /// range axioms and `requires`, under this module's `QF_LIA` encoding
    /// of SEMAPRAX's checked arithmetic.
    Proved {
        script_digest: String,
        solver_identity: &'static str,
        solver_version: String,
    },
    /// A `sat` model that independently replayed as a genuine, validated
    /// failure: either a checked-arithmetic trap or an `ensures` violation.
    /// This is proof data pointing at a real defect, not an assurance
    /// record — see the module doc's "never runs on its own" and
    /// [`to_method_record`]'s deliberate `None` for this variant.
    Refuted {
        replay: ReplayOutcome,
        script_digest: String,
    },
    /// Neither proved nor refuted: unsupported subset, no solver
    /// provisioned, timeout, `unknown`, a crashed/malformed solver run, or
    /// a `sat` model that did not validate under replay. `reason` is
    /// always closed, human-readable text; see the variants of
    /// [`solver::Verdict`] and [`UnsupportedReason`] this collapses.
    Inconclusive { reason: String },
}

fn and_all(terms: &[String]) -> String {
    match terms {
        [] => "true".to_owned(),
        [only] => only.clone(),
        many => format!("(and {})", many.join(" ")),
    }
}

fn render_declarations(encoding: &FunctionEncoding) -> String {
    encoding
        .declarations
        .iter()
        .map(|decl| {
            format!(
                "(declare-const {} {})\n",
                decl.name,
                match decl.sort {
                    Sort::Numeric(_) => "Int",
                    Sort::Bool => "Bool",
                }
            )
        })
        .collect()
}

/// Render the complete deterministic SMT-LIB2 script proving `requires =>
/// (well-defined AND ensures[ensures_index])` for `encoding`, under
/// `timeout_ms` (passed to the solver's own `:timeout` option as defense
/// in depth alongside the caller's external wall-clock [`RunLimits`]).
#[must_use]
pub fn render_postcondition_script(
    encoding: &FunctionEncoding,
    ensures_index: usize,
    timeout_ms: u64,
) -> String {
    let ensures = &encoding.ensures[ensures_index];
    let mut obligation_terms: Vec<String> = encoding
        .shared_obligations
        .iter()
        .map(SideObligation::implication)
        .collect();
    obligation_terms.extend(ensures.obligations.iter().map(SideObligation::implication));

    let mut script = String::new();
    script.push_str(&format!("(set-option :timeout {timeout_ms})\n"));
    script.push_str("(set-logic QF_LIA)\n");
    script.push_str(&render_declarations(encoding));
    for axiom in &encoding.range_axioms {
        script.push_str(&format!("(assert {axiom})\n"));
    }
    for definition in &encoding.definitions {
        script.push_str(&format!("(assert {definition})\n"));
    }
    for requires_term in &encoding.requires_terms {
        script.push_str(&format!("(assert {requires_term})\n"));
    }
    let goal = and_all(
        &obligation_terms
            .iter()
            .cloned()
            .chain(std::iter::once(ensures.term.clone()))
            .collect::<Vec<_>>(),
    );
    script.push_str(&format!("(assert (not {goal}))\n"));
    script.push_str("(check-sat)\n(get-model)\n");
    script
}

/// Render the deterministic SMT-LIB2 script checking whether `encoding`'s
/// `requires` conjunction is satisfiable at all. `unsat` here means the
/// precondition is contradictory: the function's body can never execute
/// under any input, a defect worth surfacing even though it is not itself
/// an `ensures`/`requires` obligation failure.
#[must_use]
pub fn render_precondition_consistency_script(
    encoding: &FunctionEncoding,
    timeout_ms: u64,
) -> String {
    let mut script = String::new();
    script.push_str(&format!("(set-option :timeout {timeout_ms})\n"));
    script.push_str("(set-logic QF_LIA)\n");
    script.push_str(&render_declarations(encoding));
    for axiom in &encoding.range_axioms {
        script.push_str(&format!("(assert {axiom})\n"));
    }
    for definition in &encoding.definitions {
        script.push_str(&format!("(assert {definition})\n"));
    }
    for requires_term in &encoding.requires_terms {
        script.push_str(&format!("(assert {requires_term})\n"));
    }
    script.push_str("(check-sat)\n");
    script
}

fn script_digest(script: &str) -> String {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.smt-discharge.script.v1\0");
    hasher.update((script.len() as u64).to_le_bytes());
    hasher.update(script.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

/// Attempt to discharge `function`'s `ensures` clause at `ensures_index`.
///
/// `provisioning` is `None` exactly when [`provision_from_env`] found no
/// explicit solver path; this function never itself consults the
/// environment, so a caller can test discharge logic deterministically
/// with a fixed [`Provisioning`] regardless of the host's actual state.
#[must_use]
pub fn discharge_postcondition(
    function: &Function,
    ensures_index: usize,
    provisioning: Option<&Provisioning>,
    limits: &RunLimits,
) -> DischargeOutcome {
    let encoding = match translate_function(function) {
        Ok(encoding) => encoding,
        Err(reason) => {
            return DischargeOutcome::Inconclusive {
                reason: format!("unsupported ({}): {}", reason.code(), reason.detail()),
            }
        }
    };
    if ensures_index >= encoding.ensures.len() {
        return DischargeOutcome::Inconclusive {
            reason: "ensures_index out of range for this function".to_owned(),
        };
    }
    let Some(provisioning) = provisioning else {
        return DischargeOutcome::Inconclusive {
            reason: format!("no solver provisioned; set {ENV_Z3_PATH}"),
        };
    };
    let timeout_ms = u64::try_from(limits.timeout.as_millis()).unwrap_or(u64::MAX);
    let script = render_postcondition_script(&encoding, ensures_index, timeout_ms);
    let digest = script_digest(&script);
    let verdict = run(provisioning, &script, limits);
    let version = solver_version(provisioning).unwrap_or_else(|| "unrecorded".to_owned());
    interpret_verdict(verdict, function, &digest, provisioning.identity, &version)
}

/// Attempt to prove `function`'s `requires` conjunction is satisfiable.
/// `unsat` is reported as a validated finding (`Inconclusive`, not
/// `Proved`: an unsatisfiable precondition is a distinct diagnostic-worthy
/// property, not a proof any obligation holds — see the spec's
/// "Precondition consistency is not an obligation proof").
#[must_use]
pub fn discharge_precondition_consistency(
    function: &Function,
    provisioning: Option<&Provisioning>,
    limits: &RunLimits,
) -> DischargeOutcome {
    let encoding = match translate_function(function) {
        Ok(encoding) => encoding,
        Err(reason) => {
            return DischargeOutcome::Inconclusive {
                reason: format!("unsupported ({}): {}", reason.code(), reason.detail()),
            }
        }
    };
    if encoding.requires_terms.is_empty() {
        return DischargeOutcome::Inconclusive {
            reason: "no requires clauses; precondition is vacuously satisfiable".to_owned(),
        };
    }
    let Some(provisioning) = provisioning else {
        return DischargeOutcome::Inconclusive {
            reason: format!("no solver provisioned; set {ENV_Z3_PATH}"),
        };
    };
    let timeout_ms = u64::try_from(limits.timeout.as_millis()).unwrap_or(u64::MAX);
    let script = render_precondition_consistency_script(&encoding, timeout_ms);
    match run(provisioning, &script, limits) {
        Verdict::Sat(_) => DischargeOutcome::Inconclusive {
            reason: "requires is satisfiable (no contradiction found)".to_owned(),
        },
        Verdict::Unsat => DischargeOutcome::Inconclusive {
            reason: "requires is contradictory: this function's precondition can never hold"
                .to_owned(),
        },
        other => DischargeOutcome::Inconclusive {
            reason: describe_non_result_verdict(&other),
        },
    }
}

fn describe_non_result_verdict(verdict: &Verdict) -> String {
    match verdict {
        Verdict::Unknown => "solver returned unknown".to_owned(),
        Verdict::Timeout => "solver run exceeded the bounded timeout".to_owned(),
        Verdict::CapacityExceeded => "solver output exceeded the capped output budget".to_owned(),
        Verdict::Crash { exit_code } => format!("solver process crashed (exit={exit_code:?})"),
        Verdict::Malformed { excerpt } => format!("malformed solver output: {excerpt}"),
        Verdict::NotProvisioned => format!("no solver provisioned; set {ENV_Z3_PATH}"),
        Verdict::Unsat | Verdict::Sat(_) => unreachable!("caller already matched this variant"),
    }
}

fn interpret_verdict(
    verdict: Verdict,
    function: &Function,
    script_digest: &str,
    solver_identity: &'static str,
    recorded_solver_version: &str,
) -> DischargeOutcome {
    match verdict {
        Verdict::Unsat => DischargeOutcome::Proved {
            script_digest: script_digest.to_owned(),
            solver_identity,
            solver_version: recorded_solver_version.to_owned(),
        },
        Verdict::Sat(raw_model) => match parse_model(&raw_model) {
            Err(parse_error) => DischargeOutcome::Inconclusive {
                reason: format!("sat but the model failed to parse: {parse_error}"),
            },
            Ok(model) => match replay_function(function, &model) {
                Err(evaluation_error) => DischargeOutcome::Inconclusive {
                    reason: format!("sat model failed to replay: {evaluation_error}"),
                },
                Ok(ReplayOutcome::Inconsistent { detail }) => DischargeOutcome::Inconclusive {
                    reason: format!("sat model did not validate against checked replay: {detail}"),
                },
                Ok(
                    validated @ (ReplayOutcome::Trapped { .. }
                    | ReplayOutcome::EnsuresViolated { .. }),
                ) => DischargeOutcome::Refuted {
                    replay: validated,
                    script_digest: script_digest.to_owned(),
                },
            },
        },
        other => DischargeOutcome::Inconclusive {
            reason: describe_non_result_verdict(&other),
        },
    }
}

/// Convert one [`DischargeOutcome`] into a [`MethodRecord`] suitable for
/// merging into an obligation's method list, or `None` when this outcome
/// must never appear as an assurance record at all.
///
/// [`DischargeOutcome::Refuted`] always returns `None`: a validated
/// counterexample is proof of a real defect, not evidence *for* any
/// [`AssuranceClass`] this lattice defines (there is deliberately no
/// "refuted"/"disproved" class — see
/// [`docs/SMT-DISCHARGE-V1.md`](../../docs/SMT-DISCHARGE-V1.md)
/// "Refutation has no assurance class"). Folding a refutation into
/// `attempt_inconclusive` would misreport a definitive finding as a mere
/// non-attempt, which is strictly more dangerous than omitting a record;
/// see the module's `Refuted` field for how a caller (a future compiler
/// diagnostic, out of this tranche's scope) can still act on it directly.
#[must_use]
pub fn to_method_record(outcome: &DischargeOutcome, timeout: Duration) -> Option<MethodRecord> {
    match outcome {
        DischargeOutcome::Proved {
            script_digest,
            solver_identity,
            solver_version,
        } => Some(MethodRecord {
            runtime_fallback: true,
            target: Some("bounded_smt_subset_v1".to_owned()),
            proof_ref: Some(script_digest.clone()),
            artifact_digest: Some(script_digest.clone()),
            bounds: Some(BOUNDS_V1.to_owned()),
            inputs: vec![
                "logic:QF_LIA".to_owned(),
                format!("timeout_ms:{}", timeout.as_millis()),
            ],
            detail: Some(
                "QF_LIA unsat proof: requires implies (well-definedness AND this ensures \
                 clause) over the bounded pure-arithmetic subset"
                    .to_owned(),
            ),
            ..MethodRecord::new(AssuranceClass::SmtProved, *solver_identity, solver_version)
        }),
        DischargeOutcome::Refuted { .. } => None,
        DischargeOutcome::Inconclusive { reason } => Some(MethodRecord {
            runtime_fallback: true,
            bounds: Some(BOUNDS_V1.to_owned()),
            detail: Some(reason.clone()),
            ..MethodRecord::new(
                AssuranceClass::AttemptInconclusive,
                DISCHARGE_TOOL,
                env!("CARGO_PKG_VERSION"),
            )
        }),
    }
}

/// The exact obligation id this discharge attempt targets, matching
/// `derive.rs`'s own `"ensure:{index}"` locator convention bit for bit —
/// see the module doc's "Integration status" for why this id currently
/// collides with, rather than merges into, the automatically derived
/// `runtime_guarded` obligation of the same clause.
#[must_use]
pub fn postcondition_obligation_id(declaration_id: &str, ensures_index: usize) -> String {
    obligation_id(
        ObligationKind::Postcondition,
        declaration_id,
        &format!("ensure:{ensures_index}"),
    )
}

#[cfg(test)]
mod tests;
