//! Project-authenticated, read-only LAW-09 diagnostic route.

use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedImportResultKind, ResolvedType};
use crate::native_rust_binding::foreign_law::{
    self, DeclaredForeignSummary, ForeignBoundary, ForeignLawFrontier, ForeignLawRequest,
};
use crate::native_rust_binding::ScalarBindingPlan;

use super::super::ProjectSnapshot;
use super::ProjectRevision;

impl ProjectRevision {
    /// Re-derive the exact lock from this retained Project and find the Rust
    /// import in checked HIR before recording any foreign assumption.
    pub fn foreign_law_frontier(
        &self,
        binding: &ScalarBindingPlan,
        runtime_target: &str,
        adapter_digest: &str,
        declared: &DeclaredForeignSummary,
        law: &ForeignLawRequest,
    ) -> Result<ForeignLawFrontier, Vec<Diagnostic>> {
        let workspace = self.canonical_workspace_revision()?;
        let imports = [
            self.entry_program(),
            self.public_api_program(),
            self.test_program(),
        ]
        .into_iter()
        .flat_map(|program| &program.interfaces)
        .flat_map(|interface| &interface.imports)
        .filter(|import| import.id.as_str() == binding.import_id)
        .collect::<Vec<_>>();
        let Some(first) = imports.first() else {
            return Err(vec![Diagnostic::io(
                "SPX-FL307",
                "selected foreign import is absent from authenticated Project HIR",
            )]);
        };
        if imports.iter().any(|import| *import != *first) {
            return Err(vec![Diagnostic::io(
                "SPX-FL307",
                "selected foreign import has conflicting authenticated Project HIR rows",
            )]);
        }
        foreign_law::derive(
            first,
            binding,
            ForeignBoundary {
                project_lock_digest: workspace.dependency_lock_digest(),
                target: runtime_target,
                adapter_digest,
            },
            declared,
            law,
        )
        .map_err(|error| vec![error])
    }
}

impl ProjectSnapshot {
    /// Agent diagnostic view under the ordinary before/after held-source
    /// authentication. This result grants no Rust invocation authority.
    pub fn foreign_law_frontier_json(
        &mut self,
        binding: &ScalarBindingPlan,
        runtime_target: &str,
        adapter_digest: &str,
        declared: &DeclaredForeignSummary,
        law: &ForeignLawRequest,
    ) -> Result<String, Vec<Diagnostic>> {
        self.with_authenticated_request(|snapshot| {
            let frontier = snapshot.retain_revision().foreign_law_frontier(
                binding,
                runtime_target,
                adapter_digest,
                declared,
                law,
            )?;
            Ok(frontier.public_view())
        })
    }
}

/// A bounded source-derived conditional caller result. It records the exact
/// checked Project and summary inputs so replay does not trust a report row.
/// Only direct forwarding of a guarded i64 import is admitted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ForeignCallerCertificate {
    project_revision: String,
    semantic_graph_digest: String,
    caller_id: String,
    call_expression_id: String,
    binding: ScalarBindingPlan,
    runtime_target: String,
    adapter_digest: String,
    declared: DeclaredForeignSummary,
    law: ForeignLawRequest,
    frontier: ForeignLawFrontier,
}

fn direct_foreign_call<'a>(
    expr: &'a ResolvedExpr,
) -> Option<&'a crate::hir::ResolvedNativeRustImportCall> {
    match &expr.kind {
        ResolvedExprKind::NativeRustImportCall(call) => Some(call),
        ResolvedExprKind::Block { statements, tail } if statements.is_empty() => {
            direct_foreign_call(tail)
        }
        _ => None,
    }
}

impl ProjectRevision {
    /// Prove only source routing: an exported i64 caller forwards one guarded
    /// foreign result with inert scalar arguments. Foreign behavior remains an
    /// explicit assumption; this does not prove the Rust implementation.
    pub fn foreign_caller_certificate(
        &self,
        caller_id: &str,
        binding: &ScalarBindingPlan,
        runtime_target: &str,
        adapter_digest: &str,
        declared: &DeclaredForeignSummary,
        law: &ForeignLawRequest,
    ) -> Result<ForeignCallerCertificate, Vec<Diagnostic>> {
        let frontier =
            self.foreign_law_frontier(binding, runtime_target, adapter_digest, declared, law)?;
        if !law.require_return_guard || declared.return_i64_range.is_none() {
            return Err(vec![Diagnostic::io(
                "SPX-FL309",
                "foreign caller certificate requires an exact retained i64 return guard",
            )]);
        }
        if !self
            .manifest()
            .web_exports()
            .iter()
            .any(|id| id == caller_id)
        {
            return Err(vec![Diagnostic::io(
                "SPX-FL309",
                "foreign caller is not an authenticated public export",
            )]);
        }
        let callers = self
            .public_api_program()
            .functions
            .iter()
            .filter(|function| function.id.as_str() == caller_id)
            .collect::<Vec<_>>();
        let [caller] = callers.as_slice() else {
            return Err(vec![Diagnostic::io(
                "SPX-FL309",
                "foreign caller is missing or ambiguous in checked public HIR",
            )]);
        };
        let call = direct_foreign_call(&caller.body);
        let valid = caller.return_type == ResolvedType::I64
            && caller.requires.is_empty()
            && caller.ensures.is_empty()
            && caller.yields.is_none()
            && call.is_some_and(|call| {
                call.import.as_str() == binding.import_id
                    && call.result == ResolvedImportResultKind::I64
                    && call.args.iter().all(|arg| match &arg.kind {
                        ResolvedExprKind::Place(place) => {
                            place.projections.is_empty()
                                && caller.params.iter().any(|param| param.id == place.root)
                        }
                        ResolvedExprKind::Int(_) => true,
                        _ => false,
                    })
            });
        if !valid {
            return Err(vec![Diagnostic::io(
                "SPX-FL309",
                "foreign caller is not a direct guarded scalar forwarding route",
            )]);
        }
        Ok(ForeignCallerCertificate {
            project_revision: self.project_revision().to_owned(),
            semantic_graph_digest: self.semantic_graph_digest().to_owned(),
            caller_id: caller_id.to_owned(),
            call_expression_id: call
                .expect("checked direct call")
                .expression
                .as_str()
                .to_owned(),
            binding: binding.clone(),
            runtime_target: runtime_target.to_owned(),
            adapter_digest: adapter_digest.to_owned(),
            declared: declared.clone(),
            law: law.clone(),
            frontier,
        })
    }
}

impl ForeignCallerCertificate {
    pub fn law_id(&self) -> &str {
        &self.law.law_id
    }
    pub fn import_id(&self) -> &str {
        &self.binding.import_id
    }
    pub fn caller_id(&self) -> &str {
        &self.caller_id
    }
    pub fn conditions(&self) -> &[String] {
        self.frontier.conditions()
    }
    pub fn guarded_i64_range(&self) -> Option<(i64, i64)> {
        self.declared.return_i64_range
    }
    pub fn adapter_digest(&self) -> &str {
        &self.adapter_digest
    }
    pub fn frontier(&self) -> &ForeignLawFrontier {
        &self.frontier
    }

    /// Re-derive from checked source, exact binding, lock, adapter and summary.
    pub fn replay(&self, revision: &ProjectRevision) -> Result<(), Vec<Diagnostic>> {
        let expected = revision.foreign_caller_certificate(
            &self.caller_id,
            &self.binding,
            &self.runtime_target,
            &self.adapter_digest,
            &self.declared,
            &self.law,
        )?;
        if expected != *self {
            return Err(vec![Diagnostic::io(
                "SPX-FL308",
                "conditional foreign caller certificate is stale",
            )]);
        }
        Ok(())
    }

    pub fn public_view(&self) -> String {
        let value = serde_json::json!({
            "schema": "semaprax.foreign-caller-conditional.v1",
            "status": "conditional_source_route",
            "project_revision": self.project_revision,
            "semantic_graph_digest": self.semantic_graph_digest,
            "caller_id": self.caller_id,
            "call_expression_id": self.call_expression_id,
            "law_id": self.law.law_id,
            "import_id": self.binding.import_id,
            "adapter_digest": self.adapter_digest,
            "summary_digest": self.frontier.summary_digest(),
            "guarded_i64_range": self.declared.return_i64_range,
            "conditions": self.frontier.conditions(),
            "foreign_internals_proved": false,
            "source_route_proved": true,
        });
        format!(
            "{}\n",
            serde_json::to_string(&value).expect("closed caller JSON")
        )
    }
}

fn artifact_refusal(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-FL310", message)]
}

fn verify_guarded_sdk_artifact(
    caller: &ForeignCallerCertificate,
    revision: &ProjectRevision,
    output: &std::path::Path,
) -> Result<(), Vec<Diagnostic>> {
    use sha2::{Digest as _, Sha256};
    let root = std::fs::symlink_metadata(output)
        .map_err(|_| artifact_refusal("published foreign SDK directory is absent"))?;
    if !root.is_dir() || root.file_type().is_symlink() {
        return Err(artifact_refusal(
            "published foreign SDK directory is not an ordinary directory",
        ));
    }
    let manifest_bytes = std::fs::read(output.join("semaprax.native-rust-sdk.json"))
        .map_err(|_| artifact_refusal("published foreign SDK manifest is absent"))?;
    if manifest_bytes.len() > 1_048_576 || !manifest_bytes.ends_with(b"\n") {
        return Err(artifact_refusal(
            "published foreign SDK manifest exceeds its canonical bound",
        ));
    }
    let mut hash = Sha256::new();
    hash.update(b"semaprax.project-native-rust-sdk.manifest.v1\0");
    hash.update(&manifest_bytes);
    let actual_digest = format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()));
    if actual_digest != caller.adapter_digest {
        return Err(artifact_refusal(
            "published foreign SDK manifest identity differs from the retained adapter",
        ));
    }
    let manifest: serde_json::Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|_| artifact_refusal("published foreign SDK manifest is malformed"))?;
    if manifest["schema"] != "semaprax.project-native-rust-sdk.v1"
        || manifest["project_subject"]["project_revision"] != revision.project_revision()
        || manifest["project_subject"]["workspace_revision"] != revision.workspace_revision()
        || manifest["project_subject"]["project_graph"]["digest"]
            != revision.semantic_graph_digest()
        || manifest["crate"]["target"] != caller.runtime_target
    {
        return Err(artifact_refusal(
            "published foreign SDK subject or target differs from checked Project",
        ));
    }
    let files = manifest["files"]
        .as_array()
        .ok_or_else(|| artifact_refusal("published foreign SDK file inventory is absent"))?;
    let archive = if cfg!(windows) {
        "native/semaprax_native_rust_sdk.lib"
    } else {
        "native/libsemaprax_native_rust_sdk.a"
    };
    let mut expected = vec![
        "Cargo.toml",
        "build.rs",
        "native/descriptor.json",
        archive,
        "native/semaprax.native-rust-interop.json",
        "src/lib.rs",
        "src/semaprax_native_rust_interop.rs",
        "src/semaprax_native_rust_interop_ffi.rs",
    ];
    expected.sort_unstable();
    if files.len() != expected.len() {
        return Err(artifact_refusal(
            "published foreign SDK file inventory differs",
        ));
    }
    for (row, path) in files.iter().zip(expected) {
        if row["path"] != path {
            return Err(artifact_refusal("published foreign SDK file path differs"));
        }
        let local = output.join(path);
        let metadata = std::fs::symlink_metadata(&local)
            .map_err(|_| artifact_refusal("published foreign SDK file is absent"))?;
        if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 8_388_608 {
            return Err(artifact_refusal(
                "published foreign SDK file is not an ordinary bounded file",
            ));
        }
        let bytes = std::fs::read(&local)
            .map_err(|_| artifact_refusal("published foreign SDK file cannot be read"))?;
        let digest = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(&bytes))
        );
        if row["bytes"].as_u64() != Some(bytes.len() as u64) || row["sha256"] != digest {
            return Err(artifact_refusal(
                "published foreign SDK file differs from its authenticated manifest",
            ));
        }
        if path == "src/lib.rs" {
            let lib = std::str::from_utf8(&bytes).map_err(|_| {
                artifact_refusal("published foreign SDK adapter source is not UTF-8")
            })?;
            let (minimum, maximum) = caller
                .declared
                .return_i64_range
                .expect("caller certificate requires guard");
            let guard = format!(
                "pub const SEMAPRAX_FOREIGN_RETURN_GUARD:(&str,&str,&str,i64,i64)=({:?},{:?},{:?},{minimum},{maximum});",
                caller.binding.import_id, caller.declared.assumption_id,
                caller.declared.proposition_digest,
            );
            let check = format!("if value<{minimum}||value>{maximum}");
            if !lib.contains(&guard)
                || !lib.contains(&check)
                || !lib.contains(
                    "NonZeroU32::new(40909).unwrap(),class:NativeRustSdkStatusClass::Import",
                )
            {
                return Err(artifact_refusal(
                    "published foreign SDK lacks its exact checked return guard",
                ));
            }
        }
    }
    Ok(())
}

impl ForeignCallerCertificate {
    /// Check the exact guard-bearing published SDK artifact against the
    /// independently held builder digest. This is read-only diagnostic
    /// evidence; the caller-supplied digest does not grant strict authority.
    pub fn verify_published_guard(
        &self,
        revision: &ProjectRevision,
        sdk_output: &std::path::Path,
        expected_manifest_digest: &str,
    ) -> Result<(), Vec<Diagnostic>> {
        self.replay(revision)?;
        if self.adapter_digest != expected_manifest_digest {
            return Err(artifact_refusal(
                "foreign caller adapter differs from host-selected SDK manifest",
            ));
        }
        verify_guarded_sdk_artifact(self, revision, sdk_output)
    }
}
