#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::io::Write as _;
use std::path::PathBuf;
use std::process::ExitCode;

use semaprax_native_rust_interop::build_project_native_rust_sdk;

const RESULT_SCHEMA: &str = "semaprax.project-native-rust-sdk-result.v1";

struct ProjectCommand {
    manifest_path: PathBuf,
    output: PathBuf,
}

#[derive(Clone, Copy)]
enum CliError {
    Usage,
    MissingValue,
    RepeatedOption,
    UnknownOption,
    RelativeOutput,
    ResultOutput,
}

impl CliError {
    const fn code(self) -> &'static str {
        match self {
            Self::Usage
            | Self::MissingValue
            | Self::RepeatedOption
            | Self::UnknownOption
            | Self::RelativeOutput => "SPX-B112",
            Self::ResultOutput => "SPX-I233",
        }
    }

    const fn message(self) -> &'static str {
        match self {
            Self::Usage => {
                "expected `project --manifest-path <path> --output <fresh-absolute-path>`"
            }
            Self::MissingValue => "Native Rust SDK option requires a value",
            Self::RepeatedOption => "Native Rust SDK option may not be repeated",
            Self::UnknownOption => "unknown Native Rust SDK option",
            Self::RelativeOutput => "Native Rust SDK output must be absolute",
            Self::ResultOutput => "Native Rust SDK result publication failed",
        }
    }

    fn report(self) -> ExitCode {
        eprintln!("{}: {}", self.code(), self.message());
        match self {
            Self::Usage
            | Self::MissingValue
            | Self::RepeatedOption
            | Self::UnknownOption
            | Self::RelativeOutput => ExitCode::from(2),
            Self::ResultOutput => ExitCode::FAILURE,
        }
    }
}

fn option_value(arguments: &mut impl Iterator<Item = OsString>) -> Result<OsString, CliError> {
    let value = arguments.next().ok_or(CliError::MissingValue)?;
    if value.is_empty()
        || value == OsStr::new("--manifest-path")
        || value == OsStr::new("--output")
        || value.to_str().is_some_and(|value| value.starts_with('-'))
    {
        return Err(CliError::MissingValue);
    }
    Ok(value)
}

fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<ProjectCommand, CliError> {
    let mut arguments = arguments.into_iter();
    if arguments.next().as_deref() != Some(OsStr::new("project")) {
        return Err(CliError::Usage);
    }

    let mut manifest_path = None;
    let mut output = None;
    while let Some(argument) = arguments.next() {
        if argument == OsStr::new("--manifest-path") {
            if manifest_path.is_some() {
                return Err(CliError::RepeatedOption);
            }
            manifest_path = Some(PathBuf::from(option_value(&mut arguments)?));
        } else if argument == OsStr::new("--output") {
            if output.is_some() {
                return Err(CliError::RepeatedOption);
            }
            output = Some(PathBuf::from(option_value(&mut arguments)?));
        } else {
            return Err(CliError::UnknownOption);
        }
    }

    let manifest_path = manifest_path.ok_or(CliError::Usage)?;
    let output = output.ok_or(CliError::Usage)?;
    if !output.is_absolute() {
        return Err(CliError::RelativeOutput);
    }
    Ok(ProjectCommand {
        manifest_path,
        output,
    })
}

fn run(command: ProjectCommand) -> Result<String, ExitCode> {
    let bundle = build_project_native_rust_sdk(&command.manifest_path, &command.output).map_err(
        |diagnostics| {
            for diagnostic in diagnostics {
                eprintln!("{}: Project Native Rust SDK build failed", diagnostic.code);
            }
            ExitCode::FAILURE
        },
    )?;
    let mut result = BTreeMap::new();
    result.insert("crate_name", bundle.crate_name());
    result.insert("manifest_digest", bundle.manifest_digest());
    result.insert("project_revision", bundle.project_revision());
    result.insert("schema", RESULT_SCHEMA);
    result.insert("subject_digest", bundle.subject_digest());
    result.insert("target_triple", bundle.target_triple());
    result.insert("workspace_revision", bundle.workspace_revision());
    serde_json::to_string(&result).map_err(|_| CliError::ResultOutput.report())
}

fn main() -> ExitCode {
    let command = match parse(std::env::args_os().skip(1)) {
        Ok(command) => command,
        Err(error) => return error.report(),
    };
    let result = match run(command) {
        Ok(result) => result,
        Err(code) => return code,
    };
    let mut stdout = std::io::stdout().lock();
    if writeln!(stdout, "{result}").is_err() {
        return CliError::ResultOutput.report();
    }
    ExitCode::SUCCESS
}
