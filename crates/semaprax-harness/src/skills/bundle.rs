//! Bundle formats: plain Markdown `SKILL.md` with a small front-matter subset
//! and the `skill-bundle.json` (`semaprax.skill-bundle.v1`) manifest.

use super::d;
use crate::diag::HarnessResult;
use crate::json::{self, JsonLimits};
use serde_json::Value;
use std::path::{Path, PathBuf};

const MAX_FILE_BYTES: u64 = 256 * 1024;
pub const MANIFEST_SCHEMA: &str = "semaprax.skill-bundle.v1";

/// Ecosystems that are named but not adopted (stable diagnostic SPX-HPM008).
pub const UNSUPPORTED: &[(&str, &str)] = &[
    (".cursorrules", "Cursor rules"),
    (".cursor", "Cursor rules"),
    ("mcp-prompts.json", "MCP prompts"),
    ("extension.vsixmanifest", "VS Code extensions"),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BundleFormat {
    Markdown,
    Manifest,
}

impl BundleFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Markdown => "markdown",
            Self::Manifest => "manifest-v1",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Bundle {
    pub format: BundleFormat,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub license: Option<String>,
    pub tags: Vec<String>,
    pub dependencies: Vec<String>,
    pub body: String,
    /// `scripts/<file>` entries; never executed, only listed.
    pub scripts: Vec<String>,
    pub digest: String,
    pub dir: PathBuf,
}

/// Name of an unsupported-ecosystem marker present in `dir`, if any.
pub fn detect_unsupported(dir: &Path) -> Option<(&'static str, &'static str)> {
    UNSUPPORTED
        .iter()
        .find(|(f, _)| std::fs::symlink_metadata(dir.join(f)).is_ok())
        .copied()
}

pub fn is_bundle_dir(dir: &Path) -> bool {
    dir.join("SKILL.md").is_file() || dir.join("skill-bundle.json").is_file()
}

fn read_text(path: &Path) -> HarnessResult<(Vec<u8>, String)> {
    let shown = path.display();
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| d("SPX-HPM001", format!("cannot read {shown}: {e}")))?;
    if !meta.is_file() {
        return Err(d(
            "SPX-HPM001",
            format!("{shown} is not a regular file (symlinks are refused)"),
        ));
    }
    if meta.len() > MAX_FILE_BYTES {
        return Err(d(
            "SPX-HPM001",
            format!("{shown} exceeds {MAX_FILE_BYTES} bytes"),
        ));
    }
    let bytes =
        std::fs::read(path).map_err(|e| d("SPX-HPM001", format!("cannot read {shown}: {e}")))?;
    let text = String::from_utf8(bytes.clone())
        .map_err(|_| d("SPX-HPM001", format!("{shown} is not valid UTF-8")))?;
    Ok((bytes, text))
}

/// Length-delimited, domain-separated digest over named parts.
pub fn bundle_digest(parts: &[(String, Vec<u8>)]) -> String {
    let mut buf = Vec::new();
    for (name, bytes) in parts {
        buf.extend_from_slice(name.as_bytes());
        buf.push(0);
        buf.extend_from_slice(bytes.len().to_string().as_bytes());
        buf.push(0);
        buf.extend_from_slice(bytes);
    }
    json::sha256_labeled("semaprax.skill-bundle.v1", &buf)
}

fn scripts_of(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(dir.join("scripts")) {
        for e in rd.flatten() {
            if let Some(n) = e.file_name().to_str() {
                out.push(format!("scripts/{n}"));
            }
        }
    }
    out.sort();
    out
}

fn token_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_.:/".contains(&b))
}

fn name_ok(s: &str) -> bool {
    token_ok(s) && !s.contains('/') && !s.contains(':') && !s.starts_with(['-', '.'])
}

/// Read one bundle directory; `Ok(None)` when it has neither layout.
pub fn read_bundle(dir: &Path) -> HarnessResult<Option<Bundle>> {
    let md = dir.join("SKILL.md");
    let manifest = dir.join("skill-bundle.json");
    if std::fs::symlink_metadata(&md).is_ok() {
        let (bytes, text) = read_text(&md)?;
        return parse_markdown(dir, &bytes, &text).map(Some);
    }
    if std::fs::symlink_metadata(&manifest).is_ok() {
        return parse_manifest(dir, &manifest).map(Some);
    }
    Ok(None)
}

fn parse_markdown(dir: &Path, bytes: &[u8], text: &str) -> HarnessResult<Bundle> {
    let bad = |m: String| d("SPX-HPM001", format!("{}/SKILL.md: {m}", dir.display()));
    let text_n = text.replace("\r\n", "\n");
    let rest = text_n
        .strip_prefix("---\n")
        .ok_or_else(|| bad("missing front-matter block (`---` first line)".into()))?;
    let end = rest
        .find("\n---\n")
        .map(|i| (i, i + 5))
        .or_else(|| {
            rest.strip_suffix("\n---")
                .map(|_| (rest.len() - 4, rest.len()))
        })
        .ok_or_else(|| bad("unterminated front-matter block".into()))?;
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
            .ok_or_else(|| bad(format!("front-matter line {} is not `key: value`", i + 2)))?;
        let k = k.trim();
        if !seen.insert(k.to_string()) {
            return Err(bad(format!("duplicate front-matter key `{k}`")));
        }
        let v = v.trim();
        match k {
            "name" => name = Some(scalar(v).map_err(&bad)?),
            "description" => description = Some(scalar(v).map_err(&bad)?),
            "version" => version = Some(scalar(v).map_err(&bad)?),
            "license" => license = Some(scalar(v).map_err(&bad)?),
            "tags" => tags = Some(list(v).map_err(&bad)?),
            "dependencies" => deps = Some(list(v).map_err(&bad)?),
            other => return Err(bad(format!("unsupported front-matter key `{other}`"))),
        }
    }
    let name = name.ok_or_else(|| bad("missing required key `name`".into()))?;
    let description =
        description.ok_or_else(|| bad("missing required key `description`".into()))?;
    if !name_ok(&name) {
        return Err(bad(format!("invalid skill name `{name}`")));
    }
    let tags = tags.unwrap_or_default();
    if let Some(t) = tags.iter().find(|t| !token_ok(t)) {
        return Err(bad(format!("invalid tag `{t}`")));
    }
    let scripts = scripts_of(dir);
    let mut parts = vec![("SKILL.md".to_string(), bytes.to_vec())];
    parts.extend(scripts.iter().map(|s| (s.clone(), Vec::new())));
    Ok(Bundle {
        format: BundleFormat::Markdown,
        name,
        description,
        version,
        license,
        tags,
        dependencies: deps.unwrap_or_default(),
        body: body.to_string(),
        scripts,
        digest: bundle_digest(&parts),
        dir: dir.to_path_buf(),
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

fn rel_ok(p: &str) -> bool {
    !p.is_empty()
        && !p.starts_with('/')
        && !p.contains(['\\', '\0'])
        && p.split('/').all(|s| !s.is_empty() && s != "." && s != "..")
}

fn parse_manifest(dir: &Path, path: &Path) -> HarnessResult<Bundle> {
    let (bytes, _) = read_text(path)?;
    let bad = |m: String| d("SPX-HPM002", format!("{}: {m}", path.display()));
    let v = json::parse_strict(&bytes, &JsonLimits::frame(MAX_FILE_BYTES as usize))
        .map_err(|e| bad(e.message))?;
    let m = v
        .as_object()
        .ok_or_else(|| bad("manifest must be an object".into()))?;
    const KNOWN: &[&str] = &[
        "schema",
        "name",
        "description",
        "version",
        "license",
        "tags",
        "dependencies",
        "entries",
    ];
    if let Some(k) = m.keys().find(|k| !KNOWN.contains(&k.as_str())) {
        return Err(bad(format!("unknown manifest member `{k}`")));
    }
    let s = |k: &str| -> Result<Option<String>, _> {
        match m.get(k) {
            None => Ok(None),
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(bad(format!("`{k}` must be a string"))),
        }
    };
    let strs = |k: &str| -> Result<Vec<String>, _> {
        match m.get(k) {
            None => Ok(Vec::new()),
            Some(Value::Array(a)) => a
                .iter()
                .map(|x| {
                    x.as_str()
                        .map(str::to_string)
                        .ok_or_else(|| bad(format!("`{k}` must hold strings")))
                })
                .collect(),
            Some(_) => Err(bad(format!("`{k}` must be an array"))),
        }
    };
    if s("schema")?.as_deref() != Some(MANIFEST_SCHEMA) {
        return Err(bad(format!("schema must be `{MANIFEST_SCHEMA}`")));
    }
    let name = s("name")?.ok_or_else(|| bad("missing `name`".into()))?;
    if !name_ok(&name) {
        return Err(bad(format!("invalid skill name `{name}`")));
    }
    let tags = strs("tags")?;
    if let Some(t) = tags.iter().find(|t| !token_ok(t)) {
        return Err(bad(format!("invalid tag `{t}`")));
    }
    let entries = m
        .get("entries")
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty() && a.len() <= 64)
        .ok_or_else(|| bad("`entries` must be a non-empty array of at most 64".into()))?;
    let mut parts = vec![("skill-bundle.json".to_string(), bytes.clone())];
    let mut body = String::new();
    for e in entries {
        let eo = e
            .as_object()
            .ok_or_else(|| bad("entry must be an object".into()))?;
        if eo.len() != 2 {
            return Err(bad("entry must have exactly `file` and `digest`".into()));
        }
        let file = eo
            .get("file")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("entry needs `file`".into()))?;
        let want = eo
            .get("digest")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("entry needs `digest`".into()))?;
        if !rel_ok(file) {
            return Err(bad(format!(
                "entry file `{file}` is not a bundle-relative path"
            )));
        }
        let (fb, ft) = read_text(&dir.join(file))
            .map_err(|e| d("SPX-HPM003", format!("entry `{file}`: {}", e.message)))?;
        let got = json::sha256_plain(&fb);
        if got != want {
            return Err(d(
                "SPX-HPM003",
                format!("entry `{file}` digest {got} differs from manifest digest {want}"),
            ));
        }
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(&ft);
        parts.push((file.to_string(), fb));
    }
    let description = s("description")?
        .or_else(|| {
            body.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_default();
    let scripts = scripts_of(dir);
    parts.extend(scripts.iter().map(|x| (x.clone(), Vec::new())));
    Ok(Bundle {
        format: BundleFormat::Manifest,
        name,
        description,
        version: s("version")?,
        license: s("license")?,
        tags,
        dependencies: strs("dependencies")?,
        body,
        scripts,
        digest: bundle_digest(&parts),
        dir: dir.to_path_buf(),
    })
}
