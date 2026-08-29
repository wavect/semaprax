use serde_json::Value;

use crate::bounded_output::{self, BudgetedJoin as _};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::package_lock_v2;

use super::super::auth::authenticate;
use super::super::model::{render_input, Finding};
use super::super::wire::{
    limit_error, option_error, parse_wrapper, render_wrapper, replay_error, validate_options,
    wire_error,
};
use super::compare;

pub const SCHEMA: &str = "semaprax.offline-package-compatibility-evidence.v1";
pub const MAX_FINDINGS: usize = 2_048;
pub const MAX_WORK_UNITS: usize = 10 * 1024 * 1024;
pub const MAX_JSON_DEPTH: usize = 64;
pub const MAX_INPUT_BYTES: usize = 160 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MIN_OUTPUT_BYTES: usize = 4_096;
pub(in crate::package_compatibility) const DIGEST_DOMAIN: &[u8] =
    b"semaprax.offline-package-compatibility-evidence.v1\0";
pub(in crate::package_compatibility) const INPUT_DOMAIN: &[u8] =
    b"semaprax.offline-package-compatibility-input.v1\0";

macro_rules! bf { ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) }; }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompatibilityInput {
    pub coordinate: package_lock_v2::Coordinate,
    pub report: String,
    pub lock: String,
    pub lock_subjects: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompatibilityOptions {
    pub max_bytes: usize,
}
impl CompatibilityOptions {
    pub fn new(max_bytes: usize) -> Result<Self, Diagnostic> {
        if !(MIN_OUTPUT_BYTES..=MAX_OUTPUT_BYTES).contains(&max_bytes) {
            return Err(option_error("compatibility max_bytes outside frozen range"));
        }
        Ok(Self { max_bytes })
    }
}
impl Default for CompatibilityOptions {
    fn default() -> Self {
        Self {
            max_bytes: 8 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedEvidence {
    pub outcome: String,
    pub findings: usize,
}

pub fn generate(
    base: &CompatibilityInput,
    candidate: &CompatibilityInput,
    options: &CompatibilityOptions,
) -> Result<String, Vec<Diagnostic>> {
    build(base, candidate, options).map_err(|e| vec![e])
}

pub fn verify(
    evidence: &str,
    base: &CompatibilityInput,
    candidate: &CompatibilityInput,
    options: &CompatibilityOptions,
) -> Result<VerifiedEvidence, Diagnostic> {
    validate_options(options)?;
    if evidence.len() > options.max_bytes || evidence.len() > MAX_OUTPUT_BYTES {
        return Err(limit_error("evidence output bound exceeded"));
    }
    parse_wrapper(evidence)?;
    let rebuilt = build(base, candidate, options)?;
    if rebuilt != evidence {
        return Err(replay_error(
            "evidence does not exactly replay authenticated inputs",
        ));
    }
    let value: Value =
        serde_json::from_str(evidence).map_err(|_| wire_error("replayed evidence not JSON"))?;
    Ok(VerifiedEvidence {
        outcome: value["payload"]["outcome"]
            .as_str()
            .ok_or_else(|| wire_error("outcome missing"))?
            .to_owned(),
        findings: value["payload"]["findings"]
            .as_array()
            .ok_or_else(|| wire_error("findings missing"))?
            .len(),
    })
}

fn build(
    base: &CompatibilityInput,
    candidate: &CompatibilityInput,
    options: &CompatibilityOptions,
) -> Result<String, Diagnostic> {
    validate_options(options)?;
    let mut work = 0usize;
    let mut input_bytes = 0usize;
    let base = authenticate(base, &mut work, &mut input_bytes)?;
    let candidate = authenticate(candidate, &mut work, &mut input_bytes)?;
    let (outcome, mut findings) = compare(&base, &candidate, &mut work)?;
    findings.sort();
    findings.dedup();
    if findings.len() > MAX_FINDINGS {
        return Err(limit_error("findings exceed limit"));
    }
    let (envelope, overflowed) = bounded_output::with_limit(64 * 1024 * 1024, || {
        let finding_json = findings
            .iter()
            .map(Finding::render)
            .collect::<Vec<_>>()
            .budgeted_join(",");
        let payload=bf!("{{\"schema\":{},\"scope\":\"stable_id_semantic_compatibility_only\",\"limits\":{{\"max_findings\":{MAX_FINDINGS},\"max_work_units\":{MAX_WORK_UNITS},\"max_input_bytes\":{MAX_INPUT_BYTES},\"max_output_bytes\":{MAX_OUTPUT_BYTES},\"requested_max_bytes\":{}}},\"base\":{},\"candidate\":{},\"outcome\":{},\"findings\":[{}],\"budget\":{{\"used_input_bytes\":{},\"used_work_units\":{},\"used_findings\":{}}},\"nonclaims\":[\"not_source_spelling_or_general_consumer_compatibility\",\"no_resolver_registry_fetch_build_execution_or_publication\",\"unproven_unknown_or_lock_context_drift_is_indeterminate\",\"evidence_is_not_authority\"]}}",quote_json(SCHEMA),options.max_bytes,render_input(&base),render_input(&candidate),quote_json(outcome),finding_json,input_bytes,work,findings.len());
        render_wrapper(&payload)
    });
    if overflowed {
        return Err(limit_error("evidence render budget exceeded"));
    }
    if envelope.len() > options.max_bytes || envelope.len() > MAX_OUTPUT_BYTES {
        return Err(limit_error("evidence output bound exceeded"));
    }
    Ok(envelope)
}
