//! The original strict line-oriented front-matter subset (HP-13). Kept
//! verbatim as the `legacy-v1` reference: it refuses folded/literal YAML and
//! every ecosystem key, which is exactly why official Agent Skills need the
//! [`super::agentskills`] profile. The catalog no longer uses it to read
//! skills; tests pin the contrast so a regression to it is caught.

use super::bundle::{name_ok, token_ok};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LegacyFront {
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub license: Option<String>,
    pub tags: Vec<String>,
    pub dependencies: Vec<String>,
    pub body: String,
}

/// Parse with the legacy subset. `Err` is the old `SPX-HPM001` message text.
pub fn parse_legacy_front_matter(text: &str) -> Result<LegacyFront, String> {
    let text_n = text.replace("\r\n", "\n");
    let rest = text_n
        .strip_prefix("---\n")
        .ok_or_else(|| "missing front-matter block (`---` first line)".to_string())?;
    let end = rest
        .find("\n---\n")
        .map(|i| (i, i + 5))
        .or_else(|| {
            rest.strip_suffix("\n---")
                .map(|_| (rest.len() - 4, rest.len()))
        })
        .ok_or_else(|| "unterminated front-matter block".to_string())?;
    let (front, body) = (&rest[..end.0], &rest[end.1..]);
    let mut name = None;
    let mut description = None;
    let mut version = None;
    let mut license = None;
    let mut tags = None;
    let mut deps = None;
    let mut seen = std::collections::BTreeSet::new();
    for (i, line) in front.lines().enumerate() {
        let line = line.trim_end();
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }
        let (k, v) = line
            .split_once(':')
            .ok_or_else(|| format!("front-matter line {} is not `key: value`", i + 2))?;
        let k = k.trim();
        if !seen.insert(k.to_string()) {
            return Err(format!("duplicate front-matter key `{k}`"));
        }
        let v = v.trim();
        match k {
            "name" => name = Some(scalar(v)?),
            "description" => description = Some(scalar(v)?),
            "version" => version = Some(scalar(v)?),
            "license" => license = Some(scalar(v)?),
            "tags" => tags = Some(list(v)?),
            "dependencies" => deps = Some(list(v)?),
            other => return Err(format!("unsupported front-matter key `{other}`")),
        }
    }
    let name = name.ok_or_else(|| "missing required key `name`".to_string())?;
    let description =
        description.ok_or_else(|| "missing required key `description`".to_string())?;
    if !name_ok(&name) {
        return Err(format!("invalid skill name `{name}`"));
    }
    let tags = tags.unwrap_or_default();
    if let Some(t) = tags.iter().find(|t| !token_ok(t)) {
        return Err(format!("invalid tag `{t}`"));
    }
    Ok(LegacyFront {
        name,
        description,
        version,
        license,
        tags,
        dependencies: deps.unwrap_or_default(),
        body: body.to_string(),
    })
}

fn unquote(v: &str) -> Result<String, String> {
    for q in ['"', '\''] {
        if let Some(inner) = v.strip_prefix(q) {
            return inner
                .strip_suffix(q)
                .map(str::to_string)
                .ok_or_else(|| format!("unterminated quote in `{v}`"));
        }
    }
    Ok(v.to_string())
}

fn scalar(v: &str) -> Result<String, String> {
    if v.starts_with('[') || v.starts_with('{') {
        return Err(format!("expected a scalar, found `{v}`"));
    }
    unquote(v)
}

fn list(v: &str) -> Result<Vec<String>, String> {
    let inner = v
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| format!("expected `[a, b]`, found `{v}`"))?;
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    inner.split(',').map(|s| unquote(s.trim())).collect()
}
