//! Host apply policy and protected-fact guards. Providers may suggest a
//! candidate; native compiler facts (requirements, laws, effects, test
//! verdicts) are protected and checked on the compiler's own preview output.

use super::compiler::CandidatePreview;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The compiler's fixed requirement inventory; the host always sends all of it.
pub const REQUIREMENTS: [&str; 9] = [
    "preserve_stable_identity",
    "preserve_public_exports",
    "update_all_callers",
    "no_new_effects",
    "no_new_capabilities",
    "preserve_contracts",
    "revalidate_ownership_and_cleanup",
    "preserve_project_profile_admission",
    "preserve_admitted_core_targets",
];

pub const APPLY_POLICY_SCHEMA: &str = "semaprax.harness-apply-policy.v1";

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

/// Preexisting host policy permitting automatic publication of a fully
/// checked candidate through the compiler's Git publication route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplyPolicy {
    pub auto_apply: bool,
    /// Absolute path of a `semaprax.candidate-git-host-policy.v1` file.
    pub publication_policy: PathBuf,
    pub allowed_intents: Option<Vec<String>>,
    pub digest: String,
}

impl ApplyPolicy {
    /// Load `file`; it and the publication policy must be regular files
    /// outside the project (a repository must not grant itself authority).
    pub fn load(file: &Path, project_root: &Path) -> HarnessResult<ApplyPolicy> {
        let bytes = read_regular(file, "apply policy")?;
        let v: Value = serde_json::from_slice(&bytes)
            .map_err(|e| d("SPX-HPD082", format!("apply policy is not JSON: {e}")))?;
        let m = v
            .as_object()
            .ok_or_else(|| d("SPX-HPD082", "apply policy must be an object"))?;
        for k in m.keys() {
            if ![
                "schema",
                "auto_apply",
                "publication_policy",
                "allowed_intents",
            ]
            .contains(&k.as_str())
            {
                return Err(d(
                    "SPX-HPD082",
                    format!("unknown apply policy member `{k}`"),
                ));
            }
        }
        if m.get("schema").and_then(Value::as_str) != Some(APPLY_POLICY_SCHEMA) {
            return Err(d(
                "SPX-HPD082",
                format!("apply policy schema must be `{APPLY_POLICY_SCHEMA}`"),
            ));
        }
        let auto_apply = m
            .get("auto_apply")
            .and_then(Value::as_bool)
            .ok_or_else(|| d("SPX-HPD082", "`auto_apply` must be a boolean"))?;
        let pp = PathBuf::from(
            m.get("publication_policy")
                .and_then(Value::as_str)
                .ok_or_else(|| d("SPX-HPD082", "`publication_policy` must be a string"))?,
        );
        if !pp.is_absolute() {
            return Err(d(
                "SPX-HPD082",
                "`publication_policy` must be an absolute path",
            ));
        }
        read_regular(&pp, "publication policy")?;
        for (what, p) in [("apply policy", file), ("publication policy", pp.as_path())] {
            let canon = p
                .canonicalize()
                .map_err(|e| d("SPX-HPD082", format!("{what}: {e}")))?;
            if canon.starts_with(project_root) {
                return Err(d("SPX-HPD060", format!("{what} lives inside the project; authority must be a preexisting host file outside it")));
            }
        }
        let allowed_intents = match m.get("allowed_intents") {
            None => None,
            Some(a) => Some(
                a.as_array()
                    .and_then(|a| {
                        a.iter()
                            .map(|x| x.as_str().map(str::to_string))
                            .collect::<Option<Vec<_>>>()
                    })
                    .ok_or_else(|| d("SPX-HPD082", "`allowed_intents` must be a string array"))?,
            ),
        };
        Ok(ApplyPolicy {
            auto_apply,
            publication_policy: pp,
            allowed_intents,
            digest: crate::json::sha256_plain(&bytes),
        })
    }

    pub fn permits(&self, intent_kind: &str) -> bool {
        self.auto_apply
            && self
                .allowed_intents
                .as_ref()
                .is_none_or(|a| a.iter().any(|k| k == intent_kind))
    }
}

fn read_regular(path: &Path, what: &str) -> HarnessResult<Vec<u8>> {
    let meta = std::fs::symlink_metadata(path)
        .map_err(|e| d("SPX-HPD082", format!("{what} {}: {e}", path.display())))?;
    if !meta.is_file() || meta.len() > 64 * 1024 {
        return Err(d(
            "SPX-HPD082",
            format!("{what} must be a regular file of at most 64 KiB"),
        ));
    }
    std::fs::read(path).map_err(|e| d("SPX-HPD082", format!("{what}: {e}")))
}

/// The compiler's source digest: SHA-256 over a fixed domain, the LE u64
/// length and the bytes (`semaprax.semantic-review.source-digest.v1`).
pub fn source_digest(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b"semaprax.semantic-review.source-digest.v1\0");
    h.update((bytes.len() as u64).to_le_bytes());
    h.update(bytes);
    format!(
        "sha256:{}",
        h.finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

fn law_lines(src: &str) -> Vec<String> {
    src.lines()
        .map(str::trim)
        .filter(|l| {
            ["requires ", "ensures ", "invariant "]
                .iter()
                .any(|p| l.starts_with(p))
        })
        .map(str::to_string)
        .collect()
}

fn effect_tokens(src: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for l in src.lines().map(str::trim) {
        if let Some(rest) = l.strip_prefix("uses") {
            let inner = rest
                .trim()
                .trim_start_matches('{')
                .split('}')
                .next()
                .unwrap_or("");
            out.extend(
                inner
                    .split(',')
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty()),
            );
        }
    }
    out
}

/// Verify the compiler's preview against the authenticated base. Refusals:
/// HPD041 revision binding, HPD044 weakened requirements, HPD042 deleted law,
/// HPD043 widened effect, HPD005 stale base file.
pub fn check_protected_facts(
    root: &Path,
    compiler_revision: &str,
    intent_kind: &str,
    preview: &CandidatePreview,
) -> HarnessResult<()> {
    if preview.base_revision != compiler_revision {
        return Err(d(
            "SPX-HPD041",
            format!(
                "candidate base `{}` differs from the checked revision `{compiler_revision}`",
                preview.base_revision
            ),
        ));
    }
    let full: Vec<String> = REQUIREMENTS.iter().map(|s| s.to_string()).collect();
    if preview.requirements != full || preview.change_requirements.iter().any(|r| *r != full) {
        return Err(d(
            "SPX-HPD044",
            "candidate requirements differ from the fixed protected inventory",
        ));
    }
    if preview.unresolved_holes != 0 {
        return Err(d("SPX-HPD040", "candidate has unresolved holes"));
    }
    let (mut base_effects, mut cand_effects) = (BTreeSet::new(), BTreeSet::new());
    for c in &preview.source_changes {
        let rel = Path::new(&c.path);
        if rel.is_absolute()
            || rel
                .components()
                .any(|x| matches!(x, std::path::Component::ParentDir))
        {
            return Err(d(
                "SPX-HPD040",
                format!("candidate path `{}` escapes the project", c.path),
            ));
        }
        let base = std::fs::read(root.join(rel)).map_err(|e| {
            d(
                "SPX-HPD005",
                format!("stale revision: cannot read `{}`: {e}", c.path),
            )
        })?;
        if source_digest(&base) != c.base_digest {
            return Err(d(
                "SPX-HPD005",
                format!(
                    "stale revision: `{}` differs from the candidate's base",
                    c.path
                ),
            ));
        }
        let base_src = String::from_utf8_lossy(&base).into_owned();
        let (bl, cl) = (law_lines(&base_src), law_lines(&c.replacement_source));
        let renames = matches!(intent_kind, "rename_declaration" | "move_declaration");
        let deleted = if renames {
            cl.len() < bl.len()
        } else {
            bl.iter().any(|l| !cl.contains(l))
        };
        if deleted {
            return Err(d(
                "SPX-HPD042",
                format!(
                    "candidate deletes or weakens a law (requires/ensures) in `{}`",
                    c.path
                ),
            ));
        }
        base_effects.extend(effect_tokens(&base_src));
        cand_effects.extend(effect_tokens(&c.replacement_source));
    }
    if let Some(extra) = cand_effects.difference(&base_effects).next() {
        return Err(d(
            "SPX-HPD043",
            format!("candidate widens declared effects (`{extra}`)"),
        ));
    }
    Ok(())
}

pub fn policy_json(p: &Option<ApplyPolicy>) -> Value {
    match p {
        None => json!(null),
        Some(p) => json!({"auto_apply": p.auto_apply, "digest": p.digest}),
    }
}
