use crate::diagnostic::Diagnostic;

use super::profile::{
    ProjectProfile, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2, PROJECT_COMMAND_INPUT_V1,
    PROJECT_COMMAND_STDOUT_CAPABILITY, PROJECT_LANGUAGE_COMMAND_INPUT_V1,
    PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1, PROJECT_PROFILE_LINE_COMMAND_IO_V1,
    PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1, PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2,
    PROJECT_PROFILE_USEFUL_DATA_V1, PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1,
};

/// Frozen scalar Project Manifest v1 schema.
pub const PROJECT_SCHEMA: &str = "semaprax.project.v1";
/// Additive Project Manifest v2 schema used by the Useful Text Consumer
/// profile. V1 parsing and rendering remain byte-for-byte unchanged.
pub const PROJECT_SCHEMA_V2: &str = "semaprax.project.v2";
/// Additive Project Manifest v3 schema used by the Portable Indexed Byte Data
/// public adapter. V1 and v2 parsing and rendering remain byte-for-byte
/// unchanged.
pub const PROJECT_SCHEMA_V3: &str = "semaprax.project.v3";
/// Additive Project Manifest v4 schema for one exact compiler-free command
/// adapter over the Portable Indexed Byte Data public ABI.
pub const PROJECT_SCHEMA_V4: &str = "semaprax.project.v4";
/// Additive Project Manifest v5 schema for the fixed native/Web command
/// adapter. V1-v4 parsing and rendering remain byte-for-byte unchanged.
pub const PROJECT_SCHEMA_V5: &str = "semaprax.project.v5";
/// Additive Project Manifest v6 schema for compiler-owned language command
/// input and dual success-only output transcripts.
pub const PROJECT_SCHEMA_V6: &str = "semaprax.project.v6";
/// Additive Project Manifest v7 schema for bounded line-command processing.
pub const PROJECT_SCHEMA_V7: &str = "semaprax.project.v7";
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_NAME_BYTES: usize = 64;
pub const MAX_VERSION_BYTES: usize = 128;
pub const MAX_MODULE_BYTES: usize = 240;
pub const MAX_PATH_BYTES: usize = 240;
pub const MAX_STABLE_ID_BYTES: usize = 128;
pub const MAX_SOURCES: usize = 16;
pub const MAX_WEB_EXPORTS: usize = 32;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// One exact, closed Project manifest. The schema selects the frozen v1
/// scalar shape or one additive, schema-bound public profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectManifest {
    schema: &'static str,
    name: String,
    package_version: Option<String>,
    profile: ProjectProfile,
    entry: String,
    sources: Vec<String>,
    web_exports: Vec<String>,
    command: Option<String>,
    command_input: Option<String>,
    capabilities: Vec<String>,
    test_module: String,
}

impl ProjectManifest {
    /// Parse one frozen Project v1-v7 canonical manifest.
    pub fn parse(source: &str) -> Result<Self, Vec<Diagnostic>> {
        if source.len() > MAX_MANIFEST_BYTES {
            return Err(capacity("manifest_bytes", MAX_MANIFEST_BYTES));
        }
        if source.as_bytes().contains(&0) || source.starts_with('\u{feff}') || source.contains('\r')
        {
            return Err(grammar("Project v1 manifest is not canonical UTF-8 TOML"));
        }
        let lines = source.split('\n').collect::<Vec<_>>();
        let schema = lines
            .first()
            .copied()
            .ok_or_else(|| grammar("Project manifest is empty"))
            .and_then(|line| parse_string_assignment(line, "schema"))?;
        let (schema, name, package_version, profile, entry, sources, web_exports, command, command_input, capabilities, tests) =
            match schema.as_str() {
                PROJECT_SCHEMA => {
                    if lines.len() != 7 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v1 manifest must contain exactly six ordered assignments and one terminal LF",
                        ));
                    }
                    (
                        PROJECT_SCHEMA,
                        parse_string_assignment(lines[1], "name")?,
                        None,
                        ProjectProfile::ScalarV1,
                        parse_string_assignment(lines[2], "entry")?,
                        parse_array_assignment(lines[3], "sources")?,
                        parse_array_assignment(lines[4], "web_exports")?,
                        None,
                        None,
                        Vec::new(),
                        parse_array_assignment(lines[5], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V2 => {
                    if lines.len() != 9 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v2 manifest must contain exactly eight ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v2 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    let profile = parse_string_assignment(lines[3], "profile")?;
                    if profile != PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1 {
                        return Err(grammar(
                            "Project v2 profile is not useful-text-consumer.v1",
                        ));
                    }
                    (
                        PROJECT_SCHEMA_V2,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::UsefulTextConsumerV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        None,
                        None,
                        Vec::new(),
                        parse_array_assignment(lines[7], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V3 => {
                    if lines.len() != 9 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v3 manifest must contain exactly eight ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v3 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    let profile = parse_string_assignment(lines[3], "profile")?;
                    if profile != PROJECT_PROFILE_USEFUL_DATA_V1 {
                        return Err(grammar("Project v3 profile is not useful-data.v1"));
                    }
                    (
                        PROJECT_SCHEMA_V3,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::UsefulDataV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        None,
                        None,
                        Vec::new(),
                        parse_array_assignment(lines[7], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V4 => {
                    if lines.len() != 11 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v4 manifest must contain exactly ten ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v4 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    let profile = parse_string_assignment(lines[3], "profile")?;
                    if profile != PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1 {
                        return Err(grammar(
                            "Project v4 profile is not useful-data-command.v1",
                        ));
                    }
                    let command = parse_string_assignment(lines[7], "command")?;
                    let capabilities = parse_array_assignment(lines[8], "capabilities")?;
                    if capabilities != [PROJECT_COMMAND_STDOUT_CAPABILITY] {
                        return Err(grammar(
                            "Project v4 capabilities must be exactly [\"process.stdout.write\"]",
                        ));
                    }
                    (
                        PROJECT_SCHEMA_V4,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::UsefulDataCommandV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        Some(command),
                        None,
                        capabilities,
                        parse_array_assignment(lines[9], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V5 => {
                    if lines.len() != 12 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v5 manifest must contain exactly eleven ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v5 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    let profile = parse_string_assignment(lines[3], "profile")?;
                    if profile != PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2 {
                        return Err(grammar(
                            "Project v5 profile is not useful-data-command.v2",
                        ));
                    }
                    let command = parse_string_assignment(lines[7], "command")?;
                    let input = parse_string_assignment(lines[8], "input")?;
                    if input != PROJECT_COMMAND_INPUT_V1 {
                        return Err(grammar(
                            "Project v5 input is not stdin-bytes+one-utf8-arg.v1",
                        ));
                    }
                    let capabilities = parse_array_assignment(lines[9], "capabilities")?;
                    if !capabilities
                        .iter()
                        .map(String::as_str)
                        .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2)
                    {
                        return Err(grammar(
                            "Project v5 capabilities must be exactly [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
                        ));
                    }
                    (
                        PROJECT_SCHEMA_V5,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::UsefulDataCommandV2,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        Some(command),
                        Some(input),
                        capabilities,
                        parse_array_assignment(lines[10], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V6 => {
                    if lines.len() != 12 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v6 manifest must contain exactly eleven ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v6 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    if parse_string_assignment(lines[3], "profile")?
                        != PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1
                    {
                        return Err(grammar(
                            "Project v6 profile is not language-command-io.v1",
                        ));
                    }
                    let command = parse_string_assignment(lines[7], "command")?;
                    let input = parse_string_assignment(lines[8], "input")?;
                    if input != PROJECT_LANGUAGE_COMMAND_INPUT_V1 {
                        return Err(grammar(
                            "Project v6 input is not argv-utf8+stdin-bytes.v1",
                        ));
                    }
                    let capabilities = parse_array_assignment(lines[9], "capabilities")?;
                    if !capabilities
                        .iter()
                        .map(String::as_str)
                        .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2)
                    {
                        return Err(grammar(
                            "Project v6 capabilities must be exactly [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
                        ));
                    }
                    (
                        PROJECT_SCHEMA_V6,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::LanguageCommandIoV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        Some(command),
                        Some(input),
                        capabilities,
                        parse_array_assignment(lines[10], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V7 => {
                    if lines.len() != 12 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v7 manifest must contain exactly eleven ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v7 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    if parse_string_assignment(lines[3], "profile")?
                        != PROJECT_PROFILE_LINE_COMMAND_IO_V1
                    {
                        return Err(grammar("Project v7 profile is not line-command-io.v1"));
                    }
                    let command = parse_string_assignment(lines[7], "command")?;
                    let input = parse_string_assignment(lines[8], "input")?;
                    if input != PROJECT_LANGUAGE_COMMAND_INPUT_V1 {
                        return Err(grammar(
                            "Project v7 input is not argv-utf8+stdin-bytes.v1",
                        ));
                    }
                    let capabilities = parse_array_assignment(lines[9], "capabilities")?;
                    if !capabilities
                        .iter()
                        .map(String::as_str)
                        .eq(PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2)
                    {
                        return Err(grammar(
                            "Project v7 capabilities must be exactly [\"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
                        ));
                    }
                    (
                        PROJECT_SCHEMA_V7,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::LineCommandIoV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        Some(command),
                        Some(input),
                        capabilities,
                        parse_array_assignment(lines[10], "tests")?,
                    )
                }
                _ => {
                    return Err(grammar(
                        "Project manifest schema is neither semaprax.project.v1, semaprax.project.v2, semaprax.project.v3, semaprax.project.v4, semaprax.project.v5, semaprax.project.v6, nor semaprax.project.v7",
                    ))
                }
            };
        let version_label = match schema {
            PROJECT_SCHEMA => "Project v1",
            PROJECT_SCHEMA_V2 => "Project v2",
            PROJECT_SCHEMA_V3 => "Project v3",
            PROJECT_SCHEMA_V4 => "Project v4",
            PROJECT_SCHEMA_V5 => "Project v5",
            PROJECT_SCHEMA_V6 => "Project v6",
            PROJECT_SCHEMA_V7 => "Project v7",
            _ => unreachable!("schema was selected by the closed parser"),
        };
        if !valid_name(&name) {
            return Err(grammar(format!(
                "{version_label} name must match lowercase [a-z][a-z0-9-]* and contain 1..=64 bytes"
            )));
        }
        if !valid_module(&entry) {
            return Err(grammar(format!(
                "{version_label} entry is not a bounded module name"
            )));
        }
        if !(2..=MAX_SOURCES).contains(&sources.len()) {
            return Err(if sources.len() > MAX_SOURCES {
                capacity("sources", MAX_SOURCES)
            } else {
                grammar(format!(
                    "{version_label} requires 2..=16 explicit source paths"
                ))
            });
        }
        require_strict_order(&sources, "source paths")?;
        for path in &sources {
            if path.len() > MAX_PATH_BYTES
                || !path.ends_with(".spx")
                || !crate::workspace::evidence_path_is_valid(path)
            {
                return Err(grammar(format!(
                    "{version_label} source paths must be canonical relative .spx paths of at most 240 bytes"
                )));
            }
        }
        if !(1..=MAX_WEB_EXPORTS).contains(&web_exports.len()) {
            return Err(if web_exports.len() > MAX_WEB_EXPORTS {
                capacity("web_exports", MAX_WEB_EXPORTS)
            } else {
                grammar(format!(
                    "{version_label} requires 1..=32 explicit web export identities"
                ))
            });
        }
        require_strict_order(&web_exports, "web export identities")?;
        if web_exports.iter().any(|id| !valid_stable_id(id)) {
            return Err(grammar(format!(
                "{version_label} web exports must use bounded lowercase [a-z0-9._-] stable IDs"
            )));
        }
        if let Some(command) = &command {
            if !valid_stable_id(command)
                || web_exports.len() != 1
                || web_exports.first() != Some(command)
            {
                return Err(grammar(format!(
                    "{version_label} web_exports must contain exactly the command stable ID"
                )));
            }
        }
        if tests.len() != 1 || !valid_module(&tests[0]) {
            return Err(grammar(format!(
                "{version_label} tests must contain exactly one bounded module name"
            )));
        }
        if entry == tests[0] {
            return Err(grammar(format!(
                "{version_label} entry and test modules must be distinct"
            )));
        }

        let manifest = Self {
            schema,
            name,
            package_version,
            profile,
            entry,
            sources,
            web_exports,
            command,
            command_input,
            capabilities,
            test_module: tests.into_iter().next().expect("one test module"),
        };
        if manifest.to_canonical_toml() != source {
            return Err(grammar(format!(
                "{version_label} manifest is not canonical"
            )));
        }
        Ok(manifest)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn schema(&self) -> &'static str {
        self.schema
    }

    pub fn package_version(&self) -> Option<&str> {
        self.package_version.as_deref()
    }

    pub fn profile(&self) -> Option<&'static str> {
        self.profile.name()
    }

    pub fn project_profile(&self) -> ProjectProfile {
        self.profile
    }

    pub fn is_v2(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V2
    }

    pub fn is_v3(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V3
    }

    pub fn is_v4(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V4
    }

    pub fn is_v5(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V5
    }

    pub fn is_v6(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V6
    }

    pub fn is_v7(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V7
    }

    pub fn entry(&self) -> &str {
        &self.entry
    }

    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    pub fn web_exports(&self) -> &[String] {
        &self.web_exports
    }

    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub fn command_input(&self) -> Option<&str> {
        self.command_input.as_deref()
    }

    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    pub fn test_module(&self) -> &str {
        &self.test_module
    }

    pub fn to_canonical_toml(&self) -> String {
        if self.schema == PROJECT_SCHEMA {
            format!(
                "schema = \"{PROJECT_SCHEMA}\"\nname = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
                self.name,
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.test_module,
            )
        } else if self.schema == PROJECT_SCHEMA_V2 {
            format!(
                "schema = \"{PROJECT_SCHEMA_V2}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version
                    .as_deref()
                    .expect("Project v2 carries a package version"),
                self.profile
                    .name()
                    .expect("Project v2 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.test_module,
            )
        } else if self.schema == PROJECT_SCHEMA_V3 {
            format!(
                "schema = \"{PROJECT_SCHEMA_V3}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version
                    .as_deref()
                    .expect("Project v3 carries a package version"),
                self.profile
                    .name()
                    .expect("Project v3 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.test_module,
            )
        } else if self.schema == PROJECT_SCHEMA_V4 {
            format!(
                "schema = \"{PROJECT_SCHEMA_V4}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ncommand = \"{}\"\ncapabilities = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version.as_deref().expect("Project v4 carries a package version"),
                self.profile.name().expect("Project v4 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.command.as_deref().expect("Project v4 carries a command stable ID"),
                render_array(&self.capabilities),
                self.test_module,
            )
        } else if self.schema == PROJECT_SCHEMA_V5 {
            format!(
                "schema = \"{PROJECT_SCHEMA_V5}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ncommand = \"{}\"\ninput = \"{}\"\ncapabilities = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version.as_deref().expect("Project v5 carries a package version"),
                self.profile.name().expect("Project v5 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.command.as_deref().expect("Project v5 carries a command stable ID"),
                self.command_input.as_deref().expect("Project v5 carries a command input profile"),
                render_array(&self.capabilities),
                self.test_module,
            )
        } else if self.schema == PROJECT_SCHEMA_V6 {
            debug_assert_eq!(self.schema, PROJECT_SCHEMA_V6);
            format!(
                "schema = \"{PROJECT_SCHEMA_V6}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ncommand = \"{}\"\ninput = \"{}\"\ncapabilities = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version.as_deref().expect("Project v6 carries a package version"),
                self.profile.name().expect("Project v6 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.command.as_deref().expect("Project v6 carries a command stable ID"),
                self.command_input.as_deref().expect("Project v6 carries a command input profile"),
                render_array(&self.capabilities),
                self.test_module,
            )
        } else {
            debug_assert_eq!(self.schema, PROJECT_SCHEMA_V7);
            format!(
                "schema = \"{PROJECT_SCHEMA_V7}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ncommand = \"{}\"\ninput = \"{}\"\ncapabilities = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version.as_deref().expect("Project v7 carries a package version"),
                self.profile.name().expect("Project v7 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.command.as_deref().expect("Project v7 carries a command stable ID"),
                self.command_input.as_deref().expect("Project v7 carries a command input profile"),
                render_array(&self.capabilities),
                self.test_module,
            )
        }
    }
}

/// Validate the complete SemVer 2.0.0 lexical grammar without normalization.
/// Core and numeric prerelease identifiers reject leading zeroes; build
/// identifiers intentionally permit them as the standard specifies.
fn valid_semver(value: &str) -> bool {
    if value.is_empty() || value.len() > MAX_VERSION_BYTES || !value.is_ascii() {
        return false;
    }
    let mut build_split = value.split('+');
    let Some(core_and_prerelease) = build_split.next() else {
        return false;
    };
    let build = build_split.next();
    if build_split.next().is_some() || build.is_some_and(|part| !valid_identifiers(part, false)) {
        return false;
    }
    let (core, prerelease) = core_and_prerelease
        .split_once('-')
        .map_or((core_and_prerelease, None), |(core, pre)| (core, Some(pre)));
    if prerelease.is_some_and(|part| !valid_identifiers(part, true)) {
        return false;
    }
    let parts = core.split('.').collect::<Vec<_>>();
    parts.len() == 3 && parts.into_iter().all(valid_numeric_identifier)
}

fn valid_identifiers(value: &str, reject_numeric_leading_zero: bool) -> bool {
    !value.is_empty()
        && value.split('.').all(|identifier| {
            !identifier.is_empty()
                && identifier
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                && (!reject_numeric_leading_zero
                    || !identifier.bytes().all(|byte| byte.is_ascii_digit())
                    || valid_numeric_identifier(identifier))
        })
}

fn valid_numeric_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && (value == "0" || !value.starts_with('0'))
}

fn parse_string_assignment(line: &str, key: &str) -> Result<String, Vec<Diagnostic>> {
    let prefix = format!("{key} = \"");
    let Some(value) = line
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix('"'))
    else {
        return Err(grammar(format!(
            "Project v1 manifest expected canonical `{key}` string assignment"
        )));
    };
    if value.contains(['"', '\\']) {
        return Err(grammar("Project v1 strings do not admit escapes"));
    }
    Ok(value.to_owned())
}

fn parse_array_assignment(line: &str, key: &str) -> Result<Vec<String>, Vec<Diagnostic>> {
    let prefix = format!("{key} = [");
    let Some(body) = line
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(']'))
    else {
        return Err(grammar(format!(
            "Project v1 manifest expected canonical `{key}` array assignment"
        )));
    };
    if body.is_empty() {
        return Ok(Vec::new());
    }
    body.split(", ")
        .map(|item| {
            let Some(value) = item
                .strip_prefix('"')
                .and_then(|item| item.strip_suffix('"'))
            else {
                return Err(grammar("Project v1 arrays contain only canonical strings"));
            };
            if value.is_empty() || value.contains(['"', '\\']) {
                return Err(grammar("Project v1 array strings are empty or escaped"));
            }
            Ok(value.to_owned())
        })
        .collect()
}

fn render_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{value}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn require_strict_order(values: &[String], subject: &str) -> Result<(), Vec<Diagnostic>> {
    if values.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(())
    } else {
        Err(grammar(format!(
            "Project v1 {subject} must be strictly byte-sorted and unique"
        )))
    }
}

fn valid_name(value: &str) -> bool {
    (1..=MAX_NAME_BYTES).contains(&value.len())
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn valid_module(value: &str) -> bool {
    (1..=MAX_MODULE_BYTES).contains(&value.len())
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        })
}

fn valid_stable_id(value: &str) -> bool {
    (1..=MAX_STABLE_ID_BYTES).contains(&value.len())
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

pub(super) fn grammar(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J100", message)]
}

pub(super) fn capacity(field: &str, limit: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-J101",
        format!("Project v1 `{field}` exceeds {limit}"),
    )]
}
