//! Source-backed recovery, without trusted HIR, root paths, or authority.
use std::sync::Arc;

use serde_json::{json, Value};

use crate::diagnostic::Diagnostic;
use crate::project::{ProjectManifest, MAX_MANIFEST_BYTES, MAX_PATH_BYTES, MAX_SOURCES};
use crate::semantic_workspace::SemanticWorkspaceSource;

use super::{
    build, wire, ProjectCandidate, ProjectRevision, MAX_PROJECT_CANDIDATE_RECOVERY_BYTES,
    MAX_TOTAL_SOURCE_BYTES,
};

pub const PROJECT_CANDIDATE_ARCHIVE_SCHEMA: &str = "semaprax.project-candidate-archive.v1";
pub const MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES: usize = 128 * 1024 * 1024;
const COMPATIBILITY: &str = "semaprax.project-candidate-archive-source-replay.v1";
const DOMAIN: &[u8] = b"semaprax.project-candidate-archive.payload.v1\0";
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub struct ProjectCandidateArchive {
    json: String,
    digest: String,
    candidate: String,
    base: String,
}

impl ProjectCandidateArchive {
    pub fn prepare(candidate: &ProjectCandidate, expected_candidate: &str) -> Result<Self> {
        validate_digest(expected_candidate)?;
        if candidate.candidate_digest() != expected_candidate {
            return Err(binding("archive requires the exact candidate identity"));
        }
        let base = candidate.base_revision();
        let mut value = json!({
            "schema":PROJECT_CANDIDATE_ARCHIVE_SCHEMA,"compiler":compiler(),
            "canonical_manifest":base.manifest().to_canonical_toml(),
            "base_revision":base.project_revision(),"base_workspace_revision":base.workspace_revision(),
            "base_graph_digest":base.semantic_graph_digest(),
            "sources":base.sources().iter().map(|source| json!({
                "path":source.path(),"source":source.source(),"source_digest":source.source_digest(),
                "source_revision":source.source_revision(),"source_graph_schema":source.source_graph_schema()
            })).collect::<Vec<_>>(),
            "recovery_capsule":candidate.recovery_capsule()?,
            "candidate_digest":candidate.candidate_digest(),
            "candidate_project_revision":candidate.revision().project_revision(),
            "source_authority":false,"approval_authority":false,"trusted_hir":false
        });
        let digest = wire::digest(DOMAIN, render(value.clone())?.as_bytes());
        value["archive_digest"] = json!(digest);
        Ok(Self {
            json: render(value)?,
            digest,
            candidate: expected_candidate.to_owned(),
            base: base.project_revision().to_owned(),
        })
    }

    pub fn to_json(&self) -> &str {
        &self.json
    }
    pub fn archive_digest(&self) -> &str {
        &self.digest
    }
    pub fn candidate_digest(&self) -> &str {
        &self.candidate
    }
    pub fn base_revision(&self) -> &str {
        &self.base
    }

    /// Restore from exact canonical source bytes, then replay every existing
    /// recovery intention and rederive the entire archive. No filesystem access.
    pub fn restore(
        bytes: &[u8],
        expected_archive: &str,
        expected_candidate: &str,
    ) -> Result<ProjectCandidate> {
        validate_digest(expected_archive)?;
        validate_digest(expected_candidate)?;
        preflight(bytes)?;
        let mut value: Value = serde_json::from_slice(bytes)
            .map_err(|_| invalid("archive is not valid bounded JSON"))?;
        closed(
            &value,
            &[
                "schema",
                "compiler",
                "canonical_manifest",
                "base_revision",
                "base_workspace_revision",
                "base_graph_digest",
                "sources",
                "recovery_capsule",
                "candidate_digest",
                "candidate_project_revision",
                "source_authority",
                "approval_authority",
                "trusted_hir",
                "archive_digest",
            ],
        )?;
        if value["schema"] != PROJECT_CANDIDATE_ARCHIVE_SCHEMA
            || value["compiler"] != compiler()
            || value["source_authority"] != false
            || value["approval_authority"] != false
            || value["trusted_hir"] != false
        {
            return Err(invalid(
                "archive schema, compiler compatibility, or authority claims do not match",
            ));
        }
        for field in [
            "archive_digest",
            "candidate_digest",
            "base_revision",
            "base_workspace_revision",
            "base_graph_digest",
            "candidate_project_revision",
        ] {
            validate_digest(text(&value, field)?)?;
        }
        if value["archive_digest"] != expected_archive
            || value["candidate_digest"] != expected_candidate
        {
            return Err(binding(
                "archive selectors do not match expected archive and candidate identities",
            ));
        }
        if render(value.clone())?.as_bytes() != bytes {
            return Err(invalid(
                "archive must have exact canonical JSON bytes and terminal LF",
            ));
        }
        value
            .as_object_mut()
            .expect("closed object")
            .remove("archive_digest");
        if wire::digest(DOMAIN, render(value.clone())?.as_bytes()) != expected_archive {
            return Err(binding("archive content digest does not match"));
        }
        let manifest_text = text(&value, "canonical_manifest")?;
        if manifest_text.len() > MAX_MANIFEST_BYTES {
            return Err(capacity("archive manifest exceeds its byte bound"));
        }
        let manifest = ProjectManifest::parse(manifest_text)?;
        if manifest.to_canonical_toml() != manifest_text {
            return Err(invalid("archive manifest must be canonical"));
        }
        let sources = value["sources"]
            .as_array()
            .ok_or_else(|| invalid("archive sources must be an ordered array"))?;
        if sources.len() > MAX_SOURCES {
            return Err(capacity("archive exceeds its source count bound"));
        }
        if sources.len() != manifest.sources().len() {
            return Err(invalid(
                "archive source inventory differs from the manifest",
            ));
        }
        let mut total = 0usize;
        for (source, path) in sources.iter().zip(manifest.sources()) {
            closed(
                source,
                &[
                    "path",
                    "source",
                    "source_digest",
                    "source_revision",
                    "source_graph_schema",
                ],
            )?;
            let selected = text(source, "path")?;
            if selected.len() > MAX_PATH_BYTES || selected != path {
                return Err(invalid(
                    "archive source order or logical path differs from the manifest",
                ));
            }
            validate_digest(text(source, "source_digest")?)?;
            validate_digest(text(source, "source_revision")?)?;
            text(source, "source_graph_schema")?;
            total = total
                .checked_add(text(source, "source")?.len())
                .ok_or_else(|| capacity("archive source byte accounting overflow"))?;
            if total > MAX_TOTAL_SOURCE_BYTES {
                return Err(capacity(
                    "archive sources exceed their aggregate byte bound",
                ));
            }
        }
        let capsule = text(&value, "recovery_capsule")?;
        if capsule.len() > MAX_PROJECT_CANDIDATE_RECOVERY_BYTES {
            return Err(capacity("archive recovery capsule exceeds its byte bound"));
        }
        let owned = sources
            .iter()
            .map(|source| SemanticWorkspaceSource {
                path: source["path"]
                    .as_str()
                    .expect("source path checked")
                    .to_owned(),
                source: source["source"]
                    .as_str()
                    .expect("source text checked")
                    .to_owned(),
            })
            .collect();
        let built = build::build_owned(&manifest, owned)?;
        let base = Arc::new(ProjectRevision::from_built(manifest, built));
        if base.project_revision() != text(&value, "base_revision")?
            || base.workspace_revision() != text(&value, "base_workspace_revision")?
            || base.semantic_graph_digest() != text(&value, "base_graph_digest")?
        {
            return Err(binding(
                "archive independently rebuilt original base identity disagrees",
            ));
        }
        for (fact, source) in sources.iter().zip(base.sources()) {
            if fact["source_digest"] != source.source_digest()
                || fact["source_revision"] != source.source_revision()
                || fact["source_graph_schema"] != source.source_graph_schema()
            {
                return Err(binding(
                    "archive independently rebuilt source facts disagree",
                ));
            }
        }
        let expected_base = base.project_revision().to_owned();
        let candidate = ProjectCandidate::restore(base, &expected_base, capsule.as_bytes())?;
        if candidate.candidate_digest() != expected_candidate
            || candidate.revision().project_revision()
                != text(&value, "candidate_project_revision")?
        {
            return Err(binding("archive recovered candidate identity disagrees"));
        }
        if Self::prepare(&candidate, expected_candidate)?
            .to_json()
            .as_bytes()
            != bytes
        {
            return Err(binding("archive exact independent rederivation disagrees"));
        }
        Ok(candidate)
    }
}

fn compiler() -> Value {
    json!({"package":env!("CARGO_PKG_NAME"),"version":env!("CARGO_PKG_VERSION"),"compatibility":COMPATIBILITY,"binary_identity_claimed":false})
}
fn render(value: Value) -> Result<String> {
    wire::render(value, MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES)
        .map_err(|_| capacity("archive output exceeds its byte bound"))
}
fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str> {
    value[field]
        .as_str()
        .ok_or_else(|| invalid("archive field must be text"))
}
fn closed(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("archive requires a closed object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(invalid("archive has missing or unknown fields"));
    }
    Ok(())
}
fn validate_digest(value: &str) -> Result<()> {
    wire::validate_digest(value)
        .map_err(|_| invalid("archive selector must be a canonical SHA-256 digest"))
}
// Bound potential Value allocations before parsing. Source and recovery bytes
// are strings, so the outer archive needs only a small, fixed JSON inventory.
fn preflight(bytes: &[u8]) -> Result<()> {
    if bytes.len() > MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES {
        return Err(capacity("archive input exceeds its byte bound"));
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
            return Err(capacity("archive JSON exceeds its depth or node bound"));
        }
    }
    Ok(())
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G296", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G297", message)]
}
fn binding(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G298", message)]
}
