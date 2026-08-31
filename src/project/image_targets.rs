//! Source-bound target and pathless package facts. No artifact publication.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::ProjectSemanticImage;
use crate::diagnostic::Diagnostic;
mod c;
mod openapi;

pub const IMAGE_TARGET_ADMISSION_SCHEMA: &str = "semaprax.image-target-admission.v1";
pub const IMAGE_ARTIFACT_PROJECTION_SCHEMA: &str = "semaprax.image-artifact-projection.v1";
pub const MAX_IMAGE_ARTIFACT_BUILD_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_IMAGE_ARTIFACT_REPORT_BYTES: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImageArtifactKind {
    Web,
    Npm,
    OpenApi,
    C,
}
impl ImageArtifactKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Web => "web",
            Self::Npm => "npm",
            Self::OpenApi => "openapi",
            Self::C => "c",
        }
    }
}

impl ProjectSemanticImage {
    /// Report actual compiler projection for whole linked entry/test closures.
    /// Membership is derived from retained HIR, never inferred from source text.
    /// A closure failure is not attributed to the selected function alone.
    pub fn target_admission(
        &self,
        expected: &str,
        target: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        if target.is_empty() || target.len() > 4096 || target.contains('\0') {
            return Err(error(
                "SPX-G290",
                "target admission requires a bounded function identity",
            ));
        }
        let symbol = self
            .revision()
            .semantic
            .image_symbol(target)
            .ok_or_else(|| error("SPX-G290", "target declaration is unavailable"))?;
        let function_present = self
            .revision()
            .semantic
            .image_modules()
            .iter()
            .any(|module| {
                module
                    .functions()
                    .iter()
                    .any(|function| function.id.as_str() == target)
            });
        if !function_present {
            return Err(error(
                "SPX-G290",
                "target admission selects an authored retained function",
            ));
        }
        let mut projections = super::candidate::target_projection_facts(self.revision())?;
        for projection in projections
            .as_array_mut()
            .expect("compiler target projection array")
        {
            let program = if projection["role"] == "entry" {
                self.revision().entry_program()
            } else {
                self.revision().test_program()
            };
            projection["selected_function_in_closure"] = json!(program
                .functions
                .iter()
                .any(|function| function.id.as_str() == target));
            projection["scope"] = json!("complete_linked_role_closure");
            projection["reason"] = json!("compiler_emission_for_exact_retained_role_program");
        }
        super::image::render(
            json!({
                "schema": IMAGE_TARGET_ADMISSION_SCHEMA, "image_revision": self.image_digest(),
                "project_revision": self.revision().project_revision(), "target": target,
                "source": symbol, "projections": projections,
                "evidence_owner": "compiler_native_and_wasm_emitters",
                "evidence_class": "descriptive_checked_target_projection",
                "source_authority": false, "target_execution": false,
                "nonclaims": ["no_standalone_function_admission_proof", "no_failure_attribution_to_selected_function", "no_native_compilation_or_runtime_execution", "no_package_publication_or_external_consumers"],
            }),
            false,
            MAX_IMAGE_ARTIFACT_REPORT_BYTES,
        )
    }

    /// Build and independently replay a pathless Web/npm, OpenAPI or C carrier,
    /// then return compact file bindings and source-owned export relationships.
    /// No compiler executable, package manager or filesystem publisher runs.
    pub fn artifact_projection(
        &self,
        expected: &str,
        kind: ImageArtifactKind,
        max_bytes: usize,
    ) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected)?;
        if !(1024..=MAX_IMAGE_ARTIFACT_BUILD_BYTES).contains(&max_bytes) {
            return Err(error(
                "SPX-G291",
                "artifact projection build limit is outside the host bound",
            ));
        }
        let (envelope, payload_digest, artifact_bytes) = match kind {
            ImageArtifactKind::Web => {
                let build = self.revision().build_web_inline(max_bytes)?;
                build.verify().map_err(|diagnostic| vec![diagnostic])?;
                (
                    build.envelope().to_owned(),
                    build.payload_digest().to_owned(),
                    build.artifact_bytes(),
                )
            }
            ImageArtifactKind::Npm => {
                let build = self.revision().build_npm_inline(max_bytes)?;
                build.verify().map_err(|diagnostic| vec![diagnostic])?;
                (
                    build.envelope().to_owned(),
                    build.payload_digest().to_owned(),
                    build.artifact_bytes(),
                )
            }
            ImageArtifactKind::OpenApi => openapi::projection_build(self.revision(), max_bytes)?,
            ImageArtifactKind::C => c::projection_build(self.revision(), max_bytes)?,
        };
        if envelope.len() > max_bytes {
            return Err(error(
                "SPX-G291",
                "artifact envelope exceeds its fixed build limit",
            ));
        }
        let carrier: Value = serde_json::from_str(&envelope)
            .map_err(|_| error("SPX-G292", "compiler artifact carrier is invalid JSON"))?;
        let payload = &carrier;
        if payload["project_revision"] != self.revision().project_revision()
            || payload["project_graph_digest"] != self.revision().semantic_graph_digest()
            || payload["workspace_revision"] != self.revision().workspace_revision()
        {
            return Err(error(
                "SPX-G292",
                "artifact carrier does not bind the selected image",
            ));
        }
        let rows = payload["artifacts"]
            .as_array()
            .ok_or_else(|| error("SPX-G292", "artifact carrier inventory is absent"))?;
        if rows.len() > 64 {
            return Err(error("SPX-G291", "artifact inventory exceeds its bound"));
        }
        let mut files = Vec::new();
        for row in rows {
            let hex_field = if kind == ImageArtifactKind::Web {
                "content_hex"
            } else {
                "hex"
            };
            let hex = row[hex_field]
                .as_str()
                .ok_or_else(|| error("SPX-G292", "artifact encoding is absent"))?;
            // Complete carrier replay above owns hexadecimal grammar, hashes,
            // exact file inventory and generated content/recipe correspondence.
            files.push(json!({"path": row["path"], "bytes": hex.len() / 2, "sha256": row["sha256"],
                "edge_kind": "emitted_artifact", "reason": "member_of_independently_replayed_carrier",
                "evidence_owner": "existing_project_carrier_replay"}));
            if kind == ImageArtifactKind::OpenApi {
                let file = files.last_mut().expect("just appended artifact");
                file["source_path"] = row["source_path"].clone();
                file["document_digest"] = row["document_digest"].clone();
                file["evidence_owner"] =
                    json!("full_project_source_rebuild_and_existing_openapi_renderer");
            }
            if kind == ImageArtifactKind::C {
                let file = files.last_mut().expect("just appended artifact");
                for key in [
                    "role",
                    "source_path",
                    "header_digest",
                    "function_ids",
                    "scope",
                ] {
                    if !row[key].is_null() {
                        file[key] = row[key].clone();
                    }
                }
                file["evidence_owner"] =
                    json!("full_project_source_rebuild_native_c11_emitter_and_c_header_renderer");
            }
        }
        let mut exports = Vec::new();
        for id in self.revision().manifest().web_exports() {
            let source = self.revision().semantic.image_symbol(id).ok_or_else(|| {
                error(
                    "SPX-G292",
                    "artifact export has no retained source declaration",
                )
            })?;
            exports.push(json!({"id":id, "source":source, "edge_kind":"public_export_selected_by_manifest",
                "reason":"exact_manifest_export_passed_to_compiler_carrier_builder",
                "evidence_owner":"project_manifest_and_context_bound_carrier", "classification":"descriptive"}));
            if kind == ImageArtifactKind::OpenApi {
                let mapping = payload["exports"]
                    .as_array()
                    .and_then(|exports| {
                        exports
                            .iter()
                            .find(|item| item["id"].as_str() == Some(id.as_str()))
                    })
                    .ok_or_else(|| {
                        error("SPX-G292", "OpenAPI export artifact mapping is absent")
                    })?;
                let export = exports.last_mut().expect("just appended export");
                for key in ["artifact_path", "operation_path", "operation_id"] {
                    export[key] = mapping[key].clone();
                }
                export["evidence_owner"] = json!("manifest_export_and_actual_openapi_operation");
            }
            if kind == ImageArtifactKind::C {
                let mapping = payload["exports"]
                    .as_array()
                    .and_then(|exports| {
                        exports
                            .iter()
                            .find(|item| item["id"].as_str() == Some(id.as_str()))
                    })
                    .ok_or_else(|| error("SPX-G292", "C export artifact mapping is absent"))?;
                let export = exports.last_mut().expect("just appended export");
                for key in [
                    "admission",
                    "native_artifact_path",
                    "native_relation",
                    "header_artifact_path",
                    "header_envelope_path",
                    "symbol",
                    "signature",
                    "declaration_digest",
                    "reason",
                ] {
                    export[key] = mapping[key].clone();
                }
                export["evidence_owner"] =
                    json!("manifest_export_and_actual_native_prototype_or_header_exclusion");
            }
        }
        let sources = self.revision().sources().iter().map(|source| json!({
            "path": source.path(), "source_revision": source.source_revision(), "source_digest": source.source_digest(),
            "relation":"authenticated_project_input_not_runtime_coverage",
        })).collect::<Vec<_>>();
        let envelope_digest = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(envelope.as_bytes()))
        );
        let nonclaims = if kind == ImageArtifactKind::C {
            vec![
                "no_rust_package_projection",
                "source_inspection_not_standalone_ffi_or_shared_library",
                "static_linkage_and_status_abi_unchanged",
                "excluded_exports_have_no_header_prototype",
                "no_c_compilation_linking_or_runtime_execution",
                "no_package_installation_or_external_consumer_evidence",
                "no_runtime_or_test_coverage",
                "no_filesystem_artifact_publication",
                "no_publication_authority",
            ]
        } else if kind == ImageArtifactKind::OpenApi {
            vec![
                "no_rust_c_package_projection",
                "not_http_server_or_runtime_route",
                "no_package_installation_or_external_consumer_evidence",
                "no_runtime_or_test_coverage",
                "no_filesystem_artifact_publication",
                "no_publication_authority",
            ]
        } else {
            vec![
                "no_rust_c_or_openapi_package_projection",
                "no_package_installation_or_external_consumer_evidence",
                "no_runtime_or_test_coverage",
                "no_filesystem_artifact_publication",
                "no_publication_authority",
            ]
        };
        super::image::render(
            json!({
                "schema":IMAGE_ARTIFACT_PROJECTION_SCHEMA, "image_revision":self.image_digest(),
                "project_revision":self.revision().project_revision(), "project_graph_digest":self.revision().semantic_graph_digest(),
                "kind":kind.name(), "carrier_schema":payload["schema"], "carrier_payload_digest":payload_digest,
                "carrier_envelope_sha256":envelope_digest, "carrier_envelope_bytes":envelope.len(), "artifact_bytes":artifact_bytes,
                "max_build_bytes":max_bytes, "artifacts":files, "exports":exports, "sources":sources,
                "evidence_class":"independently_replayed_pathless_compiler_artifacts", "source_authority":false,
                "artifact_materialization":false, "target_execution":false,
                "nonclaims":nonclaims,
            }),
            false,
            MAX_IMAGE_ARTIFACT_REPORT_BYTES,
        )
    }

    pub fn verify_artifact_projection(
        &self,
        expected: &str,
        kind: ImageArtifactKind,
        max_bytes: usize,
        report: &[u8],
    ) -> Result<(), Vec<Diagnostic>> {
        self.require_digest(expected)?;
        if report.len() > MAX_IMAGE_ARTIFACT_REPORT_BYTES {
            return Err(error("SPX-G291", "artifact report exceeds its byte bound"));
        }
        if self
            .artifact_projection(expected, kind, max_bytes)?
            .as_bytes()
            != report
        {
            return Err(error(
                "SPX-G293",
                "artifact report differs from exact source and carrier replay",
            ));
        }
        Ok(())
    }
}
fn error(code: &'static str, message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(code, message)]
}
