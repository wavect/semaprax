//! Private, explicitly selected durable source Agent CLI.
//!
//! The compiler owns the checked driver and source journal. This module only
//! supplies host-selected files, an exclusive latest-store lease, a stable
//! clock, and the one fixed free OpenCode provider.

mod candidate_test;
mod checkpoint;
mod offline_repair_cli;
mod options;
mod repair;
mod run;

pub use candidate_test::{
    CandidateTestCapability, CandidateTestHost, CandidateTestObservation,
    CandidateTestObservationError, CandidateTestObserver, CandidateTestSubject,
    CANDIDATE_TEST_SCHEMA, MAX_CANDIDATE_TEST_OBSERVATION_BYTES,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, PartialEq)]
struct CliError {
    reason: String,
    code: u8,
}

impl CliError {
    fn usage(reason: &'static str) -> Self {
        Self {
            reason: reason.into(),
            code: 2,
        }
    }

    fn refused(reason: &'static str) -> Self {
        Self {
            reason: reason.into(),
            code: 1,
        }
    }

    fn detail(reason: String) -> Self {
        Self { reason, code: 1 }
    }
}

/// Executes durable source-live verbs or the fixed offline repair demonstration.
/// Neither route publishes source or selects a paid provider.
pub fn run(arguments: &[String]) -> Result<String, (String, u8)> {
    let result = match arguments.split_first() {
        Some((verb, rest)) if verb == "offline-repair" => offline_repair_cli::run(rest),
        Some((verb, rest)) if verb == "offline-repair-model-wait" => {
            offline_repair_cli::run_model_wait(rest)
        }
        Some((verb, rest)) if verb == "repair" => repair::run(rest),
        Some((verb, rest)) if verb == "repair-tested" => run_repair_tested(rest),
        _ => options::Command::parse(arguments).and_then(run::execute),
    };
    result.map_err(|error| (error.reason, error.code))
}

/// Run the durable V2 repair route with an explicitly injected candidate-test
/// observer. Ordinary [`run`] calls never acquire this capability.
pub fn run_repair_with_candidate_test(
    arguments: &[String],
    capability: CandidateTestCapability,
    observer: &mut dyn CandidateTestObserver,
) -> Result<String, (String, u8)> {
    let command = repair::Command::parse(arguments).map_err(|error| (error.reason, error.code))?;
    let mut host = CandidateTestHost::new(capability, observer);
    repair::execute_with_runner_and_candidate_test(
        command,
        crate::opencode_host::ProcessOpenCodeRunner,
        Some(&mut host),
    )
    .map_err(|error| (error.reason, error.code))
}

/// Explicit fixed-profile CLI route for the bounded reference-interpreter
/// candidate test. The ordinary `repair` route retains no test capability.
fn run_repair_tested(arguments: &[String]) -> Result<String, CliError> {
    let capability =
        CandidateTestCapability::host_selected(candidate_test::REPAIR_TEST_CAPABILITY_ID)
            .map_err(CliError::detail)?;
    let mut observer = candidate_test::FixedCandidateTestObserver::new();
    run_repair_with_candidate_test(arguments, capability, &mut observer)
        .map_err(|(reason, code)| CliError { reason, code })
}
