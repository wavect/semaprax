//! Skill-injection ownership: official skills a host already has installed are
//! never injected a second time. Declarations come from the v2 handshake or from
//! a user-named project skills directory read once, read-only (never `$HOME`).

use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use crate::skills::official::OfficialSet;
use serde_json::{json, Value};
use std::path::Path;

/// Claude Code version the integration is tested against (see `claude.rs`).
pub const MIN_CLAUDE_MAJOR: u64 = 2;
const MAX_DECLARED: usize = 32;
const SKILL_MD_LIMIT: u64 = 256 * 1024;

fn bad(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPN001", msg)
}

/// Unsupported host/version fails clearly. Only Claude Code has a version rule;
/// other hosts are accepted by declaration, never inferred from a name.
pub fn check_host_support(name: &str, version: &str) -> HarnessResult<()> {
    if name != "claude-code" {
        return Ok(());
    }
    let major = version
        .split('.')
        .next()
        .and_then(|m| m.parse::<u64>().ok());
    match major {
        Some(m) if m >= MIN_CLAUDE_MAJOR => Ok(()),
        _ => Err(HarnessDiagnostic::new(
            "SPX-HPN007",
            format!(
                "unsupported Claude Code version `{version}`: the skills bridge needs MCP prompts/tools from {MIN_CLAUDE_MAJOR}.x (tested {})",
                super::claude::PINNED_VERSION
            ),
        )),
    }
}

/// `host_skills`: `[{"name": "ponytail", "digest": "sha256:..."|null}]`.
pub fn parse_declared(v: &Value) -> HarnessResult<Vec<(String, Option<String>)>> {
    let a = v
        .as_array()
        .filter(|a| a.len() <= MAX_DECLARED)
        .ok_or_else(|| bad("`host_skills` must be an array of at most 32 entries"))?;
    let mut out = Vec::new();
    for e in a {
        let m = e
            .as_object()
            .ok_or_else(|| bad("`host_skills` entries must be {name, digest?}"))?;
        if m.keys().any(|k| k != "name" && k != "digest") {
            return Err(bad("`host_skills` entries allow only `name` and `digest`"));
        }
        let name = m
            .get("name")
            .and_then(Value::as_str)
            .filter(|n| !n.is_empty() && n.len() <= 64)
            .ok_or_else(|| bad("`host_skills[].name` must be a short string"))?;
        let digest = match m.get("digest") {
            None | Some(Value::Null) => None,
            Some(Value::String(d)) if d.len() <= 100 => Some(d.clone()),
            _ => return Err(bad("`host_skills[].digest` must be null or a short string")),
        };
        out.push((name.to_string(), digest));
    }
    Ok(out)
}

/// Whether the host's copy is the revision Semaprax would deliver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Revision {
    Same,
    Different,
    Unverified,
}

impl Revision {
    pub fn as_str(self) -> &'static str {
        match self {
            Revision::Same => "same-revision",
            Revision::Different => "different-revision",
            Revision::Unverified => "revision-not-verified",
        }
    }
}

#[derive(Clone, Debug)]
pub struct HostSkill {
    /// Official skill id.
    pub id: String,
    pub source: &'static str,
    pub digest: Option<String>,
    pub revision: Revision,
}

impl HostSkill {
    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "source": self.source, "host_digest": self.digest, "revision": self.revision.as_str()})
    }
}

fn classify(set: &OfficialSet, id: &str, digest: Option<&str>) -> Revision {
    let (Some(k), Some(d)) = (set.find(id), digest) else {
        return Revision::Unverified;
    };
    let same = k.bundle_digest.as_deref() == Some(d)
        || k.files
            .iter()
            .any(|f| f.path == "SKILL.md" && f.sha256 == d);
    if same {
        Revision::Same
    } else {
        Revision::Different
    }
}

/// Entries for official skills only; unknown names are ignored (not ours).
pub fn from_declared(set: &OfficialSet, decl: &[(String, Option<String>)]) -> Vec<HostSkill> {
    decl.iter()
        .filter_map(|(n, d)| {
            let k = set.embedded_skills().find(|k| k.matches(n))?;
            Some(HostSkill {
                id: k.id.clone(),
                source: "handshake",
                digest: d.clone(),
                revision: classify(set, &k.id, d.as_deref()),
            })
        })
        .collect()
}

/// Read-only scan of `<dir>/<name>/SKILL.md`. Symlinks are never followed and
/// oversized files are skipped; the host's bytes are hashed, never executed.
pub fn scan_dir(set: &OfficialSet, dir: &Path) -> Vec<HostSkill> {
    let mut names: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.flatten().map(|e| e.path()).collect(),
        Err(_) => return Vec::new(),
    };
    names.sort();
    let mut out = Vec::new();
    for p in names {
        let Ok(meta) = std::fs::symlink_metadata(&p) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        let file = p.join("SKILL.md");
        let Ok(fm) = std::fs::symlink_metadata(&file) else {
            continue;
        };
        if !fm.is_file() || fm.len() > SKILL_MD_LIMIT {
            continue;
        }
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        let Ok(text) = std::str::from_utf8(&bytes) else {
            continue;
        };
        let dir_name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let name = crate::skills::agentskills::parse(dir_name, text)
            .map(|pr| pr.name)
            .unwrap_or_else(|_| dir_name.to_string());
        let Some(k) = set.embedded_skills().find(|k| k.matches(&name)) else {
            continue;
        };
        let digest = sha256_plain(&bytes);
        out.push(HostSkill {
            id: k.id.clone(),
            source: "project-skills-dir",
            revision: classify(set, &k.id, Some(&digest)),
            digest: Some(digest),
        });
    }
    out
}
