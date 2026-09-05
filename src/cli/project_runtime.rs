//! Project execution and profile-selected build reporting for the CLI.

use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::project;

pub(crate) fn execute_held(
    command: &str,
    manifest_path: &Path,
    options: &super::execution::ExecutionOptions,
) -> Result<(), u8> {
    let defaults = project::ProjectExecutionOptions::default();
    let execution_options = project::ProjectExecutionOptions::new(
        options.max_bytes.unwrap_or(defaults.max_bytes),
        options.max_steps.unwrap_or(defaults.max_steps),
    )
    .map_err(|error| report(&[error], options.json))?;
    let (execution, command_note) =
        project::with_authenticated_project(manifest_path, |snapshot| {
            let command_note = if command == "run"
                && matches!(
                    snapshot.manifest().project_profile(),
                    project::ProjectProfile::UsefulDataCommandV2
                        | project::ProjectProfile::LanguageCommandIoV1
                        | project::ProjectProfile::LineCommandIoV1
                ) {
                snapshot.manifest().command().map(|command_id| {
                    (
                        snapshot.entry_program().entrypoint.as_str().to_owned(),
                        command_id.to_owned(),
                    )
                })
            } else {
                None
            };
            let execution = match command {
                "run" => snapshot.execute_entry(&execution_options),
                "test" => snapshot.execute_test(&execution_options),
                _ => unreachable!("validated project execution command"),
            }?;
            Ok((execution, command_note))
        })
        .map_err(|errors| {
            let errors = super::manifest_hint::hint_missing_manifest(errors, manifest_path);
            report(&errors, options.json)
        })?;

    if !options.json {
        if let Some((entry_id, command_id)) = command_note {
            eprintln!(
                "note: project run executes entry `{entry_id}`; command function `{command_id}` is exercised by built native and web/npm adapters"
            );
        }
    }

    if options.json {
        println!("{}", execution.envelope());
    }

    match (command, execution.outcome()) {
        ("run", project::ProjectExecutionOutcome::Returned(value)) => {
            if !options.json {
                println!("{value}");
            }
            Ok(())
        }
        ("run", outcome) => {
            if !options.json {
                let line = match outcome {
                    project::ProjectExecutionOutcome::LanguageFailure(status) => format!(
                        "project execution failed with language status {}",
                        status.to_json()
                    ),
                    project::ProjectExecutionOutcome::FuelExhausted => {
                        "project execution exhausted its step budget".to_owned()
                    }
                    _ => "project execution exceeded its call-depth bound".to_owned(),
                };
                eprint!("{line}\n{}", failure_text(execution.failure()));
            }
            Err(1)
        }
        ("test", _) => report_test(&execution, options.json),
        _ => unreachable!("validated project execution command"),
    }
}

/// The human test report. `main` and every named case must return zero; each
/// failing closure is listed by stable identity with its outcome, and a
/// contract failure adds the violated clause and the call's arguments.
fn report_test(execution: &project::ProjectExecution, json: bool) -> Result<(), u8> {
    let main_failed = !matches!(
        execution.outcome(),
        project::ProjectExecutionOutcome::Returned(0)
    );
    let cases = execution.cases();
    if !json {
        for skipped in execution.skipped_cases() {
            eprintln!(
                "note: `{}` is not a test case: {}; a case is `fn test_<name>() -> i64` with an `@id` and no parameters",
                skipped.name, skipped.reason
            );
        }
    }
    let failed_cases = cases.iter().filter(|case| !case.passed()).count();
    if !main_failed && failed_cases == 0 {
        if !json {
            if cases.is_empty() {
                println!("project tests passed");
            } else {
                println!("project tests passed ({} named cases)", cases.len());
            }
        }
        return Ok(());
    }
    if json {
        return Err(1);
    }
    let mut report = String::new();
    if main_failed {
        report.push_str(&format!(
            "failed {}: {}\n{}",
            execution.stable_id(),
            outcome_text(execution.outcome()),
            failure_text(execution.failure())
        ));
    }
    for case in cases.iter().filter(|case| !case.passed()) {
        report.push_str(&format!(
            "failed {}: {}\n{}",
            case.stable_id(),
            outcome_text(case.outcome()),
            failure_text(case.failure())
        ));
    }
    let failure_count = if main_failed {
        format!("main plus {failed_cases} of {} named cases", cases.len())
    } else {
        format!("{failed_cases} of {} named cases", cases.len())
    };
    report.push_str(&format!(
        "project tests failed: {failure_count} in {}\n  help: a test passes by returning 0; a nonzero return is the failing check's code or count{}\n",
        execution.module(),
        if cases.is_empty() {
            ", so give each check its own `fn test_<name>() -> i64` in the test module to have it reported by name"
        } else {
            ""
        }
    ));
    eprint!("{report}");
    Err(1)
}

fn outcome_text(outcome: &project::ProjectExecutionOutcome) -> String {
    match outcome {
        project::ProjectExecutionOutcome::Returned(value) => format!("returned {value}"),
        project::ProjectExecutionOutcome::LanguageFailure(status) => {
            format!("language status {}", status.to_json())
        }
        project::ProjectExecutionOutcome::FuelExhausted => "step budget exhausted".to_owned(),
        project::ProjectExecutionOutcome::CallDepthExceeded => {
            "call-depth bound exceeded".to_owned()
        }
    }
}

/// Two indented lines naming the violated clause and the call's arguments, or
/// nothing when the outcome carries no contract detail.
fn failure_text(failure: Option<&project::ProjectContractFailure>) -> String {
    let Some(failure) = failure else {
        return String::new();
    };
    let arguments = if failure.arguments.is_empty() {
        "none".to_owned()
    } else {
        failure
            .arguments
            .iter()
            .map(|argument| format!("{} = {}", argument.name, argument.value))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!(
        "  contract: {} {} in {}\n  arguments: {arguments}\n",
        failure.phase, failure.clause, failure.function_id
    )
}

pub(crate) fn build_success(
    target: &str,
    profile: project::ProjectProfile,
    output: &Path,
) -> String {
    format!(
        "built {} {}",
        build_product(target, profile),
        output.display()
    )
}

fn build_product(target: &str, profile: project::ProjectProfile) -> &'static str {
    match (target, profile) {
        ("native", _) => "project native executable",
        ("rust", project::ProjectProfile::FlatOwnedRecordApiV1) => {
            "Project v9 Native Rust flat owned-record package"
        }
        ("rust", project::ProjectProfile::OwnedUtf8ApiV1) => {
            "Project v10 Native Rust owned-data package"
        }
        ("rust", project::ProjectProfile::NestedOwnedRecordApiV1) => {
            "Project v11 Native Rust nested owned-record package"
        }
        ("rust", project::ProjectProfile::ScalarV1) => "Project v1 Native Rust SDK package",
        ("rust", _) => "Project v8 Native Rust owned-data package",
        ("npm", project::ProjectProfile::FlatOwnedRecordApiV1) => "Project v9 npm package",
        ("npm", project::ProjectProfile::OwnedUtf8ApiV1) => "Project v10 npm package",
        ("npm", project::ProjectProfile::NestedOwnedRecordApiV1) => "Project v11 npm package",
        ("npm", _) => "Project v2 npm package",
        _ => "project web package",
    }
}

pub(crate) fn report_build_success(
    target: &str,
    profile: project::ProjectProfile,
    output: &Path,
    json: bool,
) {
    if json {
        println!(
            "{}",
            serde_json::json!({
                "status": "built",
                "target": target,
                "product": build_product(target, profile),
                "output": output.display().to_string(),
            })
        );
    } else {
        println!("{}", build_success(target, profile, output));
    }
}

fn report(errors: &[Diagnostic], json: bool) -> u8 {
    for error in errors {
        if json {
            println!("{}", error.json());
        } else {
            eprintln!("{error}");
        }
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_selected_success_labels_are_exact() {
        let output = Path::new("dist");
        assert_eq!(
            build_success("rust", project::ProjectProfile::ScalarV1, output),
            "built Project v1 Native Rust SDK package dist"
        );
        assert_eq!(
            build_success(
                "rust",
                project::ProjectProfile::FlatOwnedRecordApiV1,
                output,
            ),
            "built Project v9 Native Rust flat owned-record package dist"
        );
        assert_eq!(
            build_success("npm", project::ProjectProfile::FlatOwnedRecordApiV1, output),
            "built Project v9 npm package dist"
        );
        assert_eq!(
            build_success("rust", project::ProjectProfile::OwnedDataApiV1, output),
            "built Project v8 Native Rust owned-data package dist"
        );
        assert_eq!(
            build_success("npm", project::ProjectProfile::OwnedUtf8ApiV1, output),
            "built Project v10 npm package dist"
        );
        assert_eq!(
            build_success(
                "rust",
                project::ProjectProfile::NestedOwnedRecordApiV1,
                output,
            ),
            "built Project v11 Native Rust nested owned-record package dist"
        );
        assert_eq!(
            build_success(
                "npm",
                project::ProjectProfile::NestedOwnedRecordApiV1,
                output,
            ),
            "built Project v11 npm package dist"
        );
    }
}
