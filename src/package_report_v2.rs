//! Self-contained, source-authenticated Semantic Package Report v2.
//!
//! Unlike the descriptor-only v1 envelope, v2 embeds exact bounded canonical
//! source. Verification parses that source, runs the ordinary source verifier
//! and HIR resolver, rebuilds every semantic fact and target-projection fact,
//! regenerates the complete envelope, and exact-compares the submitted bytes.
//! A digest re-mint around self-asserted semantic fields is therefore
//! insufficient.

use std::path::{Path, PathBuf};

use crate::diagnostic::Diagnostic;
use crate::{bounded_output, format, graph, hir, parse, patch, verify};

mod contract;
mod model;
mod wire;

pub const SCHEMA: &str = "semaprax.semantic-package-report.v2";
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_FUNCTIONS: usize = 1024;
pub const MAX_CONTRACT_DEPTH: usize = 48;
pub const MAX_CONTRACT_NODES: usize = 65_536;
pub const MAX_REACHABLE_TYPES: usize = 1024;
pub const MAX_OUTPUT_BYTES: usize = graph::MAX_AGENT_CONTEXT_BYTES;
/// Exact cumulative budget charged to every report-rendered or intermediate
/// String byte. Non-string parser/HIR/container storage is separately bounded
/// by source bytes and closed cardinality limits.
pub const MAX_RENDER_STRING_BYTES: usize = 64 * 1024 * 1024;
pub const TARGET_PROJECTION_MAX_BYTES: usize = 16 * 1024 * 1024;
const DEFAULT_MAX_BYTES: usize = 4 * 1024 * 1024;
const SUBJECT_PATH: &str = "semantic-package-report-v2-subject.spx";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackageReportV2Options {
    pub max_bytes: usize,
}

impl PackageReportV2Options {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(graph::MIN_AGENT_CONTEXT_BYTES..=MAX_OUTPUT_BYTES).contains(&max_bytes) {
            return Err(option_error(format!(
                "semantic package-report v2 max_bytes must be between {} and {MAX_OUTPUT_BYTES}",
                graph::MIN_AGENT_CONTEXT_BYTES
            )));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for PackageReportV2Options {
    fn default() -> Self {
        Self {
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSemanticPackageReport {
    pub package: String,
    pub source_revision: String,
    pub exports_admitted: usize,
    pub exports_unproven: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedPackageBuildSource {
    pub(crate) package: String,
    pub(crate) source_revision: String,
    pub(crate) canonical_source: String,
}

/// Generate v2 from one held, bounded source snapshot. The embedded subject is
/// the canonical source projection, not a path or a claim supplied separately.
pub fn generate(
    source_path: &Path,
    options: &PackageReportV2Options,
) -> Result<String, Vec<Diagnostic>> {
    validate_options(options).map_err(|error| vec![error])?;
    let canonical_path = patch::canonical_source_path(source_path)?;
    let snapshot =
        patch::read_source_snapshot_bounded(&canonical_path, MAX_SOURCE_BYTES, "SPX-P402")?;
    let program = parse(snapshot.source(), source_path).map_err(|error| vec![error])?;
    let canonical_source =
        bounded_output::with_limit(MAX_SOURCE_BYTES, || format::canonical(&program));
    if canonical_source.1 || canonical_source.0.len() > MAX_SOURCE_BYTES {
        return Err(vec![limit_error(format!(
            "canonical source exceeds {MAX_SOURCE_BYTES} bytes"
        ))]);
    }
    let revision = graph::revision_from_canonical_source(&canonical_source.0);
    let envelope = build_from_canonical_source(&canonical_source.0, options)?;
    patch::validate_source_unchanged_bounded(
        &canonical_path,
        source_path,
        &snapshot,
        &revision,
        MAX_SOURCE_BYTES,
    )?;
    Ok(envelope)
}

/// Verify a submitted v2 envelope by rebuilding it from its exact embedded
/// canonical source subject. No submitted semantic or target field is trusted
/// as an input to reconstruction.
pub fn verify_envelope(envelope: &str) -> Result<VerifiedSemanticPackageReport, Diagnostic> {
    verify_envelope_impl(envelope, false)
}

pub(crate) fn verify_envelope_for_resolution(
    envelope: &str,
) -> Result<VerifiedSemanticPackageReport, Diagnostic> {
    verify_envelope_impl(envelope, true)
}

pub(crate) fn verify_envelope_for_package_build(
    envelope: &str,
) -> Result<VerifiedPackageBuildSource, Diagnostic> {
    let subject = wire::parse_subject_for_resolution(envelope)?;
    let receipt = verify_envelope_impl(envelope, true)?;
    Ok(VerifiedPackageBuildSource {
        package: receipt.package,
        source_revision: receipt.source_revision,
        canonical_source: subject.source,
    })
}

fn verify_envelope_impl(
    envelope: &str,
    preserve_bound_diagnostic: bool,
) -> Result<VerifiedSemanticPackageReport, Diagnostic> {
    let subject = if preserve_bound_diagnostic {
        wire::parse_subject_for_resolution(envelope)
    } else {
        wire::parse_subject(envelope)
    }?;
    let rebuilt =
        build_from_canonical_source(&subject.source, &subject.options).map_err(|errors| {
            if preserve_bound_diagnostic {
                if let Some(error) = errors
                    .into_iter()
                    .find(|error| matches!(error.code, "SPX-P401" | "SPX-P402"))
                {
                    return error;
                }
            }
            consistency_error("embedded source does not rebuild a valid v2 envelope")
        })?;
    if rebuilt != envelope {
        return Err(consistency_error(
            "submitted v2 envelope does not exactly replay its embedded source subject",
        ));
    }
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|_| consistency_error("replayed v2 envelope is not JSON"))?;
    let payload = &value["payload"];
    let package = payload["package"]["name"]
        .as_str()
        .ok_or_else(|| consistency_error("replayed v2 package name is missing"))?
        .to_owned();
    let source_revision = payload["source"]["revision"]
        .as_str()
        .ok_or_else(|| consistency_error("replayed v2 source revision is missing"))?
        .to_owned();
    let exports_admitted = json_usize(&payload["package"]["exports_admitted"], "exports_admitted")?;
    let exports_unproven = json_usize(&payload["package"]["exports_unproven"], "exports_unproven")?;
    Ok(VerifiedSemanticPackageReport {
        package,
        source_revision,
        exports_admitted,
        exports_unproven,
    })
}

fn build_from_canonical_source(
    canonical_source: &str,
    options: &PackageReportV2Options,
) -> Result<String, Vec<Diagnostic>> {
    validate_options(options).map_err(|error| vec![error])?;
    enforce_source_limit(canonical_source.len()).map_err(|error| vec![error])?;
    let subject_path = PathBuf::from(SUBJECT_PATH);
    let program = parse(canonical_source, &subject_path).map_err(|error| vec![error])?;
    enforce_function_limit(program.functions.len()).map_err(|error| vec![error])?;
    let (reformatted, overflowed) =
        bounded_output::with_limit(MAX_SOURCE_BYTES, || format::canonical(&program));
    if overflowed || reformatted.len() > MAX_SOURCE_BYTES {
        return Err(vec![limit_error(format!(
            "canonical source exceeds {MAX_SOURCE_BYTES} bytes"
        ))]);
    }
    if reformatted != canonical_source {
        return Err(vec![consistency_error(
            "embedded v2 source subject is not the exact canonical projection",
        )]);
    }
    let source_diagnostics = verify::verify(&program);
    if source_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == crate::diagnostic::Severity::Error)
    {
        return Err(source_diagnostics);
    }
    let resolved = hir::resolve(&program)?;
    // Revision projection hashes the already bounded canonical subject and
    // returns one fixed-size digest String before report String metering.
    let revision = graph::revision_from_canonical_source(canonical_source);
    // Target projections have their own compiler-owned bounds. Their discarded
    // artifact bytes never consume the report builder budget.
    let targets = model::target_proofs(&program, &resolved);
    let (envelope, overflowed) = bounded_output::with_limit(MAX_RENDER_STRING_BYTES, || {
        let payload = model::render_payload(
            canonical_source,
            &revision,
            &program,
            &resolved,
            options,
            &targets,
        )?;
        Ok::<_, Diagnostic>(wire::render_envelope(&payload))
    });
    let envelope = envelope.map_err(|error| vec![error])?;
    if overflowed {
        return Err(vec![limit_error(format!(
            "v2 rendering exceeded the frozen {MAX_RENDER_STRING_BYTES}-byte cumulative String budget"
        ))]);
    }
    enforce_output_limit(envelope.len(), options.max_bytes).map_err(|error| vec![error])?;
    Ok(envelope)
}

fn validate_options(options: &PackageReportV2Options) -> Result<(), Diagnostic> {
    PackageReportV2Options::new(options.max_bytes).map(|_| ())
}

/// `diagnostic::quote_json` already charges exact retained output bytes. Its
/// only transient String is the six-byte `\\uXXXX` formatting buffer for a
/// non-short-form control scalar; charge that buffer before construction too.
fn report_quote_json(value: &str) -> String {
    let transient = value
        .chars()
        .filter(|character| character.is_control() && !matches!(character, '\n' | '\r' | '\t'))
        .count()
        .checked_mul(6);
    let Some(transient) = transient else {
        let _ = bounded_output::reserve_active(usize::MAX);
        return String::new();
    };
    if !bounded_output::reserve_active(transient) {
        return String::new();
    }
    crate::diagnostic::quote_json(value)
}

fn enforce_source_limit(bytes: usize) -> Result<(), Diagnostic> {
    if bytes > MAX_SOURCE_BYTES {
        return Err(limit_error(format!(
            "canonical source exceeds {MAX_SOURCE_BYTES} bytes"
        )));
    }
    Ok(())
}

fn enforce_function_limit(count: usize) -> Result<(), Diagnostic> {
    if count > MAX_FUNCTIONS {
        return Err(limit_error(format!(
            "functions exceeds the {MAX_FUNCTIONS} v2 limit"
        )));
    }
    Ok(())
}

fn enforce_output_limit(bytes: usize, requested: usize) -> Result<(), Diagnostic> {
    let effective = requested.min(MAX_OUTPUT_BYTES);
    if bytes > effective {
        return Err(limit_error(format!(
            "v2 envelope needs {bytes} bytes but the effective limit is {effective}"
        )));
    }
    Ok(())
}

fn json_usize(value: &serde_json::Value, label: &str) -> Result<usize, Diagnostic> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| consistency_error(format!("replayed v2 {label} is invalid")))
}

fn option_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-P401", message.into())
}

fn limit_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-P402", message.into())
}

fn consistency_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-P403", message.into())
}

fn projection_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-P404", message.into())
}

#[cfg(test)]
mod tests;
