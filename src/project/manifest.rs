use crate::diagnostic::Diagnostic;

/// Frozen scalar Project Manifest v1 schema.
pub const PROJECT_SCHEMA: &str = "semaprax.project.v1";
/// Additive Project Manifest v2 schema used by the Useful Text Consumer
/// profile. V1 parsing and rendering remain byte-for-byte unchanged.
pub const PROJECT_SCHEMA_V2: &str = "semaprax.project.v2";
pub const PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1: &str = "useful-text-consumer.v1";
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
/// scalar shape or the additive v2 Useful Text Consumer shape.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectManifest {
    schema: &'static str,
    name: String,
    package_version: Option<String>,
    profile: Option<&'static str>,
    entry: String,
    sources: Vec<String>,
    web_exports: Vec<String>,
    test_module: String,
}

impl ProjectManifest {
    /// Parse either frozen Project v1 or additive Project v2 canonical TOML.
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
        let (schema, name, package_version, profile, entry, sources, web_exports, tests) =
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
                        None,
                        parse_string_assignment(lines[2], "entry")?,
                        parse_array_assignment(lines[3], "sources")?,
                        parse_array_assignment(lines[4], "web_exports")?,
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
                        Some(PROJECT_PROFILE_USEFUL_TEXT_CONSUMER_V1),
                        parse_string_assignment(lines[4], "entry")?,
                        parse_array_assignment(lines[5], "sources")?,
                        parse_array_assignment(lines[6], "web_exports")?,
                        parse_array_assignment(lines[7], "tests")?,
                    )
                }
                _ => {
                    return Err(grammar(
                        "Project manifest schema is neither semaprax.project.v1 nor semaprax.project.v2",
                    ))
                }
            };
        let version_label = if schema == PROJECT_SCHEMA {
            "Project v1"
        } else {
            "Project v2"
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
        self.profile
    }

    pub fn is_v2(&self) -> bool {
        self.schema == PROJECT_SCHEMA_V2
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
        } else {
            format!(
                "schema = \"{PROJECT_SCHEMA_V2}\"\nname = \"{}\"\nversion = \"{}\"\nprofile = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
                self.name,
                self.package_version
                    .as_deref()
                    .expect("Project v2 carries a package version"),
                self.profile.expect("Project v2 carries a profile"),
                self.entry,
                render_array(&self.sources),
                render_array(&self.web_exports),
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
