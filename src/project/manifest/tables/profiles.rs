//! Closed profile lowering for the canonical table manifest.
use super::*;

#[cfg(test)]
mod tests;
#[cfg(test)]
#[path = "profiles/nested_outcome.rs"]
mod nested_outcome_tests;

/// Check the profile-specific rules the frozen schemas encode positionally and
/// return the frozen profile contract the manifest lowers to.
pub(super) fn lower_profile(
    profile: ProjectProfile,
    command: Option<&str>,
    input: Option<&str>,
    capabilities: &[String],
) -> Result<&'static str, Vec<Diagnostic>> {
    if profile == ProjectProfile::SourceCommandV1 {
        if command.is_none() || input != Some(PROJECT_SOURCE_COMMAND_INPUT_V1) {
            return Err(grammar("source-command.v1 requires an explicit command function and input = \"argv-utf8+file-text.v1\""));
        }
        if !crate::source_command::selects(capabilities)
            || !capabilities.windows(2).all(|v| v[0] < v[1])
        {
            return Err(grammar("source-command.v1 requires a strictly sorted nonempty subset of fs.read, process.args.read, process.stderr.write, process.stdout.write, other than stdout alone"));
        }
        return Ok(PROJECT_SCHEMA_V26);
    }
    if profile == ProjectProfile::SourceCommandResourceOutputV1 {
        if command.is_none() || input != Some(PROJECT_SOURCE_COMMAND_INPUT_V1) {
            return Err(grammar("source-command.resource-output.v1 requires an explicit command function and input = \"argv-utf8+file-text.v1\""));
        }
        if !crate::source_command::selects_resource_output(capabilities)
            || !capabilities.windows(2).all(|v| v[0] < v[1])
        {
            return Err(grammar("source-command.resource-output.v1 requires a strictly sorted nonempty subset of fs.read, process.args.read, process.stderr.write, process.stdout.write, other than stdout alone"));
        }
        return Ok(PROJECT_SCHEMA_V28);
    }
    let profile_name = profile.name().unwrap_or("scalar");
    let (schema, expected_input, expected_capabilities): (&str, Option<&str>, &[&str]) =
        match profile {
            ProjectProfile::ProcessIoV1 => {
                (PROJECT_SCHEMA_V18, None, &PROJECT_PROCESS_CAPABILITIES_V1)
            }
            ProjectProfile::EnvironmentIoV1 => (
                PROJECT_SCHEMA_V17,
                None,
                &PROJECT_ENVIRONMENT_CAPABILITIES_V1,
            ),
            ProjectProfile::FilesystemIoV2 => (
                PROJECT_SCHEMA_V15,
                None,
                &PROJECT_FILESYSTEM_CAPABILITIES_V1,
            ),
            ProjectProfile::FilesystemIoV3 => (
                PROJECT_SCHEMA_V19,
                None,
                &PROJECT_FILESYSTEM_CAPABILITIES_V1,
            ),
            ProjectProfile::FilesystemIoV1 => (
                PROJECT_SCHEMA_V14,
                None,
                &PROJECT_FILESYSTEM_CAPABILITIES_V1,
            ),
            ProjectProfile::SourceCommandV1 => unreachable!("handled above"),
            ProjectProfile::SourceCommandResourceOutputV1 => unreachable!("handled above"),
            ProjectProfile::ScalarV1 => (PROJECT_SCHEMA, None, &[]),
            ProjectProfile::UsefulTextConsumerV1 => (PROJECT_SCHEMA_V2, None, &[]),
            ProjectProfile::UsefulDataV2 => (PROJECT_SCHEMA_V16, None, &[]),
            ProjectProfile::UsefulDataV1 => (PROJECT_SCHEMA_V3, None, &[]),
            ProjectProfile::UsefulDataCommandV1 => (
                PROJECT_SCHEMA_V4,
                None,
                &[PROJECT_COMMAND_STDOUT_CAPABILITY],
            ),
            ProjectProfile::UsefulDataCommandV2 => (
                PROJECT_SCHEMA_V5,
                Some(PROJECT_COMMAND_INPUT_V1),
                &PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
            ),
            ProjectProfile::LanguageCommandIoV1 => (
                PROJECT_SCHEMA_V6,
                Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1),
                &PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
            ),
            ProjectProfile::StdinStreamCommandIoV1
            | ProjectProfile::StdinStreamCommandIoV2
            | ProjectProfile::StdinStreamTextCommandIoV1
            | ProjectProfile::StdinStreamDataCommandIoV1
            | ProjectProfile::StdinStreamDataCommandIoV2
            | ProjectProfile::StdinStreamOwnedDataCommandIoV1 => (
                if profile == ProjectProfile::StdinStreamOwnedDataCommandIoV1 {
                    PROJECT_SCHEMA_V30
                } else if profile == ProjectProfile::StdinStreamDataCommandIoV2 {
                    PROJECT_SCHEMA_V29
                } else if profile == ProjectProfile::StdinStreamDataCommandIoV1 {
                    PROJECT_SCHEMA_V27
                } else if profile == ProjectProfile::StdinStreamTextCommandIoV1 {
                    PROJECT_SCHEMA_V25
                } else if profile == ProjectProfile::StdinStreamCommandIoV2 {
                    PROJECT_SCHEMA_V24
                } else {
                    PROJECT_SCHEMA_V23
                },
                Some(PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1),
                &PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
            ),
            ProjectProfile::StdinStreamCollectionRecordCommandIoV1 => (
                PROJECT_SCHEMA_V31,
                Some(PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1),
                &PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
            ),
            ProjectProfile::StdinStreamNestedOutcomeCommandIoV1 => (
                PROJECT_SCHEMA_V32,
                Some(PROJECT_LANGUAGE_COMMAND_STREAM_INPUT_V1),
                &PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
            ),
            ProjectProfile::LineCommandIoV1 => (
                PROJECT_SCHEMA_V7,
                Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1),
                &PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
            ),
            ProjectProfile::OwnedDataApiV1 => (PROJECT_SCHEMA_V8, None, &[]),
            ProjectProfile::FlatOwnedRecordApiV1 => (PROJECT_SCHEMA_V9, None, &[]),
            ProjectProfile::OwnedUtf8ApiV1 => (PROJECT_SCHEMA_V10, None, &[]),
            ProjectProfile::NestedOwnedRecordApiV1 => (PROJECT_SCHEMA_V11, None, &[]),
            ProjectProfile::PublicGenericWasmProviderV1 => (PROJECT_SCHEMA_V20, None, &[]),
            ProjectProfile::SourceLocalFutureV1 => (PROJECT_SCHEMA_V21, None, &[]),
            ProjectProfile::SourceLocalFutureIndexedRustV1 => (PROJECT_SCHEMA_V22, None, &[]),
            ProjectProfile::NetworkCommandIoV1 => (
                PROJECT_SCHEMA_V12,
                Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1),
                &PROJECT_NETWORK_COMMAND_CAPABILITIES_V1,
            ),
            ProjectProfile::HttpsCommandIoV1 => (
                PROJECT_SCHEMA_V13,
                Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1),
                &PROJECT_HTTPS_COMMAND_CAPABILITIES_V1,
            ),
        };
    let is_command_profile = !expected_capabilities.is_empty();
    match (is_command_profile, command) {
        (true, None) => {
            return Err(grammar(format!(
                "{LABEL} profile `{profile_name}` requires a `[command]` table with `function`"
            )));
        }
        (false, Some(_)) => {
            return Err(grammar(format!(
                "{LABEL} profile `{profile_name}` does not admit a `[command]` table"
            )));
        }
        _ => {}
    }
    if input != expected_input {
        return Err(grammar(match expected_input {
            Some(expected) => format!(
                "{LABEL} profile `{profile_name}` requires `[command] input = \"{expected}\"`"
            ),
            None => format!("{LABEL} profile `{profile_name}` does not admit `[command] input`"),
        }));
    }
    if if profile == ProjectProfile::EnvironmentIoV1 {
        !valid_environment_capabilities(capabilities)
    } else if profile == ProjectProfile::ProcessIoV1 {
        !valid_process_capabilities(capabilities)
    } else {
        !capabilities
            .iter()
            .map(String::as_str)
            .eq(expected_capabilities.iter().copied())
    } {
        return Err(grammar(if expected_capabilities.is_empty() {
            format!("{LABEL} profile `{profile_name}` does not admit a `[capabilities]` table")
        } else {
            format!(
                "{LABEL} profile `{profile_name}` requires `[capabilities] required = {}`",
                super::super::render_array(
                    &expected_capabilities
                        .iter()
                        .map(|capability| (*capability).to_owned())
                        .collect::<Vec<_>>()
                )
            )
        }));
    }
    Ok(schema)
}

pub(super) fn profile_by_name(name: &str) -> Option<ProjectProfile> {
    Some(match name {
        PROJECT_PROFILE_SOURCE_COMMAND_V1 => ProjectProfile::SourceCommandV1,
        PROJECT_PROFILE_SOURCE_COMMAND_RESOURCE_OUTPUT_V1 => {
            ProjectProfile::SourceCommandResourceOutputV1
        }
        PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1 => ProjectProfile::UsefulTextConsumerV1,
        PROJECT_PROFILE_USEFUL_DATA_V2 => ProjectProfile::UsefulDataV2,
        PROJECT_PROFILE_USEFUL_DATA_V1 => ProjectProfile::UsefulDataV1,
        PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1 => ProjectProfile::UsefulDataCommandV1,
        PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2 => ProjectProfile::UsefulDataCommandV2,
        PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1 => ProjectProfile::LanguageCommandIoV1,
        PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V1 => ProjectProfile::StdinStreamCommandIoV1,
        PROJECT_PROFILE_STDIN_STREAM_COMMAND_IO_V2 => ProjectProfile::StdinStreamCommandIoV2,
        PROJECT_PROFILE_STDIN_STREAM_TEXT_COMMAND_IO_V1 => {
            ProjectProfile::StdinStreamTextCommandIoV1
        }
        PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V2 => {
            ProjectProfile::StdinStreamDataCommandIoV2
        }
        PROJECT_PROFILE_STDIN_STREAM_OWNED_DATA_COMMAND_IO_V1 => {
            ProjectProfile::StdinStreamOwnedDataCommandIoV1
        }
        PROJECT_PROFILE_STDIN_STREAM_COLLECTION_RECORD_COMMAND_IO_V1 => {
            ProjectProfile::StdinStreamCollectionRecordCommandIoV1
        }
        PROJECT_PROFILE_STDIN_STREAM_NESTED_OUTCOME_COMMAND_IO_V1 => {
            ProjectProfile::StdinStreamNestedOutcomeCommandIoV1
        }
        PROJECT_PROFILE_STDIN_STREAM_DATA_COMMAND_IO_V1 => {
            ProjectProfile::StdinStreamDataCommandIoV1
        }
        PROJECT_PROFILE_LINE_COMMAND_IO_V1 => ProjectProfile::LineCommandIoV1,
        PROJECT_PROFILE_OWNED_DATA_API_V1 => ProjectProfile::OwnedDataApiV1,
        PROJECT_PROFILE_FLAT_OWNED_RECORD_API_V1 => ProjectProfile::FlatOwnedRecordApiV1,
        PROJECT_PROFILE_OWNED_UTF8_API_V1 => ProjectProfile::OwnedUtf8ApiV1,
        PROJECT_PROFILE_NESTED_OWNED_RECORD_API_V1 => ProjectProfile::NestedOwnedRecordApiV1,
        PROJECT_PROFILE_PUBLIC_GENERIC_WASM_PROVIDER_V1 => {
            ProjectProfile::PublicGenericWasmProviderV1
        }
        PROJECT_PROFILE_SOURCE_LOCAL_FUTURE_V1 => ProjectProfile::SourceLocalFutureV1,
        PROJECT_PROFILE_SOURCE_LOCAL_FUTURE_INDEXED_RUST_V1 => {
            ProjectProfile::SourceLocalFutureIndexedRustV1
        }
        PROJECT_PROFILE_NETWORK_COMMAND_IO_V1 => ProjectProfile::NetworkCommandIoV1,
        PROJECT_PROFILE_HTTPS_COMMAND_IO_V1 => ProjectProfile::HttpsCommandIoV1,
        PROJECT_PROFILE_ENVIRONMENT_IO_V1 => ProjectProfile::EnvironmentIoV1,
        PROJECT_PROFILE_PROCESS_IO_V1 => ProjectProfile::ProcessIoV1,
        PROJECT_PROFILE_FILESYSTEM_IO_V2 => ProjectProfile::FilesystemIoV2,
        PROJECT_PROFILE_FILESYSTEM_IO_V3 => ProjectProfile::FilesystemIoV3,
        PROJECT_PROFILE_FILESYSTEM_IO_V1 => ProjectProfile::FilesystemIoV1,
        _ => return None,
    })
}
