//! Actual native C11 text and existing header projections, kept as source
//! inspection artifacts. No C compilation, linkage changes or FFI authority.

use super::*;
use crate::project::{ProjectManifest, ProjectRevision};
use crate::semantic_workspace::SemanticWorkspaceSource;
use std::collections::BTreeMap;

const SCHEMA: &str = "semaprax.project-c-build.v1";
const DOMAIN: &[u8] = b"semaprax.project-c-build.payload.v1\0";
const NATIVE_PATH: &str = "native/entry.c";

impl ProjectRevision {
    /// Return a canonical pathless carrier containing actual native C11 and
    /// per-source C header envelopes/headers. Exclusions use the unchanged
    /// CHeader profile; excluded exports never acquire invented prototypes.
    /// All canonical Project sources are independently rebuilt and the exact
    /// carrier regenerated before return. The 1 KiB–16 MiB output bound does
    /// not grant compiler/process, filesystem, publication or FFI authority.
    pub fn build_c_inline(&self, max_bytes: usize) -> Result<String, Vec<Diagnostic>> {
        if !(1024..=MAX_IMAGE_ARTIFACT_BUILD_BYTES).contains(&max_bytes) {
            return Err(error(
                "SPX-G291",
                "C carrier limit is outside the host bound",
            ));
        }
        let first = generate(self, max_bytes)?;
        let manifest = ProjectManifest::parse(&self.manifest().to_canonical_toml())?;
        let sources = self
            .sources()
            .iter()
            .map(|source| SemanticWorkspaceSource {
                path: source.path().to_owned(),
                source: source.source().to_owned(),
            })
            .collect();
        let built = super::super::build::build_owned(&manifest, sources)?;
        let replay = ProjectRevision::from_built(manifest, built);
        if replay.project_revision() != self.project_revision()
            || replay.workspace_revision() != self.workspace_revision()
            || replay.semantic_graph() != self.semantic_graph()
            || replay.sources().len() != self.sources().len()
            || replay
                .sources()
                .iter()
                .zip(self.sources())
                .any(|(left, right)| left.path() != right.path() || left.source() != right.source())
        {
            return Err(error(
                "SPX-G292",
                "C Project source rebuild differs from the retained subject",
            ));
        }
        if generate(&replay, max_bytes)? != first {
            return Err(error(
                "SPX-G293",
                "C carrier differs from exact full-source regeneration",
            ));
        }
        Ok(first)
    }
}

pub(super) fn projection_build(
    revision: &ProjectRevision,
    max_bytes: usize,
) -> Result<(String, String, usize), Vec<Diagnostic>> {
    let envelope = revision.build_c_inline(max_bytes)?;
    let value: Value = serde_json::from_str(&envelope)
        .map_err(|_| error("SPX-G292", "C carrier is invalid JSON"))?;
    let digest = value["payload_digest"]
        .as_str()
        .ok_or_else(|| error("SPX-G292", "C carrier payload digest is absent"))?
        .to_owned();
    let bytes = value["artifact_bytes"]
        .as_u64()
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| error("SPX-G292", "C artifact byte count is absent"))?;
    Ok((envelope, digest, bytes))
}

fn generate(revision: &ProjectRevision, max_bytes: usize) -> Result<String, Vec<Diagnostic>> {
    let (native, overflowed) = crate::bounded_output::with_limit(max_bytes, || {
        crate::codegen::emit_hir_c(revision.entry_program())
    });
    if overflowed {
        return Err(error(
            "SPX-G291",
            "Native C projection exceeds its bounded emission work",
        ));
    }
    let native = native.map_err(|diagnostic| vec![diagnostic])?;
    let mut total = 0usize;
    let mut artifacts = vec![artifact(
        NATIVE_PATH,
        &native,
        "native_c11",
        None,
        &mut total,
        max_bytes,
    )?];
    artifacts[0]["function_ids"] = json!(revision
        .entry_program()
        .functions
        .iter()
        .map(|function| function.id.as_str())
        .collect::<Vec<_>>());
    artifacts[0]["scope"] = json!("complete_linked_entry_with_manifest_export_roots");
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for id in revision.manifest().web_exports() {
        let source = revision
            .semantic
            .image_symbol(id)
            .ok_or_else(|| error("SPX-G292", "C export has no authenticated source"))?;
        let path = source["path"]
            .as_str()
            .ok_or_else(|| error("SPX-G292", "C export source path is absent"))?;
        groups.entry(path.to_owned()).or_default().push(id.clone());
    }
    if groups.is_empty() || groups.len() > 16 || revision.manifest().web_exports().len() > 32 {
        return Err(error(
            "SPX-G291",
            "C source or export inventory exceeds its bound",
        ));
    }
    let mut exports = BTreeMap::new();
    for (path, selected) in groups {
        let (envelope, header) =
            crate::c_header::project_source_header(revision, &path, &selected, &native, max_bytes)?;
        let parsed: Value = serde_json::from_str(&envelope)
            .map_err(|_| error("SPX-G292", "C header envelope is invalid JSON"))?;
        let payload = &parsed["payload"];
        let functions = payload["functions"]
            .as_array()
            .ok_or_else(|| error("SPX-G292", "C header function inventory is absent"))?;
        let exclusions = payload["exclusions"]
            .as_array()
            .ok_or_else(|| error("SPX-G292", "C header exclusion inventory is absent"))?;
        let header_path = format!("c-header/{path}.h");
        let envelope_path = format!("c-header/{path}.json");
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .ok_or_else(|| error("SPX-G292", "C header source is absent"))?;
        for id in selected {
            if !revision
                .entry_program()
                .functions
                .iter()
                .any(|function| function.id.as_str() == id)
            {
                return Err(error(
                    "SPX-G292",
                    "C export is absent from actual linked native input",
                ));
            }
            let emitted = functions
                .iter()
                .find(|function| function["stable_id"].as_str() == Some(id.as_str()));
            let excluded = exclusions
                .iter()
                .find(|function| function["stable_id"].as_str() == Some(id.as_str()));
            let declaration = revision
                .semantic
                .image_symbol(&id)
                .ok_or_else(|| error("SPX-G292", "C export declaration is absent"))?;
            let mut row = json!({"id":id,"source":declaration,"native_artifact_path":NATIVE_PATH,"native_relation":"member_of_whole_linked_source_not_public_linkage","header_artifact_path":null,"header_envelope_path":envelope_path,"symbol":null,"signature":null,"reason":null});
            match (emitted, excluded) {
                (Some(function), None) => {
                    if function["matches_native"] != true {
                        return Err(error(
                            "SPX-G292",
                            "C header prototype lacks actual native correspondence",
                        ));
                    }
                    row["admission"] = json!("admitted");
                    row["header_artifact_path"] = json!(header_path);
                    row["symbol"] = function["symbol"].clone();
                    row["signature"] = function["signature"].clone();
                    row["declaration_digest"] = function["declaration_sha256"].clone();
                }
                (None, Some(exclusion)) => {
                    row["admission"] = json!("excluded");
                    row["reason"] = exclusion["reason"].clone();
                }
                _ => {
                    return Err(error(
                        "SPX-G292",
                        "C header export mapping is missing or ambiguous",
                    ))
                }
            }
            if exports.insert(id, row).is_some() {
                return Err(error("SPX-G292", "C export mapping is duplicated"));
            }
        }
        for (artifact_path, text, role) in [
            (envelope_path.as_str(), envelope.as_str(), "header_envelope"),
            (header_path.as_str(), header.as_str(), "header"),
        ] {
            let mut row = artifact(
                artifact_path,
                text,
                role,
                Some(&path),
                &mut total,
                max_bytes,
            )?;
            row["source_revision"] = json!(source.source_revision());
            row["source_digest"] = json!(source.source_digest());
            row["header_digest"] = payload["header_sha256"].clone();
            artifacts.push(row);
        }
    }
    artifacts.sort_by(|left, right| left["path"].as_str().cmp(&right["path"].as_str()));
    let manifest = revision.manifest().to_canonical_toml();
    let sources=revision.sources().iter().map(|source|json!({"path":source.path(),"source_revision":source.source_revision(),"source_digest":source.source_digest()})).collect::<Vec<_>>();
    let mut value = json!({"schema":SCHEMA,"project_revision":revision.project_revision(),"workspace_revision":revision.workspace_revision(),"project_graph_digest":revision.semantic_graph_digest(),"manifest":{"source":manifest,"sha256":sha(manifest.as_bytes())},"sources":sources,"artifacts":artifacts,"exports":exports.into_values().collect::<Vec<_>>(),"artifact_bytes":total,"max_bytes":max_bytes,"evidence_owner":"full_project_source_rebuild_native_c11_emitter_and_c_header_renderer","source_authority":false,"artifact_materialization":false,"target_execution":false,"nonclaims":["source_inspection_not_standalone_ffi_or_shared_library","static_linkage_and_status_abi_unchanged","excluded_exports_have_no_header_prototype","no_c_compilation_linking_or_runtime_execution","no_filesystem_or_publication_authority"]});
    let payload = super::super::image::render(value.clone(), false, max_bytes)
        .map_err(|_| error("SPX-G291", "C carrier exceeds its output limit"))?;
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update((payload.len() as u64).to_le_bytes());
    digest.update(payload.as_bytes());
    value["payload_digest"] = json!(format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    ));
    super::super::image::render(value, true, max_bytes)
        .map_err(|_| error("SPX-G291", "C carrier exceeds its output limit"))
}

fn artifact(
    path: &str,
    text: &str,
    role: &str,
    source: Option<&str>,
    total: &mut usize,
    max_bytes: usize,
) -> Result<Value, Vec<Diagnostic>> {
    *total = total
        .checked_add(text.len())
        .ok_or_else(|| error("SPX-G291", "C artifact byte count overflow"))?;
    if total
        .checked_mul(2)
        .is_none_or(|encoded| encoded > max_bytes)
    {
        return Err(error(
            "SPX-G291",
            "C artifact encoding exceeds the carrier limit",
        ));
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut hex = String::with_capacity(text.len() * 2);
    for byte in text.bytes() {
        hex.push(char::from(HEX[usize::from(byte >> 4)]));
        hex.push(char::from(HEX[usize::from(byte & 15)]));
    }
    Ok(
        json!({"path":path,"role":role,"source_path":source,"bytes":text.len(),"sha256":sha(text.as_bytes()),"hex":hex}),
    )
}
fn sha(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
