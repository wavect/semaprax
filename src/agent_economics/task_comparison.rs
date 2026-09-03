//! Authority-free normalization of externally completed task-comparison records.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

pub const AGENT_TASK_COMPARISON_OBSERVATION_SET_SCHEMA: &str =
    "semaprax.agent-task-comparison-observation-set.v1";
pub const AGENT_TASK_COMPARISON_NORMALIZED_REPORT_SCHEMA: &str =
    "semaprax.agent-task-comparison-normalized-report.v1";
pub const MAX_AGENT_TASK_COMPARISON_INPUT_BYTES: usize = 7 * 1024 * 1024;
pub const MAX_AGENT_TASK_COMPARISON_REPORT_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_AGENT_TASK_COMPARISON_IDENTIFIER_BYTES: usize = 256;

const REPORT_DOMAIN: &[u8] = b"semaprax.agent-task-comparison-normalized-report.v1\0";
const REQUIRED_LANES: [&str; 2] = ["semaprax-graph-operational", "semaprax-source-first"];
const ZERO_LANE: &str = "zero-graph-native";
const METRICS: [&str; 12] = [
    "model_input_tokens",
    "model_output_tokens",
    "presented_context_bytes",
    "tool_calls",
    "tool_request_bytes",
    "tool_response_bytes",
    "failed_attempts",
    "stale_failures",
    "stale_recovery_actions",
    "validation_wall_ms",
    "review_wall_ms",
    "human_interventions",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Validate exact, canonical caller-supplied completed observations and return
/// deterministic descriptive differences. No model, tool, validator or timer
/// is invoked and no missing metric is inferred.
pub fn normalize_task_comparison_observations(
    observations: &[u8],
    expected_sha256: &str,
) -> Result<String> {
    if observations.is_empty() || observations.len() > MAX_AGENT_TASK_COMPARISON_INPUT_BYTES {
        return Err(capacity(
            "task comparison observation bytes exceed their bound",
        ));
    }
    require_digest(expected_sha256)?;
    if sha256(observations) != expected_sha256 {
        return Err(binding("task comparison observation digest disagrees"));
    }
    let value: Value = serde_json::from_slice(observations)
        .map_err(|_| invalid("task comparison observations are not valid JSON"))?;
    let canonical = render(&value, MAX_AGENT_TASK_COMPARISON_INPUT_BYTES)?;
    if canonical.as_bytes() != observations {
        return Err(invalid(
            "task comparison observations are not exact canonical JSON",
        ));
    }
    let root = object(&value, "task comparison observation set")?;
    require_keys(
        root,
        &[
            "schema",
            "plan_sha256",
            "repository_head",
            "task",
            "corpus",
            "model",
            "observations",
        ],
        "task comparison observation set",
    )?;
    if root["schema"] != AGENT_TASK_COMPARISON_OBSERVATION_SET_SCHEMA {
        return Err(invalid("task comparison observation schema is unsupported"));
    }
    let task = identifier(root, "task")?;
    let corpus = identifier(root, "corpus")?;
    let model = identifier(root, "model")?;
    digest_field(root, "plan_sha256", false)?;
    require_commit(identifier(root, "repository_head")?)?;
    let rows = root["observations"]
        .as_array()
        .filter(|rows| (2..=3).contains(&rows.len()))
        .ok_or_else(|| lanes("task comparison requires two or three observation lanes"))?;
    let mut by_lane = BTreeMap::new();
    for row in rows {
        let row = validate_row(row, root, task, corpus, model)?;
        let lane = row["lane"].as_str().expect("validated lane").to_owned();
        if !REQUIRED_LANES.contains(&lane.as_str()) && lane != ZERO_LANE {
            return Err(lanes("task comparison observation lane is unsupported"));
        }
        if by_lane.insert(lane, row).is_some() {
            return Err(lanes("task comparison observation lane is duplicated"));
        }
    }
    if !REQUIRED_LANES
        .iter()
        .all(|lane| by_lane.contains_key(*lane))
    {
        return Err(lanes(
            "task comparison requires semantic and source-first observations",
        ));
    }

    let semantic = &by_lane[REQUIRED_LANES[0]];
    let source = &by_lane[REQUIRED_LANES[1]];
    for right in by_lane
        .values()
        .filter(|row| row["lane"] != REQUIRED_LANES[0])
    {
        for field in [
            "trial",
            "state",
            "model",
            "tokenizer",
            "model_configuration",
            "harness",
            "host",
            "prompt_sha256",
        ] {
            if semantic[field] != right[field] {
                return Err(binding(
                    "task comparison paired observation bindings disagree",
                ));
            }
        }
    }
    let mut comparisons = vec![comparison(semantic, Some(source), REQUIRED_LANES[1])?];
    comparisons.push(comparison(semantic, by_lane.get(ZERO_LANE), ZERO_LANE)?);
    let ordered = [REQUIRED_LANES[0], REQUIRED_LANES[1], ZERO_LANE]
        .iter()
        .filter_map(|lane| by_lane.get(*lane).cloned())
        .collect::<Vec<_>>();
    let mut report = json!({
        "schema":AGENT_TASK_COMPARISON_NORMALIZED_REPORT_SCHEMA,
        "input_sha256":expected_sha256,
        "plan_sha256":root["plan_sha256"],
        "repository_head":root["repository_head"],
        "task":task,
        "corpus":corpus,
        "model":model,
        "observations":ordered,
        "comparisons":comparisons,
        "evidence_class":"caller_supplied_completed_external_observations",
        "superiority":"not_assessed",
        "source_authority":false,
        "model_execution":false,
        "tool_execution":false,
        "validation_execution":false,
        "filesystem_observation":false,
        "network_observation":false,
        "runtime_observation":false,
        "publication_authority":false,
        "nonclaims":[
            "caller_supplied_metrics_are_not_independently_observed_or_verified",
            "missing_lanes_or_metrics_are_never_imputed",
            "descriptive_differences_are_not_productivity_causality_or_superiority_evidence",
            "different_correctness_or_validation_outcomes_are_not_economically_compared",
            "artifact_metadata_and_references_are_checked_but_artifact_bytes_are_not_rehashed"
            ,"plan_and_task_documents_are_not_supplied_or_replayed"
        ]
    });
    let core = render(&report, MAX_AGENT_TASK_COMPARISON_REPORT_BYTES)?;
    report["report_revision"] = json!(domain_digest(REPORT_DOMAIN, core.as_bytes()));
    render(&report, MAX_AGENT_TASK_COMPARISON_REPORT_BYTES)
}

fn validate_row<'a>(
    value: &'a Value,
    root: &Map<String, Value>,
    task: &str,
    corpus: &str,
    model: &str,
) -> Result<Value> {
    let row = object(value, "task comparison embedded observation record")?;
    require_keys(
        row,
        &[
            "schema",
            "lane",
            "task",
            "corpus",
            "tool",
            "model",
            "source_revision",
            "image_revision",
            "candidate_revision",
            "wall_time_ms",
            "protocol_bytes",
            "source_bytes",
            "observation",
            "observation_sha256",
        ],
        "task comparison embedded observation record",
    )?;
    if row["schema"] != "semaprax.agent-task-comparison-embedded-observation.v1" {
        return Err(invalid(
            "task comparison embedded observation schema is unsupported",
        ));
    }
    for (field, expected) in [("task", task), ("corpus", corpus), ("model", model)] {
        if identifier(row, field)? != expected {
            return Err(binding("task comparison observation identifier disagrees"));
        }
    }
    let lane = identifier(row, "lane")?;
    digest_field(row, "source_revision", false)?;
    digest_field(row, "image_revision", lane != "semaprax-graph-operational")?;
    digest_field(
        row,
        "candidate_revision",
        lane != "semaprax-graph-operational",
    )?;
    for metric in ["wall_time_ms", "protocol_bytes", "source_bytes"] {
        if row[metric].as_u64().is_none() {
            return Err(invalid(
                "task comparison wrapper metric is missing or invalid",
            ));
        }
    }
    let observation = row["observation"]
        .as_str()
        .filter(|bytes| !bytes.is_empty() && bytes.len() <= 1024 * 1024)
        .ok_or_else(|| capacity("embedded task comparison observation exceeds its bound"))?;
    require_digest(
        row["observation_sha256"]
            .as_str()
            .ok_or_else(|| binding("embedded observation digest is missing"))?,
    )?;
    if sha256(observation.as_bytes()) != row["observation_sha256"] {
        return Err(binding("embedded observation digest disagrees"));
    }
    let document: Value = serde_json::from_str(observation)
        .map_err(|_| invalid("embedded observation is not valid JSON"))?;
    if render(&document, MAX_AGENT_TASK_COMPARISON_INPUT_BYTES)?.as_bytes()
        != observation.as_bytes()
    {
        return Err(invalid("embedded observation is not exact canonical JSON"));
    }
    validate_existing_observation(&document, row, root)?;
    let mut normalized = value.clone();
    normalized
        .as_object_mut()
        .expect("validated wrapper")
        .remove("observation");
    normalized["metrics"] = document["metrics"].clone();
    normalized["outcome"] = document["outcome"].clone();
    normalized["acceptance"] = document["acceptance"].clone();
    for field in [
        "trial",
        "state",
        "tokenizer",
        "model_configuration",
        "harness",
        "host",
        "toolchain",
        "prompt_sha256",
    ] {
        normalized[field] = document[field].clone();
    }
    normalized["correctness"] = json!(if document["outcome"] == "completed" {
        "passed"
    } else {
        "failed"
    });
    normalized["validation"] = json!("observed_not_independently_verified");
    Ok(normalized)
}

fn validate_existing_observation(
    value: &Value,
    wrapper: &Map<String, Value>,
    root: &Map<String, Value>,
) -> Result<()> {
    let observation = object(value, "embedded existing observation")?;
    require_keys(
        observation,
        &[
            "schema",
            "plan_sha256",
            "task",
            "lane",
            "trial",
            "state",
            "model",
            "tokenizer",
            "model_configuration",
            "harness",
            "host",
            "toolchain",
            "prompt_sha256",
            "artifacts",
            "metrics",
            "acceptance",
            "outcome",
        ],
        "embedded existing observation",
    )?;
    if observation["schema"] != "semaprax.agent-task-comparison-observation.v1"
        || observation["plan_sha256"] != root["plan_sha256"]
        || observation["task"] != wrapper["task"]
        || observation["lane"] != wrapper["lane"]
        || observation["model"] != wrapper["model"]
        || observation["toolchain"] != wrapper["tool"]
    {
        return Err(binding("embedded observation exact bindings disagree"));
    }
    if observation["trial"]
        .as_u64()
        .filter(|trial| *trial > 0)
        .is_none()
        || !matches!(observation["state"].as_str(), Some("cold" | "warm"))
        || !matches!(
            observation["outcome"].as_str(),
            Some("completed" | "failed" | "aborted")
        )
    {
        return Err(invalid(
            "embedded observation trial, state or outcome is invalid",
        ));
    }
    for field in [
        "tokenizer",
        "model_configuration",
        "harness",
        "host",
        "prompt_sha256",
    ] {
        embedded_text(observation, field)?;
    }
    digest_field(observation, "prompt_sha256", false)?;
    let artifacts = observation["artifacts"]
        .as_array()
        .filter(|rows| !rows.is_empty() && rows.len() <= 64)
        .ok_or_else(|| invalid("embedded observation artifact inventory is invalid"))?;
    let mut evidence = BTreeSet::new();
    let mut evidence_bytes = 0u64;
    for artifact in artifacts {
        let artifact = object(artifact, "embedded observation artifact")?;
        require_keys(
            artifact,
            &["id", "path", "bytes", "sha256", "kind"],
            "embedded observation artifact",
        )?;
        let id = identifier(artifact, "id")?;
        let bytes = artifact["bytes"]
            .as_u64()
            .filter(|bytes| *bytes <= 32 * 1024 * 1024);
        if !evidence.insert(id.to_owned()) || bytes.is_none() {
            return Err(invalid("embedded observation artifact binding is invalid"));
        }
        evidence_bytes = evidence_bytes.saturating_add(bytes.unwrap());
        if evidence_bytes > 64 * 1024 * 1024 {
            return Err(capacity(
                "embedded observation artifact bytes exceed their bound",
            ));
        }
        digest_field(artifact, "sha256", false)?;
        embedded_text(artifact, "path")?;
        embedded_text(artifact, "kind")?;
    }
    let metrics = object(&observation["metrics"], "embedded observation metrics")?;
    let expected = METRICS.iter().copied().collect::<BTreeSet<_>>();
    if metrics.keys().map(String::as_str).collect::<BTreeSet<_>>() != expected {
        return Err(invalid("embedded observation metric inventory disagrees"));
    }
    for metric in METRICS {
        let item = object(&metrics[metric], "embedded observation metric")?;
        require_keys(
            item,
            &["status", "value", "method", "evidence"],
            "embedded observation metric",
        )?;
        if item["status"] != "observed" || item["value"].as_u64().is_none() {
            return Err(invalid("embedded observation metric is not observed"));
        }
        embedded_text(item, "method")?;
        require_evidence(&item["evidence"], &evidence)?;
    }
    let acceptance = observation["acceptance"]
        .as_array()
        .filter(|rows| !rows.is_empty())
        .ok_or_else(|| invalid("embedded observation acceptance is absent"))?;
    let mut all_passed = true;
    for row in acceptance {
        let row = object(row, "embedded observation acceptance row")?;
        require_keys(
            row,
            &["id", "outcome", "evidence"],
            "embedded observation acceptance row",
        )?;
        embedded_text(row, "id")?;
        all_passed &= row["outcome"] == "passed";
        if !matches!(row["outcome"].as_str(), Some("passed" | "failed")) {
            return Err(invalid(
                "embedded observation acceptance outcome is invalid",
            ));
        }
        require_evidence(&row["evidence"], &evidence)?;
    }
    if (observation["outcome"] == "completed") != all_passed {
        return Err(binding(
            "embedded observation outcome disagrees with acceptance",
        ));
    }
    Ok(())
}

fn require_evidence(value: &Value, known: &BTreeSet<String>) -> Result<()> {
    if !value.as_array().is_some_and(|refs| {
        !refs.is_empty()
            && refs.iter().all(|reference| {
                reference
                    .as_str()
                    .is_some_and(|reference| known.contains(reference))
            })
    }) {
        return Err(binding(
            "embedded observation evidence reference is invalid",
        ));
    }
    Ok(())
}

fn comparison(left: &Value, right: Option<&Value>, right_lane: &str) -> Result<Value> {
    let Some(right) = right else {
        return Ok(json!({
            "left_lane":REQUIRED_LANES[0], "right_lane":right_lane,
            "status":"not_assessed_missing_observation", "normalized_deltas":null,
            "superiority":"not_assessed"
        }));
    };
    if left["outcome"] != right["outcome"] {
        return Ok(json!({
            "left_lane":REQUIRED_LANES[0], "right_lane":right_lane,
            "status":"not_assessed_outcomes_differ", "normalized_deltas":null,
            "left_outcome":{"correctness":left["correctness"],"validation":left["validation"]},
            "right_outcome":{"correctness":right["correctness"],"validation":right["validation"]},
            "superiority":"not_assessed"
        }));
    }
    let mut deltas = Map::new();
    for metric in METRICS {
        let left_value = left["metrics"][metric]["value"]
            .as_u64()
            .expect("validated metric");
        let right_value = right["metrics"][metric]["value"]
            .as_u64()
            .expect("validated metric");
        deltas.insert(
            metric.to_owned(),
            json!({
                "left":left_value,
                "right":right_value,
                "left_minus_right":i128::from(left_value)-i128::from(right_value),
                "unit":"count_or_declared_metric_unit"
            }),
        );
    }
    for metric in ["wall_time_ms", "protocol_bytes", "source_bytes"] {
        let left_value = left[metric].as_u64().expect("validated wrapper metric");
        let right_value = right[metric].as_u64().expect("validated wrapper metric");
        deltas.insert(
            metric.to_owned(),
            json!({
                "left":left_value,
                "right":right_value,
                "left_minus_right":i128::from(left_value)-i128::from(right_value),
                "unit":"milliseconds_or_bytes"
            }),
        );
    }
    Ok(json!({
        "left_lane":REQUIRED_LANES[0], "right_lane":right_lane,
        "status":"descriptive_only_matching_outcomes", "normalized_deltas":deltas,
        "superiority":"not_assessed"
    }))
}

fn object<'a>(value: &'a Value, label: &str) -> Result<&'a Map<String, Value>> {
    value
        .as_object()
        .ok_or_else(|| invalid(&format!("{label} must be an object")))
}

fn require_keys(object: &Map<String, Value>, keys: &[&str], label: &str) -> Result<()> {
    let expected = keys.iter().copied().collect::<BTreeSet<_>>();
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    if actual != expected {
        return Err(invalid(&format!("{label} keys disagree")));
    }
    Ok(())
}

fn identifier<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    object[field]
        .as_str()
        .filter(|value| {
            !value.is_empty() && value.len() <= MAX_AGENT_TASK_COMPARISON_IDENTIFIER_BYTES
        })
        .ok_or_else(|| invalid("task comparison identifier is invalid"))
}

fn embedded_text<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str> {
    object[field]
        .as_str()
        .filter(|value| !value.is_empty() && value.len() <= 65_536)
        .ok_or_else(|| invalid("embedded observation text is invalid"))
}

fn digest_field(object: &Map<String, Value>, field: &str, null: bool) -> Result<()> {
    if null {
        return if object[field].is_null() {
            Ok(())
        } else {
            Err(binding(
                "task comparison inapplicable revision must be null",
            ))
        };
    }
    object[field]
        .as_str()
        .ok_or_else(|| binding("task comparison revision binding is missing"))
        .and_then(require_digest)
}

fn require_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(binding("task comparison digest is invalid"));
    }
    Ok(())
}

fn require_commit(value: &str) -> Result<()> {
    if !matches!(value.len(), 40 | 64)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(binding("task comparison repository head is invalid"));
    }
    Ok(())
}

fn render(value: &Value, maximum: usize) -> Result<String> {
    let mut value = value.clone();
    value.sort_all_objects();
    let mut output = serde_json::to_string(&value)
        .map_err(|_| invalid("task comparison JSON cannot be rendered"))?;
    output.push('\n');
    if output.len() > maximum {
        return Err(capacity("task comparison JSON exceeds its byte bound"));
    }
    Ok(output)
}

fn sha256(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn invalid(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "SPX-G485",
        message,
        crate::ast::Span::default(),
    )]
}
fn capacity(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "SPX-G486",
        message,
        crate::ast::Span::default(),
    )]
}
fn binding(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "SPX-G487",
        message,
        crate::ast::Span::default(),
    )]
}
fn lanes(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "SPX-G488",
        message,
        crate::ast::Span::default(),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical(mut value: Value) -> String {
        value.sort_all_objects();
        serde_json::to_string(&value).unwrap() + "\n"
    }

    fn observation(lane: &str, outcome: &str, base: u64) -> String {
        let metrics = METRICS
            .iter()
            .map(|metric| {
                (
                    (*metric).to_owned(),
                    json!({"status":"observed","value":base,
                "method":"external_counter","evidence":["ledger"]}),
                )
            })
            .collect::<Map<_, _>>();
        canonical(json!({
            "schema":"semaprax.agent-task-comparison-observation.v1",
            "plan_sha256":format!("sha256:{}","1".repeat(64)),
            "task":"signature-migration-v1","lane":lane,"trial":1,"state":"cold",
            "model":"model:v1","tokenizer":"provider:v1","model_configuration":"fixed:v1",
            "harness":"harness:v1","host":"host:v1","toolchain":format!("{lane}:tool:v1"),
            "prompt_sha256":format!("sha256:{}","2".repeat(64)),
            "artifacts":[{"id":"ledger","path":"ledger.json","bytes":1,
                "sha256":format!("sha256:{}","3".repeat(64)),"kind":"typed_event_ledger"}],
            "metrics":metrics,
            "acceptance":[{"id":"correct","outcome":if outcome=="completed"{"passed"}else{"failed"},"evidence":["ledger"]}],
            "outcome":outcome
        }))
    }

    fn row(lane: &str, outcome: &str, base: u64) -> Value {
        let observation = observation(lane, outcome, base);
        json!({
            "schema":"semaprax.agent-task-comparison-embedded-observation.v1",
            "lane":lane,"task":"signature-migration-v1","corpus":"agent-task-comparison-v1",
            "tool":format!("{lane}:tool:v1"),"model":"model:v1",
            "source_revision":format!("sha256:{}","4".repeat(64)),
            "image_revision":if lane==REQUIRED_LANES[0]{json!(format!("sha256:{}","5".repeat(64)))}else{Value::Null},
            "candidate_revision":if lane==REQUIRED_LANES[0]{json!(format!("sha256:{}","6".repeat(64)))}else{Value::Null},
            "wall_time_ms":base,"protocol_bytes":base,"source_bytes":base,
            "observation_sha256":sha256(observation.as_bytes()),"observation":observation
        })
    }

    fn input(rows: Vec<Value>) -> String {
        canonical(json!({
            "schema":AGENT_TASK_COMPARISON_OBSERVATION_SET_SCHEMA,
            "plan_sha256":format!("sha256:{}","1".repeat(64)),
            "repository_head":"a".repeat(40),"task":"signature-migration-v1",
            "corpus":"agent-task-comparison-v1","model":"model:v1","observations":rows
        }))
    }

    #[test]
    fn exact_existing_observations_are_descriptive_and_zero_is_not_inferred() {
        let input = input(vec![
            row(REQUIRED_LANES[0], "completed", 4),
            row(REQUIRED_LANES[1], "completed", 7),
        ]);
        let report =
            normalize_task_comparison_observations(input.as_bytes(), &sha256(input.as_bytes()))
                .unwrap();
        let report: Value = serde_json::from_str(&report).unwrap();
        assert_eq!(report["superiority"], "not_assessed");
        assert_eq!(
            report["comparisons"][0]["normalized_deltas"]["tool_calls"]["left_minus_right"],
            -3
        );
        assert_eq!(
            report["comparisons"][1]["status"],
            "not_assessed_missing_observation"
        );
    }

    #[test]
    fn duplicate_lanes_and_different_outcomes_never_become_comparative_evidence() {
        let duplicate = input(vec![
            row(REQUIRED_LANES[0], "completed", 1),
            row(REQUIRED_LANES[0], "completed", 2),
        ]);
        assert_eq!(
            normalize_task_comparison_observations(
                duplicate.as_bytes(),
                &sha256(duplicate.as_bytes())
            )
            .unwrap_err()[0]
                .code,
            "SPX-G488"
        );
        let differing = input(vec![
            row(REQUIRED_LANES[0], "completed", 1),
            row(REQUIRED_LANES[1], "failed", 2),
        ]);
        let report: Value = serde_json::from_str(
            &normalize_task_comparison_observations(
                differing.as_bytes(),
                &sha256(differing.as_bytes()),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            report["comparisons"][0]["status"],
            "not_assessed_outcomes_differ"
        );
        assert!(report["comparisons"][0]["normalized_deltas"].is_null());
    }
}
