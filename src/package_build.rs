//! Authority-free Offline Effect-Free Scalar Core-Wasm Package Build v1.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::package_resolver::{ResolutionInput, ResolutionOptions};

mod admission;
mod model;
pub(crate) mod wire;

pub(crate) use model::RUNTIME_IMPORTS;
pub use model::{
    OfflinePackageBuild, OfflinePackageBuildOptions, VerifiedOfflinePackageBuild, EVIDENCE_SCHEMA,
    MANIFEST_SCHEMA, MAX_ARTIFACT_BYTES, MAX_EVIDENCE_BYTES, MAX_EVIDENCE_RENDER_BYTES, PROFILE,
};

pub fn generate(
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    build_options: &OfflinePackageBuildOptions,
) -> Result<OfflinePackageBuild, Vec<Diagnostic>> {
    build(
        resolution_evidence,
        resolution_input,
        resolution_options,
        build_options,
    )
    .map(|built| built.artifacts)
    .map_err(|error| vec![error])
}

pub fn verify(
    submitted: &OfflinePackageBuild,
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    build_options: &OfflinePackageBuildOptions,
) -> Result<VerifiedOfflinePackageBuild, Diagnostic> {
    admission::validate_options(build_options)?;
    if submitted.module_wasm.len() > build_options.max_artifact_bytes {
        return Err(limit_error(
            "submitted package-build Wasm exceeds the artifact bound",
        ));
    }
    let _ = artifact_bytes_with_limit(submitted, build_options.max_artifact_bytes)?;
    wire::validate_submitted_manifest(&submitted.manifest_json, build_options.max_artifact_bytes)?;
    wire::validate_submitted_evidence(&submitted.evidence_json, build_options.max_evidence_bytes)?;
    let rebuilt = build(
        resolution_evidence,
        resolution_input,
        resolution_options,
        build_options,
    )?;
    if &rebuilt.artifacts != submitted {
        return Err(replay_error(
            "submitted package build does not exactly replay its inputs",
        ));
    }
    let artifact_bytes = artifact_bytes(&rebuilt.artifacts)?;
    Ok(VerifiedOfflinePackageBuild {
        root_package: build_options.root_package.clone(),
        packages: rebuilt.packages,
        wasm_sha256: wire::wasm_digest(&rebuilt.artifacts.module_wasm),
        artifact_bytes,
    })
}

struct BuiltPackage {
    artifacts: OfflinePackageBuild,
    packages: Vec<crate::package_lock_v2::Coordinate>,
}

fn build(
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    build_options: &OfflinePackageBuildOptions,
) -> Result<BuiltPackage, Diagnostic> {
    admission::validate_options(build_options)?;
    let resolution = crate::package_resolver::verify_for_package_build(
        resolution_evidence,
        resolution_input,
        resolution_options,
    )
    .map_err(|error| {
        map_nested_error(&error, "package-build resolver or Subject-v2 replay failed")
    })?;
    let packages = resolution.resolution.packages.clone();
    let (subject, coordinate, lock) =
        admission::select_subject(resolution_input, build_options, resolution)?;
    let (program, resolved) = admission::verify_source(&subject, build_options)?;
    let link_digest = {
        let graph = crate::graph::to_json(&program).map_err(|errors| {
            errors.first().map_or_else(
                || profile_error("package-build Graph projection failed"),
                |error| map_nested_error(error, "package-build Graph projection failed"),
            )
        })?;
        wire::link_digest(&graph)
    };

    // The caller's artifact limit bounds returned bytes, not cumulative
    // compiler scratch writes. Keep emission work under the frozen global
    // ceiling, then enforce the exact caller limit on the emitted inventory.
    let (emitted, overflowed) = crate::bounded_output::with_limit(MAX_ARTIFACT_BYTES, || {
        crate::wasm::emit_resolved_package_scalar_exports(&resolved, &build_options.exports)
    });
    if overflowed {
        return Err(limit_error(
            "package-build Wasm emission exceeded the artifact builder bound",
        ));
    }
    let (module_wasm, exports) = emitted
        .map_err(|error| map_nested_error(&error, "package-build scalar Wasm admission failed"))?;
    validate_wasm_inventory(&module_wasm, &exports)?;
    if module_wasm.len() > build_options.max_artifact_bytes {
        return Err(limit_error("package-build Wasm exceeds max_artifact_bytes"));
    }

    let facts = model::BuildFacts {
        coordinate,
        subject_digest: subject.subject_digest,
        subject_bytes: subject.subject_bytes,
        report_digest: subject.report_digest,
        source_revision: subject.source_revision,
        source_bytes: subject.canonical_source.len(),
        source_set_digest: wire::source_set_digest(&subject.canonical_source),
        link_digest,
        resolution_digest: wire::wrapper_digest(resolution_evidence, "resolution evidence")?,
        resolution_bytes: resolution_evidence.len(),
        lock_digest: wire::wrapper_digest(&lock, "Lock v2")?,
        lock_bytes: lock.len(),
        exports,
    };
    let wasm_sha256 = wire::wasm_digest(&module_wasm);
    let (manifest_json, overflowed) = crate::bounded_output::with_limit(MAX_ARTIFACT_BYTES, || {
        wire::render_manifest(&facts, build_options, &wasm_sha256, module_wasm.len())
    });
    if overflowed {
        return Err(limit_error(
            "package-build manifest render exceeded the artifact builder bound",
        ));
    }
    let manifest_digest = wire::manifest_digest(&manifest_json);
    let artifact_prefix_bytes = module_wasm
        .len()
        .checked_add(manifest_json.len())
        .ok_or_else(|| limit_error("package-build artifact byte sum overflowed"))?;
    if artifact_prefix_bytes > build_options.max_artifact_bytes {
        return Err(limit_error(
            "package-build Wasm and manifest exceed max_artifact_bytes",
        ));
    }
    let evidence_limit = build_options
        .max_evidence_bytes
        .min(build_options.max_artifact_bytes - artifact_prefix_bytes);
    let (evidence, overflowed) =
        crate::bounded_output::with_limit(MAX_EVIDENCE_RENDER_BYTES, || {
            wire::render_evidence(
                &facts,
                build_options,
                &manifest_json,
                &manifest_digest,
                &wasm_sha256,
                module_wasm.len(),
            )
        });
    if overflowed {
        return Err(limit_error(
            "package-build evidence fixed-point render exceeded its cumulative builder bound",
        ));
    }
    let evidence_json = evidence?;
    if evidence_json.len() > evidence_limit {
        return Err(limit_error(
            "package-build evidence exceeds its final evidence or artifact bound",
        ));
    }
    let built = OfflinePackageBuild {
        module_wasm,
        manifest_json,
        evidence_json,
    };
    let _ = artifact_bytes_with_limit(&built, build_options.max_artifact_bytes)?;
    Ok(BuiltPackage {
        artifacts: built,
        packages,
    })
}

pub(crate) fn validate_wasm_inventory(
    wasm: &[u8],
    exports: &[crate::wasm::PackageScalarExportFact],
) -> Result<(), Diagnostic> {
    use wasmparser::{ExternalKind, Parser, Payload, TypeRef, Validator, WasmFeatures};

    Validator::new_with_features(WasmFeatures::all())
        .validate_all(wasm)
        .map_err(|_| profile_error("compiler-emitted package Wasm failed structural validation"))?;
    let mut imports = Vec::new();
    let mut actual_exports = Vec::new();
    let mut seen_exports = BTreeSet::new();
    for payload in Parser::new(0).parse_all(wasm) {
        match payload.map_err(|_| profile_error("package-build Wasm inventory is malformed"))? {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import
                        .map_err(|_| profile_error("package-build Wasm import is malformed"))?;
                    if !matches!(import.ty, TypeRef::Func(_)) {
                        return Err(profile_error(
                            "package-build Wasm contains a non-function import",
                        ));
                    }
                    imports.push((import.module.to_owned(), import.name.to_owned()));
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export
                        .map_err(|_| profile_error("package-build Wasm export is malformed"))?;
                    if export.kind != ExternalKind::Func
                        || !seen_exports.insert(export.name.to_owned())
                    {
                        return Err(profile_error(
                            "package-build Wasm export inventory is not unique functions",
                        ));
                    }
                    actual_exports.push(export.name.to_owned());
                }
            }
            _ => {}
        }
    }
    let expected_imports = model::RUNTIME_IMPORTS
        .iter()
        .map(|name| ("env".to_owned(), (*name).to_owned()))
        .collect::<Vec<_>>();
    if imports != expected_imports {
        return Err(profile_error(
            "package-build Wasm runtime import inventory is not exact",
        ));
    }
    let expected_exports = exports
        .iter()
        .map(|export| export.wasm_export.clone())
        .collect::<Vec<_>>();
    if actual_exports != expected_exports {
        return Err(profile_error(
            "package-build Wasm public export inventory is not exact",
        ));
    }
    Ok(())
}

fn artifact_bytes(build: &OfflinePackageBuild) -> Result<usize, Diagnostic> {
    build
        .module_wasm
        .len()
        .checked_add(build.manifest_json.len())
        .and_then(|value| value.checked_add(build.evidence_json.len()))
        .ok_or_else(|| limit_error("package-build artifact byte sum overflowed"))
}

fn artifact_bytes_with_limit(
    build: &OfflinePackageBuild,
    maximum: usize,
) -> Result<usize, Diagnostic> {
    let bytes = artifact_bytes(build)?;
    if bytes > maximum {
        return Err(limit_error(
            "package-build cumulative artifacts exceed max_artifact_bytes",
        ));
    }
    Ok(bytes)
}

fn option_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB501", message.into())
}

fn authentication_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB502", message.into())
}

fn association_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB503", message.into())
}

fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB504", message.into())
}

fn limit_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB505", message.into())
}

fn wire_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB506", message.into())
}

fn replay_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PB507", message.into())
}

fn map_nested_error(error: &Diagnostic, context: &str) -> Diagnostic {
    match error.code {
        "SPX-PR501" | "SPX-PL501" | "SPX-P401" => option_error(context),
        "SPX-PR503" | "SPX-PL504" | "SPX-PL505" => association_error(context),
        "SPX-PR504" | "SPX-P404" | "SPX-W115" => profile_error(context),
        "SPX-PR505" | "SPX-PL506" | "SPX-P402" | "SPX-W116" => limit_error(context),
        "SPX-PR502" | "SPX-PR506" | "SPX-PR507" | "SPX-PL502" | "SPX-PL503" | "SPX-PL507"
        | "SPX-P403" => authentication_error(context),
        _ => profile_error(context),
    }
}

#[cfg(test)]
mod tests;
