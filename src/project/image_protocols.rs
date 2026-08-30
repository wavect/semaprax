//! Source-backed static conformance over an already admitted Project revision.
//! This sidecar deliberately leaves the runtime Graph and Image v1 unchanged.

use serde_json::json;

use super::ProjectSemanticImage;
use crate::diagnostic::Diagnostic;

pub const IMAGE_PROTOCOL_CONFORMANCE_SCHEMA: &str = "semaprax.image-protocol-conformance.v1";
pub const MAX_IMAGE_PROTOCOL_CONFORMANCE_BYTES: usize = 8 * 1024 * 1024;

impl ProjectSemanticImage {
    /// Derive exact local protocol/member bindings from the canonical sources
    /// of this checked image. Imports cannot serve as local implementations.
    /// The retained Project revision has already checked function bodies and
    /// linked calls; the declaration sidecar itself proves signatures only.
    pub fn protocol_conformance(
        &self,
        expected_image_digest: &str,
    ) -> Result<String, Vec<Diagnostic>> {
        self.require_digest(expected_image_digest)?;
        let mut modules = Vec::new();
        for source in self.revision().sources() {
            let program =
                crate::parse(source.source(), source.path()).map_err(|error| vec![error])?;
            if program.protocols.is_empty() && program.implementations.is_empty() {
                continue;
            }
            let declarations = crate::static_protocol::declaration_facts(&program)?;
            modules.push(json!({
                "path": source.path(), "module": program.module,
                "source_revision": source.source_revision(),
                "source_digest": source.source_digest(),
                "declarations": declarations,
            }));
        }
        // Source order is already canonical in ProjectRevision. Each local
        // inventory is stable-ID sorted by the conformance producer.
        super::image::render(
            json!({
                "schema": IMAGE_PROTOCOL_CONFORMANCE_SCHEMA,
                "image_revision": self.image_digest(),
                "project_revision": self.revision().project_revision(),
                "project_graph_digest": self.revision().semantic_graph_digest(),
                "modules": modules,
                "evidence_class": "source_backed_static_signature_conformance",
                "project_source_admission": true,
                "source_authority": false,
                "nonclaims": [
                    "no_dynamic_dispatch_or_witness_runtime",
                    "no_protocol_nodes_in_runtime_graph_v1",
                    "no_cross_module_implementation_bindings",
                    "no_behavioral_contract_proof_or_target_execution",
                    "no_publication_authority",
                ],
            }),
            false,
            MAX_IMAGE_PROTOCOL_CONFORMANCE_BYTES,
        )
    }

    /// Reports are evidence only: replay compares their complete bytes against
    /// a fresh derivation from the selected checked image, never trusts them.
    pub fn verify_protocol_conformance(
        &self,
        expected_image_digest: &str,
        report: &[u8],
    ) -> Result<(), Vec<Diagnostic>> {
        self.require_digest(expected_image_digest)?;
        if report.len() > MAX_IMAGE_PROTOCOL_CONFORMANCE_BYTES {
            return Err(vec![Diagnostic::io(
                "SPX-G220",
                "protocol conformance report exceeds its byte bound",
            )]);
        }
        if self.protocol_conformance(expected_image_digest)?.as_bytes() != report {
            return Err(vec![Diagnostic::io(
                "SPX-G221",
                "protocol conformance report does not match exact image replay",
            )]);
        }
        Ok(())
    }
}
