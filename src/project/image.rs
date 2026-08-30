//! Derived, immutable semantic images over already authenticated Project inputs.
//!
//! Serialized images are evidence, never a source of HIR or filesystem authority.
//! Replay rebuilds the complete bytes from a retained checked revision and compares
//! them exactly; it never deserializes an input image into compiler state.

use std::io::{self, Write};
use std::sync::Arc;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;
use crate::workspace_analysis::{
    WorkspaceAnalysisTargetKind, WorkspaceContextOptions, WorkspaceImpactOptions,
};

use super::ProjectRevision;

pub const PROJECT_SEMANTIC_IMAGE_SCHEMA: &str = "semaprax.semantic-workspace-image.v1";
pub const PROJECT_SEMANTIC_IMAGE_SYMBOL_SCHEMA: &str =
    "semaprax.semantic-workspace-image-symbol.v1";
/// A manually versioned serialization compatibility identity, not a compiler
/// executable digest or a claim that two compiler builds are identical.
pub const PROJECT_SEMANTIC_IMAGE_COMPATIBILITY: &str =
    "semaprax.semantic-workspace-image-compatibility.v1";
pub const MAX_SEMANTIC_IMAGE_BYTES: usize = 32 * 1024 * 1024;
const MAX_SYMBOL_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_SYMBOL_ID_BYTES: usize = 4096;
const DIGEST_BYTES: usize = 71;

/// Authority-free image retaining validated HIR and typed indexes through one
/// immutable revision. The canonical bytes contain derived facts, not HIR that
/// a reader may trust or execute without reconstructing that revision.
pub struct ProjectSemanticImage {
    revision: Arc<ProjectRevision>,
    json: String,
    digest: String,
}

impl ProjectSemanticImage {
    /// Derive an image only for the caller's exact expected Project revision.
    pub fn derive(
        revision: Arc<ProjectRevision>,
        expected_revision: &str,
    ) -> Result<Self, Vec<Diagnostic>> {
        validate_digest(expected_revision)?;
        if expected_revision != revision.project_revision() {
            return Err(stale("semantic image Project revision is stale"));
        }
        // This JSON is emitted by the compiler and held by ProjectRevision. No
        // untrusted image bytes are parsed by either derivation or replay.
        let graph: Value = serde_json::from_str(revision.semantic_graph())
            .map_err(|_| grammar("retained Project semantic graph is invalid"))?;
        let sources = revision
            .sources()
            .iter()
            .map(|source| {
                json!({
                    "path": source.path(),
                    "source_graph_schema": source.source_graph_schema(),
                    "source_revision": source.source_revision(),
                    "source_digest": source.source_digest(),
                })
            })
            .collect::<Vec<_>>();
        let value = json!({
            "schema": PROJECT_SEMANTIC_IMAGE_SCHEMA,
            "compiler": {
                "package": env!("CARGO_PKG_NAME"),
                "version": env!("CARGO_PKG_VERSION"),
                "image_compatibility": PROJECT_SEMANTIC_IMAGE_COMPATIBILITY,
            },
            "project_revision": revision.project_revision(),
            "workspace_revision": revision.workspace_revision(),
            "canonical_manifest": revision.manifest().to_canonical_toml(),
            "canonical_workspace_manifest": revision.workspace_manifest(),
            "sources": sources,
            "project_graph_digest": revision.semantic_graph_digest(),
            "project_graph": graph,
            "indexes": revision.semantic.image_indexes(),
            "limits": {"max_image_bytes": MAX_SEMANTIC_IMAGE_BYTES},
            "nonclaims": [
                "no_source_or_publication_authority",
                "no_untrusted_hir_deserialization",
                "no_compiler_binary_identity",
                "no_incremental_compilation_or_persistent_cache",
                "no_new_analysis_facets_or_target_execution",
            ],
        });
        let json = render(value, true, MAX_SEMANTIC_IMAGE_BYTES)?;
        let mut digest = Sha256::new();
        digest.update(b"semaprax.semantic-workspace-image.digest.v1\0");
        digest.update((json.len() as u64).to_le_bytes());
        digest.update(json.as_bytes());
        let digest = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(digest.finalize())
        );
        Ok(Self {
            revision,
            json,
            digest,
        })
    }

    /// Compare an input capsule byte-for-byte against a fresh derivation. Even
    /// otherwise equivalent JSON, changed whitespace, and a missing LF fail.
    pub fn replay(
        revision: Arc<ProjectRevision>,
        expected_revision: &str,
        bytes: &[u8],
    ) -> Result<Self, Vec<Diagnostic>> {
        if bytes.len() > MAX_SEMANTIC_IMAGE_BYTES {
            return Err(limit("semantic image exceeds its byte limit"));
        }
        let image = Self::derive(revision, expected_revision)?;
        if image.to_json().as_bytes() != bytes {
            return Err(stale("semantic image does not match exact revision replay"));
        }
        Ok(image)
    }

    /// Canonical compact JSON including exactly one terminal LF. The digest
    /// binds these exact bytes, including that LF.
    pub fn to_json(&self) -> &str {
        &self.json
    }

    pub fn image_digest(&self) -> &str {
        &self.digest
    }

    pub fn revision(&self) -> &Arc<ProjectRevision> {
        &self.revision
    }

    /// Compact declaration lookup through the existing retained typed index.
    /// Query output follows Project queries and has no terminal LF.
    pub fn symbol(&self, expected_image_digest: &str, id: &str) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected_image_digest)?;
        if id.len() > MAX_SYMBOL_ID_BYTES {
            return Err(limit("semantic image symbol ID exceeds its byte limit"));
        }
        if id.is_empty() || id.contains('\0') {
            return Err(grammar("semantic image symbol ID is invalid"));
        }
        let symbol = self
            .revision
            .semantic
            .image_symbol(id)
            .ok_or_else(|| grammar("semantic image declaration is unavailable"))?;
        render(
            json!({
                "schema": PROJECT_SEMANTIC_IMAGE_SYMBOL_SCHEMA,
                "image_revision": self.image_digest(),
                "project_revision": self.revision.project_revision(),
                "symbol": symbol,
            }),
            false,
            MAX_SYMBOL_RESPONSE_BYTES,
        )
    }

    /// Delegate bounded Context to the retained revision after image selection.
    /// Its existing Project schema is unchanged; the result is not image proof.
    pub fn context(
        &self,
        expected_image_digest: &str,
        target_kind: WorkspaceAnalysisTargetKind,
        target: &str,
        options: WorkspaceContextOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected_image_digest)?;
        self.revision.semantic_context(target_kind, target, options)
    }

    /// Delegate bounded Impact to the retained revision after image selection.
    /// Its existing Project schema is unchanged; the result is not image proof.
    pub fn impact(
        &self,
        expected_image_digest: &str,
        target_kind: WorkspaceAnalysisTargetKind,
        target: &str,
        options: WorkspaceImpactOptions,
    ) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected_image_digest)?;
        self.revision.semantic_impact(target_kind, target, options)
    }

    pub(super) fn require_digest(&self, expected: &str) -> Result<(), Vec<Diagnostic>> {
        validate_digest(expected)?;
        if expected != self.image_digest() {
            return Err(stale("semantic image digest is stale"));
        }
        Ok(())
    }
}

fn validate_digest(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() > DIGEST_BYTES {
        return Err(limit(
            "semantic image revision digest exceeds its byte limit",
        ));
    }
    if value.len() != DIGEST_BYTES
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(grammar("semantic image revision digest is invalid"));
    }
    Ok(())
}

/// Enforce the wire bound before extending the output buffer, including LF.
struct BoundedWriter {
    bytes: Vec<u8>,
    max_bytes: usize,
}

impl Write for BoundedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.max_bytes.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("semantic image byte limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn render(
    mut value: Value,
    terminal_lf: bool,
    max_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    // Remain deterministic even if a downstream consumer enables serde_json's
    // preserve_order feature through Cargo feature unification.
    value.sort_all_objects();
    let mut writer = BoundedWriter {
        bytes: Vec::new(),
        max_bytes,
    };
    serde_json::to_writer(&mut writer, &value)
        .map_err(|_| limit("semantic image exceeds its byte limit"))?;
    if terminal_lf {
        writer
            .write_all(b"\n")
            .map_err(|_| limit("semantic image exceeds its byte limit"))?;
    }
    String::from_utf8(writer.bytes)
        .map_err(|_| grammar("semantic image serialization is not UTF-8"))
}

fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G219", message)]
}

fn limit(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G220", message)]
}

fn stale(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G221", message)]
}
