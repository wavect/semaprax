//! The existing closed codec-plan decoder shared by bundle and starter routes.
use super::{parse_json_codec_profile, JsonCodecPlanEntry};
use crate::diagnostic::Diagnostic;
use serde::Deserialize;
const MAX_PLAN_BYTES: u64 = 16_384;
fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-J181", message)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    schema: String,
    #[serde(default)]
    expected_project_revision: Option<String>,
    entries: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    source: String,
    record: String,
    profile: String,
    #[serde(default)]
    max_string_bytes: Option<usize>,
    #[serde(default)]
    max_array_items: Option<usize>,
}

fn decode(bytes: &[u8]) -> Result<Plan, Diagnostic> {
    if bytes.len() > MAX_PLAN_BYTES as usize {
        return Err(error("codec plan exceeds 16384 bytes"));
    }
    let plan: Plan =
        serde_json::from_slice(bytes).map_err(|_| error("invalid closed codec plan JSON"))?;
    if plan.schema != "semaprax.json-codec-plan.v1" || !(1..=8).contains(&plan.entries.len()) {
        return Err(error("codec plan requires schema v1 and 1..8 entries"));
    }
    Ok(plan)
}

pub fn decode_json_codec_plan(
    bytes: &[u8],
) -> Result<(Option<String>, Vec<JsonCodecPlanEntry>), Diagnostic> {
    let plan = decode(bytes)?;
    let mut entries = Vec::new();
    for entry in plan.entries {
        let bytes = entry.max_string_bytes.map(|n| n.to_string());
        let items = entry.max_array_items.map(|n| n.to_string());
        let selector = if entry.profile == "flat-scalars.v1" {
            None
        } else {
            Some(entry.profile.as_str())
        };
        let profile = parse_json_codec_profile(selector, bytes.as_deref(), items.as_deref())
            .ok_or_else(|| error("invalid codec member profile or bounds"))?;
        entries.push(JsonCodecPlanEntry {
            source_path: entry.source,
            record_id: entry.record,
            profile,
        });
    }
    Ok((plan.expected_project_revision, entries))
}
