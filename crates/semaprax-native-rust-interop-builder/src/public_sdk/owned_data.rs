//! Compatibility entry point for the standalone owned-data SDK evidence.
//!
//! Semantic derivation stays in the root compiler. The cycle-free lower crate
//! independently replays the descriptor and exclusively owns tool execution
//! and publication.

use super::*;

pub fn build_native_rust_owned_data_sdk(
    program: &semaprax::hir::ResolvedProgram,
    selected: &[String],
    subject: semaprax::project::PublicApiSubject<'_>,
    descriptor_bytes: &[u8],
    descriptor_digest: &str,
    output: &Path,
) -> Result<NativeRustOwnedDataSdkBundle, Vec<Diagnostic>> {
    let descriptor = semaprax::project::replay_public_api_descriptor(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
    )
    .map_err(|error| vec![error])?;
    if descriptor.canonical_bytes() != descriptor_bytes || descriptor.digest() != descriptor_digest
    {
        return Err(vec![error("owned-data SDK descriptor replay disagrees")]);
    }
    let provider = semaprax::codegen::emit_native_owned_data_provider(
        program,
        selected,
        subject,
        descriptor_bytes,
        descriptor_digest,
    )
    .map_err(|error| vec![error])?;
    if provider.descriptor() != descriptor_bytes
        || provider.descriptor_digest() != descriptor_digest
    {
        return Err(vec![error(
            "owned-data provider artifact authentication failed",
        )]);
    }
    let provider_bytes = provider.source().as_bytes().to_vec();
    let plan = semaprax_native_rust_owned_data_package::PackagePlan::new(
        descriptor_bytes.to_vec(),
        descriptor_digest.to_owned(),
        selected.to_vec(),
        provider_bytes.clone(),
        semaprax_native_rust_owned_data_package::provider_sha256(&provider_bytes),
        semaprax_native_rust_owned_data_package::PackageMode::StandaloneEvidence,
    );
    let bundle = semaprax_native_rust_owned_data_package::build_and_publish(plan, output)
        .map_err(|failure| vec![lower_error(failure)])?;
    Ok(NativeRustOwnedDataSdkBundle {
        output_directory: bundle.output_directory().to_path_buf(),
        manifest_path: bundle.manifest_path().to_path_buf(),
        manifest_digest: bundle.manifest_digest().to_owned(),
        descriptor_digest: bundle.descriptor_digest().to_owned(),
        crate_name: bundle.crate_name().to_owned(),
        target_triple: bundle.target_triple().to_owned(),
    })
}

fn lower_error(failure: semaprax_native_rust_owned_data_package::PackageError) -> Diagnostic {
    use semaprax_native_rust_owned_data_package::PackageErrorKind;

    match failure.kind() {
        PackageErrorKind::Descriptor | PackageErrorKind::Provider => {
            error("owned-data SDK lower replay failed")
        }
        PackageErrorKind::ToolConfiguration | PackageErrorKind::Publication => {
            Diagnostic::io("SPX-I234", "Native Rust owned-data SDK publication failed")
        }
    }
}

fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B114", message)
}
