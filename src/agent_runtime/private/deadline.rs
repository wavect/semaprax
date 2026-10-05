use super::execution::termination_from_diagnostic;
use super::*;

pub(super) fn boundary_termination<H: AgentHost>(
    profile: &Profile,
    host: &H,
    cancellation: &AgentCancellation,
    policy_epoch: u64,
) -> Option<Termination> {
    if cancellation.is_cancelled() {
        return Some(termination_from_diagnostic(operational(
            "SPX-I220",
            "Agent Runtime run was cancelled",
        )));
    }
    if host.elapsed_ms() >= profile.limits.max_elapsed_ms {
        return Some(termination_from_diagnostic(operational(
            "SPX-I221",
            "Agent Runtime deadline was exceeded",
        )));
    }
    if host.policy_epoch() != policy_epoch {
        return Some(termination_from_diagnostic(g207("policy revoked")));
    }
    None
}

pub(super) fn remaining_deadline_ms<H: AgentHost>(
    profile: &Profile,
    host: &H,
    cancellation: &AgentCancellation,
    policy_epoch: u64,
) -> Result<u64, Termination> {
    if let Some(termination) = boundary_termination(profile, host, cancellation, policy_epoch) {
        return Err(termination);
    }
    profile
        .limits
        .max_elapsed_ms
        .checked_sub(host.elapsed_ms())
        .filter(|remaining| *remaining > 0)
        .ok_or_else(|| {
            termination_from_diagnostic(operational(
                "SPX-I221",
                "Agent Runtime deadline was exceeded",
            ))
        })
}

pub(super) fn termination_for_status(status: RunStatus) -> Termination {
    match status {
        RunStatus::Cancelled => {
            termination_from_diagnostic(operational("SPX-I220", "Agent Runtime run was cancelled"))
        }
        RunStatus::DeadlineExceeded => termination_from_diagnostic(operational(
            "SPX-I221",
            "Agent Runtime deadline was exceeded",
        )),
        RunStatus::PolicyRejected => termination_from_diagnostic(g207("policy revoked")),
        _ => termination_from_diagnostic(g209()),
    }
}
