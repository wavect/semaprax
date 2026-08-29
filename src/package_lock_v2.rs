//! Source-authenticated Offline Semantic Package Lock v2.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::diagnostic::Diagnostic;

mod graph;
mod model;
mod subject;
mod wire;

pub const SCHEMA: &str = "semaprax.offline-semantic-package-lock.v2";
pub const SUBJECT_SCHEMA: &str = "semaprax.offline-semantic-package-subject.v2";
pub const MAX_PACKAGES: usize = 4;
pub const MAX_SUBJECT_BYTES: usize = 17 * 1024 * 1024;
pub const MAX_TOTAL_SUBJECT_BYTES: usize = 64 * 1024 * 1024;
pub const MAX_DEPENDENCIES: usize = 64;
pub const MAX_EDGES: usize = 256;
pub const MAX_DEPTH: usize = 32;
pub const MAX_CAPABILITIES: usize = 256;
pub const MAX_WORK_UNITS: usize = 8 * 1024 * 1024;
pub const MAX_JSON_DEPTH: usize = 128;
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
const MIN_OUTPUT_BYTES: usize = 4_096;
const SUBJECT_DOMAIN: &[u8] = b"semaprax.offline-semantic-package-subject.v2\0";
const LOCK_DOMAIN: &[u8] = b"semaprax.offline-semantic-package-lock.v2\0";
const REPORT_DOMAIN: &[u8] = b"semaprax.offline-semantic-package-report.v2\0";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Coordinate {
    pub package: String,
    pub version: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LockOptions {
    pub max_bytes: usize,
}

impl LockOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(MIN_OUTPUT_BYTES..=MAX_OUTPUT_BYTES).contains(&max_bytes) {
            return Err(wire::option_error(
                "semantic lock v2 max_bytes is outside the frozen range",
            ));
        }
        Ok(Self { max_bytes })
    }
}

impl Default for LockOptions {
    fn default() -> Self {
        Self {
            max_bytes: MAX_OUTPUT_BYTES,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedLock {
    pub packages: Vec<Coordinate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolutionSubject {
    pub(crate) coordinate: Coordinate,
    pub(crate) subject_digest: String,
    pub(crate) subject_bytes: usize,
    pub(crate) dependencies: Vec<Coordinate>,
    pub(crate) capabilities: Vec<String>,
    pub(crate) targets: BTreeMap<String, String>,
}

pub(crate) fn authenticate_subject_for_resolution(
    bytes: &str,
    work: &mut usize,
) -> Result<ResolutionSubject, Diagnostic> {
    let subject = subject::parse_subject(bytes, work)?;
    Ok(ResolutionSubject {
        coordinate: subject.coordinate,
        subject_digest: subject.digest,
        subject_bytes: subject.bytes,
        dependencies: subject.dependencies,
        capabilities: subject.capabilities,
        targets: subject.targets,
    })
}

pub fn create_subject(
    coordinate: &Coordinate,
    report: &str,
    dependencies: &[Coordinate],
    capabilities: &[String],
) -> Result<String, Vec<Diagnostic>> {
    subject::create_subject(coordinate, report, dependencies, capabilities)
}

pub fn generate(subjects: &[String], options: &LockOptions) -> Result<String, Vec<Diagnostic>> {
    graph::build(subjects, options).map_err(|error| vec![error])
}

pub fn verify(
    lock: &str,
    subjects: &[String],
    options: &LockOptions,
) -> Result<VerifiedLock, Diagnostic> {
    validate_options(options)?;
    if lock.len() > options.max_bytes || lock.len() > MAX_OUTPUT_BYTES {
        return Err(wire::limit_error("semantic lock exceeds output bound"));
    }
    wire::parse_wrapper(lock, SCHEMA, LOCK_DOMAIN, "lock")?;
    let rebuilt = graph::build(subjects, options)?;
    if rebuilt != lock {
        return Err(wire::replay_error(
            "semantic lock does not exactly replay subjects",
        ));
    }
    let value: Value =
        serde_json::from_str(lock).map_err(|_| wire::wire_error("replayed lock is not JSON"))?;
    let packages = value["payload"]["packages"]
        .as_array()
        .ok_or_else(|| wire::wire_error("packages missing"))?
        .iter()
        .map(|row| {
            Ok(Coordinate {
                package: wire::required_str(row, "package")?.to_owned(),
                version: wire::required_str(row, "version")?.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok(VerifiedLock { packages })
}

fn validate_options(options: &LockOptions) -> Result<(), Diagnostic> {
    LockOptions::new(options.max_bytes).map(|_| ())
}

#[cfg(test)]
use graph::aggregate_targets;
#[cfg(test)]
use model::Subject;
#[cfg(test)]
use wire::charge;

#[cfg(test)]
mod tests;
