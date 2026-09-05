mod tables;

pub use tables::{
    ManifestLayout, PackageDependency, PackageDependencySource, RustDependency, MAX_DEPENDENCIES,
    MAX_DEPENDENCY_SOURCES, MAX_RUST_DEPENDENCIES, PACKAGE_MANIFEST_RESERVED_TABLES,
    PACKAGE_MANIFEST_SCHEMA, PACKAGE_MANIFEST_TABLES, PACKAGE_RESERVED_KEYS,
    PACKAGE_TARGET_NATIVE64, PACKAGE_TARGET_WASM32,
};

use crate::diagnostic::Diagnostic;

use super::profile::{
    ProjectProfile, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2, PROJECT_COMMAND_INPUT_V1,
    PROJECT_COMMAND_STDOUT_CAPABILITY, PROJECT_HTTPS_COMMAND_CAPABILITIES_V1,
    PROJECT_LANGUAGE_COMMAND_INPUT_V1, PROJECT_NETWORK_COMMAND_CAPABILITIES_V1,
    PROJECT_PROFILE_FLAT_OWNED_RECORD_API_V1, PROJECT_PROFILE_HTTPS_COMMAND_IO_V1,
    PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1, PROJECT_PROFILE_LINE_COMMAND_IO_V1,
    PROJECT_PROFILE_NESTED_OWNED_RECORD_API_V1, PROJECT_PROFILE_NETWORK_COMMAND_IO_V1,
    PROJECT_PROFILE_OWNED_DATA_API_V1, PROJECT_PROFILE_OWNED_UTF8_API_V1,
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
/// Additive Project Manifest v8 schema for the bounded public owned-data API.
pub const PROJECT_SCHEMA_V8: &str = "semaprax.project.v8";
/// Additive Project Manifest v9 schema for the flat owned-record API.
pub const PROJECT_SCHEMA_V9: &str = "semaprax.project.v9";
/// Additive Project Manifest v10 schema for length-delimited owned UTF-8.
pub const PROJECT_SCHEMA_V10: &str = "semaprax.project.v10";
/// Additive Project Manifest v11 schema for bounded nested owned-record results.
pub const PROJECT_SCHEMA_V11: &str = "semaprax.project.v11";
/// Additive Project Manifest v12 schema for bounded language network commands.
pub const PROJECT_SCHEMA_V12: &str = "semaprax.project.v12";
/// Additive Project Manifest v13 schema for bounded HTTPS commands.
pub const PROJECT_SCHEMA_V13: &str = "semaprax.project.v13";
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_NAME_BYTES: usize = 64;
pub const MAX_VERSION_BYTES: usize = 128;
pub const MAX_MODULE_BYTES: usize = 240;
pub const MAX_PATH_BYTES: usize = 240;
pub const MAX_STABLE_ID_BYTES: usize = 128;
pub const MAX_SOURCES: usize = 16;
pub const MAX_WEB_EXPORTS: usize = 32;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// One exact, closed Project manifest. `schema` is the frozen profile contract
/// the manifest lowers to: a frozen `semaprax.project.v1`-`v13` layout names it
/// directly, and the extensible `semaprax.manifest.v1` table layout selects it
/// through `[package] profile`. Every project route reads only the contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectManifest {
    schema: &'static str,
    layout: ManifestLayout,
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
    dependencies: Vec<PackageDependency>,
    dependency_sources: Vec<PackageDependencySource>,
    rust_dependencies: Vec<RustDependency>,
    target_matrix: Option<Vec<String>>,
}

impl ProjectManifest {
    /// Parse one canonical manifest: a frozen Project v1-v13 layout or the
    /// extensible Package Manifest v1 table layout.
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
        let mut layout = ManifestLayout::Frozen;
        let mut dependencies = Vec::new();
        let mut dependency_sources = Vec::new();
        let mut rust_dependencies = Vec::new();
        let mut target_matrix = None;
        let (
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
            tests,
        ) = if schema == PACKAGE_MANIFEST_SCHEMA {
            let parts = tables::parse(&lines)?;
            layout = ManifestLayout::Tables;
            dependencies = parts.dependencies;
            dependency_sources = parts.dependency_sources;
            rust_dependencies = parts.rust_dependencies;
            target_matrix = parts.target_matrix;
            (
                parts.schema,
                parts.name,
                Some(parts.version),
                parts.profile,
                parts.entry,
                parts.sources,
                parts.web_exports,
                parts.command,
                parts.command_input,
                parts.capabilities,
                parts.tests,
            )
        } else {
            match schema.as_str() {
                PROJECT_SCHEMA => {
                    if lines.len() != 7 || lines.last() != Some(&"") {
                        return Err(grammar_with_help(
                            "Project v1 manifest must contain exactly six ordered assignments and one terminal LF",
                            V1_SHAPE_HELP,
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
                PROJECT_SCHEMA_V8 => {
                    if lines.len() != 9 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v8 manifest must contain exactly eight ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v8 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    if parse_string_assignment(lines[3], "profile")?
                        != PROJECT_PROFILE_OWNED_DATA_API_V1
                    {
                        return Err(grammar("Project v8 profile is not owned-data-api.v1"));
                    }
                    (
                        PROJECT_SCHEMA_V8,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::OwnedDataApiV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        None,
                        None,
                        Vec::new(),
                        parse_array_assignment(lines[7], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V9 => {
                    if lines.len() != 9 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v9 manifest must contain exactly eight ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v9 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    if parse_string_assignment(lines[3], "profile")?
                        != PROJECT_PROFILE_FLAT_OWNED_RECORD_API_V1
                    {
                        return Err(grammar(
                            "Project v9 profile is not flat-owned-record-api.v1",
                        ));
                    }
                    (
                        PROJECT_SCHEMA_V9,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::FlatOwnedRecordApiV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        None,
                        None,
                        Vec::new(),
                        parse_array_assignment(lines[7], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V10 => {
                    if lines.len() != 9 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v10 manifest must contain exactly eight ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v10 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    if parse_string_assignment(lines[3], "profile")?
                        != PROJECT_PROFILE_OWNED_UTF8_API_V1
                    {
                        return Err(grammar("Project v10 profile is not owned-utf8-api.v1"));
                    }
                    (
                        PROJECT_SCHEMA_V10,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::OwnedUtf8ApiV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        None,
                        None,
                        Vec::new(),
                        parse_array_assignment(lines[7], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V11 => {
                    if lines.len() != 9 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v11 manifest must contain exactly eight ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v11 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    if parse_string_assignment(lines[3], "profile")?
                        != PROJECT_PROFILE_NESTED_OWNED_RECORD_API_V1
                    {
                        return Err(grammar(
                            "Project v11 profile is not nested-owned-record-api.v1",
                        ));
                    }
                    (
                        PROJECT_SCHEMA_V11,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::NestedOwnedRecordApiV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        None,
                        None,
                        Vec::new(),
                        parse_array_assignment(lines[7], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V12 => {
                    if lines.len() != 12 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v12 manifest must contain exactly eleven ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v12 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    if parse_string_assignment(lines[3], "profile")?
                        != PROJECT_PROFILE_NETWORK_COMMAND_IO_V1
                    {
                        return Err(grammar("Project v12 profile is not network-command-io.v1"));
                    }
                    let command = parse_string_assignment(lines[7], "command")?;
                    let input = parse_string_assignment(lines[8], "input")?;
                    if input != PROJECT_LANGUAGE_COMMAND_INPUT_V1 {
                        return Err(grammar(
                            "Project v12 input is not argv-utf8+stdin-bytes.v1",
                        ));
                    }
                    let capabilities = parse_array_assignment(lines[9], "capabilities")?;
                    if !capabilities
                        .iter()
                        .map(String::as_str)
                        .eq(PROJECT_NETWORK_COMMAND_CAPABILITIES_V1)
                    {
                        return Err(grammar(
                            "Project v12 capabilities must be exactly [\"network.connect\", \"network.read\", \"network.write\", \"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
                        ));
                    }
                    (
                        PROJECT_SCHEMA_V12,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::NetworkCommandIoV1,
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        Some(command),
                        Some(input),
                        capabilities,
                        parse_array_assignment(lines[10], "tests")?,
                    )
                }
                PROJECT_SCHEMA_V13 => {
                    if lines.len() != 12 || lines.last() != Some(&"") {
                        return Err(grammar(
                            "Project v13 manifest must contain exactly eleven ordered assignments and one terminal LF",
                        ));
                    }
                    let version = parse_string_assignment(lines[2], "version")?;
                    if !valid_semver(&version) {
                        return Err(grammar(
                            "Project v13 version must be canonical Semantic Versioning text of at most 128 bytes",
                        ));
                    }
                    if parse_string_assignment(lines[3], "profile")?
                        != PROJECT_PROFILE_HTTPS_COMMAND_IO_V1
                    {
                        return Err(grammar("Project v13 profile is not https-command-io.v1"));
                    }
                    let command = parse_string_assignment(lines[7], "command")?;
                    let input = parse_string_assignment(lines[8], "input")?;
                    if input != PROJECT_LANGUAGE_COMMAND_INPUT_V1 {
                        return Err(grammar(
                            "Project v13 input is not argv-utf8+stdin-bytes.v1",
                        ));
                    }
                    let capabilities = parse_array_assignment(lines[9], "capabilities")?;
                    if !capabilities
                        .iter()
                        .map(String::as_str)
                        .eq(PROJECT_HTTPS_COMMAND_CAPABILITIES_V1)
                    {
                        return Err(grammar(
                            "Project v13 capabilities must be exactly [\"network.http\", \"process.args.read\", \"process.stderr.write\", \"process.stdin.read\", \"process.stdout.write\"]",
                        ));
                    }
                    (
                        PROJECT_SCHEMA_V13,
                        parse_string_assignment(lines[1], "name")?,
                        Some(version),
                        ProjectProfile::HttpsCommandIoV1,
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
                        "Project manifest schema is neither semaprax.manifest.v1 nor an admitted semaprax.project.v1-v13 frozen schema",
                    ))
                }
            }
        };
        let version_label = match schema {
            _ if layout == ManifestLayout::Tables => "Package Manifest v1",
            PROJECT_SCHEMA => "Project v1",
            PROJECT_SCHEMA_V2 => "Project v2",
            PROJECT_SCHEMA_V3 => "Project v3",
            PROJECT_SCHEMA_V4 => "Project v4",
            PROJECT_SCHEMA_V5 => "Project v5",
            PROJECT_SCHEMA_V6 => "Project v6",
            PROJECT_SCHEMA_V7 => "Project v7",
            PROJECT_SCHEMA_V8 => "Project v8",
            PROJECT_SCHEMA_V9 => "Project v9",
            PROJECT_SCHEMA_V10 => "Project v10",
            PROJECT_SCHEMA_V11 => "Project v11",
            PROJECT_SCHEMA_V12 => "Project v12",
            PROJECT_SCHEMA_V13 => "Project v13",
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
            layout,
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
            dependencies,
            dependency_sources,
            rust_dependencies,
            target_matrix,
        };
        let canonical = manifest.to_canonical_toml();
        if canonical != source {
            return Err(if layout == ManifestLayout::Tables {
                tables::canonical_mismatch(source, &canonical)
            } else {
                grammar(format!("{version_label} manifest is not canonical"))
            });
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

    pub fn is_v8(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V8
    }

    pub fn is_v9(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V9
    }

    pub fn is_v10(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V10
    }

    pub fn is_v11(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V11
    }

    pub fn is_v12(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V12
    }

    pub fn is_v13(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V13
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

    /// Package Manifest v1: the canonical text of this manifest with one more
    /// `[dependencies]` row, kept in strict byte order. The rendered text is
    /// re-parsed before it is returned, so a rejected package identity or
    /// range surfaces as the manifest's own grammar diagnostic. A frozen
    /// Project v1-v13 layout has no dependency table and is rejected, as is a
    /// dependency the manifest already declares.
    pub fn with_dependency(&self, name: &str, range: &str) -> Result<String, Vec<Diagnostic>> {
        if self.layout != ManifestLayout::Tables {
            return Err(vec![Diagnostic::io(
                "SPX-J127",
                format!(
                    "`{}` manifests carry no `[dependencies]` table; only the Package Manifest v1 table layout does",
                    self.schema
                ),
            )
            .with_help(
                "recreate the project with `semaprax project-scaffold --layout tables` or `semaprax new`, which writes the table layout",
            )]);
        }
        if self
            .dependencies
            .iter()
            .any(|dependency| dependency.name() == name)
        {
            return Err(vec![Diagnostic::io(
                "SPX-J127",
                format!("dependency `{name}` is already declared in `[dependencies]`"),
            )]);
        }
        let mut manifest = self.clone();
        manifest
            .dependencies
            .push(tables::PackageDependency::new(name, range));
        manifest
            .dependencies
            .sort_by(|left, right| left.name().cmp(right.name()));
        let text = tables::render(&manifest);
        Self::parse(&text)?;
        Ok(text)
    }

    pub fn capabilities(&self) -> &[String] {
        &self.capabilities
    }

    pub fn test_module(&self) -> &str {
        &self.test_module
    }

    pub fn to_canonical_toml(&self) -> String {
        if self.layout == ManifestLayout::Tables {
            tables::render(self)
        } else if self.schema == PROJECT_SCHEMA {
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
        } else if self.schema == PROJECT_SCHEMA_V7 {
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
        } else if self.schema == PROJECT_SCHEMA_V8 {
            debug_assert_eq!(self.schema, PROJECT_SCHEMA_V8);
            format!(
                "schema = \"{PROJECT_SCHEMA_V8}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version
                    .as_deref()
                    .expect("Project v8 carries a package version"),
                self.profile
                    .name()
                    .expect("Project v8 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.test_module,
            )
        } else if self.schema == PROJECT_SCHEMA_V9 {
            debug_assert_eq!(self.schema, PROJECT_SCHEMA_V9);
            format!(
                "schema = \"{PROJECT_SCHEMA_V9}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version
                    .as_deref()
                    .expect("Project v9 carries a package version"),
                self.profile
                    .name()
                    .expect("Project v9 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.test_module,
            )
        } else if self.schema == PROJECT_SCHEMA_V10 {
            debug_assert_eq!(self.schema, PROJECT_SCHEMA_V10);
            format!(
                "schema = \"{PROJECT_SCHEMA_V10}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version.as_deref().expect("Project v10 carries a package version"),
                self.profile.name().expect("Project v10 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.test_module,
            )
        } else if self.schema == PROJECT_SCHEMA_V11 {
            format!(
                "schema = \"{PROJECT_SCHEMA_V11}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version.as_deref().expect("Project v11 carries a package version"),
                self.profile.name().expect("Project v11 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.test_module,
            )
        } else if self.schema == PROJECT_SCHEMA_V12 {
            debug_assert_eq!(self.schema, PROJECT_SCHEMA_V12);
            format!(
                "schema = \"{PROJECT_SCHEMA_V12}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ncommand = \"{}\"\ninput = \"{}\"\ncapabilities = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version
                    .as_deref()
                    .expect("Project v12 carries a package version"),
                self.profile
                    .name()
                    .expect("Project v12 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.command
                    .as_deref()
                    .expect("Project v12 carries a command stable ID"),
                self.command_input
                    .as_deref()
                    .expect("Project v12 carries a command input profile"),
                render_array(&self.capabilities),
                self.test_module,
            )
        } else {
            debug_assert_eq!(self.schema, PROJECT_SCHEMA_V13);
            format!(
                "schema = \"{PROJECT_SCHEMA_V13}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ncommand = \"{}\"\ninput = \"{}\"\ncapabilities = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version
                    .as_deref()
                    .expect("Project v13 carries a package version"),
                self.profile
                    .name()
                    .expect("Project v13 carries a named profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
                self.command
                    .as_deref()
                    .expect("Project v13 carries a command stable ID"),
                self.command_input
                    .as_deref()
                    .expect("Project v13 carries a command input profile"),
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

/// The exact line shape of a Project v1 manifest, for the reader who wrote
/// the keys in another order or left one out.
const V1_SHAPE_HELP: &str = "write exactly these six lines in this order, then one final newline: \
                             `schema = \"semaprax.project.v1\"`, `name = \"…\"`, \
                             `entry = \"module.with.main\"`, `sources = [\"src/….spx\", …]`, \
                             `web_exports = [\"stable.id\", …]` (byte-sorted), \
                             `tests = [\"module.tests\"]`";

fn grammar_with_help(message: impl Into<String>, help: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J100", message).with_help(help)]
}

pub(super) fn capacity(field: &str, limit: usize) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-J101",
        format!("Project v1 `{field}` exceeds {limit}"),
    )]
}
