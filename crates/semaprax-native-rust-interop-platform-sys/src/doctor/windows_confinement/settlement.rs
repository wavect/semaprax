//! Host-independent settlement state machine for the Windows confinement
//! contract in
//! [`DOCTOR-PRODUCTION-PROVISIONER-WINDOWS-V1`][doc]'s "Settlement contract"
//! section. This module contains no Win32 call: it is the same sticky,
//! four-outcome shape `doctor::darwin_confinement` proves for macOS, adapted
//! to the vocabulary a Windows job object reports (`ActiveProcesses`, a
//! process exit code, or a job-limit violation) rather than a process-group
//! signal probe. [`super::primitive`] is the `#[cfg(windows)]` code that
//! actually observes a job object and drives this state machine; it is
//! untypechecked and unexecuted on this host (see that module's
//! documentation).
//!
//! [doc]: https://github.com/wavect/semaprax/blob/main/docs/DOCTOR-PRODUCTION-PROVISIONER-WINDOWS-V1.md

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FailureReason {
    /// The leader process exited on its own with this nonzero code.
    ExitCode(u32),
    /// The bounded pipe reader observed more stdout/stderr bytes than the
    /// primitive admits and selected failure before terminating its owned job.
    OutputLimit,
    /// The job's observed user-mode CPU accounting reached its configured
    /// process-time ceiling before the terminated leader became signaled.
    CpuTimeLimit,
    /// A job-object limit (`JOB_OBJECT_LIMIT_ACTIVE_PROCESS`,
    /// `JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION`, or a UI restriction)
    /// terminated the whole job. This is the Windows analog of the Linux
    /// contract's `memory.oom.group = 1`: an overshoot kills the whole scope
    /// rather than refusing cleanly mid-invocation.
    JobLimitViolation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UncertainReason {
    /// `WaitForSingleObject` on the leader process handle itself failed or
    /// returned an unexpected code.
    WaitFailed,
    /// `QueryInformationJobObject` itself failed or errored.
    QueryFailed,
    /// The leader process handle was observed signaled, but a subsequent
    /// `JobObjectBasicAccountingInformation.ActiveProcesses` reread reports
    /// nonzero. The Windows analog of the Linux contract's empty-cgroup
    /// proof and macOS's `GroupStillPresent`: the leader alone exiting is
    /// not sufficient, the whole confined job must be quiescent.
    ActiveProcessesNonZero,
    /// The post-timeout `TerminateJobObject`/`TerminateProcess` call could
    /// not be distinguished from "already gone".
    KillAmbiguous,
    /// Termination was requested, but the leader did not become signaled
    /// within the fixed post-kill cleanup grace.
    KillWaitTimedOut,
    /// The parent-only bounded output reader failed before it could account
    /// for both child streams through EOF.
    OutputReadFailed,
}

/// The four settlement outcomes this contract requires stay distinct. See
/// [`StickySettlement`] for why they cannot collapse into each other.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Settlement {
    Completed,
    Failed(FailureReason),
    Cancelled,
    Uncertain(UncertainReason),
}

/// Sticky failure/cancellation/uncertainty selection: once a non-`Completed`
/// status is selected, no later call can move it ("cleanup can never replace
/// the selected status", `AGENTS.md`). A pending `Completed` is the only
/// status a later call may still override, because it was never a final
/// proof of settlement by itself -- the `ActiveProcesses == 0` reread that
/// runs after it is part of proving settlement, not cleanup after it. This
/// is the same rule `doctor::darwin_confinement::StickySettlement` enforces.
#[derive(Debug, Default)]
pub struct StickySettlement(Option<Settlement>);

impl StickySettlement {
    pub fn select(&mut self, candidate: Settlement) {
        match self.0 {
            None => self.0 = Some(candidate),
            Some(Settlement::Completed) if !matches!(candidate, Settlement::Completed) => {
                self.0 = Some(candidate);
            }
            Some(_) => {}
        }
    }

    pub fn resolve(self) -> Option<Settlement> {
        self.0
    }

    pub fn is_selected(&self) -> bool {
        self.0.is_some()
    }
}

#[cfg(test)]
mod tests;
