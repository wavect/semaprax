//! Self-contained, source-authenticated Semantic Package Report v2.
//!
//! Unlike the descriptor-only v1 envelope, v2 embeds exact bounded canonical
//! source. Verification parses that source, runs the ordinary source verifier
//! and HIR resolver, rebuilds every semantic fact and target-projection fact,
//! regenerates the complete envelope, and exact-compares the submitted bytes.
//! A digest re-mint around self-asserted semantic fields is therefore
//! insufficient.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sha2::{Digest as _, Sha256};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScalarInterfaceParameter {
    pub(crate) ty: String,
    pub(crate) ownership: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScalarInterfaceFunction {
    pub(crate) stable_id: String,
    pub(crate) parameters: Vec<ScalarInterfaceParameter>,
    pub(crate) result_type: String,
    pub(crate) result_ownership: String,
    pub(crate) effects: Vec<String>,
    pub(crate) requires: Vec<String>,
    pub(crate) ensures: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScalarPackageInterface {
    pub(crate) package: String,
    pub(crate) source_revision: String,
    pub(crate) digest: String,
    pub(crate) functions: Vec<ScalarInterfaceFunction>,
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

pub(crate) fn verify_scalar_interface_for_package_source(
    envelope: &str,
) -> Result<ScalarPackageInterface, Diagnostic> {
    let receipt = verify_envelope_impl(envelope, true)?;
    let value: serde_json::Value = serde_json::from_str(envelope)
        .map_err(|_| consistency_error("replayed v2 envelope is not JSON"))?;
    let payload = &value["payload"];
    for key in ["unproven_exports", "types", "unproven_types"] {
        if !payload[key].as_array().is_some_and(|rows| rows.is_empty()) {
            return Err(consistency_error(
                "package-source scalar interface contains unproven or authored type facts",
            ));
        }
    }
    let rows = payload["exports"]
        .as_array()
        .ok_or_else(|| consistency_error("replayed v2 exports are missing"))?;
    let mut functions = Vec::with_capacity(rows.len());
    for row in rows {
        functions.push(parse_scalar_interface_function(row)?);
    }
    functions.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    if functions
        .windows(2)
        .any(|pair| pair[0].stable_id == pair[1].stable_id)
    {
        return Err(consistency_error(
            "package-source scalar interface identities are duplicated",
        ));
    }
    let digest = scalar_interface_digest(&receipt.package, &functions);
    Ok(ScalarPackageInterface {
        package: receipt.package,
        source_revision: receipt.source_revision,
        digest,
        functions,
    })
}

pub(crate) fn scalar_interface_from_resolved(
    package: &str,
    functions: &[&hir::ResolvedFunction],
) -> Result<ScalarPackageInterface, Diagnostic> {
    let mut facts = Vec::with_capacity(functions.len());
    for function in functions {
        let (requires, ensures) = contract::normalize(function)?;
        let canonical_contracts = |values: Vec<String>| {
            values
                .into_iter()
                .map(|value| {
                    let value: serde_json::Value = serde_json::from_str(&value).map_err(|_| {
                        consistency_error("compiler scalar contract JSON is malformed")
                    })?;
                    serde_json::to_string(&value).map_err(|_| {
                        consistency_error("compiler scalar contract cannot be canonicalized")
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()
        };
        let parameters = function
            .params
            .iter()
            .map(|parameter| {
                Ok(ScalarInterfaceParameter {
                    ty: scalar_type_json(&parameter.ty)?,
                    ownership: ownership_text(parameter.ownership).to_owned(),
                })
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        let effects = function
            .effects
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        facts.push(ScalarInterfaceFunction {
            stable_id: function.id.as_str().to_owned(),
            parameters,
            result_type: scalar_type_json(&function.return_type)?,
            result_ownership: "value".to_owned(),
            effects,
            requires: canonical_contracts(requires)?,
            ensures: canonical_contracts(ensures)?,
        });
    }
    facts.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    if facts
        .windows(2)
        .any(|pair| pair[0].stable_id == pair[1].stable_id)
    {
        return Err(consistency_error(
            "linked package scalar interface identities are duplicated",
        ));
    }
    Ok(ScalarPackageInterface {
        package: package.to_owned(),
        source_revision: String::new(),
        digest: scalar_interface_digest(package, &facts),
        functions: facts,
    })
}

fn parse_scalar_interface_function(
    row: &serde_json::Value,
) -> Result<ScalarInterfaceFunction, Diagnostic> {
    let stable_id = row["stable_id"]
        .as_str()
        .ok_or_else(|| consistency_error("v2 scalar interface stable_id is missing"))?
        .to_owned();
    let parameters = row["parameters"]
        .as_array()
        .ok_or_else(|| consistency_error("v2 scalar interface parameters are missing"))?
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            if parameter["index"].as_u64() != Some(index as u64) {
                return Err(consistency_error(
                    "v2 scalar interface parameter positions are noncanonical",
                ));
            }
            Ok(ScalarInterfaceParameter {
                ty: scalar_type_value(&parameter["type"])?,
                ownership: parameter["ownership"]
                    .as_str()
                    .ok_or_else(|| consistency_error("v2 scalar interface ownership is missing"))?
                    .to_owned(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let result_type = scalar_type_value(&row["result"]["type"])?;
    let result_ownership = row["result"]["ownership"]
        .as_str()
        .ok_or_else(|| consistency_error("v2 scalar result ownership is missing"))?
        .to_owned();
    let effects = string_array(&row["effects"], "effects")?;
    let requires = contract_fact_array(&row["requires"])?;
    let ensures = contract_fact_array(&row["ensures"])?;
    Ok(ScalarInterfaceFunction {
        stable_id,
        parameters,
        result_type,
        result_ownership,
        effects,
        requires,
        ensures,
    })
}

fn scalar_type_json(ty: &hir::ResolvedType) -> Result<String, Diagnostic> {
    match ty {
        hir::ResolvedType::I64 | hir::ResolvedType::Bool => {
            let value: serde_json::Value = serde_json::from_str(&model::type_json(ty))
                .map_err(|_| consistency_error("compiler scalar type JSON is malformed"))?;
            scalar_type_value(&value)
        }
        _ => Err(consistency_error(
            "package-source interface is outside the i64/bool scalar profile",
        )),
    }
}

fn scalar_type_value(value: &serde_json::Value) -> Result<String, Diagnostic> {
    let object = value
        .as_object()
        .ok_or_else(|| consistency_error("v2 scalar interface type must be an object"))?;
    let primitive = object.get("kind").and_then(serde_json::Value::as_str) == Some("primitive")
        && matches!(
            object.get("name").and_then(serde_json::Value::as_str),
            Some("i64" | "bool")
        )
        && object.len() == 2;
    if !primitive {
        return Err(consistency_error(
            "v2 package interface is outside the i64/bool scalar profile",
        ));
    }
    serde_json::to_string(value)
        .map_err(|_| consistency_error("v2 scalar interface type cannot be canonicalized"))
}

fn contract_fact_array(value: &serde_json::Value) -> Result<Vec<String>, Diagnostic> {
    value
        .as_array()
        .ok_or_else(|| consistency_error("v2 scalar interface contract vector is missing"))?
        .iter()
        .enumerate()
        .map(|(index, row)| {
            if row["index"].as_u64() != Some(index as u64) {
                return Err(consistency_error(
                    "v2 scalar interface contract positions are noncanonical",
                ));
            }
            serde_json::to_string(&row["fact"])
                .map_err(|_| consistency_error("v2 scalar interface contract is malformed"))
        })
        .collect()
}

fn string_array(value: &serde_json::Value, label: &str) -> Result<Vec<String>, Diagnostic> {
    value
        .as_array()
        .ok_or_else(|| consistency_error(format!("v2 scalar interface {label} are missing")))?
        .iter()
        .map(|value| {
            value.as_str().map(str::to_owned).ok_or_else(|| {
                consistency_error(format!("v2 scalar interface {label} are invalid"))
            })
        })
        .collect()
}

fn ownership_text(value: hir::OwnershipMode) -> &'static str {
    match value {
        hir::OwnershipMode::Value => "value",
        hir::OwnershipMode::Own => "own",
        hir::OwnershipMode::Borrow => "borrow",
        hir::OwnershipMode::Shared => "shared",
    }
}

fn scalar_interface_digest(package: &str, functions: &[ScalarInterfaceFunction]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.offline-package-scalar-interface.v1\0");
    hash_interface_field(&mut hasher, package.as_bytes());
    hasher.update((functions.len() as u64).to_le_bytes());
    for function in functions {
        hash_interface_field(&mut hasher, function.stable_id.as_bytes());
        hasher.update((function.parameters.len() as u64).to_le_bytes());
        for parameter in &function.parameters {
            hash_interface_field(&mut hasher, parameter.ty.as_bytes());
            hash_interface_field(&mut hasher, parameter.ownership.as_bytes());
        }
        hash_interface_field(&mut hasher, function.result_type.as_bytes());
        hash_interface_field(&mut hasher, function.result_ownership.as_bytes());
        for values in [&function.effects, &function.requires, &function.ensures] {
            hasher.update((values.len() as u64).to_le_bytes());
            for value in values {
                hash_interface_field(&mut hasher, value.as_bytes());
            }
        }
    }
    crate::bounded_output::budgeted_format(format_args!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    ))
}

fn hash_interface_field(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
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
