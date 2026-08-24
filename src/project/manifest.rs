use crate::diagnostic::Diagnostic;

pub const PROJECT_SCHEMA: &str = "semaprax.project.v1";
pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
pub const MAX_NAME_BYTES: usize = 64;
pub const MAX_MODULE_BYTES: usize = 240;
pub const MAX_PATH_BYTES: usize = 240;
pub const MAX_STABLE_ID_BYTES: usize = 128;
pub const MAX_SOURCES: usize = 16;
pub const MAX_WEB_EXPORTS: usize = 32;
pub const MAX_TOTAL_SOURCE_BYTES: usize = 16 * 1024 * 1024;

/// The exact, closed Project v1 manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectManifest {
    name: String,
    entry: String,
    sources: Vec<String>,
    web_exports: Vec<String>,
    test_module: String,
}

impl ProjectManifest {
    /// Parse the fixed canonical TOML subset used by Project v1.
    pub fn parse(source: &str) -> Result<Self, Vec<Diagnostic>> {
        if source.len() > MAX_MANIFEST_BYTES {
            return Err(capacity("manifest_bytes", MAX_MANIFEST_BYTES));
        }
        if source.as_bytes().contains(&0) || source.starts_with('\u{feff}') || source.contains('\r')
        {
            return Err(grammar("Project v1 manifest is not canonical UTF-8 TOML"));
        }
        let lines = source.split('\n').collect::<Vec<_>>();
        if lines.len() != 7 || lines.last() != Some(&"") {
            return Err(grammar(
                "Project v1 manifest must contain exactly six ordered assignments and one terminal LF",
            ));
        }
        let schema = parse_string_assignment(lines[0], "schema")?;
        let name = parse_string_assignment(lines[1], "name")?;
        let entry = parse_string_assignment(lines[2], "entry")?;
        let sources = parse_array_assignment(lines[3], "sources")?;
        let web_exports = parse_array_assignment(lines[4], "web_exports")?;
        let tests = parse_array_assignment(lines[5], "tests")?;

        if schema != PROJECT_SCHEMA {
            return Err(grammar(
                "Project v1 manifest schema is not semaprax.project.v1",
            ));
        }
        if !valid_name(&name) {
            return Err(grammar(
                "Project v1 name must match lowercase [a-z][a-z0-9-]* and contain 1..=64 bytes",
            ));
        }
        if !valid_module(&entry) {
            return Err(grammar("Project v1 entry is not a bounded module name"));
        }
        if !(2..=MAX_SOURCES).contains(&sources.len()) {
            return Err(if sources.len() > MAX_SOURCES {
                capacity("sources", MAX_SOURCES)
            } else {
                grammar("Project v1 requires 2..=16 explicit source paths")
            });
        }
        require_strict_order(&sources, "source paths")?;
        for path in &sources {
            if path.len() > MAX_PATH_BYTES
                || !path.ends_with(".spx")
                || !crate::workspace::evidence_path_is_valid(path)
            {
                return Err(grammar(
                    "Project v1 source paths must be canonical relative .spx paths of at most 240 bytes",
                ));
            }
        }
        if !(1..=MAX_WEB_EXPORTS).contains(&web_exports.len()) {
            return Err(if web_exports.len() > MAX_WEB_EXPORTS {
                capacity("web_exports", MAX_WEB_EXPORTS)
            } else {
                grammar("Project v1 requires 1..=32 explicit web export identities")
            });
        }
        require_strict_order(&web_exports, "web export identities")?;
        if web_exports.iter().any(|id| !valid_stable_id(id)) {
            return Err(grammar(
                "Project v1 web exports must use bounded lowercase [a-z0-9._-] stable IDs",
            ));
        }
        if tests.len() != 1 || !valid_module(&tests[0]) {
            return Err(grammar(
                "Project v1 tests must contain exactly one bounded module name",
            ));
        }
        if entry == tests[0] {
            return Err(grammar(
                "Project v1 entry and test modules must be distinct",
            ));
        }

        let manifest = Self {
            name,
            entry,
            sources,
            web_exports,
            test_module: tests.into_iter().next().expect("one test module"),
        };
        if manifest.to_canonical_toml() != source {
            return Err(grammar("Project v1 manifest is not canonical"));
        }
        Ok(manifest)
    }

    pub fn name(&self) -> &str {
        &self.name
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
        format!(
            "schema = \"{PROJECT_SCHEMA}\"\nname = \"{}\"\nentry = \"{}\"\nsources = {}\nweb_exports = {}\ntests = [\"{}\"]\n",
            self.name,
            self.entry,
            render_array(&self.sources),
            render_array(&self.web_exports),
            self.test_module,
        )
    }
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
