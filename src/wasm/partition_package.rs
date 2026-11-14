//! Fresh-directory publication of the checked Core Wasm partition runner.
use super::shared_owner::{partition::PartitionModule, HOST_SOURCE, REGISTRY_SOURCE};
use crate::{ast::Program, diagnostic::Diagnostic, hir::byte_partition::ValidatedPartitionProgram};
use sha2::{Digest, Sha256};
use std::path::Path;

/// The generated runner embeds its exact checked module. The fixed host and
/// registry are shipped alongside it and callers cannot replace them through
/// this API.
pub struct PartitionPackage {
    entry: String,
    manifest: String,
}

impl PartitionPackage {
    /// Publish all paired files together to a fresh directory.
    pub fn publish(&self, output: &Path) -> Result<(), Diagnostic> {
        super::publish_scalar_package(
            output,
            &[
                ("byte-partition-v1.json", self.manifest.as_bytes()),
                ("entry.mjs", self.entry.as_bytes()),
                ("host.mjs", HOST_SOURCE.as_bytes()),
                ("registry.mjs", REGISTRY_SOURCE.as_bytes()),
            ],
        )
    }
}

pub(crate) fn emit(source: &Program, work: &mut usize) -> Result<PartitionPackage, Diagnostic> {
    let result = (|| {
        let owner = ValidatedPartitionProgram::compile_source(source, work)?;
        let module = PartitionModule::emit(&owner, work)?;
        let entry = module.entry_source();
        let digest =
            |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
        let manifest = serde_json::json!({
            "schema": "semaprax.byte-partition-package.v1",
            "profile": crate::interpreter::partition_source::PROFILE,
            "source_revision": crate::graph::revision(source),
            "target": "wasm",
            "entry": "entry.mjs",
            "module_sha256": digest(module.bytes()),
            "files_sha256": {
                "entry.mjs": digest(entry.as_bytes()),
                "host.mjs": digest(HOST_SOURCE.as_bytes()),
                "registry.mjs": digest(REGISTRY_SOURCE.as_bytes()),
            },
        })
        .to_string();
        Ok(PartitionPackage { entry, manifest })
    })();
    if result.is_err() {
        *work = 0;
    }
    result
}
