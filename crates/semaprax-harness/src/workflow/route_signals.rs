//! MR-01 routing signals of one attempt, derived from facts the workflow
//! already holds: the attempt number, the host-recorded failures (stage and
//! diagnostic code of the compiler/check pipeline) and the task mode. No
//! model classifies anything here.

use super::attempt::PromptCtx;
use super::stages::{Task, TaskMode};
use crate::decision::{Phase, PreviousFailure, RouteSignals};

/// Order of the host's attempt pipeline stages; a later failing stage is
/// verified progress over an earlier one.
fn stage_rank(stage: &str) -> Option<u32> {
    ["proposal", "preview", "oracle", "checks", "acceptance"]
        .iter()
        .position(|s| *s == stage)
        .map(|i| i as u32)
}

/// Host classification of a recorded failure (stage and diagnostic code are
/// host/compiler facts; no model classifies anything).
fn classify_failure(stage: &str, code: &str) -> PreviousFailure {
    match stage {
        "proposal" if code.starts_with("SPX-HPC") => PreviousFailure::ToolTransport,
        "proposal" => PreviousFailure::ParseSchema,
        "preview" | "oracle" => PreviousFailure::SemanticLaw,
        "checks" | "acceptance" => PreviousFailure::Acceptance,
        "budget" => PreviousFailure::Budget,
        _ => PreviousFailure::Unknown,
    }
}

/// MR-01 routing signals for one attempt, derived from the prompt context the
/// host already holds: attempt number, recorded failures, the task mode.
pub(super) fn route_signals(
    task: &Task,
    p: &PromptCtx,
    remaining_budget_micros: Option<u64>,
) -> RouteSignals {
    let failures: Vec<(&str, &str)> = p
        .feedback
        .iter()
        .filter_map(|e| Some((e["stage"].as_str()?, e["code"].as_str().unwrap_or(""))))
        .collect();
    let ranks: Vec<Option<u32>> = failures.iter().map(|(s, _)| stage_rank(s)).collect();
    let verified_progress = ranks
        .windows(2)
        .filter(|w| matches!((w[0], w[1]), (Some(a), Some(b)) if b > a))
        .count() as u32;
    let no_progress = ranks
        .windows(2)
        .rev()
        .take_while(|w| !matches!((w[0], w[1]), (Some(a), Some(b)) if b > a))
        .count() as u32;
    let attempt_index = p.attempt.saturating_sub(1).min(64);
    let previous_failure = match failures.last() {
        Some((s, c)) => classify_failure(s, c),
        None if attempt_index == 0 => PreviousFailure::None,
        None => PreviousFailure::Unknown,
    };
    let phase = if task.mode == TaskMode::Plan {
        Phase::Plan
    } else if p.scratch_repair || !failures.is_empty() || task.mode == TaskMode::Repair {
        Phase::Repair
    } else {
        Phase::Implement
    };
    RouteSignals {
        task_profile: Some(task.family.clone()),
        phase,
        attempt_index,
        previous_failure,
        verified_progress: verified_progress.min(1000),
        no_progress: no_progress.min(1000),
        remaining_budget_micros,
        ..RouteSignals::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx<'a>(feedback: &'a [serde_json::Value], attempt: u32) -> PromptCtx<'a> {
        PromptCtx {
            revision: "r",
            seed: None,
            diag_view: "",
            kept: &[],
            ops: &[],
            feedback,
            attempt,
            scratch_repair: false,
        }
    }

    fn task(mode: TaskMode) -> Task {
        Task {
            mode,
            family: "mechanical".into(),
            ..Task::default()
        }
    }

    #[test]
    fn first_attempt_of_a_change_is_an_implement_step_with_no_failure() {
        let s = route_signals(&task(TaskMode::Change), &ctx(&[], 1), Some(500));
        assert_eq!(s.phase, Phase::Implement);
        assert_eq!(s.attempt_index, 0);
        assert_eq!(s.previous_failure, PreviousFailure::None);
        assert_eq!(s.task_profile.as_deref(), Some("mechanical"));
        assert_eq!(s.remaining_budget_micros, Some(500));
    }

    #[test]
    fn recorded_failures_classify_phase_failure_and_progress() {
        let fb = vec![
            json!({"attempt": 1, "stage": "proposal", "code": "SPX-HPD090", "message": "m"}),
            json!({"attempt": 2, "stage": "oracle", "code": "SPX-HPD114", "message": "m"}),
            json!({"attempt": 3, "stage": "oracle", "code": "SPX-HPD114", "message": "m"}),
        ];
        let s = route_signals(&task(TaskMode::Change), &ctx(&fb, 4), None);
        assert_eq!(s.phase, Phase::Repair);
        assert_eq!(s.attempt_index, 3);
        assert_eq!(s.previous_failure, PreviousFailure::SemanticLaw);
        assert_eq!(s.verified_progress, 1);
        assert_eq!(s.no_progress, 1);
        let t = vec![json!({"stage": "proposal", "code": "SPX-HPC008"})];
        assert_eq!(
            route_signals(&task(TaskMode::Change), &ctx(&t, 2), None).previous_failure,
            PreviousFailure::ToolTransport
        );
        let c = vec![json!({"stage": "checks", "code": "SPX-HPD120"})];
        assert_eq!(
            route_signals(&task(TaskMode::Change), &ctx(&c, 2), None).previous_failure,
            PreviousFailure::Acceptance
        );
        assert_eq!(
            route_signals(&task(TaskMode::Plan), &ctx(&[], 1), None).phase,
            Phase::Plan
        );
        // A later attempt with no recorded failure is unknown, never `none`.
        assert_eq!(
            route_signals(&task(TaskMode::Change), &ctx(&[], 3), None).previous_failure,
            PreviousFailure::Unknown
        );
    }
}
