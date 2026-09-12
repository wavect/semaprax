//! Bounded model checking (`semaprax.model-checking.v1`): deterministic,
//! explicit-state, bounded exploration of small finite-state protocol
//! projections, feeding the `model_checked` class of
//! [`crate::assurance_manifest`]'s assurance lattice.
//!
//! See [`docs/BOUNDED-MODEL-CHECKING-V1.md`](../../../docs/BOUNDED-MODEL-CHECKING-V1.md)
//! for the full specification: the engine's traversal and determinism
//! guarantees, the two committed models and their invariants, the vacuity
//! defenses, and the exact, narrow scope this tranche claims (and the
//! considerably longer list of things — partial-order reduction, executable
//! fixture replay, arbitrary heap exploration, unbounded liveness — it does
//! not).
//!
//! This module is proof data, not permission: [`check_safety`] and
//! [`check_reachable`] never execute a target, spawn a process, touch the
//! filesystem, or grant any authority. [`model_checked_record`] never
//! records anything for a bound-exhausted, violated, dead-state, or
//! empty-state-space outcome — only a fully closed, non-violating
//! exploration ever becomes a `model_checked` [`MethodRecord`].

pub mod authorization_model;
pub mod digest;
pub mod engine;
pub mod handle_model;

#[cfg(test)]
mod tests;

pub use digest::{model_digest, ModelDescriptor};
pub use engine::{
    check_reachable, check_safety, Bounds, ExploreCounters, ExploreReport, LimitHit,
    ReachabilityOutcome, ReachabilityReport, SafetyOutcome, Step, Trace, TransitionSystem,
};

use super::lattice::AssuranceClass;
use super::obligation::MethodRecord;

/// The tool name recorded on every `model_checked` method record this
/// module produces.
pub const MODEL_CHECKER_TOOL: &str = "semaprax-bounded-model-checker";

/// Build the `model_checked` [`MethodRecord`] for one bounded safety
/// exploration, or `None` when it is anything other than
/// [`SafetyOutcome::Verified`].
///
/// This is the only bridge between this module and the Assurance Manifest:
/// a violation, a dead state, an empty state space, or a bound exhaustion
/// is never turned into a method record of any kind — exactly as
/// `smt_discharge`'s `Verdict::Sat`/`Refuted`/`Unknown`/… are never turned
/// into one either (the assurance lattice has no "disproved" or
/// "incomplete" class; recording a false positive here would be strictly
/// worse than recording nothing). `bounds` and the exact explored
/// state/transition counts are always included so a reader never has to
/// trust an unqualified "model checked" claim.
#[must_use]
pub fn model_checked_record<S, E>(
    report: &ExploreReport<S, E>,
    descriptor: &ModelDescriptor,
) -> Option<MethodRecord> {
    if !matches!(report.outcome, SafetyOutcome::Verified) {
        return None;
    }
    let digest = model_digest(descriptor, report.bounds);
    let bounds_text = format!(
        "max_states={} max_depth={} max_transitions={}",
        report.bounds.max_states, report.bounds.max_depth, report.bounds.max_transitions
    );
    let method = MethodRecord::new(
        AssuranceClass::ModelChecked,
        MODEL_CHECKER_TOOL,
        env!("CARGO_PKG_VERSION"),
    );
    Some(MethodRecord {
        bounds: Some(bounds_text),
        artifact_digest: Some(digest),
        detail: Some(format!(
            "model={} version={} explored_states={} explored_transitions={} \
             max_depth_reached={} reductions=none (no partial-order reduction implemented)",
            descriptor.name,
            descriptor.version,
            report.counters.explored_states,
            report.counters.explored_transitions,
            report.counters.max_depth_reached,
        )),
        ..method
    })
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn verified_authorization_model_yields_a_model_checked_record() {
        let report = check_safety(&authorization_model::Correct, authorization_model::BOUNDS);
        let record = model_checked_record(&report, &authorization_model::DESCRIPTOR)
            .expect("a Verified outcome must yield a record");
        assert_eq!(record.class, AssuranceClass::ModelChecked);
        assert_eq!(record.tool, MODEL_CHECKER_TOOL);
        assert!(record.bounds.as_deref().unwrap().contains("max_states=64"));
        assert!(record
            .artifact_digest
            .as_deref()
            .unwrap()
            .starts_with("sha256:"));
    }

    #[test]
    fn a_violated_outcome_never_yields_a_model_checked_record() {
        let faulty =
            authorization_model::Faulty(authorization_model::Fault::DispatchWithoutAuthorization);
        let report = check_safety(&faulty, authorization_model::BOUNDS);
        assert!(model_checked_record(&report, &authorization_model::DESCRIPTOR).is_none());
    }

    #[test]
    fn a_bound_exhausted_outcome_never_yields_a_model_checked_record() {
        let tiny = Bounds {
            max_states: 1,
            max_depth: 1,
            max_transitions: 1,
        };
        let report = check_safety(&authorization_model::Correct, tiny);
        assert!(matches!(
            report.outcome,
            SafetyOutcome::BoundExhausted { .. }
        ));
        assert!(model_checked_record(&report, &authorization_model::DESCRIPTOR).is_none());
    }

    #[test]
    fn a_dead_state_outcome_never_yields_a_model_checked_record() {
        use super::tests::IncompleteToy;
        let report = check_safety(
            &IncompleteToy,
            Bounds {
                max_states: 8,
                max_depth: 8,
                max_transitions: 8,
            },
        );
        assert!(matches!(report.outcome, SafetyOutcome::DeadState { .. }));
        let descriptor = ModelDescriptor {
            name: "toy",
            version: "v1",
            invariants: &[],
            terminal_states: &[],
        };
        assert!(model_checked_record(&report, &descriptor).is_none());
    }
}
