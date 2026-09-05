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
    valid_semver, ProjectManifest, MAX_VERSION_BYTES, PROJECT_SCHEMA, PROJECT_SCHEMA_V10,
    PROJECT_SCHEMA_V11, PROJECT_SCHEMA_V2, PROJECT_SCHEMA_V3, PROJECT_SCHEMA_V4, PROJECT_SCHEMA_V5,
    PROJECT_SCHEMA_V6, PROJECT_SCHEMA_V7, PROJECT_SCHEMA_V8, PROJECT_SCHEMA_V9,
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
/// Upper bound on exact local Semantic Package Subject-v3 inputs.
pub const MAX_DEPENDENCY_SOURCES: usize = 4;
/// Upper bound on exact crates.io dependencies carried by a generated Rust SDK.
pub const MAX_RUST_DEPENDENCIES: usize = 32;
/// The 64-bit native target identity admitted in `[targets] matrix`.
pub const PACKAGE_TARGET_NATIVE64: &str = "native64";
/// The 32-bit WebAssembly target identity admitted in `[targets] matrix`.
pub const PACKAGE_TARGET_WASM32: &str = "wasm32";
/// Tables this toolchain admits, in canonical order.
pub const PACKAGE_MANIFEST_TABLES: [&str; 9] = [
    "package",
    "modules",
    "exports",
    "command",
    "capabilities",
    "dependencies",
    "dependency-sources",
    "rust-dependencies",
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
const CODE_TARGET_OUTSIDE_MATRIX: &str = "SPX-J122";
const LABEL: &str = "Package Manifest v1";
const MAX_RANGE_BYTES: usize = 33;
const SCAFFOLD_HELP: &str = "start from `semaprax new <destination>` or render a canonical template with `semaprax project-scaffold --name <name> --layout tables`";

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

/// One exact local Semantic Package Subject-v3 input. The package key is
/// repeated in the authenticated subject and the path is project-relative.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageDependencySource {
    name: String,
    path: String,
}

impl PackageDependencySource {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

/// One exact crates.io dependency for a generated Native Rust SDK. The table
/// key determines the package and stable public re-export name; the generated
/// Cargo dependency itself uses a private positional alias. The first array
/// member is exact; remaining members are byte-sorted Cargo feature names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustDependency {
    name: String,
    version: String,
    features: Vec<String>,
}

impl RustDependency {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn features(&self) -> &[String] {
        &self.features
    }

    pub fn crate_ident(&self) -> String {
        self.name.replace('-', "_")
    }
}

impl PackageDependency {
    pub(super) fn new(name: &str, range: &str) -> Self {
        Self {
            name: name.to_owned(),
            range: range.to_owned(),
        }
    }

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
    pub(super) dependency_sources: Vec<PackageDependencySource>,
    pub(super) rust_dependencies: Vec<RustDependency>,
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

    let structural = structural_diagnostics(&tables);
    if !structural.is_empty() {
        return Err(structural);
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

    let dependency_sources = match optional_table(&tables, "dependency-sources") {
        None => Vec::new(),
        Some(sources) => parse_dependency_sources(sources, &dependencies)?,
    };

    let rust_dependencies = match optional_table(&tables, "rust-dependencies") {
        None => Vec::new(),
        Some(dependencies) => parse_rust_dependencies(dependencies)?,
    };
    if profile != ProjectProfile::ScalarV1
        && (!dependency_sources.is_empty() || !rust_dependencies.is_empty())
    {
        return Err(grammar(format!(
            "{LABEL} `[dependency-sources]` and `[rust-dependencies]` require the scalar profile"
        )));
    }
    if !dependency_sources.is_empty()
        && sources.iter().any(|path| path.starts_with("dependencies/"))
    {
        return Err(grammar(format!(
            "{LABEL} project sources may not use the reserved `dependencies/` prefix when `[dependency-sources]` is present"
        )));
    }

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
        dependency_sources,
        rust_dependencies,
        target_matrix,
    })
}

fn structural_diagnostics(tables: &[Table<'_>]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (table, keys) in [
        ("package", &["name", "version"][..]),
        ("modules", &["entry", "sources", "tests"][..]),
        ("exports", &["web"][..]),
    ] {
        let Some(found) = tables.iter().find(|candidate| candidate.name == table) else {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} requires a `[{table}]` table"
            )));
            continue;
        };
        for key in keys {
            if !found.entries.iter().any(|(candidate, _)| candidate == key) {
                diagnostics.push(scaffold_diagnostic(format!(
                    "{LABEL} table `[{table}]` requires `{key}`"
                )));
            }
        }
    }

    let profile = table_text(tables, "package", "profile").unwrap_or("scalar");
    if profile != "scalar" && profile_by_name(profile).is_none() {
        diagnostics.push(scaffold_diagnostic(format!(
            "{LABEL} `[package] profile` `{profile}` is not an admitted profile"
        )));
    }
    if let Some(name) = table_text(tables, "package", "name") {
        if !super::valid_name(name) {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} name must match lowercase [a-z][a-z0-9-]* and contain 1..=64 bytes"
            )));
        }
    }
    if let Some(version) = table_text(tables, "package", "version") {
        if !valid_semver(version) {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} `[package] version` must be canonical Semantic Versioning text of at most {MAX_VERSION_BYTES} bytes"
            )));
        }
    }
    if let Some(entry) = table_text(tables, "modules", "entry") {
        if !super::valid_module(entry) {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} entry is not a bounded module name"
            )));
        }
    }

    let command_profile = matches!(
        profile,
        PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1
            | PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2
            | PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1
            | PROJECT_PROFILE_LINE_COMMAND_IO_V1
    );
    if command_profile {
        if let Some(command) = tables.iter().find(|table| table.name == "command") {
            if !command.entries.iter().any(|(key, _)| *key == "function") {
                diagnostics.push(scaffold_diagnostic(format!(
                    "{LABEL} table `[command]` requires `function`"
                )));
            }
        } else {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} profile `{profile}` requires a `[command]` table with `function`"
            )));
        }
        if let Some(capabilities) = tables.iter().find(|table| table.name == "capabilities") {
            if !capabilities
                .entries
                .iter()
                .any(|(key, _)| *key == "required")
            {
                diagnostics.push(scaffold_diagnostic(format!(
                    "{LABEL} table `[capabilities]` requires `required`"
                )));
            }
        } else {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} profile `{profile}` requires a `[capabilities]` table with `required`"
            )));
        }
        let expected_input = match profile {
            PROJECT_PROFILE_USEFUL_DATA_COMMAND_V2 => Some(PROJECT_COMMAND_INPUT_V1),
            PROJECT_PROFILE_LANGUAGE_COMMAND_IO_V1 | PROJECT_PROFILE_LINE_COMMAND_IO_V1 => {
                Some(PROJECT_LANGUAGE_COMMAND_INPUT_V1)
            }
            _ => None,
        };
        let input = table_text(tables, "command", "input");
        if input != expected_input {
            diagnostics.push(scaffold_diagnostic(match expected_input {
                Some(expected) => format!(
                    "{LABEL} profile `{profile}` requires `[command] input = \"{expected}\"`"
                ),
                None => format!("{LABEL} profile `{profile}` does not admit `[command] input`"),
            }));
        }
        let expected_capabilities: &[&str] = if profile == PROJECT_PROFILE_USEFUL_DATA_COMMAND_V1 {
            &[PROJECT_COMMAND_STDOUT_CAPABILITY]
        } else {
            &PROJECT_COMMAND_ADAPTER_CAPABILITIES_V2
        };
        if let Some(required) = table_list(tables, "capabilities", "required") {
            if !required
                .iter()
                .map(String::as_str)
                .eq(expected_capabilities.iter().copied())
            {
                diagnostics.push(scaffold_diagnostic(format!(
                    "{LABEL} profile `{profile}` requires `[capabilities] required = {}`",
                    super::render_array(
                        &expected_capabilities
                            .iter()
                            .map(|capability| (*capability).to_owned())
                            .collect::<Vec<_>>()
                    )
                )));
            }
        }
    } else {
        if tables.iter().any(|table| table.name == "command") {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} profile `{profile}` does not admit a `[command]` table"
            )));
        }
        if tables.iter().any(|table| table.name == "capabilities") {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} profile `{profile}` does not admit a `[capabilities]` table"
            )));
        }
    }

    if let Some(sources) = table_list(tables, "modules", "sources") {
        if !(2..=super::MAX_SOURCES).contains(&sources.len()) {
            diagnostics.push(if sources.len() > super::MAX_SOURCES {
                capacity("sources", super::MAX_SOURCES).remove(0)
            } else {
                scaffold_diagnostic(format!("{LABEL} requires 2..=16 explicit source paths"))
            });
        }
        if !sources.windows(2).all(|pair| pair[0] < pair[1]) {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} source paths must be strictly byte-sorted and unique"
            )));
        }
        if sources.iter().any(|path| {
            path.len() > super::MAX_PATH_BYTES
                || !path.ends_with(".spx")
                || !crate::workspace::evidence_path_is_valid(path)
        }) {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} source paths must be canonical relative .spx paths of at most 240 bytes"
            )));
        }
    }
    if let Some(exports) = table_list(tables, "exports", "web") {
        if !(1..=super::MAX_WEB_EXPORTS).contains(&exports.len()) {
            diagnostics.push(if exports.len() > super::MAX_WEB_EXPORTS {
                capacity("web_exports", super::MAX_WEB_EXPORTS).remove(0)
            } else {
                scaffold_diagnostic(format!(
                    "{LABEL} requires 1..=32 explicit web export identities"
                ))
            });
        }
        if !exports.windows(2).all(|pair| pair[0] < pair[1]) {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} web export identities must be strictly byte-sorted and unique"
            )));
        }
        if exports.iter().any(|id| !super::valid_stable_id(id)) {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} web exports must use bounded lowercase [a-z0-9._-] stable IDs"
            )));
        }
        if let Some(command) = table_text(tables, "command", "function") {
            if exports.len() != 1 || exports.first().map(String::as_str) != Some(command) {
                diagnostics.push(scaffold_diagnostic(format!(
                    "{LABEL} web_exports must contain exactly the command stable ID"
                )));
            }
        }
    }
    if let Some(tests) = table_list(tables, "modules", "tests") {
        if tests.len() != 1 || !tests.first().is_some_and(|name| super::valid_module(name)) {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} tests must contain exactly one bounded module name"
            )));
        }
        if tests.len() == 1
            && table_text(tables, "modules", "entry") == tests.first().map(String::as_str)
        {
            diagnostics.push(scaffold_diagnostic(format!(
                "{LABEL} entry and test modules must be distinct"
            )));
        }
    }

    for table in tables {
        let admitted: &[&str] = match table.name {
            "package" => &["name", "version", "profile"],
            "modules" => &["entry", "sources", "tests"],
            "exports" => &["web"],
            "command" => &["function", "input"],
            "capabilities" => &["required"],
            "dependencies" => continue,
            "targets" => &["matrix"],
            _ => continue,
        };
        for (key, _) in &table.entries {
            if admitted.contains(key) {
                continue;
            }
            let message = if table.name == "package" && PACKAGE_RESERVED_KEYS.contains(key) {
                format!(
                    "key `[package] {key}` is reserved for an additive {LABEL} revision and is not admitted by this toolchain"
                )
            } else {
                format!(
                    "{LABEL} table `[{}]` does not admit key `{key}`",
                    table.name
                )
            };
            diagnostics.push(Diagnostic::io(CODE_UNADMITTED, message).with_help(SCAFFOLD_HELP));
        }
    }
    diagnostics
}

fn table_text<'a>(tables: &'a [Table<'a>], table: &str, key: &str) -> Option<&'a str> {
    let (_, value) = tables
        .iter()
        .find(|candidate| candidate.name == table)?
        .entries
        .iter()
        .find(|(candidate, _)| *candidate == key)?;
    match value {
        Value::Text(value) => Some(value),
        Value::List(_) => None,
    }
}

fn table_list<'a>(tables: &'a [Table<'a>], table: &str, key: &str) -> Option<&'a [String]> {
    let (_, value) = tables
        .iter()
        .find(|candidate| candidate.name == table)?
        .entries
        .iter()
        .find(|(candidate, _)| *candidate == key)?;
    match value {
        Value::List(values) => Some(values),
        Value::Text(_) => None,
    }
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

fn parse_dependency_sources(
    table: TableReader<'_>,
    requirements: &[PackageDependency],
) -> Result<Vec<PackageDependencySource>, Vec<Diagnostic>> {
    if table.entries.len() > MAX_DEPENDENCY_SOURCES {
        return Err(capacity("dependency_sources", MAX_DEPENDENCY_SOURCES));
    }
    let mut sources = Vec::with_capacity(table.entries.len());
    for (name, value) in table.entries {
        if !valid_dependency_identity(name) {
            return Err(grammar(format!(
                "{LABEL} dependency-source names must be valid dependency identities; found `{name}`"
            )));
        }
        let Value::Text(path) = value else {
            return Err(grammar(format!(
                "{LABEL} dependency source `{name}` must be one project-relative Subject-v3 `.json` path"
            )));
        };
        let portable_probe = path.strip_suffix(".json").map(|stem| format!("{stem}.spx"));
        if path.len() > super::MAX_PATH_BYTES
            || portable_probe
                .as_deref()
                .is_none_or(|probe| !crate::workspace::evidence_path_is_valid(probe))
        {
            return Err(grammar(format!(
                "{LABEL} dependency source `{name}` must be a canonical project-relative `.json` path of at most {} bytes",
                super::MAX_PATH_BYTES
            )));
        }
        sources.push(PackageDependencySource {
            name: name.to_owned(),
            path,
        });
    }
    if !sources.windows(2).all(|pair| pair[0].name < pair[1].name) {
        return Err(grammar(format!(
            "{LABEL} dependency sources must be strictly byte-sorted by name"
        )));
    }
    for source in &sources {
        if !requirements
            .iter()
            .any(|requirement| requirement.name == source.name)
            && requirements.is_empty()
        {
            return Err(grammar(format!(
                "{LABEL} dependency source `{}` requires a `[dependencies]` root requirement",
                source.name
            )));
        }
    }
    Ok(sources)
}

fn parse_rust_dependencies(table: TableReader<'_>) -> Result<Vec<RustDependency>, Vec<Diagnostic>> {
    if table.entries.len() > MAX_RUST_DEPENDENCIES {
        return Err(capacity("rust_dependencies", MAX_RUST_DEPENDENCIES));
    }
    let mut dependencies = Vec::with_capacity(table.entries.len());
    for (name, value) in table.entries {
        if !valid_rust_dependency_name(name) {
            return Err(grammar(format!(
                "{LABEL} Rust dependency names must match lowercase Cargo package names [a-z][a-z0-9_-]*; found `{name}`"
            )));
        }
        let Value::List(mut values) = value else {
            return Err(grammar(format!(
                "{LABEL} Rust dependency `{name}` must be an array whose first item is an exact version and whose remaining items are feature names"
            )));
        };
        if values.is_empty() {
            return Err(grammar(format!(
                "{LABEL} Rust dependency `{name}` requires an exact `=x.y.z` version"
            )));
        }
        let version = values.remove(0);
        if !version.starts_with('=')
            || package_range::parse_range(&version, dependency_range_error).is_err()
        {
            return Err(grammar(format!(
                "{LABEL} Rust dependency `{name}` version must use exact `=x.y.z` syntax"
            )));
        }
        if !values.windows(2).all(|pair| pair[0] < pair[1])
            || values.iter().any(|feature| !valid_rust_feature(feature))
        {
            return Err(grammar(format!(
                "{LABEL} Rust dependency `{name}` features must be strictly byte-sorted Cargo feature names"
            )));
        }
        dependencies.push(RustDependency {
            name: name.to_owned(),
            version,
            features: values,
        });
    }
    if !dependencies
        .windows(2)
        .all(|pair| pair[0].name < pair[1].name)
    {
        return Err(grammar(format!(
            "{LABEL} Rust dependencies must be strictly byte-sorted by name"
        )));
    }
    let mut crate_idents = dependencies
        .iter()
        .map(RustDependency::crate_ident)
        .collect::<Vec<_>>();
    crate_idents.sort();
    if crate_idents.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(grammar(format!(
            "{LABEL} Rust dependency names collide after Cargo maps `-` to `_`"
        )));
    }
    Ok(dependencies)
}

fn valid_rust_dependency_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && name.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
        && !name.ends_with(['-', '_'])
}

fn valid_rust_feature(feature: &str) -> bool {
    (1..=64).contains(&feature.len())
        && feature
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'/' | b'.'))
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
    let mut blocks = Vec::with_capacity(10);
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
    if !manifest.dependency_sources.is_empty() {
        let mut block = String::from("[dependency-sources]\n");
        for source in &manifest.dependency_sources {
            block.push_str(&format!("{} = \"{}\"\n", source.name, source.path));
        }
        blocks.push(block);
    }
    if !manifest.rust_dependencies.is_empty() {
        let mut block = String::from("[rust-dependencies]\n");
        for dependency in &manifest.rust_dependencies {
            let mut values = vec![dependency.version.clone()];
            values.extend(dependency.features.iter().cloned());
            block.push_str(&format!(
                "{} = {}\n",
                dependency.name,
                super::render_array(&values)
            ));
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
        "{help}; tables follow the order {}, keys follow the specification order, arrays are one line with `, ` separators, and blocks are separated by one blank line; {SCAFFOLD_HELP}",
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

    /// Exact local Subject-v3 files that close ordinary SEMAPRAX dependency
    /// resolution without registry or ambient-cache access.
    pub fn dependency_sources(&self) -> &[PackageDependencySource] {
        &self.dependency_sources
    }

    /// Exact crates.io dependencies carried into a generated Native Rust SDK.
    pub fn rust_dependencies(&self) -> &[RustDependency] {
        &self.rust_dependencies
    }

    /// The declared `[targets] matrix`, or `None` when the manifest leaves every
    /// target admitted.
    pub fn target_matrix(&self) -> Option<&[String]> {
        self.target_matrix.as_deref()
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

fn scaffold_diagnostic(message: String) -> Diagnostic {
    Diagnostic::io("SPX-J100", message).with_help(SCAFFOLD_HELP)
}

fn grammar(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![scaffold_diagnostic(message.into())]
}

fn capacity(field: &str, limit: usize) -> Vec<Diagnostic> {
    vec![
        Diagnostic::io("SPX-J101", format!("Project v1 `{field}` exceeds {limit}"))
            .with_help(SCAFFOLD_HELP),
    ]
}

fn unadmitted(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::io(CODE_UNADMITTED, message).with_help(SCAFFOLD_HELP)]
}
