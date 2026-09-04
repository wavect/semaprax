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
    let execution = project::with_authenticated_project(manifest_path, |snapshot| match command {
        "run" => snapshot.execute_entry(&execution_options),
        "test" => snapshot.execute_test(&execution_options),
        _ => unreachable!("validated project execution command"),
    })
    .map_err(|errors| {
        let errors = super::manifest_hint::hint_missing_manifest(errors, manifest_path);
        report(&errors, options.json)
    })?;

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
        ("test", project::ProjectExecutionOutcome::Returned(0)) => {
            if !options.json {
                println!("project tests passed");
            }
            Ok(())
        }
        ("test", project::ProjectExecutionOutcome::Returned(value)) => {
            if !options.json {
                eprintln!("project tests failed with result {value}");
            }
            Err(1)
        }
        (_, project::ProjectExecutionOutcome::LanguageFailure(status)) => {
            if !options.json {
                eprintln!(
                    "project execution failed with language status {}",
                    status.to_json()
                );
            }
            Err(1)
        }
        (_, project::ProjectExecutionOutcome::FuelExhausted) => {
            if !options.json {
                eprintln!("project execution exhausted its step budget");
            }
            Err(1)
        }
        (_, project::ProjectExecutionOutcome::CallDepthExceeded) => {
            if !options.json {
                eprintln!("project execution exceeded its call-depth bound");
            }
            Err(1)
        }
        _ => unreachable!("validated project execution command"),
    }
}

pub(crate) fn build_success(
    target: &str,
    profile: project::ProjectProfile,
    output: &Path,
) -> String {
    let product = match (target, profile) {
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
        ("rust", _) => "Project v8 Native Rust owned-data package",
        ("npm", project::ProjectProfile::FlatOwnedRecordApiV1) => "Project v9 npm package",
        ("npm", project::ProjectProfile::OwnedUtf8ApiV1) => "Project v10 npm package",
        ("npm", project::ProjectProfile::NestedOwnedRecordApiV1) => "Project v11 npm package",
        ("npm", _) => "Project v2 npm package",
        _ => "project web package",
    };
    format!("built {product} {}", output.display())
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
