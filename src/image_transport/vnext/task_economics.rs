//! Exact-input normalization of caller-supplied completed task observations.

use super::*;
use crate::agent_economics::{
    normalize_task_comparison_observations, AGENT_TASK_COMPARISON_NORMALIZED_REPORT_SCHEMA,
};

pub(super) const PAYLOAD_SCHEMA: &str = "semaprax.image-agent-task-comparison-report.v1";
pub(super) const MAX_TRANSPORT_INPUT_BYTES: usize = 28 * 1024;
pub(super) const MAX_TRANSPORT_REPORT_BYTES: usize = 384 * 1024;

const METHOD: Method = Method {
    name: "agent/task-comparison",
    operation: Operation::VNext(Action::AgentTaskComparison),
    parameters: &[
        REVISION,
        Parameter {
            name: "observations",
            kind: ParameterKind::CanonicalJsonText(MAX_TRANSPORT_INPUT_BYTES),
            required: true,
        },
        Parameter {
            name: "observations_sha256",
            kind: ParameterKind::Digest,
            required: true,
        },
    ],
    query: true,
    payload_schema: PAYLOAD_SCHEMA,
};

pub(super) fn method() -> &'static Method {
    &METHOD
}

pub(super) fn prepare(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G489",
            "task comparison image revision is stale",
        ));
    }
    let report = normalize_task_comparison_observations(
        text(params, "observations").as_bytes(),
        text(params, "observations_sha256"),
    )?;
    if report.len() > MAX_TRANSPORT_REPORT_BYTES {
        return Err(failure(
            "SPX-G489",
            "task comparison report exceeds its transport bound",
        ));
    }
    Ok(json!({
        "schema":PAYLOAD_SCHEMA,
        "report_schema":AGENT_TASK_COMPARISON_NORMALIZED_REPORT_SCHEMA,
        "image_revision":image.image_digest(),
        "observations_sha256":text(params,"observations_sha256"),
        "report":report,
        "report_sha256":sha256(report.as_bytes()),
        "superiority":"not_assessed",
        "source_authority":false,
        "model_execution":false,
        "tool_execution":false,
        "validation_execution":false,
        "filesystem_observation":false,
        "network_observation":false,
        "runtime_observation":false,
        "publication_authority":false,
    }))
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}
