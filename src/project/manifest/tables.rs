//! Package Manifest v1: the one extensible, table-structured `semaprax.toml`.
//!
//! Each frozen Project v1-v11 manifest fixes a whole-file line shape, so every
//! feature tranche so far has added a new schema string. This layout instead
//! admits one closed catalog of optional tables and keys and lowers every
//! admitted manifest onto the frozen profile contract it selects. Existing
//! project routes therefore keep their exact `project_schema`, descriptor, and
//! digest behavior, and a future revision adds a table or key without a new
//! whole-project schema.
//!
//! Canonical bytes stay exact, as for the frozen layouts: the parser rejects a
//! manifest whose bytes differ from its own rendering and names the first
//! differing line, so agents get a byte-precise fix instead of a shape error.

use super::{
    capacity, grammar, valid_semver, ProjectManifest, MAX_VERSION_BYTES, PROJECT_SCHEMA,
    PROJECT_SCHEMA_V10, PROJECT_SCHEMA_V11, PROJECT_SCHEMA_V2, PROJECT_SCHEMA_V3,
    PROJECT_SCHEMA_V4, PROJECT_SCHEMA_V5, PROJECT_SCHEMA_V6, PROJECT_SCHEMA_V7, PROJECT_SCHEMA_V8,
    PROJECT_SCHEMA_V9,
};
use crate::diagnostic::Diagnostic;
use crate::package_range;
use crate::project::profile::{
    ProjectProfile, PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2, PROJECT_COMMAND_INPUT_V1,
    PROJECT_COMMAND_STDOUT_CAPABILITY, PROJECT_LANGUAGE_COMMAND_INPUT_V1,
    PROJECT_PROFILE_FLAT_OWNED_RECORD_API_V1, PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1,
    PROJECT_PROFILE_LINE_COMMAND_IO_V1, PROJECT_PROFILE_NESTED_OWNED_RECORD_API_V1,
    PROJECT_PROFILE_OWNED_DATA_API_V1, PROJECT_PROFILE_OWNED_UTF8_API_V1,
    PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1, PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2,
    PROJECT_PROFILE_USEFUL_DATA_V1, PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1,
};

/// The schema string of the extensible table layout.
pub const PACKAGE_MANIFEST_SCHEMA: &str = "semaprax.manifest.v1";
/// Upper bound on `[dependencies]` rows.
pub const MAX_DEPENDENCIES: usize = 64;
/// The 64-bit native target identity admitted in `[targets] matrix`.
pub const PACKAGE_TARGET_NATIVE64: &str = "native64";
/// The 32-bit WebAssembly target identity admitted in `[targets] matrix`.
pub const PACKAGE_TARGET_WASM32: &str = "wasm32";
/// Tables this toolchain admits, in canonical order.
pub const PACKAGE_MANIFEST_TABLES: [&str; 7] = [
    "package",
    "modules",
    "exports",
    "command",
    "capabilities",
    "dependencies",
    "targets",
];
/// Tables the specification reserves for additive revisions. They reject with
/// `SPX-J120` today so an older toolchain fails closed on a newer manifest.
pub const PACKAGE_MANIFEST_RESERVED_TABLES: [&str; 6] = [
    "agents",
    "artifacts",
    "compatibility",
    "features",
    "interfaces",
    "profiles",
];
/// Keys the specification reserves inside `[package]`.
pub const PACKAGE_RESERVED_KEYS: [&str; 3] = ["compatibility", "license", "description"];

const CODE_UNADMITTED: &str = "SPX-J120";
const CODE_UNRESOLVED_DEPENDENCIES: &str = "SPX-J121";
const CODE_TARGET_OUTSIDE_MATRIX: &str = "SPX-J122";
const LABEL: &str = "Package Manifest v1";
const MAX_RANGE_BYTES: usize = 33;

/// Which source layout a manifest was parsed from. The frozen layouts and the
/// table layout lower to the same profile contract and differ only in bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ManifestLayout {
    /// One of the frozen `semaprax.project.v1`-`v11` whole-file line shapes.
    Frozen,
    /// The extensible `semaprax.manifest.v1` table layout.
    Tables,
}

/// One declared dependency requirement: a package name and a closed
/// exact/tilde/caret range over three canonical `u32` components.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageDependency {
    name: String,
    range: String,
}

impl PackageDependency {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn range(&self) -> &str {
        &self.range
    }
}

/// The lowered facts of one admitted table manifest, before the shared
/// validation the frozen layouts also run.
pub(super) struct TableParts {
    pub(super) schema: &'static str,
    pub(super) name: String,
    pub(super) version: String,
    pub(super) profile: ProjectProfile,
    pub(super) entry: String,
    pub(super) sources: Vec<String>,
    pub(super) web_exports: Vec<String>,
    pub(super) command: Option<String>,
    pub(super) command_input: Option<String>,
    pub(super) capabilities: Vec<String>,
    pub(super) tests: Vec<String>,
    pub(super) dependencies: Vec<PackageDependency>,
    pub(super) target_matrix: Option<Vec<String>>,
}

enum Value {
    Text(String),
    List(Vec<String>),
}

struct Table<'a> {
    name: &'a str,
    entries: Vec<(&'a str, Value)>,
}

pub(super) fn parse(lines: &[&str]) -> Result<TableParts, Vec<Diagnostic>> {
    if lines.last() != Some(&"") {
        return Err(grammar(format!("{LABEL} must end with one terminal LF")));
    }
    let mut tables: Vec<Table<'_>> = Vec::new();
    for line in &lines[1..lines.len() - 1] {
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            if !valid_table_name(name) {
                return Err(grammar(format!(
                    "{LABEL} table names are lowercase [a-z][a-z-]*; found `[{name}]`"
                )));
            }
            if tables.iter().any(|table| table.name == name) {
                return Err(grammar(format!("{LABEL} table `[{name}]` appears twice")));
            }
            if PACKAGE_MANIFEST_RESERVED_TABLES.contains(&name) {
                return Err(unadmitted(format!(
                    "table `[{name}]` is reserved for an additive {LABEL} revision and is not admitted by this toolchain"
                )));
            }
            if !PACKAGE_MANIFEST_TABLES.contains(&name) {
                return Err(unadmitted(format!(
                    "{LABEL} does not admit table `[{name}]`; admitted tables are {}",
                    PACKAGE_MANIFEST_TABLES
                        .iter()
                        .map(|table| format!("`[{table}]`"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
            tables.push(Table {
                name,
                entries: Vec::new(),
            });
            continue;
        }
        let Some(table) = tables.last_mut() else {
            return Err(grammar(format!(
                "{LABEL} admits only `schema` before the first table; found `{line}`"
            )));
        };
        let (key, value) = parse_assignment(line, table.name)?;
        if table.entries.iter().any(|(existing, _)| *existing == key) {
            return Err(grammar(format!(
                "{LABEL} key `{key}` appears twice in `[{}]`",
                table.name
            )));
        }
        table.entries.push((key, value));
    }

    let mut package = require_table(&tables, "package")?;
    let name = package.text("name")?;
    let version = package.text("version")?;
    if !valid_semver(&version) {
        return Err(grammar(format!(
            "{LABEL} `[package] version` must be canonical Semantic Versioning text of at most {MAX_VERSION_BYTES} bytes"
        )));
    }
    let profile = match package.optional_text("profile")? {
        None => ProjectProfile::ScalarV1,
        Some(profile) => profile_by_name(&profile).ok_or_else(|| {
            grammar(format!(
                "{LABEL} `[package] profile` `{profile}` is not an admitted profile"
            ))
        })?,
    };
    package.finish()?;

    let mut modules = require_table(&tables, "modules")?;
    let entry = modules.text("entry")?;
    let sources = modules.list("sources")?;
    let tests = modules.list("tests")?;
    modules.finish()?;

    let mut exports = require_table(&tables, "exports")?;
    let web_exports = exports.list("web")?;
    exports.finish()?;

    let (command, command_input) = match optional_table(&tables, "command") {
        None => (None, None),
        Some(mut command) => {
            let function = command.text("function")?;
            let input = command.optional_text("input")?;
            command.finish()?;
            (Some(function), input)
        }
    };

    let capabilities = match optional_table(&tables, "capabilities") {
        None => Vec::new(),
        Some(mut capabilities) => {
            let required = capabilities.list("required")?;
            capabilities.finish()?;
            required
        }
    };

    let dependencies = match optional_table(&tables, "dependencies") {
        None => Vec::new(),
        Some(dependencies) => parse_dependencies(dependencies)?,
    };

    let target_matrix = match optional_table(&tables, "targets") {
        None => None,
        Some(mut targets) => {
            let matrix = targets.list("matrix")?;
            targets.finish()?;
            Some(validate_target_matrix(matrix)?)
        }
    };

    let schema = lower_profile(
        profile,
        command.as_deref(),
        command_input.as_deref(),
        &capabilities,
    )?;
    Ok(TableParts {
        schema,
        name,
        version,
        profile,
        entry,
        sources,
        web_exports,
        command,
        command_input,
        capabilities,
        tests,
        dependencies,
        target_matrix,
    })
}

/// Check the profile-specific rules the frozen schemas encode positionally and
/// return the frozen profile contract the manifest lowers to.
fn lower_profile(
    profile: ProjectProfile,
    command: Option<&str>,
    input: Option<&str>,
    capabilities: &[String],
) -> Result<&'static str, Vec<Diagnostic>> {
    let profile_name = profile.name().unwrap_or("scalar");
    let (schema, expected_input, expected_capabilities): (&str, Option<&str>, &[&str]) =
        match profile {
            ProjectProfile::ScalarV1 => (PROJECT_SCHEMA, None, &[]),
            ProjectProfile::UsefulTextConsumerV1 => (PROJECT_SCHEMA_V2, None, &[]),
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
            ProjectProfile::LineCommandIoV1 => (
                PROJECT_SCHEMA_V7,
                Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1),
                &PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2,
            ),
            ProjectProfile::OwnedDataApiV1 => (PROJECT_SCHEMA_V8, None, &[]),
            ProjectProfile::FlatOwnedRecordApiV1 => (PROJECT_SCHEMA_V9, None, &[]),
            ProjectProfile::OwnedUtf8ApiV1 => (PROJECT_SCHEMA_V10, None, &[]),
            ProjectProfile::NestedOwnedRecordApiV1 => (PROJECT_SCHEMA_V11, None, &[]),
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
    if !capabilities
        .iter()
        .map(String::as_str)
        .eq(expected_capabilities.iter().copied())
    {
        return Err(grammar(if expected_capabilities.is_empty() {
            format!("{LABEL} profile `{profile_name}` does not admit a `[capabilities]` table")
        } else {
            format!(
                "{LABEL} profile `{profile_name}` requires `[capabilities] required = {}`",
                super::render_array(
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

fn parse_dependencies(table: TableReader<'_>) -> Result<Vec<PackageDependency>, Vec<Diagnostic>> {
    if table.entries.len() > MAX_DEPENDENCIES {
        return Err(capacity("dependencies", MAX_DEPENDENCIES));
    }
    let mut dependencies = Vec::with_capacity(table.entries.len());
    for (name, value) in table.entries {
        if !valid_dependency_identity(name) {
            return Err(grammar(format!(
                "{LABEL} dependency names are dotted lowercase package identities [a-z][a-z0-9._-]* of 1..=128 bytes with non-empty segments; found `{name}`"
            )));
        }
        let Value::Text(range) = value else {
            return Err(grammar(format!(
                "{LABEL} dependency `{name}` must be one range string such as \"^1.2.0\""
            )));
        };
        if range.len() > MAX_RANGE_BYTES {
            return Err(grammar(format!(
                "{LABEL} dependency `{name}` range exceeds {MAX_RANGE_BYTES} bytes"
            )));
        }
        package_range::parse_range(&range, dependency_range_error).map_err(|error| {
            vec![error.with_help(format!(
                "dependency `{name}` admits only `=x.y.z`, `~x.y.z`, or `^x.y.z` with canonical u32 components"
            ))]
        })?;
        dependencies.push(PackageDependency {
            name: name.to_owned(),
            range,
        });
    }
    if !dependencies
        .windows(2)
        .all(|pair| pair[0].name < pair[1].name)
    {
        return Err(grammar(format!(
            "{LABEL} dependencies must be strictly byte-sorted by name"
        )));
    }
    Ok(dependencies)
}

fn dependency_range_error(message: String) -> Diagnostic {
    Diagnostic::io("SPX-J100", format!("{LABEL} dependency {message}"))
}

fn validate_target_matrix(matrix: Vec<String>) -> Result<Vec<String>, Vec<Diagnostic>> {
    if matrix.is_empty() {
        return Err(grammar(format!(
            "{LABEL} `[targets] matrix` must name at least one target"
        )));
    }
    for target in &matrix {
        if target != PACKAGE_TARGET_NATIVE64 && target != PACKAGE_TARGET_WASM32 {
            return Err(grammar(format!(
                "{LABEL} `[targets] matrix` admits only \"{PACKAGE_TARGET_NATIVE64}\" and \"{PACKAGE_TARGET_WASM32}\"; found `{target}`"
            )));
        }
    }
    if !matrix.windows(2).all(|pair| pair[0] < pair[1]) {
        return Err(grammar(format!(
            "{LABEL} `[targets] matrix` must be strictly byte-sorted and unique"
        )));
    }
    Ok(matrix)
}

/// Render the canonical table layout. Empty optional tables are omitted, and
/// the `profile` key is omitted for the scalar contract.
pub(super) fn render(manifest: &ProjectManifest) -> String {
    let mut blocks = Vec::with_capacity(8);
    blocks.push(format!("schema = \"{PACKAGE_MANIFEST_SCHEMA}\"\n"));
    let mut package = format!(
        "[package]\nname = \"{}\"\nversion = \"{}\"\n",
        manifest.name,
        manifest
            .package_version
            .as_deref()
            .expect("the table layout always carries a package version"),
    );
    if let Some(profile) = manifest.profile.name() {
        package.push_str(&format!("profile = \"{profile}\"\n"));
    }
    blocks.push(package);
    blocks.push(format!(
        "[modules]\nentry = \"{}\"\nsources = {}\ntests = [\"{}\"]\n",
        manifest.entry,
        super::render_array(&manifest.sources),
        manifest.test_module,
    ));
    blocks.push(format!(
        "[exports]\nweb = {}\n",
        super::render_array(&manifest.web_exports)
    ));
    if let Some(command) = &manifest.command {
        let mut block = format!("[command]\nfunction = \"{command}\"\n");
        if let Some(input) = &manifest.command_input {
            block.push_str(&format!("input = \"{input}\"\n"));
        }
        blocks.push(block);
    }
    if !manifest.capabilities.is_empty() {
        blocks.push(format!(
            "[capabilities]\nrequired = {}\n",
            super::render_array(&manifest.capabilities)
        ));
    }
    if !manifest.dependencies.is_empty() {
        let mut block = String::from("[dependencies]\n");
        for dependency in &manifest.dependencies {
            block.push_str(&format!("{} = \"{}\"\n", dependency.name, dependency.range));
        }
        blocks.push(block);
    }
    if let Some(matrix) = &manifest.target_matrix {
        blocks.push(format!(
            "[targets]\nmatrix = {}\n",
            super::render_array(matrix)
        ));
    }
    blocks.join("\n")
}

/// Explain a non-canonical table manifest by its first differing line.
pub(super) fn canonical_mismatch(source: &str, canonical: &str) -> Vec<Diagnostic> {
    let mut found = source.split('\n');
    let mut expected = canonical.split('\n');
    let mut line = 1usize;
    let help = loop {
        match (expected.next(), found.next()) {
            (Some(expected), Some(found)) if expected == found => line += 1,
            (Some(expected), Some(found)) => {
                break format!("line {line}: expected `{expected}`, found `{found}`");
            }
            (Some(expected), None) => break format!("line {line}: expected `{expected}`"),
            (None, Some(found)) => {
                break format!("line {line}: expected end of manifest, found `{found}`");
            }
            (None, None) => break "the manifest differs only in its final bytes".to_owned(),
        }
    };
    vec![Diagnostic::io(
        "SPX-J100",
        format!("{LABEL} manifest is not canonical"),
    )
    .with_help(format!(
        "{help}; tables follow the order {}, keys follow the specification order, arrays are one line with `, ` separators, and blocks are separated by one blank line",
        PACKAGE_MANIFEST_TABLES
            .iter()
            .map(|table| format!("`[{table}]`"))
            .collect::<Vec<_>>()
            .join(", ")
    ))]
}

impl ProjectManifest {
    /// The schema string of the bytes this manifest was parsed from. The frozen
    /// layouts return the same value as [`Self::schema`]; the table layout
    /// returns `semaprax.manifest.v1` while `schema` returns the lowered
    /// profile contract.
    pub fn manifest_schema(&self) -> &'static str {
        match self.layout {
            ManifestLayout::Frozen => self.schema,
            ManifestLayout::Tables => PACKAGE_MANIFEST_SCHEMA,
        }
    }

    pub fn layout(&self) -> ManifestLayout {
        self.layout
    }

    /// Declared dependency requirements in byte-sorted order. This toolchain
    /// admits their grammar and rejects any project build that declares one.
    pub fn dependencies(&self) -> &[PackageDependency] {
        &self.dependencies
    }

    /// The declared `[targets] matrix`, or `None` when the manifest leaves every
    /// target admitted.
    pub fn target_matrix(&self) -> Option<&[String]> {
        self.target_matrix.as_deref()
    }

    /// Fail closed when a manifest declares dependencies: no resolution, lock,
    /// or acquisition route exists yet, so a build cannot honor them.
    pub(crate) fn admit_dependency_free_build(&self) -> Result<(), Vec<Diagnostic>> {
        if self.dependencies.is_empty() {
            return Ok(());
        }
        Err(vec![Diagnostic::io(
            CODE_UNRESOLVED_DEPENDENCIES,
            format!(
                "{LABEL} declares {} {} but this toolchain admits no dependency resolution",
                self.dependencies.len(),
                if self.dependencies.len() == 1 {
                    "dependency"
                } else {
                    "dependencies"
                }
            ),
        )
        .with_help(
            "remove the `[dependencies]` table until a semantic lock route exists; dependency ranges are admitted so the manifest need not change when it does",
        )])
    }

    /// Reject a CLI build target that the declared `[targets] matrix` excludes.
    /// A manifest without the table admits every target.
    pub fn admit_build_target(&self, target: &str) -> Result<(), Vec<Diagnostic>> {
        let Some(matrix) = &self.target_matrix else {
            return Ok(());
        };
        let required = match target {
            "web" | "wasm" | "npm" => PACKAGE_TARGET_WASM32,
            _ => PACKAGE_TARGET_NATIVE64,
        };
        if matrix.iter().any(|declared| declared == required) {
            return Ok(());
        }
        Err(vec![Diagnostic::io(
            CODE_TARGET_OUTSIDE_MATRIX,
            format!(
                "build target `{target}` needs `{required}`, but the manifest `[targets] matrix` declares only {}",
                super::render_array(matrix)
            ),
        )
        .with_help(format!(
            "add \"{required}\" to `[targets] matrix` or build one of the declared targets"
        ))])
    }
}

struct TableReader<'a> {
    name: &'a str,
    entries: Vec<(&'a str, Value)>,
}

impl<'a> TableReader<'a> {
    fn take(&mut self, key: &str) -> Option<Value> {
        let index = self.entries.iter().position(|(name, _)| *name == key)?;
        Some(self.entries.remove(index).1)
    }

    fn text(&mut self, key: &str) -> Result<String, Vec<Diagnostic>> {
        match self.take(key) {
            Some(Value::Text(value)) => Ok(value),
            Some(Value::List(_)) => Err(grammar(format!(
                "{LABEL} `[{}] {key}` must be one string",
                self.name
            ))),
            None => Err(grammar(format!(
                "{LABEL} table `[{}]` requires `{key}`",
                self.name
            ))),
        }
    }

    fn optional_text(&mut self, key: &str) -> Result<Option<String>, Vec<Diagnostic>> {
        match self.take(key) {
            Some(Value::Text(value)) => Ok(Some(value)),
            Some(Value::List(_)) => Err(grammar(format!(
                "{LABEL} `[{}] {key}` must be one string",
                self.name
            ))),
            None => Ok(None),
        }
    }

    fn list(&mut self, key: &str) -> Result<Vec<String>, Vec<Diagnostic>> {
        match self.take(key) {
            Some(Value::List(values)) => Ok(values),
            Some(Value::Text(_)) => Err(grammar(format!(
                "{LABEL} `[{}] {key}` must be an array of strings",
                self.name
            ))),
            None => Err(grammar(format!(
                "{LABEL} table `[{}]` requires `{key}`",
                self.name
            ))),
        }
    }

    fn finish(self) -> Result<(), Vec<Diagnostic>> {
        let Some((key, _)) = self.entries.first() else {
            return Ok(());
        };
        if self.name == "package" && PACKAGE_RESERVED_KEYS.contains(key) {
            return Err(unadmitted(format!(
                "key `[package] {key}` is reserved for an additive {LABEL} revision and is not admitted by this toolchain"
            )));
        }
        Err(unadmitted(format!(
            "{LABEL} table `[{}]` does not admit key `{key}`",
            self.name
        )))
    }
}

fn require_table<'a>(
    tables: &[Table<'a>],
    name: &'static str,
) -> Result<TableReader<'a>, Vec<Diagnostic>> {
    optional_table(tables, name)
        .ok_or_else(|| grammar(format!("{LABEL} requires a `[{name}]` table")))
}

fn optional_table<'a>(tables: &[Table<'a>], name: &'static str) -> Option<TableReader<'a>> {
    tables
        .iter()
        .find(|table| table.name == name)
        .map(|table| TableReader {
            name,
            entries: table
                .entries
                .iter()
                .map(|(key, value)| {
                    (
                        *key,
                        match value {
                            Value::Text(text) => Value::Text(text.clone()),
                            Value::List(values) => Value::List(values.clone()),
                        },
                    )
                })
                .collect(),
        })
}

fn parse_assignment<'a>(line: &'a str, table: &str) -> Result<(&'a str, Value), Vec<Diagnostic>> {
    let Some((key, value)) = line.split_once(" = ") else {
        return Err(grammar(format!(
            "{LABEL} expected `key = value` in `[{table}]`; found `{line}`"
        )));
    };
    // `.` is admitted so a `[dependencies]` key can be a dotted package
    // identity such as `examples.meaning`. Every other table has a closed key
    // set, so a dotted key there rejects as an unknown key rather than nesting.
    if key.is_empty()
        || !key.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
    {
        return Err(grammar(format!(
            "{LABEL} keys are lowercase [a-z0-9._-]+; found `{key}` in `[{table}]`"
        )));
    }
    if value.starts_with('[') {
        Ok((key, Value::List(super::parse_array_assignment(line, key)?)))
    } else {
        Ok((key, Value::Text(super::parse_string_assignment(line, key)?)))
    }
}

/// A dotted lowercase package identity: `[a-z][a-z0-9._-]*` of 1..=128 bytes
/// with non-empty `.`-separated segments and no leading or trailing separator.
/// It is a canonical subset of the Lock v3 package identity grammar, so a
/// manifest dependency name is always a valid resolver requirement package.
fn valid_dependency_identity(name: &str) -> bool {
    if !(1..=128).contains(&name.len())
        || !name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        || name.ends_with(['.', '-', '_'])
    {
        return false;
    }
    name.split('.').all(|segment| {
        !segment.is_empty()
            && segment.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
            })
    })
}

fn valid_table_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'-')
}

fn profile_by_name(name: &str) -> Option<ProjectProfile> {
    Some(match name {
        PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1 => ProjectProfile::UsefulTextConsumerV1,
        PROJECT_PROFILE_USEFUL_DATA_V1 => ProjectProfile::UsefulDataV1,
        PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1 => ProjectProfile::UsefulDataCommandV1,
        PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2 => ProjectProfile::UsefulDataCommandV2,
        PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1 => ProjectProfile::LanguageCommandIoV1,
        PROJECT_PROFILE_LINE_COMMAND_IO_V1 => ProjectProfile::LineCommandIoV1,
        PROJECT_PROFILE_OWNED_DATA_API_V1 => ProjectProfile::OwnedDataApiV1,
        PROJECT_PROFILE_FLAT_OWNED_RECORD_API_V1 => ProjectProfile::FlatOwnedRecordApiV1,
        PROJECT_PROFILE_OWNED_UTF8_API_V1 => ProjectProfile::OwnedUtf8ApiV1,
        PROJECT_PROFILE_NESTED_OWNED_RECORD_API_V1 => ProjectProfile::NestedOwnedRecordApiV1,
        _ => return None,
    })
}

fn unadmitted(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::io(CODE_UNADMITTED, message)]
}
