//! Frozen command-input manifests whose fields share Project v6 ordering.

use super::{
    ProjectProfile, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2, PROJECT_LANGUAGE_COMMAND_INPUT_V1,
    PROJECT_SCHEMA_V23, PROJECT_SCHEMA_V24, PROJECT_SCHEMA_V25, PROJECT_SCHEMA_V6,
};
use crate::diagnostic::Diagnostic;
use crate::project::profile::{
    PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1, PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1,
    PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V1, PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V2,
    PROJECT_PROFILE_STDIN_STREAM_TEXT_COMMAND_IO_V1,
};

pub(super) type ParsedCommandManifest = (
    &'static str,
    String,
    Option<String>,
    ProjectProfile,
    String,
    Vec<String>,
    Vec<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
);

pub(super) fn parse(
    lines: &[&str],
    schema: &str,
) -> Result<ParsedCommandManifest, Vec<Diagnostic>> {
    let (schema, profile_name, profile, input_name, label) = match schema {
        PROJECT_SCHEMA_V6 => (
            PROJECT_SCHEMA_V6,
            PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1,
            ProjectProfile::LanguageCommandIoV1,
            PROJECT_LANGUAGE_COMMAND_INPUT_V1,
            "Project v6",
        ),
        PROJECT_SCHEMA_V23 => (
            PROJECT_SCHEMA_V23,
            PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V1,
            ProjectProfile::StdinStreamCommandIoV1,
            PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1,
            "Project v23",
        ),
        PROJECT_SCHEMA_V24 => (
            PROJECT_SCHEMA_V24,
            PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V2,
            ProjectProfile::StdinStreamCommandIoV2,
            PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1,
            "Project v24",
        ),
        PROJECT_SCHEMA_V25 => (
            PROJECT_SCHEMA_V25,
            PROJECT_PROFILE_STDIN_STREAM_TEXT_COMMAND_IO_V1,
            ProjectProfile::StdinStreamTextCommandIoV1,
            PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1,
            "Project v25",
        ),
        _ => unreachable!("only the frozen command input schemas use this parser"),
    };
    if lines.len() != 12 || lines.last() != Some(&"") {
        return Err(super::grammar(format!(
            "{label} manifest must contain exactly eleven ordered assignments and one terminal LF"
        )));
    }
    let version = super::parse_string_assignment(lines[2], "version")?;
    if !super::valid_semver(&version) {
        return Err(super::grammar(format!(
            "{label} version must be canonical Semantic Versioning text of at most 128 bytes"
        )));
    }
    if super::parse_string_assignment(lines[3], "profile")? != profile_name {
        return Err(super::grammar(format!(
            "{label} profile is not {profile_name}"
        )));
    }
    let command = super::parse_string_assignment(lines[7], "command")?;
    let input = super::parse_string_assignment(lines[8], "input")?;
    if input != input_name {
        return Err(super::grammar(format!("{label} input is not {input_name}")));
    }
    let capabilities = super::parse_array_assignment(lines[9], "capabilities")?;
    if !capabilities
        .iter()
        .map(String::as_str)
        .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2)
    {
        return Err(super::grammar(format!(
            "{label} capabilities must be exactly [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]"
        )));
    }
    Ok((
        schema,
        super::parse_string_assignment(lines[1], "name")?,
        Some(version),
        profile,
        super::parse_string_assignment(lines[4], "entry")?,
        super::parse_array_assignment(lines[5], "sources")?,
        super::parse_array_assignment(lines[6], "web_exports")?,
        Some(command),
        Some(input),
        capabilities,
        super::parse_array_assignment(lines[10], "tests")?,
    ))
}
