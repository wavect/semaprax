//! Authority-free Linked Effect-Free Scalar Core-Wasm Package Build v2.

use crate::diagnostic::Diagnostic;
use crate::package_resolver::{ResolutionInput, ResolutionOptions};
use crate::package_source_capsule::{PackageSource, SourceCapsuleOptions};

mod admission;
mod model;
mod wire;

pub use model::{
    LinkedOfflinePackageBuild, LinkedOfflinePackageBuildOptions, VerifiedLinkedOfflinePackageBuild,
    EVIDENCE_SCHEMA, MANIFEST_SCHEMA, MAX_ARTIFACT_BYTES, MAX_EVIDENCE_BYTES,
    MAX_EVIDENCE_RENDER_BYTES, PROFILE,
};

pub fn generate(
    capsule: &str,
    sources: &[PackageSource],
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    capsule_options: &SourceCapsuleOptions,
    build_options: &LinkedOfflinePackageBuildOptions,
) -> Result<LinkedOfflinePackageBuild, Vec<Diagnostic>> {
    build(
        capsule,
        sources,
        resolution_evidence,
        resolution_input,
        resolution_options,
        capsule_options,
        build_options,
    )
    .map(|value| value.artifacts)
    .map_err(|error| vec![error])
}

pub fn verify(
    submitted: &LinkedOfflinePackageBuild,
    capsule: &str,
    sources: &[PackageSource],
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    capsule_options: &SourceCapsuleOptions,
    build_options: &LinkedOfflinePackageBuildOptions,
) -> Result<VerifiedLinkedOfflinePackageBuild, Diagnostic> {
    admission::validate_options(build_options)?;
    let _ = artifact_bytes_with_limit(submitted, build_options.max_artifact_bytes)?;
    wire::validate_submitted_manifest(&submitted.manifest_json, build_options.max_artifact_bytes)?;
    wire::validate_submitted_evidence(&submitted.evidence_json, build_options.max_evidence_bytes)?;
    let rebuilt = build(
        capsule,
        sources,
        resolution_evidence,
        resolution_input,
        resolution_options,
        capsule_options,
        build_options,
    )?;
    if &rebuilt.artifacts != submitted {
        return Err(replay_error(
            "submitted linked package build does not exactly replay its inputs",
        ));
    }
    Ok(VerifiedLinkedOfflinePackageBuild {
        root_package: build_options.root_package.clone(),
        packages: rebuilt.packages,
        capsule_digest: rebuilt.capsule_digest,
        wasm_sha256: wire::wasm_digest(&submitted.module_wasm),
        artifact_bytes: artifact_bytes(submitted)?,
    })
}

struct BuiltPackage {
    artifacts: LinkedOfflinePackageBuild,
    packages: Vec<crate::package_lock_v2::Coordinate>,
    capsule_digest: String,
}

#[allow(clippy::too_many_arguments)]
fn build(
    capsule: &str,
    sources: &[PackageSource],
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    capsule_options: &SourceCapsuleOptions,
    options: &LinkedOfflinePackageBuildOptions,
) -> Result<BuiltPackage, Diagnostic> {
    admission::validate_options(options)?;
    if capsule_options.root_package != options.root_package {
        return Err(association_error(
            "linked package-build root differs from capsule options",
        ));
    }
    let linked = crate::package_source_capsule::verify_for_linked_build(
        capsule,
        sources,
        resolution_evidence,
        resolution_input,
        resolution_options,
        capsule_options,
    )
    .map_err(|error| map_nested_error(&error))?;
    if linked.receipt.root_package() != options.root_package {
        return Err(association_error(
            "linked package-build root differs from authenticated capsule root",
        ));
    }
    admission::validate_root_exports(options, linked.receipt.exports())?;
    let packages = linked.receipt.packages().to_vec();
    let root = packages
        .iter()
        .find(|coordinate| coordinate.package == options.root_package)
        .cloned()
        .ok_or_else(|| association_error("linked package-build root coordinate is absent"))?;
    let source_bytes = linked
        .package_facts
        .iter()
        .try_fold(0usize, |sum, fact| sum.checked_add(fact.source_bytes))
        .ok_or_else(|| limit_error("linked package-build source byte sum overflowed"))?;
    let (emitted, overflowed) = crate::bounded_output::with_limit(MAX_ARTIFACT_BYTES, || {
        crate::wasm::emit_resolved_package_scalar_exports(&linked.program, &options.exports)
    });
    if overflowed {
        return Err(limit_error(
            "linked package-build Wasm emission exceeded its builder bound",
        ));
    }
    let (module_wasm, exports) = emitted.map_err(|error| map_compiler_error(&error))?;
    crate::package_build::validate_wasm_inventory(&module_wasm, &exports)
        .map_err(|_| profile_error("linked package-build Wasm inventory is not exact"))?;
    if module_wasm.len() > options.max_artifact_bytes {
        return Err(limit_error(
            "linked package-build Wasm exceeds max_artifact_bytes",
        ));
    }
    let facts = model::BuildFacts {
        root,
        packages: packages.clone(),
        capsule_digest: linked.receipt.digest().to_owned(),
        capsule_schema: linked.receipt.schema().to_owned(),
        capsule_bytes: linked.receipt.bytes(),
        source_set_digest: linked.receipt.source_set_digest().to_owned(),
        link_digest: linked.receipt.link_digest().to_owned(),
        source_bytes,
        exports,
    };
    let wasm_sha256 = wire::wasm_digest(&module_wasm);
    let (manifest_json, overflowed) = crate::bounded_output::with_limit(MAX_ARTIFACT_BYTES, || {
        wire::render_manifest(&facts, options, &wasm_sha256, module_wasm.len())
    });
    if overflowed {
        return Err(limit_error(
            "linked package-build manifest render exceeded its builder bound",
        ));
    }
    let prefix = module_wasm
        .len()
        .checked_add(manifest_json.len())
        .ok_or_else(|| limit_error("linked package-build artifact byte sum overflowed"))?;
    if prefix > options.max_artifact_bytes {
        return Err(limit_error(
            "linked package-build Wasm and manifest exceed max_artifact_bytes",
        ));
    }
    let evidence_limit = options
        .max_evidence_bytes
        .min(options.max_artifact_bytes - prefix);
    let manifest_sha256 = wire::manifest_digest(&manifest_json);
    let (evidence, overflowed) =
        crate::bounded_output::with_limit(MAX_EVIDENCE_RENDER_BYTES, || {
            wire::render_evidence(
                &facts,
                options,
                &manifest_json,
                &manifest_sha256,
                &wasm_sha256,
                module_wasm.len(),
            )
        });
    if overflowed {
        return Err(limit_error("linked package-build evidence fixed-point render exceeded its cumulative builder bound"));
    }
    let evidence_json = evidence?;
    if evidence_json.len() > evidence_limit {
        return Err(limit_error(
            "linked package-build evidence exceeds its final evidence or artifact bound",
        ));
    }
    let built = LinkedOfflinePackageBuild {
        module_wasm,
        manifest_json,
        evidence_json,
    };
    let _ = artifact_bytes_with_limit(&built, options.max_artifact_bytes)?;
    Ok(BuiltPackage {
        artifacts: built,
        packages,
        capsule_digest: facts.capsule_digest,
    })
}

fn artifact_bytes(build: &LinkedOfflinePackageBuild) -> Result<usize, Diagnostic> {
    build
        .module_wasm
        .len()
        .checked_add(build.manifest_json.len())
        .and_then(|v| v.checked_add(build.evidence_json.len()))
        .ok_or_else(|| limit_error("linked package-build artifact byte sum overflowed"))
}
fn artifact_bytes_with_limit(
    build: &LinkedOfflinePackageBuild,
    maximum: usize,
) -> Result<usize, Diagnostic> {
    let bytes = artifact_bytes(build)?;
    if bytes > maximum {
        Err(limit_error(
            "linked package-build cumulative artifacts exceed max_artifact_bytes",
        ))
    } else {
        Ok(bytes)
    }
}
fn option_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB601", message.into())
}
fn authentication_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB602", message.into())
}
fn association_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB603", message.into())
}
fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB604", message.into())
}
fn limit_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB605", message.into())
}
fn wire_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB606", message.into())
}
fn replay_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB607", message.into())
}
fn map_compiler_error(_: &Diagnostic) -> Diagnostic {
    profile_error("linked package-build scalar Wasm admission failed")
}
fn map_nested_error(error: &Diagnostic) -> Diagnostic {
    match error.code {
        "SPX-PS501" => option_error("linked package-build capsule option replay failed"),
        "SPX-PS502" => authentication_error("linked package-build capsule authentication failed"),
        "SPX-PS503" => association_error("linked package-build capsule association failed"),
        "SPX-PS504" => profile_error("linked package-build capsule profile failed"),
        "SPX-PS505" => limit_error("linked package-build capsule bound failed"),
        "SPX-PS506" => wire_error("linked package-build capsule wire failed"),
        "SPX-PS507" => replay_error("linked package-build capsule replay failed"),
        _ => authentication_error("linked package-build capsule replay failed closed"),
    }
}

#[cfg(test)]
mod tests;
