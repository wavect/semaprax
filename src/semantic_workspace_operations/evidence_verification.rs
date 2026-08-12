//! Strict parsing of untrusted Operations-intent Evidence.

use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde_json::{Map, Value};

use super::evidence_artifact::{
    APPLICATION_RECEIPT_SCHEMA, EVIDENCE_SCHEMA, MAX_CHANGE_EVIDENCE_BYTES,
    MAX_OPERATIONS_EVIDENCE_BYTES, MAX_TOTAL_BYTES, NONCLAIMS, VERIFICATION_RECEIPT_SCHEMA,
};
use super::{DERIVATION_SCHEMA, SCHEMA, WORKSPACE_MANIFEST_SCHEMA};
use crate::diagnostic::Diagnostic;
use crate::semantic_workspace_change;

const TOP_KEYS: [&str; 12] = [
    "schema",
    "workspace_manifest_schema",
    "base_workspace_revision",
    "candidate_workspace_revision",
    "entry_module",
    "operations_proposal",
    "operations_derivation",
    "derived_workspace_change_proposal",
    "workspace_change_evidence",
    "limits",
    "budget",
    "nonclaims",
];
const REF_KEYS: [&str; 3] = ["schema", "digest", "bytes"];
const CHILD_REF_KEYS: [&str; 4] = ["schema", "digest", "bytes", "document"];
const LIMIT_KEYS: [&str; 8] = [
    "max_workspace_change_evidence_bytes",
    "max_operations_evidence_bytes",
    "max_receipt_bytes",
    "max_total_operations_artifact_bytes",
    "max_json_depth",
    "max_retained_generations",
    "max_staging_attempts",
    "max_unexpected_inventory_entries",
];
const LIMIT_VALUES: [u64; 8] = [1_048_576, 4_194_304, 65_536, 150_994_944, 8, 32, 32, 0];
const BUDGET_KEYS: [&str; 10] = [
    "used_operations_proposal_bytes",
    "used_derivation_bytes",
    "used_workspace_change_total_artifact_bytes",
    "used_workspace_change_evidence_bytes",
    "used_operations_evidence_bytes",
    "used_receipt_bytes",
    "used_total_operations_artifact_bytes",
    "used_retained_generations",
    "used_staging_attempts",
    "used_unexpected_inventory_entries",
];

pub(super) struct SubmittedEvidence {
    child: String,
}

pub(super) fn read_evidence(path: &Path) -> Result<String, Vec<Diagnostic>> {
    let mut file = File::open(path).map_err(|_| io("open failed"))?;
    let metadata = file
        .metadata()
        .map_err(|_| io("metadata inspection failed"))?;
    if !metadata.is_file() {
        return Err(io("input is not a regular file"));
    }
    if metadata.len() > MAX_OPERATIONS_EVIDENCE_BYTES as u64 {
        return Err(super::limit(
            "operations_evidence_bytes",
            MAX_OPERATIONS_EVIDENCE_BYTES,
        ));
    }
    let mut bytes = Vec::with_capacity((metadata.len() as usize).saturating_add(1));
    file.by_ref()
        .take((MAX_OPERATIONS_EVIDENCE_BYTES as u64) + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io("read failed"))?;
    if bytes.len() > MAX_OPERATIONS_EVIDENCE_BYTES {
        return Err(super::limit(
            "operations_evidence_bytes",
            MAX_OPERATIONS_EVIDENCE_BYTES,
        ));
    }
    String::from_utf8(bytes).map_err(|_| io("input is not UTF-8"))
}

pub(super) fn parse_evidence(source: &str) -> Result<SubmittedEvidence, Vec<Diagnostic>> {
    if source.len() > MAX_OPERATIONS_EVIDENCE_BYTES {
        return Err(super::limit(
            "operations_evidence_bytes",
            MAX_OPERATIONS_EVIDENCE_BYTES,
        ));
    }
    if source.as_bytes().first() == Some(&0xef)
        || source.contains('\r')
        || !source.ends_with('\n')
        || source[..source.len().saturating_sub(1)].contains('\n')
    {
        return Err(canonical());
    }
    let body = &source[..source.len() - 1];
    let value: Value = serde_json::from_str(body).map_err(|_| canonical())?;
    if json_depth(&value) > 8 {
        return Err(super::limit("json_depth", 8));
    }
    let top = object(&value)?;
    let schema = string(top.get("schema"))?;
    if schema == VERIFICATION_RECEIPT_SCHEMA
        || schema == APPLICATION_RECEIPT_SCHEMA
        || schema == semantic_workspace_change::EVIDENCE_SCHEMA
        || schema != EVIDENCE_SCHEMA
    {
        return Err(canonical());
    }
    if top.len() != TOP_KEYS.len() || TOP_KEYS.iter().any(|key| !top.contains_key(*key)) {
        return Err(canonical());
    }
    if string(top.get("workspace_manifest_schema"))? != WORKSPACE_MANIFEST_SCHEMA {
        return Err(canonical());
    }
    let base_revision = string(top.get("base_workspace_revision"))?;
    let candidate_revision = string(top.get("candidate_workspace_revision"))?;
    let entry_module = string(top.get("entry_module"))?;
    if !super::valid_digest(base_revision)
        || !super::valid_digest(candidate_revision)
        || base_revision == candidate_revision
        || !super::valid_qualified_module(entry_module)
    {
        return Err(canonical());
    }
    validate_ref(&top["operations_proposal"], SCHEMA)?;
    validate_ref(&top["operations_derivation"], DERIVATION_SCHEMA)?;
    validate_ref(
        &top["derived_workspace_change_proposal"],
        semantic_workspace_change::SCHEMA,
    )?;
    let child_ref = object(&top["workspace_change_evidence"])?;
    if child_ref.len() != CHILD_REF_KEYS.len()
        || CHILD_REF_KEYS
            .iter()
            .any(|key| !child_ref.contains_key(*key))
        || string(child_ref.get("schema"))? != semantic_workspace_change::EVIDENCE_SCHEMA
    {
        return Err(canonical());
    }
    let child = string(child_ref.get("document"))?.to_owned();
    let child_bytes = number(child_ref.get("bytes"))?;
    let child_digest = string(child_ref.get("digest"))?;
    let limits = object(&top["limits"])?;
    if limits.len() != LIMIT_KEYS.len() || LIMIT_KEYS.iter().any(|key| !limits.contains_key(*key)) {
        return Err(canonical());
    }
    LIMIT_KEYS
        .iter()
        .map(|key| number(limits.get(*key)))
        .collect::<Result<Vec<_>, _>>()?;
    let budget = object(&top["budget"])?;
    if budget.len() != BUDGET_KEYS.len() || BUDGET_KEYS.iter().any(|key| !budget.contains_key(*key))
    {
        return Err(canonical());
    }
    let used = BUDGET_KEYS
        .iter()
        .map(|key| number(budget.get(*key)))
        .collect::<Result<Vec<_>, _>>()?;
    let _ = LIMIT_VALUES;
    let nonclaims = top["nonclaims"].as_array().ok_or_else(canonical)?;
    if nonclaims.len() != NONCLAIMS.len() || nonclaims.iter().any(|claim| !claim.is_string()) {
        return Err(canonical());
    }
    if render_canonical(top)? != source {
        return Err(canonical());
    }
    if used[3] != child_bytes
        || used[4] != source.len() as u64
        || used[5] != 0
        || used[7] > 32
        || used[8] > 32
        || used[9] != 0
        || used[6]
            != used[0]
                .checked_add(used[1])
                .and_then(|value| value.checked_add(used[2]))
                .and_then(|value| value.checked_add(used[4]))
                .unwrap_or(u64::MAX)
        || used[6] > MAX_TOTAL_BYTES as u64
    {
        return Err(child_or_budget());
    }
    if child.len() > MAX_CHANGE_EVIDENCE_BYTES {
        return Err(super::limit(
            "workspace_change_evidence_bytes",
            MAX_CHANGE_EVIDENCE_BYTES,
        ));
    }
    if child_bytes != child.len() as u64
        || child_digest != semantic_workspace_change::evidence_artifact_digest(&child)
    {
        return Err(child_or_budget());
    }
    semantic_workspace_change::validate_evidence_document(&child)?;
    Ok(SubmittedEvidence { child })
}

pub(super) fn verify_replay(
    submitted: &SubmittedEvidence,
    submitted_source: &str,
    regenerated_source: &str,
    regenerated_child: &str,
) -> Result<(), Vec<Diagnostic>> {
    if submitted.child != regenerated_child {
        return Err(replay_error());
    }
    if submitted_source == regenerated_source {
        return Ok(());
    }
    let _: Value = serde_json::from_str(&regenerated_source[..regenerated_source.len() - 1])
        .map_err(|_| replay_error())?;
    Err(replay_error())
}

fn validate_ref(value: &Value, schema: &str) -> Result<(), Vec<Diagnostic>> {
    let reference = object(value)?;
    if reference.len() != REF_KEYS.len()
        || REF_KEYS.iter().any(|key| !reference.contains_key(*key))
        || string(reference.get("schema"))? != schema
        || !valid_digest(string(reference.get("digest"))?)
    {
        return Err(canonical());
    }
    number(reference.get("bytes"))?;
    Ok(())
}

fn render_canonical(top: &Map<String, Value>) -> Result<String, Vec<Diagnostic>> {
    let mut output = String::new();
    output.push('{');
    for (index, key) in TOP_KEYS.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_scalar(&mut output, &Value::String((*key).to_owned()))?;
        output.push(':');
        match *key {
            "operations_proposal"
            | "operations_derivation"
            | "derived_workspace_change_proposal" => {
                push_object(&mut output, object(&top[*key])?, &REF_KEYS)?;
            }
            "workspace_change_evidence" => {
                push_object(&mut output, object(&top[*key])?, &CHILD_REF_KEYS)?;
            }
            "limits" => push_object(&mut output, object(&top[*key])?, &LIMIT_KEYS)?,
            "budget" => push_object(&mut output, object(&top[*key])?, &BUDGET_KEYS)?,
            _ => push_scalar(&mut output, &top[*key])?,
        }
    }
    output.push_str("}\n");
    Ok(output)
}

fn push_object(
    output: &mut String,
    object: &Map<String, Value>,
    keys: &[&str],
) -> Result<(), Vec<Diagnostic>> {
    output.push('{');
    for (index, key) in keys.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_scalar(output, &Value::String((*key).to_owned()))?;
        output.push(':');
        push_scalar(output, &object[*key])?;
    }
    output.push('}');
    Ok(())
}

fn push_scalar(output: &mut String, value: &Value) -> Result<(), Vec<Diagnostic>> {
    output.push_str(&serde_json::to_string(value).map_err(|_| canonical())?);
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn object(value: &Value) -> Result<&Map<String, Value>, Vec<Diagnostic>> {
    value.as_object().ok_or_else(canonical)
}

fn string(value: Option<&Value>) -> Result<&str, Vec<Diagnostic>> {
    value.and_then(Value::as_str).ok_or_else(canonical)
}

fn number(value: Option<&Value>) -> Result<u64, Vec<Diagnostic>> {
    value.and_then(Value::as_u64).ok_or_else(canonical)
}

pub(super) fn json_depth(value: &Value) -> usize {
    match value {
        Value::Array(values) => 1 + values.iter().map(json_depth).max().unwrap_or(0),
        Value::Object(values) => 1 + values.values().map(json_depth).max().unwrap_or(0),
        _ => 1,
    }
}

fn canonical() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G201",
        "Semantic Workspace Operations Evidence artifact is not canonical",
    )]
}

fn child_or_budget() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G202",
        "Semantic Workspace Operations Evidence child references or budget facts disagree with exact replay",
    )]
}

fn replay_error() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G203",
        "submitted Semantic Workspace Operations Evidence does not exactly replay the authenticated Operations proposal and derived Change Evidence",
    )]
}

fn io(detail: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-I217",
        format!("could not read Semantic Workspace Operations Evidence: {detail}"),
    )]
}
