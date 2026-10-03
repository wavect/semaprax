//! Read-only, exact-certificate trust-chain projection. The compiler/artifact
//! edge remains trusted; this view never promotes a byte digest into a
//! translation theorem or an unexecuted target into runtime evidence.

use std::path::Path;

use serde_json::json;

use crate::diagnostic::Diagnostic;

use super::{verify_certificate_against_artifact, verify_certificate_against_source, LeanKernel};

pub const TRUST_CHAIN_VIEW_SCHEMA: &str = "semaprax.law-trust-chain-view.v1";

/// Replay source and artifact bindings before emitting a machine-readable
/// chain. Supplying a kernel additionally rechecks the exact certificate with
/// that kernel; omission is represented as `recorded_only`, never `proved`.
/// The runtime boundary is always `unexecuted` in this API because it receives
/// no execution capability or authenticated adapter observation.
pub fn render_trust_chain_view(
    certificate: &str,
    source_path: &Path,
    artifact_bytes: &[u8],
    kernel: Option<&dyn LeanKernel>,
) -> Result<String, Diagnostic> {
    render_trust_chain_view_for_target(
        certificate,
        source_path,
        artifact_bytes,
        super::certificate::ARTIFACT_TARGET,
        None,
        kernel,
    )
}

/// Refuse a consumer's attempted target or runtime-adapter rebinding before
/// replay. This certificate binds one Core-Wasm target and no runtime adapter;
/// an adapter result needs a separately authenticated invocation receipt.
pub fn render_trust_chain_view_for_target(
    certificate: &str,
    source_path: &Path,
    artifact_bytes: &[u8],
    requested_target: &str,
    adapter_identity: Option<&str>,
    kernel: Option<&dyn LeanKernel>,
) -> Result<String, Diagnostic> {
    if requested_target != super::certificate::ARTIFACT_TARGET {
        return Err(Diagnostic::io(
            "SPX-Z112",
            "requested target differs from this exact Core-Wasm certificate target".to_owned(),
        ));
    }
    if adapter_identity.is_some() {
        return Err(Diagnostic::io(
            "SPX-Z112",
            "this certificate has no authenticated runtime adapter association".to_owned(),
        ));
    }
    let source = verify_certificate_against_source(certificate, source_path)?;
    let artifact = verify_certificate_against_artifact(certificate, artifact_bytes)?;
    if source != artifact {
        return Err(Diagnostic::io(
            "SPX-Z112",
            "source and artifact replays selected different certificates".to_owned(),
        ));
    }
    let proof_status = if let Some(kernel) = kernel {
        let replayed = super::verify_certificate_with_kernel(certificate, source_path, kernel)?;
        if replayed != source {
            return Err(Diagnostic::io(
                "SPX-Z112",
                "kernel replay selected a different certificate".to_owned(),
            ));
        }
        "proved_by_replayed_kernel"
    } else {
        "recorded_only"
    };
    let view = json!({
        "schema": TRUST_CHAIN_VIEW_SCHEMA,
        "law_statement": {
            "obligation_id": source.obligation_id,
            "theorem_name": source.theorem_name,
            "status": "checked_source_binding"
        },
        "normalized_subject": {
            "declaration_id": source.declaration_id,
            "ensures_index": source.ensures_index,
            "status": "checked_source_binding"
        },
        "checked_source_semantics": {
            "revision": source.revision,
            "source_sha256": source.source_sha256,
            "status": "checked_by_verifier"
        },
        "external_proof_result": {
            "status": proof_status,
            "kernel_identity": super::KERNEL_IDENTITY,
            "toolchain": super::PINNED_TOOLCHAIN,
            "tcb": "caller_supplied_kernel_capability"
        },
        "compiler_lowering_identity": {
            "compiler_version": source.compiler_version,
            "profile": super::PROFILE_V1,
            "status": "trusted_unproved_lowering",
            "tcb": "compiler_and_codegen_for_exact_version"
        },
        "artifact_binding": {
            "target": super::certificate::ARTIFACT_TARGET,
            "sha256": source.artifact_sha256,
            "bytes": source.artifact_bytes,
            "status": "checked_exact_bytes"
        },
        "runtime_boundary": {
            "status": "unexecuted",
            "adapter_identity": null,
            "target_result": null,
            "tcb": "no_runtime_executor_or_adapter_observation"
        },
        "nonclaims": [
            "artifact binding does not prove semantic preservation",
            "no runtime or adapter result was observed by this view"
        ]
    });
    serde_json::to_string(&view)
        .map_err(|error| Diagnostic::io("SPX-Z111", format!("trust-chain view: {error}")))
}

#[cfg(test)]
mod tests {
    use super::render_trust_chain_view_for_target;
    use std::path::Path;

    #[test]
    fn wrong_target_and_unauthenticated_adapter_refuse_before_certificate_replay() {
        let nonexistent = Path::new("no-source-is-opened-for-wrong-association.spx");
        let wrong_target =
            render_trust_chain_view_for_target("", nonexistent, &[], "native-llvm-v1", None, None)
                .unwrap_err();
        assert_eq!(wrong_target.code, "SPX-Z112");
        let wrong_adapter = render_trust_chain_view_for_target(
            "",
            nonexistent,
            &[],
            "wasm-core-module-v1",
            Some("forged-adapter"),
            None,
        )
        .unwrap_err();
        assert_eq!(wrong_adapter.code, "SPX-Z112");
    }
}
