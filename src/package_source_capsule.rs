//! Authority-free Offline Multi-Package Source Capsule v1.

use crate::diagnostic::Diagnostic;
use crate::package_resolver::{ResolutionInput, ResolutionOptions};

mod admission;
mod model;
mod wire;

#[allow(
    unused_imports,
    reason = "frozen crate-private seam consumed by Offline Package Build v2"
)]
pub(crate) use model::{
    LinkedPackageImportFact, LinkedPackageSourceFact, VerifiedLinkedSourceCapsule,
};
pub use model::{
    PackageSource, SourceCapsuleOptions, VerifiedSourceCapsule, MAX_IMPORTS, MAX_OUTPUT_BYTES,
    MAX_PACKAGES, MAX_RENDER_BYTES, MAX_SOURCE_BYTES, MAX_TOTAL_SOURCE_BYTES, MIN_PACKAGES, SCHEMA,
};

pub fn generate(
    sources: &[PackageSource],
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    options: &SourceCapsuleOptions,
) -> Result<String, Vec<Diagnostic>> {
    admission::build(
        sources,
        resolution_evidence,
        resolution_input,
        resolution_options,
        options,
    )
    .map(|built| built.json)
    .map_err(|error| vec![error])
}

pub fn verify(
    capsule: &str,
    sources: &[PackageSource],
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    options: &SourceCapsuleOptions,
) -> Result<VerifiedSourceCapsule, Diagnostic> {
    wire::validate_submitted(capsule, options.max_bytes)?;
    let built = admission::build(
        sources,
        resolution_evidence,
        resolution_input,
        resolution_options,
        options,
    )?;
    if built.json != capsule {
        return Err(replay_error(
            "submitted package-source capsule does not exactly replay its inputs",
        ));
    }
    Ok(built.receipt)
}

#[allow(
    dead_code,
    reason = "frozen crate-private seam consumed by Offline Package Build v2"
)]
pub(crate) fn verify_for_linked_build(
    capsule: &str,
    sources: &[PackageSource],
    resolution_evidence: &str,
    resolution_input: &ResolutionInput,
    resolution_options: &ResolutionOptions,
    options: &SourceCapsuleOptions,
) -> Result<VerifiedLinkedSourceCapsule, Diagnostic> {
    wire::validate_submitted(capsule, options.max_bytes)?;
    let built = admission::build(
        sources,
        resolution_evidence,
        resolution_input,
        resolution_options,
        options,
    )?;
    if built.json != capsule {
        return Err(replay_error(
            "submitted package-source capsule does not exactly replay its inputs",
        ));
    }
    Ok(VerifiedLinkedSourceCapsule {
        receipt: built.receipt,
        program: built.program,
        selected_subjects: built.selected_subjects,
        package_facts: built.package_facts,
        import_facts: built.import_facts,
    })
}

fn option_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS501", message.into())
}
fn authentication_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS502", message.into())
}
fn association_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS503", message.into())
}
fn profile_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS504", message.into())
}
fn limit_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS505", message.into())
}
fn wire_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS506", message.into())
}
fn replay_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-PS507", message.into())
}

fn map_nested_error(error: &Diagnostic) -> Diagnostic {
    match error.code {
        "SPX-PR505" | "SPX-PL506" | "SPX-P402" => {
            limit_error("package-source resolver or selected-subject bound failed")
        }
        "SPX-PR501" => option_error("package-source resolver input is invalid"),
        "SPX-PR503" | "SPX-PR504" | "SPX-PL504" | "SPX-PL505" => {
            association_error("package-source resolver selection is inconsistent")
        }
        _ => authentication_error("package-source resolver or selected-subject replay failed"),
    }
}

fn map_graph_error(error: Diagnostic) -> Diagnostic {
    match error.code {
        "SPX-G171" | "SPX-G175" => limit_error("package-source workspace bound failed"),
        "SPX-PS503" => association_error(error.message),
        "SPX-PS504" | "SPX-G172" | "SPX-H006" => profile_error(error.message),
        _ => authentication_error("package-source unified source replay failed"),
    }
}

#[cfg(test)]
mod tests;
