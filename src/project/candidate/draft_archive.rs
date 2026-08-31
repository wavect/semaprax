//! Self-contained source recovery for typed drafts, without exposing last-valid
//! candidate state as the meaning of unresolved intentions.
use std::sync::Arc;

use serde_json::{json, Value};

use super::{ProjectCandidateDraft, MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES};
use crate::diagnostic::Diagnostic;
use crate::project::candidate::wire;
use crate::project::{
    ProjectCandidateArchive, ProjectManifest, ProjectRevision, MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES,
};

pub const PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA: &str =
    "semaprax.project-candidate-draft-archive.v1";
pub const PROJECT_CANDIDATE_DRAFT_ARCHIVE_COMPATIBILITY: &str =
    "semaprax.project-candidate-draft-archive-source-replay.v1";
pub const MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES: usize = 128 * 1024 * 1024;
const DOMAIN: &[u8] = b"semaprax.project-candidate-draft-archive.payload.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Canonical source-backed draft transport. This owns neither source authority
/// nor an approval, and never releases an unresolved draft as a candidate.
pub struct ProjectCandidateDraftArchive {
    json: String,
    digest: String,
    draft: String,
    base: String,
}

impl ProjectCandidateDraft {
    /// Narrow host admission predicate; no prior candidate or revision escapes
    /// the unfinished draft. Historical source requires the same live manifest.
    pub(crate) fn matches_manifest(&self, manifest: &ProjectManifest) -> bool {
        let expected = manifest.to_canonical_toml();
        self.last_valid
            .base_revision()
            .manifest()
            .to_canonical_toml()
            == expected
            && self.last_valid.revision().manifest().to_canonical_toml() == expected
    }
}

impl ProjectCandidateDraftArchive {
    pub fn prepare(draft: &ProjectCandidateDraft, expected_draft: &str) -> Result<Self> {
        validate_digest(expected_draft)?;
        if draft.draft_digest() != expected_draft {
            return Err(binding("draft archive requires the exact draft identity"));
        }
        let candidate = &draft.last_valid;
        let archive = ProjectCandidateArchive::prepare(candidate, candidate.candidate_digest())?;
        let capsule = draft.recovery_capsule()?;
        let mut value = json!({
            "schema":PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA,"compiler":compiler(),
            "base_revision":archive.base_revision(),
            "candidate_digest":archive.candidate_digest(),
            "candidate_archive_digest":archive.archive_digest(),
            "candidate_archive":archive.to_json(),"draft_recovery_capsule":capsule,
            "draft_digest":expected_draft,
            "source_authority":false,"approval_authority":false,"trusted_hir":false
        });
        let digest = wire::digest(DOMAIN, render(value.clone())?.as_bytes());
        value["archive_digest"] = json!(digest);
        let json = render(value)?;
        preflight(json.as_bytes())?;
        Ok(Self {
            json,
            digest,
            draft: expected_draft.to_owned(),
            base: archive.base_revision().to_owned(),
        })
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn archive_digest(&self) -> &str {
        &self.digest
    }
    pub fn draft_digest(&self) -> &str {
        &self.draft
    }
    pub fn base_revision(&self) -> &str {
        &self.base
    }

    /// Rebuild the original Project exclusively from archived canonical bytes,
    /// replay the checked candidate history and pending selectors, then compare
    /// the entire regenerated archive. No caller-supplied base or file access.
    /// Byte limits include JSON escaping and do not promise a total-memory or
    /// replay-time bound. Completion remains a separate operation.
    pub fn restore(
        bytes: &[u8],
        expected_archive: &str,
        expected_draft: &str,
    ) -> Result<ProjectCandidateDraft> {
        validate_digest(expected_archive)?;
        validate_digest(expected_draft)?;
        preflight(bytes)?;
        let mut value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("draft archive is not valid bounded JSON"))?;
        closed(
            &value,
            &[
                "schema",
                "compiler",
                "base_revision",
                "candidate_digest",
                "candidate_archive_digest",
                "candidate_archive",
                "draft_recovery_capsule",
                "draft_digest",
                "source_authority",
                "approval_authority",
                "trusted_hir",
                "archive_digest",
            ],
        )?;
        if value["schema"] != PROJECT_CANDIDATE_DRAFT_ARCHIVE_SCHEMA
            || value["compiler"] != compiler()
            || value["source_authority"] != false
            || value["approval_authority"] != false
            || value["trusted_hir"] != false
        {
            return Err(invalid(
                "draft archive schema, compiler compatibility, or authority claims disagree",
            ));
        }
        for field in [
            "archive_digest",
            "draft_digest",
            "base_revision",
            "candidate_digest",
            "candidate_archive_digest",
        ] {
            validate_digest(text(&value, field)?)?;
        }
        if value["archive_digest"] != expected_archive || value["draft_digest"] != expected_draft {
            return Err(binding("draft archive selectors disagree"));
        }
        if text(&value, "candidate_archive")?.len() > MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES
            || text(&value, "draft_recovery_capsule")?.len()
                > MAX_PROJECT_CANDIDATE_DRAFT_RECOVERY_BYTES
        {
            return Err(capacity(
                "draft archive nested transport exceeds its byte bound",
            ));
        }
        if render(value.clone())?.as_bytes() != bytes {
            return Err(invalid(
                "draft archive requires exact canonical JSON and terminal LF",
            ));
        }
        value
            .as_object_mut()
            .expect("closed object")
            .remove("archive_digest");
        if wire::digest(DOMAIN, render(value.clone())?.as_bytes()) != expected_archive {
            return Err(binding("draft archive content digest disagrees"));
        }
        // Both existing transport owners retain their independent raw parsing,
        // compiler admission, source/history bounds and exact replay checks.
        let candidate = ProjectCandidateArchive::restore(
            text(&value, "candidate_archive")?.as_bytes(),
            text(&value, "candidate_archive_digest")?,
            text(&value, "candidate_digest")?,
        )?;
        if candidate.base_revision().project_revision() != text(&value, "base_revision")? {
            return Err(binding("draft archive rebuilt original base disagrees"));
        }
        let draft = ProjectCandidateDraft::restore(
            Arc::clone(candidate.base_revision()),
            text(&value, "base_revision")?,
            text(&value, "draft_recovery_capsule")?.as_bytes(),
        )?;
        if draft.last_valid.candidate_digest() != candidate.candidate_digest()
            || draft.draft_digest() != expected_draft
        {
            return Err(binding(
                "draft archive replayed candidate or draft identity disagrees",
            ));
        }
        if draft.recovery_capsule()? != text(&value, "draft_recovery_capsule")?
            || Self::prepare(&draft, expected_draft)?.to_json().as_bytes() != bytes
        {
            return Err(binding("draft archive exact independent replay disagrees"));
        }
        Ok(draft)
    }

    /// Host-only historical startup admission: source may be historical, but
    /// paths, profiles, entries, tests and grants must match the live manifest.
    pub(crate) fn restore_for_manifest(
        bytes: &[u8],
        expected_archive: &str,
        expected_draft: &str,
        manifest: &ProjectManifest,
    ) -> Result<ProjectCandidateDraft> {
        let draft = Self::restore(bytes, expected_archive, expected_draft)?;
        if !draft.matches_manifest(manifest) {
            return Err(binding(
                "draft archive manifest disagrees with the live host manifest",
            ));
        }
        Ok(draft)
    }

    /// In-session import also requires the live original base. A ready draft
    /// cannot bypass the separate host-selected historical startup boundary.
    pub(crate) fn restore_for_base(
        bytes: &[u8],
        expected_archive: &str,
        expected_draft: &str,
        base: &ProjectRevision,
    ) -> Result<ProjectCandidateDraft> {
        let draft =
            Self::restore_for_manifest(bytes, expected_archive, expected_draft, base.manifest())?;
        if draft.last_valid.base_revision().project_revision() != base.project_revision() {
            return Err(binding(
                "draft archive original base disagrees with the live session base",
            ));
        }
        Ok(draft)
    }
}

fn compiler() -> Value {
    json!({"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),
        "compatibility":PROJECT_CANDIDATE_DRAFT_ARCHIVE_COMPATIBILITY,"binary_identity_claimed":false})
}
fn render(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES)
        .map_err(|_| capacity("draft archive output exceeds its byte bound"))
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| invalid("draft archive field must be text"))
}
fn closed(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("draft archive requires a closed object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("draft archive has missing or unknown fields"));
    }
    Ok(())
}
fn validate_digest(value: &str) -> Result<()> {
    wire::validate_digest(value)
        .map_err(|_| invalid("draft archive selector must be canonical SHA-256"))
}

// Nested transports are strings, so this raw scan needs only a small fixed
// outer inventory. Nested owners separately bound their decoded JSON inputs.
fn preflight(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_PROJECT_CANDIDATE_DRAFT_ARCHIVE_BYTES {
        return Err(capacity("draft archive input exceeds its byte bound"));
    }
    let (mut string, mut escape, mut scalar) = (false, false, false);
    let (mut depth, mut nodes) = (0usize, 0usize);
    for byte in bytes {
        if string {
            if escape {
                escape = false;
            } else if *byte == b'\\' {
                escape = true;
            } else if *byte == b'"' {
                string = false;
            }
            continue;
        }
        match byte {
            b'"' => {
                string = true;
                scalar = false;
                nodes += 1;
            }
            b'{' | b'[' => {
                depth += 1;
                scalar = false;
                nodes += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                scalar = false;
            }
            b':' | b',' | b' ' | b'\t' | b'\r' | b'\n' => scalar = false,
            _ => {
                if !scalar {
                    scalar = true;
                    nodes += 1;
                }
            }
        }
        if depth > 16 || nodes > 1024 {
            return Err(capacity(
                "draft archive JSON exceeds its depth or node bound",
            ));
        }
    }
    Ok(())
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G340", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G341", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G342", message)]
}
