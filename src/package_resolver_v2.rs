//! Authority-free Offline Deterministic Package Resolver v2.

use serde_json::Value;

use crate::bounded_output;
use crate::diagnostic::Diagnostic;
use crate::package_lock_v3;

mod catalog;
mod model;
mod semver;
mod solver;
mod wire;

pub const SCHEMA: &str = "semaprax.offline-package-resolution-evidence.v2";
pub const MAX_REQUIREMENTS: usize = 4;
pub const MAX_SUBJECTS: usize = 64;
pub const MAX_VERSIONS_PER_PACKAGE: usize = 32;
pub const MAX_SELECTED_PACKAGES: usize = 4;
pub const MAX_ALLOWED_CAPABILITIES: usize = 256;
pub const MAX_SUBJECT_BYTES: usize = 17 * 1024 * 1024;
pub const MAX_TOTAL_SUBJECT_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_EDGES: usize = 256;
pub const MAX_DEPTH: usize = 32;
pub const MAX_DECISIONS: usize = 4_096;
pub const MAX_WORK_UNITS: usize = 8 * 1024 * 1024;
pub const MAX_JSON_DEPTH: usize = 128;
pub const MAX_RENDER_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MIN_OUTPUT_BYTES: usize = 4_096;
const DIGEST_DOMAIN: &[u8] = b"semaprax.offline-package-resolution-evidence.v2\0";
const CATALOG_DOMAIN: &[u8] = b"semaprax.offline-package-resolution-catalog.v2\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Requirement {
    pub package: String,
    pub range: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolutionInput {
    pub requirements: Vec<Requirement>,
    pub subjects: Vec<String>,
    pub target: String,
    pub allowed_capabilities: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolutionOptions {
    pub max_bytes: usize,
}

impl ResolutionOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(MIN_OUTPUT_BYTES..=MAX_OUTPUT_BYTES).contains(&max_bytes) {
            return Err(wire::option_error(
                "package resolution max_bytes is outside the frozen range",
            ));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for ResolutionOptions {
    fn default() -> Self {
        Self {
            max_bytes: MAX_OUTPUT_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedResolution {
    pub packages: Vec<package_lock_v3::Coordinate>,
    pub lock: String,
}

pub fn generate(
    input: &ResolutionInput,
    options: &ResolutionOptions,
) -> Result<String, Vec<Diagnostic>> {
    bounded_build(input, options)
        .map(|built| built.evidence)
        .map_err(|error| vec![error])
}

pub fn verify(
    evidence: &str,
    input: &ResolutionInput,
    options: &ResolutionOptions,
) -> Result<VerifiedResolution, Diagnostic> {
    validate_options(options)?;
    if evidence.len() > options.max_bytes || evidence.len() > MAX_OUTPUT_BYTES {
        return Err(wire::limit_error(
            "resolution evidence exceeds output bound",
        ));
    }
    let (result, overflowed) = bounded_output::with_limit(MAX_RENDER_BYTES, || {
        wire::parse_wrapper(evidence)?;
        let rebuilt = build(input, options)?;
        if rebuilt.evidence != evidence {
            return Err(wire::replay_error(
                "resolution evidence does not exactly replay inputs",
            ));
        }
        receipt(evidence)
    });
    if overflowed {
        return Err(wire::limit_error(
            "resolution cumulative String budget exceeded",
        ));
    }
    result
}

fn bounded_build(
    input: &ResolutionInput,
    options: &ResolutionOptions,
) -> Result<BuiltResolution, Diagnostic> {
    let (result, overflowed) =
        bounded_output::with_limit(MAX_RENDER_BYTES, || build(input, options));
    if overflowed {
        return Err(wire::limit_error(
            "resolution cumulative String budget exceeded",
        ));
    }
    result
}

struct BuiltResolution {
    evidence: String,
}

fn build(
    input: &ResolutionInput,
    options: &ResolutionOptions,
) -> Result<BuiltResolution, Diagnostic> {
    validate_options(options)?;
    let mut work = 0usize;
    let requirements = model::validate_input(input, &mut work)?;
    let catalog = catalog::authenticate(input, &mut work)?;
    if !catalog.target_inventory.contains(&input.target) {
        return Err(wire::input_error("requested target is unknown"));
    }
    let solved = solver::solve(input, &requirements, &catalog, &mut work)?;
    let selected_subjects = solved
        .selected
        .values()
        .map(|entry| bounded_output::budgeted_clone(entry.bytes))
        .collect::<Vec<_>>();
    let lock_options = package_lock_v3::LockOptions::default();
    let lock = package_lock_v3::generate(&selected_subjects, &lock_options)
        .map_err(|errors| wire::map_lock_errors(&errors, "final Lock-v3 generation failed"))?;
    package_lock_v3::verify(&lock, &selected_subjects, &lock_options)
        .map_err(|error| wire::map_lock_error(&error, "final Lock-v3 replay failed"))?;
    model::recheck_lock_policy(&lock, input)?;
    let envelope = model::render_evidence(
        input,
        options,
        &requirements,
        &catalog,
        &solved,
        &lock,
        work,
    )?;
    if envelope.len() > options.max_bytes || envelope.len() > MAX_OUTPUT_BYTES {
        return Err(wire::limit_error(
            "resolution evidence exceeds output bound",
        ));
    }
    Ok(BuiltResolution { evidence: envelope })
}

fn receipt(evidence: &str) -> Result<VerifiedResolution, Diagnostic> {
    let value: Value = serde_json::from_str(evidence)
        .map_err(|_| wire::wire_error("replayed evidence not JSON"))?;
    let payload = &value["payload"];
    let packages = payload["selected"]
        .as_array()
        .ok_or_else(|| wire::wire_error("selected rows missing"))?
        .iter()
        .map(|row| {
            Ok(package_lock_v3::Coordinate {
                package: wire::required_str(row, "package")?.to_owned(),
                version: wire::required_str(row, "version")?.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    let lock = bounded_output::budgeted_clone(model::exact_lock_bytes(evidence)?);
    Ok(VerifiedResolution { packages, lock })
}

fn validate_options(options: &ResolutionOptions) -> Result<(), Diagnostic> {
    ResolutionOptions::new(options.max_bytes).map(|_| ())
}

#[cfg(test)]
mod tests;
