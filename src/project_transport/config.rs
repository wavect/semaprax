use std::ffi::OsString;
use std::path::PathBuf;

use super::framing::StdioLimits;
use crate::proof_export::installed::ToolKind;

const DEFAULT_MANIFEST: &str = "semaprax.toml";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ServerConfig {
    manifest_path: PathBuf,
    limits: StdioLimits,
    profile: ServerProfile,
    law_tool: Option<LawToolConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct LawToolConfig {
    pub(crate) kind: ToolKind,
    pub(crate) executable: PathBuf,
    pub(crate) version_line: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ServerProfile {
    ReadOnlyV2,
    ProjectRenameV1,
    ProjectWorkflowV1,
    ProjectOwnedDataV1,
    ProjectPublicApiV1,
    ProjectLawWorkflowV1,
}

impl ServerProfile {
    pub(crate) const fn accepts_project_profile(
        self,
        profile: crate::project::ProjectProfile,
    ) -> bool {
        match self {
            Self::ProjectOwnedDataV1 => {
                matches!(profile, crate::project::ProjectProfile::OwnedDataApiV1)
            }
            Self::ProjectPublicApiV1 => matches!(
                profile,
                crate::project::ProjectProfile::OwnedDataApiV1
                    | crate::project::ProjectProfile::FlatOwnedRecordApiV1
                    | crate::project::ProjectProfile::OwnedUtf8ApiV1
                    | crate::project::ProjectProfile::NestedOwnedRecordApiV1
            ),
            Self::ReadOnlyV2
            | Self::ProjectRenameV1
            | Self::ProjectWorkflowV1
            | Self::ProjectLawWorkflowV1 => true,
        }
    }
}

impl ServerConfig {
    pub(crate) fn parse(arguments: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut arguments = arguments.into_iter();
        let _program = arguments.next();
        let mut stdio = false;
        let mut manifest_path = None;
        let mut max_request_bytes = None;
        let mut max_response_bytes = None;
        let mut allow_project_rename = false;
        let mut allow_project_workflow = false;
        let mut allow_project_owned_data = false;
        let mut allow_project_public_api = false;
        let mut allow_project_law_workflow = false;
        let mut law_tool = None;
        let mut law_executable = None;
        let mut law_version_line = None;

        while let Some(argument) = arguments.next() {
            let Some(option) = argument.to_str() else {
                return Err("semapraxd options must be UTF-8".to_owned());
            };
            match option {
                "--stdio" if !stdio => stdio = true,
                "--stdio" => return Err("--stdio may not be repeated".to_owned()),
                "--manifest-path" if manifest_path.is_none() => {
                    manifest_path = Some(required_path(&mut arguments, option)?);
                }
                "--manifest-path" => {
                    return Err("--manifest-path may not be repeated".to_owned());
                }
                "--max-request-bytes" if max_request_bytes.is_none() => {
                    max_request_bytes = Some(required_number(&mut arguments, option)?);
                }
                "--max-request-bytes" => {
                    return Err("--max-request-bytes may not be repeated".to_owned());
                }
                "--max-response-bytes" if max_response_bytes.is_none() => {
                    max_response_bytes = Some(required_number(&mut arguments, option)?);
                }
                "--max-response-bytes" => {
                    return Err("--max-response-bytes may not be repeated".to_owned());
                }
                "--allow-project-rename" if !allow_project_rename => {
                    allow_project_rename = true;
                }
                "--allow-project-rename" => {
                    return Err("--allow-project-rename may not be repeated".to_owned());
                }
                "--allow-project-workflow" if !allow_project_workflow => {
                    allow_project_workflow = true;
                }
                "--allow-project-workflow" => {
                    return Err("--allow-project-workflow may not be repeated".to_owned());
                }
                "--allow-project-owned-data" if !allow_project_owned_data => {
                    allow_project_owned_data = true;
                }
                "--allow-project-owned-data" => {
                    return Err("--allow-project-owned-data may not be repeated".to_owned());
                }
                "--allow-project-public-api" if !allow_project_public_api => {
                    allow_project_public_api = true;
                }
                "--allow-project-public-api" => {
                    return Err("--allow-project-public-api may not be repeated".to_owned());
                }
                "--allow-project-law-workflow" if !allow_project_law_workflow => {
                    allow_project_law_workflow = true;
                }
                "--allow-project-law-workflow" => {
                    return Err("--allow-project-law-workflow may not be repeated".to_owned());
                }
                "--law-tool" if law_tool.is_none() => {
                    let value = required_text(&mut arguments, option)?;
                    law_tool = Some(match value.as_str() {
                        "z3" => ToolKind::Z3,
                        "lean" => ToolKind::Lean,
                        _ => return Err("--law-tool must be z3 or lean".to_owned()),
                    });
                }
                "--law-executable" if law_executable.is_none() => {
                    law_executable = Some(required_path(&mut arguments, option)?);
                }
                "--law-version-line" if law_version_line.is_none() => {
                    law_version_line = Some(required_text(&mut arguments, option)?);
                }
                unknown => return Err(format!("unknown semapraxd option `{unknown}`")),
            }
        }
        if !stdio {
            return Err("semapraxd requires --stdio".to_owned());
        }
        if allow_project_law_workflow {
            if allow_project_rename
                || allow_project_workflow
                || allow_project_owned_data
                || allow_project_public_api
            {
                return Err(
                    "--allow-project-law-workflow is exclusive to the selected-law profile"
                        .to_owned(),
                );
            }
            if law_tool.is_none() || law_executable.is_none() || law_version_line.is_none() {
                return Err("selected-law workflow requires --law-tool, --law-executable and --law-version-line".to_owned());
            }
            if !manifest_path
                .as_ref()
                .is_some_and(|path: &PathBuf| path.is_absolute())
                || !law_executable
                    .as_ref()
                    .is_some_and(|path: &PathBuf| path.is_absolute())
            {
                return Err(
                    "selected-law workflow requires absolute manifest and executable paths"
                        .to_owned(),
                );
            }
        } else if law_tool.is_some() || law_executable.is_some() || law_version_line.is_some() {
            return Err("law tool options require --allow-project-law-workflow".to_owned());
        }
        let defaults = StdioLimits::default();
        let limits = StdioLimits::new(
            max_request_bytes.unwrap_or(defaults.request_bytes()),
            max_response_bytes.unwrap_or(defaults.response_bytes()),
        )?;
        if allow_project_rename && allow_project_workflow {
            return Err(
                "--allow-project-rename and --allow-project-workflow are mutually exclusive"
                    .to_owned(),
            );
        }
        if allow_project_owned_data
            && (allow_project_rename || allow_project_workflow || allow_project_public_api)
        {
            return Err(
                "--allow-project-owned-data is mutually exclusive with every other Project daemon authority profile"
                    .to_owned(),
            );
        }
        if allow_project_public_api && (allow_project_rename || allow_project_workflow) {
            return Err(
                "--allow-project-public-api is mutually exclusive with every other Project daemon authority profile"
                    .to_owned(),
            );
        }
        Ok(Self {
            manifest_path: manifest_path.unwrap_or_else(|| PathBuf::from(DEFAULT_MANIFEST)),
            limits,
            profile: if allow_project_public_api {
                ServerProfile::ProjectPublicApiV1
            } else if allow_project_law_workflow {
                ServerProfile::ProjectLawWorkflowV1
            } else if allow_project_owned_data {
                ServerProfile::ProjectOwnedDataV1
            } else if allow_project_workflow {
                ServerProfile::ProjectWorkflowV1
            } else if allow_project_rename {
                ServerProfile::ProjectRenameV1
            } else {
                ServerProfile::ReadOnlyV2
            },
            law_tool: if allow_project_law_workflow {
                Some(LawToolConfig {
                    kind: law_tool.expect("checked law tool"),
                    executable: law_executable.expect("checked law executable"),
                    version_line: law_version_line.expect("checked version pin"),
                })
            } else {
                None
            },
        })
    }

    pub(crate) fn manifest_path(&self) -> &std::path::Path {
        &self.manifest_path
    }

    pub(crate) const fn limits(&self) -> StdioLimits {
        self.limits
    }

    pub(crate) const fn profile(&self) -> ServerProfile {
        self.profile
    }

    pub(crate) fn law_tool(&self) -> Option<&LawToolConfig> {
        self.law_tool.as_ref()
    }
}

fn required_text(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<String, String> {
    let value = arguments
        .next()
        .ok_or_else(|| format!("{option} requires a value"))?;
    let value = value
        .into_string()
        .map_err(|_| format!("{option} must be UTF-8"))?;
    if value.is_empty() || value.len() > 1024 {
        return Err(format!("{option} requires 1..=1024 bytes"));
    }
    Ok(value)
}

fn required_path(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<PathBuf, String> {
    let value = arguments
        .next()
        .ok_or_else(|| format!("{option} requires a path"))?;
    if value.is_empty() {
        return Err(format!("{option} requires a nonempty path"));
    }
    Ok(PathBuf::from(value))
}

fn required_number(
    arguments: &mut impl Iterator<Item = OsString>,
    option: &str,
) -> Result<usize, String> {
    let value = arguments
        .next()
        .ok_or_else(|| format!("{option} requires a number"))?;
    let value = value
        .to_str()
        .ok_or_else(|| format!("{option} requires an ASCII decimal number"))?;
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!("{option} requires a canonical decimal number"));
    }
    value
        .parse::<usize>()
        .map_err(|_| format!("{option} number is outside the host usize range"))
}

#[cfg(test)]
mod tests {
    use super::{ServerConfig, ServerProfile, DEFAULT_MANIFEST};
    use std::ffi::OsString;

    fn parse(arguments: &[&str]) -> Result<ServerConfig, String> {
        ServerConfig::parse(arguments.iter().map(OsString::from))
    }

    #[test]
    fn stdio_defaults_bind_one_fixed_manifest() {
        let config = parse(&["semapraxd", "--stdio"]).unwrap();
        assert_eq!(config.manifest_path().to_str(), Some(DEFAULT_MANIFEST));
        assert_eq!(config.profile(), ServerProfile::ReadOnlyV2);
    }

    #[test]
    fn mutation_authority_is_explicit_and_nonrepeating() {
        let config = parse(&["semapraxd", "--stdio", "--allow-project-rename"]).unwrap();
        assert_eq!(config.profile(), ServerProfile::ProjectRenameV1);
        assert!(parse(&[
            "semapraxd",
            "--stdio",
            "--allow-project-rename",
            "--allow-project-rename"
        ])
        .is_err());

        let owned = parse(&["semapraxd", "--stdio", "--allow-project-owned-data"]).unwrap();
        assert_eq!(owned.profile(), ServerProfile::ProjectOwnedDataV1);
        assert!(parse(&[
            "semapraxd",
            "--stdio",
            "--allow-project-owned-data",
            "--allow-project-owned-data"
        ])
        .is_err());
        for conflicting in [
            "--allow-project-rename",
            "--allow-project-workflow",
            "--allow-project-public-api",
        ] {
            assert!(parse(&[
                "semapraxd",
                "--stdio",
                "--allow-project-owned-data",
                conflicting
            ])
            .is_err());
        }

        let public = parse(&["semapraxd", "--stdio", "--allow-project-public-api"]).unwrap();
        assert_eq!(public.profile(), ServerProfile::ProjectPublicApiV1);
        assert!(parse(&[
            "semapraxd",
            "--stdio",
            "--allow-project-public-api",
            "--allow-project-public-api"
        ])
        .is_err());
        for conflicting in ["--allow-project-rename", "--allow-project-workflow"] {
            assert!(parse(&[
                "semapraxd",
                "--stdio",
                "--allow-project-public-api",
                conflicting
            ])
            .is_err());
        }

        let workflow = parse(&["semapraxd", "--stdio", "--allow-project-workflow"]).unwrap();
        assert_eq!(workflow.profile(), ServerProfile::ProjectWorkflowV1);
        assert!(parse(&[
            "semapraxd",
            "--stdio",
            "--allow-project-workflow",
            "--allow-project-workflow"
        ])
        .is_err());
        assert!(parse(&[
            "semapraxd",
            "--stdio",
            "--allow-project-rename",
            "--allow-project-workflow"
        ])
        .is_err());
    }

    #[test]
    fn startup_authority_is_closed_and_nonrepeating() {
        assert!(parse(&["semapraxd"]).is_err());
        assert!(parse(&["semapraxd", "--stdio", "--stdio"]).is_err());
        assert!(parse(&["semapraxd", "--stdio", "--manifest-path"]).is_err());
        assert!(parse(&[
            "semapraxd",
            "--stdio",
            "--manifest-path",
            "a",
            "--manifest-path",
            "b"
        ])
        .is_err());
        assert!(parse(&["semapraxd", "--stdio", "--unknown"]).is_err());
        assert!(parse(&["semapraxd", "--stdio", "--max-request-bytes", "01"]).is_err());
    }

    #[test]
    fn public_api_profile_accepts_exactly_project_v8_through_v11() {
        use crate::project::ProjectProfile;
        let public = ServerProfile::ProjectPublicApiV1;
        for profile in [
            ProjectProfile::OwnedDataApiV1,
            ProjectProfile::FlatOwnedRecordApiV1,
            ProjectProfile::OwnedUtf8ApiV1,
            ProjectProfile::NestedOwnedRecordApiV1,
        ] {
            assert!(public.accepts_project_profile(profile));
        }
        for profile in [
            ProjectProfile::ScalarV1,
            ProjectProfile::UsefulTextConsumerV1,
            ProjectProfile::UsefulDataV1,
            ProjectProfile::UsefulDataCommandV1,
            ProjectProfile::UsefulDataCommandV2,
            ProjectProfile::LanguageCommandIoV1,
            ProjectProfile::LineCommandIoV1,
            ProjectProfile::NetworkCommandIoV1,
            ProjectProfile::HttpsCommandIoV1,
        ] {
            assert!(!public.accepts_project_profile(profile));
        }
        assert!(ServerProfile::ProjectOwnedDataV1
            .accepts_project_profile(ProjectProfile::OwnedDataApiV1));
        assert!(!ServerProfile::ProjectOwnedDataV1
            .accepts_project_profile(ProjectProfile::FlatOwnedRecordApiV1));
    }
}
