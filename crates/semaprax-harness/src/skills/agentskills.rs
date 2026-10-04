//! Agent Skills compatibility profile (HN-03): `SKILL.md` front matter parsed
//! with the bounded YAML subset in [`super::yaml`], normalized into a derived,
//! versioned view. The original file bytes are never rewritten; the view is
//! recomputed from them.
//!
//! Standard fields: `name`, `description`, `license`, `compatibility`,
//! `metadata` (string map), `allowed-tools`. Legacy Semaprax fields `version`,
//! `tags`, `dependencies` stay readable. Ecosystem fields (`argument-hint`,
//! `hooks`, ...) and namespaced keys (`x-*`, `vendor.key`, `vendor:key`) are
//! preserved as inert data. `allowed-tools` and `hooks` are *requests* only:
//! parsing grants nothing and starts nothing.

use super::bundle::{name_ok, token_ok};
use super::d;
use super::yaml::{self, Yaml, YamlError, YamlLimits};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::Value;
use std::collections::BTreeMap;

/// Version of the normalized metadata view derived from the original bytes.
pub const VIEW_VERSION: &str = "semaprax.skill-view.v1";
pub const DESCRIPTION_MAX_CHARS: usize = 1024;
pub const COMPATIBILITY_MAX_CHARS: usize = 500;

/// Ecosystem keys kept as inert extension data.
const ECOSYSTEM_KEYS: &[&str] = &[
    "argument-hint",
    "user-invocable",
    "disable-model-invocation",
    "model",
    "context",
    "agent",
    "effort",
    "paths",
    "shell",
    "when_to_use",
    "hooks",
    "allowed-tools",
];

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Profile {
    pub name: String,
    pub description: String,
    pub license: Option<String>,
    pub compatibility: Option<String>,
    pub metadata: BTreeMap<String, String>,
    pub version: Option<String>,
    pub tags: Vec<String>,
    pub dependencies: Vec<String>,
    /// Requested capabilities (`tool:<pattern>`, `hook:declared`): requests
    /// that need host authorization, never grants.
    pub requested: Vec<String>,
    /// Inert ecosystem/namespaced data, canonical JSON per key.
    pub extensions: BTreeMap<String, Value>,
    pub body: String,
}

fn bad(dir: &str, m: impl AsRef<str>) -> HarnessDiagnostic {
    d("SPX-HPM001", format!("{dir}/SKILL.md: {}", m.as_ref()))
}

fn split(text: &str) -> Option<(String, String)> {
    let t = text.replace("\r\n", "\n");
    let rest = t.strip_prefix("---\n")?;
    if let Some(b) = rest.strip_prefix("---\n") {
        return Some((String::new(), b.to_string()));
    }
    if rest == "---" {
        return Some((String::new(), String::new()));
    }
    let (end, next) = rest.find("\n---\n").map(|i| (i, i + 5)).or_else(|| {
        rest.strip_suffix("\n---")
            .map(|_| (rest.len() - 4, rest.len()))
    })?;
    Some((rest[..end].to_string(), rest[next..].to_string()))
}

/// Paren-aware whitespace split (`Bash(git add:*) Read`).
fn split_tools(s: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth) = (Vec::new(), String::new(), 0i32);
    for c in s.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = (depth - 1).max(0),
            c if c.is_whitespace() && depth == 0 => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn inert_key(k: &str) -> bool {
    k.starts_with("x-") || k.contains('.') || k.contains(':')
}

/// Parse `SKILL.md` text. `shown` names the directory in diagnostics.
pub fn parse(shown: &str, text: &str) -> HarnessResult<Profile> {
    let (front, body) = split(text)
        .ok_or_else(|| bad(shown, "missing or unterminated front-matter block (`---`)"))?;
    let doc = yaml::parse(&front, &YamlLimits::FRONT_MATTER).map_err(|e| match e {
        YamlError::Syntax(m) => bad(shown, format!("front matter is not valid YAML: {m}")),
        YamlError::Unsafe(m) => d("SPX-HPM030", format!("{shown}/SKILL.md: {m}")),
        YamlError::Bound(m) => d("SPX-HPM031", format!("{shown}/SKILL.md: {m}")),
        YamlError::Duplicate(k) => d(
            "SPX-HPM032",
            format!("{shown}/SKILL.md: duplicate front-matter key `{k}`"),
        ),
    })?;
    let Yaml::Map(entries) = doc else {
        return Err(bad(shown, "front matter must be a mapping"));
    };
    let mut p = Profile {
        body,
        ..Profile::default()
    };
    let (mut name, mut description) = (None, None);
    let scalar = |k: &str, v: &Yaml| -> HarnessResult<String> {
        v.as_str()
            .map(str::to_string)
            .ok_or_else(|| bad(shown, format!("`{k}` must be a scalar")))
    };
    let list = |k: &str, v: &Yaml| -> HarnessResult<Vec<String>> {
        match v {
            Yaml::Seq(items) => items
                .iter()
                .map(|i| {
                    i.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| bad(shown, format!("`{k}` must hold scalars")))
                })
                .collect(),
            _ => Err(bad(
                shown,
                format!("expected a list for `{k}`, e.g. `{k}: [a, b]`"),
            )),
        }
    };
    for (k, v) in &entries {
        match k.as_str() {
            "name" => name = Some(scalar(k, v)?),
            "description" => description = Some(scalar(k, v)?.trim_end().to_string()),
            "license" => p.license = Some(scalar(k, v)?.trim_end().to_string()),
            "compatibility" => {
                let c = scalar(k, v)?.trim_end().to_string();
                if c.is_empty() || c.chars().count() > COMPATIBILITY_MAX_CHARS {
                    return Err(bad(
                        shown,
                        format!("`compatibility` must be 1-{COMPATIBILITY_MAX_CHARS} characters"),
                    ));
                }
                p.compatibility = Some(c);
            }
            "version" => p.version = Some(scalar(k, v)?),
            "tags" => p.tags = list(k, v)?,
            "dependencies" => p.dependencies = list(k, v)?,
            "metadata" => {
                let Yaml::Map(m) = v else {
                    return Err(bad(shown, "`metadata` must be a map of strings"));
                };
                for (mk, mv) in m {
                    let s = mv
                        .as_str()
                        .ok_or_else(|| bad(shown, format!("`metadata.{mk}` must be a string")))?;
                    p.metadata.insert(mk.clone(), s.trim_end().to_string());
                }
            }
            "allowed-tools" => {
                let tools = match v {
                    Yaml::Str(s) => split_tools(s),
                    other => list(k, other)?,
                };
                p.requested
                    .extend(tools.into_iter().map(|t| format!("tool:{t}")));
                p.extensions.insert(k.clone(), v.to_json());
            }
            "hooks" => {
                p.requested.push("hook:declared".to_string());
                p.extensions.insert(k.clone(), v.to_json());
            }
            other if ECOSYSTEM_KEYS.contains(&other) || inert_key(other) => {
                p.extensions.insert(k.clone(), v.to_json());
            }
            other => {
                return Err(bad(
                    shown,
                    format!("unsupported front-matter key `{other}`"),
                ))
            }
        }
    }
    p.requested.sort();
    p.requested.dedup();
    let name = name.ok_or_else(|| bad(shown, "missing required key `name`"))?;
    let description =
        description.ok_or_else(|| bad(shown, "missing required key `description`"))?;
    if !name_ok(&name) {
        return Err(bad(shown, format!("invalid skill name `{name}`")));
    }
    if description.is_empty() || description.chars().count() > DESCRIPTION_MAX_CHARS {
        return Err(bad(
            shown,
            format!("`description` must be 1-{DESCRIPTION_MAX_CHARS} characters"),
        ));
    }
    if let Some(t) = p.tags.iter().find(|t| !token_ok(t)) {
        return Err(bad(shown, format!("invalid tag `{t}`")));
    }
    p.name = name;
    p.description = description;
    Ok(p)
}
