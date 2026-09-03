//! Actual native C11 text and existing header projections, kept as source
//! inspection artifacts. No C compilation, linkage changes or FFI authority.

use super::*;
use crate::project::{ProjectManifest, ProjectRevision};
use crate::semantic_workspace::SemanticWorkspaceSource;
use std::collections::{BTreeMap, BTreeSet};

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

/// Facts recovered by independently replaying one retained pathless carrier.
/// This parser grants no trust to transported HIR or permission to materialize
/// the decoded artifacts.
pub(in crate::project) struct ReplayedCarrier {
    pub(in crate::project) payload_digest: String,
    pub(in crate::project) artifact_bytes: usize,
}

/// Rebind a retained compiler-produced C carrier to one exact admitted
/// revision without invoking either target emitter. The caller-owned target
/// cache is the only current consumer; untrusted transport is intentionally
/// outside this seam.
pub(in crate::project) fn replay_carrier(
    revision: &ProjectRevision,
    envelope: &str,
    max_bytes: usize,
) -> Result<ReplayedCarrier, Vec<Diagnostic>> {
    if !(1024..=MAX_IMAGE_ARTIFACT_BUILD_BYTES).contains(&max_bytes) || envelope.len() > max_bytes {
        return Err(error(
            "SPX-G292",
            "retained C carrier exceeds its exact bound",
        ));
    }
    let value: Value = serde_json::from_str(envelope)
        .map_err(|_| error("SPX-G292", "retained C carrier is invalid JSON"))?;
    require_keys(
        &value,
        &[
            "artifact_bytes",
            "artifact_materialization",
            "artifacts",
            "evidence_owner",
            "exports",
            "manifest",
            "max_bytes",
            "nonclaims",
            "payload_digest",
            "project_graph_digest",
            "project_revision",
            "schema",
            "source_authority",
            "sources",
            "target_execution",
            "workspace_revision",
        ],
        "retained C carrier has an open top-level shape",
    )?;
    if value["schema"] != SCHEMA
        || value["project_revision"] != revision.project_revision()
        || value["workspace_revision"] != revision.workspace_revision()
        || value["project_graph_digest"] != revision.semantic_graph_digest()
        || value["max_bytes"].as_u64() != u64::try_from(max_bytes).ok()
        || value["evidence_owner"]
            != "full_project_source_rebuild_native_c11_emitter_and_c_header_renderer"
        || value["source_authority"] != false
        || value["artifact_materialization"] != false
        || value["target_execution"] != false
        || value["nonclaims"]
            != json!([
                "source_inspection_not_standalone_ffi_or_shared_library",
                "static_linkage_and_status_abi_unchanged",
                "excluded_exports_have_no_header_prototype",
                "no_c_compilation_linking_or_runtime_execution",
                "no_filesystem_or_publication_authority"
            ])
    {
        return Err(error(
            "SPX-G292",
            "retained C carrier does not bind the exact admitted subject",
        ));
    }
    let canonical_manifest = revision.manifest().to_canonical_toml();
    require_keys(
        &value["manifest"],
        &["sha256", "source"],
        "retained C manifest binding has an open shape",
    )?;
    if value["manifest"]["source"] != canonical_manifest
        || value["manifest"]["sha256"] != sha(canonical_manifest.as_bytes())
    {
        return Err(error(
            "SPX-G292",
            "retained C manifest binding disagrees with the admitted revision",
        ));
    }
    let expected_sources = revision
        .sources()
        .iter()
        .map(|source| {
            json!({"path":source.path(),"source_revision":source.source_revision(),"source_digest":source.source_digest()})
        })
        .collect::<Vec<_>>();
    let sources = value["sources"]
        .as_array()
        .ok_or_else(|| error("SPX-G292", "retained C source inventory is absent"))?;
    for source in sources {
        require_keys(
            source,
            &["path", "source_digest", "source_revision"],
            "retained C source row has an open shape",
        )?;
    }
    if *sources != expected_sources {
        return Err(error(
            "SPX-G292",
            "retained C source inventory disagrees with the admitted revision",
        ));
    }
    let payload_digest = value["payload_digest"]
        .as_str()
        .ok_or_else(|| error("SPX-G292", "retained C payload digest is absent"))?
        .to_owned();
    let mut unsigned = value.clone();
    unsigned
        .as_object_mut()
        .expect("top-level object checked")
        .remove("payload_digest");
    let payload = super::super::image::render(unsigned, false, max_bytes)
        .map_err(|_| error("SPX-G292", "retained C payload exceeds its exact bound"))?;
    let mut digest = Sha256::new();
    digest.update(DOMAIN);
    digest.update((payload.len() as u64).to_le_bytes());
    digest.update(payload.as_bytes());
    let expected_digest = format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    );
    if payload_digest != expected_digest
        || super::super::image::render(value.clone(), true, max_bytes)
            .map_err(|_| error("SPX-G292", "retained C carrier exceeds its exact bound"))?
            != envelope
    {
        return Err(error(
            "SPX-G292",
            "retained C carrier is not its canonical digest-bound encoding",
        ));
    }
    let artifacts = value["artifacts"]
        .as_array()
        .ok_or_else(|| error("SPX-G292", "retained C artifact inventory is absent"))?;
    let mut expected = vec![(NATIVE_PATH.to_owned(), "native_c11", None::<String>)];
    let mut groups = BTreeMap::<String, Vec<String>>::new();
    for id in revision.manifest().web_exports() {
        let source = revision
            .semantic
            .image_symbol(id)
            .and_then(|row| row["path"].as_str().map(str::to_owned))
            .ok_or_else(|| error("SPX-G292", "retained C export source is absent"))?;
        groups.entry(source).or_default().push(id.clone());
    }
    for path in groups.keys() {
        expected.push((format!("c-header/{path}.h"), "header", Some(path.clone())));
        expected.push((
            format!("c-header/{path}.json"),
            "header_envelope",
            Some(path.clone()),
        ));
    }
    expected.sort_by(|left, right| left.0.cmp(&right.0));
    if artifacts.len() != expected.len() {
        return Err(error(
            "SPX-G292",
            "retained C artifact inventory has the wrong cardinality",
        ));
    }
    let mut decoded = BTreeMap::<String, Vec<u8>>::new();
    let mut total = 0usize;
    for (row, (path, role, source_path)) in artifacts.iter().zip(&expected) {
        let keys = if *role == "native_c11" {
            &[
                "bytes",
                "function_ids",
                "hex",
                "path",
                "role",
                "scope",
                "sha256",
                "source_path",
            ][..]
        } else {
            &[
                "bytes",
                "header_digest",
                "hex",
                "path",
                "role",
                "sha256",
                "source_digest",
                "source_path",
                "source_revision",
            ][..]
        };
        require_keys(row, keys, "retained C artifact row has an open shape")?;
        let source_matches = match source_path {
            Some(path) => row["source_path"].as_str() == Some(path.as_str()),
            None => row["source_path"].is_null(),
        };
        if row["path"].as_str() != Some(path.as_str())
            || row["role"].as_str() != Some(*role)
            || !source_matches
        {
            return Err(error(
                "SPX-G292",
                "retained C artifact identity or source binding disagrees",
            ));
        }
        if let Some(source_path) = source_path {
            let source = revision
                .sources()
                .iter()
                .find(|source| source.path() == source_path)
                .ok_or_else(|| error("SPX-G292", "retained C artifact source is absent"))?;
            if row["source_revision"] != source.source_revision()
                || row["source_digest"] != source.source_digest()
            {
                return Err(error(
                    "SPX-G292",
                    "retained C artifact source digest disagrees",
                ));
            }
        } else if row["function_ids"]
            != json!(revision
                .entry_program()
                .functions
                .iter()
                .map(|function| function.id.as_str())
                .collect::<Vec<_>>())
            || row["scope"] != "complete_linked_entry_with_manifest_export_roots"
        {
            return Err(error(
                "SPX-G292",
                "retained native C closure inventory disagrees",
            ));
        }
        let bytes = decode_artifact(row, max_bytes)?;
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| error("SPX-G292", "retained C artifact bytes overflow"))?;
        if decoded.insert(path.clone(), bytes).is_some() {
            return Err(error("SPX-G292", "retained C artifact path is duplicated"));
        }
    }
    let artifact_bytes = value["artifact_bytes"]
        .as_u64()
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| error("SPX-G292", "retained C artifact byte count is absent"))?;
    if total != artifact_bytes || total > max_bytes {
        return Err(error(
            "SPX-G292",
            "retained C cumulative artifact byte count disagrees",
        ));
    }
    let mut header_payloads = BTreeMap::<String, Value>::new();
    for path in groups.keys() {
        let envelope_path = format!("c-header/{path}.json");
        let header_path = format!("c-header/{path}.h");
        let envelope_text = std::str::from_utf8(&decoded[&envelope_path])
            .map_err(|_| error("SPX-G292", "retained C header envelope is not UTF-8"))?;
        let header = crate::c_header::verify_envelope(envelope_text)
            .map_err(|_| error("SPX-G292", "retained C header envelope replay failed"))?;
        if header.as_bytes() != decoded[&header_path] {
            return Err(error(
                "SPX-G292",
                "retained C header envelope and header artifact disagree",
            ));
        }
        let parsed: Value = serde_json::from_str(envelope_text)
            .map_err(|_| error("SPX-G292", "retained C header envelope is invalid JSON"))?;
        let payload = parsed["payload"].clone();
        for artifact_path in [&envelope_path, &header_path] {
            let row = artifacts
                .iter()
                .find(|row| row["path"] == *artifact_path)
                .expect("expected artifact row retained");
            if row["header_digest"] != payload["header_sha256"] {
                return Err(error(
                    "SPX-G292",
                    "retained C header digest binding disagrees",
                ));
            }
        }
        header_payloads.insert(path.clone(), payload);
    }
    verify_exports(revision, &value["exports"], &groups, &header_payloads)?;
    Ok(ReplayedCarrier {
        payload_digest,
        artifact_bytes,
    })
}

fn require_keys(
    value: &Value,
    keys: &[&str],
    message: &'static str,
) -> Result<(), Vec<Diagnostic>> {
    let Some(object) = value.as_object() else {
        return Err(error("SPX-G292", message));
    };
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = keys.iter().copied().collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(error("SPX-G292", message));
    }
    Ok(())
}

fn decode_artifact(row: &Value, max_bytes: usize) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let declared = row["bytes"]
        .as_u64()
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or_else(|| error("SPX-G292", "retained C artifact byte count is invalid"))?;
    let hex = row["hex"]
        .as_str()
        .ok_or_else(|| error("SPX-G292", "retained C artifact encoding is absent"))?;
    if declared > max_bytes || hex.len() != declared.saturating_mul(2) {
        return Err(error(
            "SPX-G292",
            "retained C artifact encoding length disagrees",
        ));
    }
    let mut bytes = Vec::with_capacity(declared);
    for pair in hex.as_bytes().chunks_exact(2) {
        let digit = |byte: u8| match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            _ => None,
        };
        let high = digit(pair[0])
            .ok_or_else(|| error("SPX-G292", "retained C artifact hex is not canonical"))?;
        let low = digit(pair[1])
            .ok_or_else(|| error("SPX-G292", "retained C artifact hex is not canonical"))?;
        bytes.push((high << 4) | low);
    }
    if row["sha256"] != sha(&bytes) {
        return Err(error(
            "SPX-G292",
            "retained C artifact digest disagrees with decoded bytes",
        ));
    }
    Ok(bytes)
}

fn verify_exports(
    revision: &ProjectRevision,
    value: &Value,
    groups: &BTreeMap<String, Vec<String>>,
    headers: &BTreeMap<String, Value>,
) -> Result<(), Vec<Diagnostic>> {
    let rows = value
        .as_array()
        .ok_or_else(|| error("SPX-G292", "retained C export inventory is absent"))?;
    let expected_ids = groups.values().flatten().cloned().collect::<BTreeSet<_>>();
    if rows.len() != expected_ids.len() {
        return Err(error(
            "SPX-G292",
            "retained C export inventory has the wrong cardinality",
        ));
    }
    let actual_order = rows
        .iter()
        .filter_map(|row| row["id"].as_str())
        .collect::<Vec<_>>();
    let expected_order = expected_ids.iter().map(String::as_str).collect::<Vec<_>>();
    if actual_order != expected_order {
        return Err(error(
            "SPX-G292",
            "retained C export inventory is not in canonical identity order",
        ));
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        let id = row["id"]
            .as_str()
            .ok_or_else(|| error("SPX-G292", "retained C export identity is absent"))?;
        if !expected_ids.contains(id) || !seen.insert(id) {
            return Err(error(
                "SPX-G292",
                "retained C export identity is unknown or duplicated",
            ));
        }
        let source = revision
            .semantic
            .image_symbol(id)
            .ok_or_else(|| error("SPX-G292", "retained C export source is absent"))?;
        let source_path = source["path"]
            .as_str()
            .ok_or_else(|| error("SPX-G292", "retained C export source path is absent"))?;
        let payload = &headers[source_path];
        let function = payload["functions"]
            .as_array()
            .and_then(|rows| rows.iter().find(|candidate| candidate["stable_id"] == id));
        let exclusion = payload["exclusions"]
            .as_array()
            .and_then(|rows| rows.iter().find(|candidate| candidate["stable_id"] == id));
        let common = row["source"] == source
            && row["native_artifact_path"] == NATIVE_PATH
            && row["native_relation"] == "member_of_whole_linked_source_not_public_linkage"
            && row["header_envelope_path"] == format!("c-header/{source_path}.json");
        let exact = match (function, exclusion) {
            (Some(function), None) => {
                require_keys(
                    row,
                    &[
                        "admission",
                        "declaration_digest",
                        "header_artifact_path",
                        "header_envelope_path",
                        "id",
                        "native_artifact_path",
                        "native_relation",
                        "reason",
                        "signature",
                        "source",
                        "symbol",
                    ],
                    "retained admitted C export has an open shape",
                )?;
                common
                    && function["matches_native"] == true
                    && row["admission"] == "admitted"
                    && row["header_artifact_path"] == format!("c-header/{source_path}.h")
                    && row["symbol"] == function["symbol"]
                    && row["signature"] == function["signature"]
                    && row["declaration_digest"] == function["declaration_sha256"]
                    && row["reason"].is_null()
            }
            (None, Some(exclusion)) => {
                require_keys(
                    row,
                    &[
                        "admission",
                        "header_artifact_path",
                        "header_envelope_path",
                        "id",
                        "native_artifact_path",
                        "native_relation",
                        "reason",
                        "signature",
                        "source",
                        "symbol",
                    ],
                    "retained excluded C export has an open shape",
                )?;
                common
                    && row["admission"] == "excluded"
                    && row["header_artifact_path"].is_null()
                    && row["symbol"].is_null()
                    && row["signature"].is_null()
                    && row["reason"] == exclusion["reason"]
            }
            _ => false,
        };
        if !exact {
            return Err(error(
                "SPX-G292",
                "retained C export mapping disagrees with its replayed header",
            ));
        }
    }
    Ok(())
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
