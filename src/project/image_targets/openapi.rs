//! Source-canonical, pathless Project OpenAPI artifacts. Complete Project
//! rebuilding precedes exact repeated generation; neither pass executes code.

use super::*;
use crate::project::{ProjectManifest, ProjectRevision};
use crate::semantic_workspace::SemanticWorkspaceSource;
use std::collections::BTreeMap;

const SCHEMA: &str = "semaprax.project-openapi-build.v1";
const DOMAIN: &[u8] = b"semaprax.project-openapi-build.payload.v1\0";

impl ProjectRevision {
    /// Return actual OpenAPI v1 envelopes for manifest-selected exports, in a
    /// canonical pathless Project carrier. The entire canonical Project is
    /// independently rebuilt and generation repeated before bytes return.
    /// `max_bytes` bounds the carrier (1 KiB–16 MiB); existing Project/source
    /// bounds and OpenAPI scalar admission also apply. No code is executed and
    /// no file, server, package or publication authority is acquired.
    pub fn build_openapi_inline(&self, max_bytes: usize) -> Result<String, Vec<Diagnostic>> {
        if !(1024..=MAX_IMAGE_ARTIFACT_BUILD_BYTES).contains(&max_bytes) {
            return Err(error(
                "SPX-G291",
                "OpenAPI carrier limit is outside the host bound",
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
                "OpenAPI Project source rebuild differs from the retained subject",
            ));
        }
        if generate(&replay, max_bytes)? != first {
            return Err(error(
                "SPX-G293",
                "OpenAPI carrier differs from exact full-source regeneration",
            ));
        }
        Ok(first)
    }
}

pub(super) fn projection_build(
    revision: &ProjectRevision,
    max_bytes: usize,
) -> Result<(String, String, usize), Vec<Diagnostic>> {
    let envelope = revision.build_openapi_inline(max_bytes)?;
    let carrier: Value = serde_json::from_str(&envelope)
        .map_err(|_| error("SPX-G292", "OpenAPI carrier is invalid JSON"))?;
    let digest = carrier["payload_digest"]
        .as_str()
        .ok_or_else(|| error("SPX-G292", "OpenAPI payload digest is absent"))?
        .to_owned();
    let bytes = carrier["artifact_bytes"]
        .as_u64()
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| error("SPX-G292", "OpenAPI artifact byte count is absent"))?;
    Ok((envelope, digest, bytes))
}

fn generate(revision: &ProjectRevision, max_bytes: usize) -> Result<String, Vec<Diagnostic>> {
    let options = crate::openapi::OpenApiOptions::new(max_bytes).map_err(|error| vec![error])?;
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for id in revision.manifest().web_exports() {
        let declaration = revision.semantic.image_symbol(id).ok_or_else(|| {
            error(
                "SPX-G292",
                "OpenAPI export has no authenticated source declaration",
            )
        })?;
        let path = declaration["path"]
            .as_str()
            .ok_or_else(|| error("SPX-G292", "OpenAPI export source path is absent"))?;
        groups.entry(path.to_owned()).or_default().push(id.clone());
    }
    if groups.is_empty()
        || groups.len() > 16
        || revision.manifest().web_exports().len() > crate::openapi::MAX_FUNCTIONS
    {
        return Err(error(
            "SPX-G291",
            "OpenAPI source or export inventory exceeds the admitted bound",
        ));
    }
    let mut artifacts = Vec::new();
    let mut exports = BTreeMap::new();
    let mut artifact_bytes = 0usize;
    for (path, selected) in groups {
        let text = crate::openapi::project_source_envelope(revision, &path, &selected, &options)?;
        artifact_bytes = artifact_bytes
            .checked_add(text.len())
            .ok_or_else(|| error("SPX-G291", "OpenAPI artifact accounting overflow"))?;
        if artifact_bytes > max_bytes {
            return Err(error(
                "SPX-G291",
                "OpenAPI artifact bytes exceed the carrier limit",
            ));
        }
        if artifact_bytes
            .checked_mul(2)
            .is_none_or(|encoded| encoded > max_bytes)
        {
            return Err(error(
                "SPX-G291",
                "OpenAPI aggregate artifact encoding exceeds the carrier limit",
            ));
        }
        let envelope: Value = serde_json::from_str(&text)
            .map_err(|_| error("SPX-G292", "OpenAPI source envelope is invalid JSON"))?;
        let document = &envelope["document"];
        let artifact_path = format!("openapi/{path}.json");
        for id in &selected {
            let operation_path = format!("/{id}");
            let operation = &document["paths"][&operation_path]["post"];
            if operation["x-stable-id"].as_str() != Some(id.as_str()) {
                return Err(error(
                    "SPX-G292",
                    "OpenAPI operation does not bind its selected export",
                ));
            }
            let operation_id = operation["operationId"]
                .as_str()
                .ok_or_else(|| error("SPX-G292", "OpenAPI operation identity is absent"))?;
            let source = revision
                .semantic
                .image_symbol(id)
                .ok_or_else(|| error("SPX-G292", "OpenAPI export source is absent"))?;
            if exports.insert(id.clone(),json!({"id":id,"artifact_path":artifact_path,"operation_path":operation_path,"operation_id":operation_id,"source":source})).is_some() {
                return Err(error("SPX-G292","OpenAPI export mapping is duplicated"));
            }
        }
        let source = revision
            .sources()
            .iter()
            .find(|source| source.path() == path)
            .ok_or_else(|| error("SPX-G292", "OpenAPI source is absent"))?;
        let hex_bytes = text
            .len()
            .checked_mul(2)
            .ok_or_else(|| error("SPX-G291", "OpenAPI artifact encoding overflow"))?;
        if hex_bytes > max_bytes {
            return Err(error(
                "SPX-G291",
                "OpenAPI artifact encoding exceeds the carrier limit",
            ));
        }
        let mut hex = String::with_capacity(hex_bytes);
        const HEX: &[u8; 16] = b"0123456789abcdef";
        for byte in text.bytes() {
            hex.push(char::from(HEX[usize::from(byte >> 4)]));
            hex.push(char::from(HEX[usize::from(byte & 15)]));
        }
        artifacts.push(json!({"path":artifact_path,"source_path":path,"source_revision":source.source_revision(),"source_digest":source.source_digest(),"schema":crate::openapi::SCHEMA,"document_digest":envelope["sha256"],"bytes":text.len(),"sha256":sha(text.as_bytes()),"hex":hex}));
    }
    let manifest = revision.manifest().to_canonical_toml();
    let sources=revision.sources().iter().map(|source|json!({"path":source.path(),"source_revision":source.source_revision(),"source_digest":source.source_digest()})).collect::<Vec<_>>();
    let mut carrier = json!({"schema":SCHEMA,"project_revision":revision.project_revision(),"workspace_revision":revision.workspace_revision(),"project_graph_digest":revision.semantic_graph_digest(),"manifest":{"source":manifest,"sha256":sha(manifest.as_bytes())},"sources":sources,"artifacts":artifacts,"exports":exports.into_values().collect::<Vec<_>>(),"artifact_bytes":artifact_bytes,"max_bytes":max_bytes,"evidence_owner":"full_project_source_rebuild_and_existing_openapi_renderer","source_authority":false,"artifact_materialization":false,"target_execution":false,"nonclaims":["not_http_server_or_runtime_route","no_schema_import_or_external_compatibility_proof","no_native_wasm_or_application_execution","no_filesystem_or_publication_authority"]});
    let payload = super::super::image::render(carrier.clone(), false, max_bytes)
        .map_err(|_| error("SPX-G291", "OpenAPI carrier exceeds the output limit"))?;
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update((payload.len() as u64).to_le_bytes());
    digest.update(payload.as_bytes());
    carrier["payload_digest"] = json!(format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    ));
    super::super::image::render(carrier, true, max_bytes)
        .map_err(|_| error("SPX-G291", "OpenAPI carrier exceeds the output limit"))
}

fn sha(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
