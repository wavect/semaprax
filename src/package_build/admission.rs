use std::path::Path;

use crate::ast::Type;
use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::PackageBuildSubject;
use crate::package_resolver::{ResolutionInput, VerifiedPackageBuildResolution};

use super::model::{
    OfflinePackageBuildOptions, MAX_ARTIFACT_BYTES, MAX_EVIDENCE_BYTES, MAX_EXPORTS,
    MAX_STABLE_ID_BYTES, MIN_LIMIT_BYTES,
};

pub(crate) fn validate_options(options: &OfflinePackageBuildOptions) -> Result<(), Diagnostic> {
    if !(MIN_LIMIT_BYTES..=MAX_ARTIFACT_BYTES).contains(&options.max_artifact_bytes) {
        return Err(super::option_error(
            "package-build max_artifact_bytes is outside the frozen range",
        ));
    }
    if !(MIN_LIMIT_BYTES..=MAX_EVIDENCE_BYTES).contains(&options.max_evidence_bytes) {
        return Err(super::option_error(
            "package-build max_evidence_bytes is outside the frozen range",
        ));
    }
    validate_package(&options.root_package)?;
    if !(1..=MAX_EXPORTS).contains(&options.exports.len()) {
        return Err(super::option_error(
            "package-build exports must contain 1..=32 stable IDs",
        ));
    }
    let mut previous = None;
    for stable_id in &options.exports {
        if stable_id.is_empty()
            || stable_id.len() > MAX_STABLE_ID_BYTES
            || !stable_id.bytes().all(|byte| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            })
        {
            return Err(super::option_error(
                "package-build export stable ID is outside the scalar profile grammar",
            ));
        }
        if previous.is_some_and(|value: &String| value.as_bytes() >= stable_id.as_bytes()) {
            return Err(super::option_error(
                "package-build exports must be strictly byte-sorted and unique",
            ));
        }
        previous = Some(stable_id);
    }
    Ok(())
}

fn validate_package(value: &str) -> Result<(), Diagnostic> {
    if value.len() > 255 || crate::workspace_graph::validate_entry_module(value).is_err() {
        return Err(super::option_error(
            "package-build root package is outside the canonical module-name grammar",
        ));
    }
    Ok(())
}

pub(crate) fn select_subject(
    input: &ResolutionInput,
    options: &OfflinePackageBuildOptions,
    resolution: VerifiedPackageBuildResolution,
) -> Result<
    (
        PackageBuildSubject,
        crate::package_lock_v2::Coordinate,
        String,
    ),
    Diagnostic,
> {
    if input.target != "wasm32" {
        return Err(super::profile_error(
            "package-build resolver target must be wasm32",
        ));
    }
    if !input.allowed_capabilities.is_empty() {
        return Err(super::profile_error(
            "package-build resolver capability allowlist must be empty",
        ));
    }
    let [requirement] = input.requirements.as_slice() else {
        return Err(super::association_error(
            "package-build requires exactly one resolver root requirement",
        ));
    };
    if requirement.package != options.root_package {
        return Err(super::association_error(
            "package-build root package differs from the resolver requirement",
        ));
    }
    let [coordinate] = resolution.resolution.packages.as_slice() else {
        return Err(super::association_error(
            "package-build resolver must select exactly one package",
        ));
    };
    let [subject] = resolution.selected_subjects.as_slice() else {
        return Err(super::authentication_error(
            "package-build resolver selected-subject receipt is not singular",
        ));
    };
    if coordinate != &subject.coordinate || coordinate.package != options.root_package {
        return Err(super::association_error(
            "package-build selected coordinate, subject, and root disagree",
        ));
    }
    if !subject.dependencies.is_empty() {
        return Err(super::association_error(
            "package-build v1 does not admit Subject-v2 dependencies",
        ));
    }
    if !subject.capabilities.is_empty() {
        return Err(super::profile_error(
            "package-build selected subject capability inventory must be empty",
        ));
    }
    if subject.targets.get("wasm32").map(String::as_str) != Some("available") {
        return Err(super::profile_error(
            "package-build selected subject lacks an available wasm32 projection",
        ));
    }
    Ok((
        subject.clone(),
        coordinate.clone(),
        resolution.resolution.lock,
    ))
}

pub(crate) fn verify_source(
    subject: &PackageBuildSubject,
    options: &OfflinePackageBuildOptions,
) -> Result<(crate::ast::Program, crate::hir::ResolvedProgram), Diagnostic> {
    let path = Path::new("offline-package-build-subject.spx");
    let program = crate::check(&subject.canonical_source, path)
        .map_err(|_| super::authentication_error("package-build canonical source check failed"))?;
    let canonical = crate::format::canonical(&program);
    if canonical != subject.canonical_source {
        return Err(super::authentication_error(
            "package-build Subject-v2 source is not the exact canonical projection",
        ));
    }
    if program.module != subject.coordinate.package {
        return Err(super::association_error(
            "package-build source module differs from its package identity",
        ));
    }
    if !program.module_uses.is_empty() {
        return Err(super::association_error(
            "package-build v1 source must have an empty module-use inventory",
        ));
    }
    if !program.permits.is_empty()
        || !program.interfaces.is_empty()
        || !program.types.is_empty()
        || !program.protocols.is_empty()
        || program
            .functions
            .iter()
            .any(|function| !function.type_parameters.is_empty() || !function.effects.is_empty())
    {
        return Err(super::profile_error(
            "package-build source is outside the effect-free scalar profile",
        ));
    }
    let mains = program
        .functions
        .iter()
        .filter(|function| function.name == "main")
        .collect::<Vec<_>>();
    let [main] = mains.as_slice() else {
        return Err(super::profile_error(
            "package-build source must declare exactly one main function",
        ));
    };
    if !main.explicit_id || !main.params.is_empty() || main.return_type != Type::I64 {
        return Err(super::profile_error(
            "package-build main must have an explicit identity and signature fn main() -> i64",
        ));
    }
    let resolved = crate::hir::resolve(&program)
        .map_err(|_| super::profile_error("package-build HIR resolution failed"))?;
    crate::hir::validate(&resolved)
        .map_err(|_| super::profile_error("package-build HIR validation failed"))?;
    for stable_id in &options.exports {
        let Some(function) = program
            .functions
            .iter()
            .find(|function| function.stable_id == *stable_id)
        else {
            return Err(super::profile_error(
                "package-build export does not name a root-module function",
            ));
        };
        if !function.explicit_id {
            return Err(super::profile_error(
                "package-build export identity must be explicit",
            ));
        }
    }
    Ok((program, resolved))
}
