//! Bounded plan decoding and one no-overwrite bundle publication.
use super::*;
use semaprax::project::{derive_json_codec_plan, JsonCodecPlanEntry};
use serde::Deserialize;
use std::io::Read as _;

const USAGE: &str = "json-codec <project> --plan <plan-json> --output <new-bundle>";
const MAX_PLAN_BYTES: u64 = 16_384;

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

pub(super) fn run(args: &[String], report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    let usage = || {
        eprintln!("{USAGE}");
        2
    };
    if args.len() != 5 || args[0].is_empty() || args[0].starts_with('-') {
        return Err(usage());
    }
    let mut input = None;
    let mut output = None;
    for pair in args[1..].as_chunks::<2>().0 {
        if pair[1].is_empty() || pair[1].starts_with('-') {
            return Err(usage());
        }
        match pair[0].as_str() {
            "--plan" if input.is_none() => input = Some(PathBuf::from(&pair[1])),
            "--output" if output.is_none() => output = Some(PathBuf::from(&pair[1])),
            _ => return Err(usage()),
        }
    }
    let input = input.ok_or_else(usage)?;
    let output = output.ok_or_else(usage)?;
    let mut bytes = Vec::new();
    fs::File::open(input)
        .and_then(|f| f.take(MAX_PLAN_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|e| report(&[error(format!("cannot read codec plan: {e}"))]))?;
    let plan = decode(&bytes).map_err(|e| report(&[e]))?;
    let mut entries = Vec::new();
    for entry in &plan.entries {
        let bytes = entry.max_string_bytes.map(|n| n.to_string());
        let items = entry.max_array_items.map(|n| n.to_string());
        let selector = if entry.profile == "flat-scalars.v1" {
            None
        } else {
            Some(entry.profile.as_str())
        };
        let profile = profile::parse(selector, bytes.as_deref(), items.as_deref())
            .ok_or_else(|| report(&[error("invalid codec member profile or bounds")]))?;
        entries.push(JsonCodecPlanEntry {
            source_path: entry.source.clone(),
            record_id: entry.record.clone(),
            profile,
        });
    }
    let project = super::super::project::resolve_positional(PathBuf::from(&args[0]));
    let bundle = semaprax::project::with_authenticated_project(&project, |snapshot| {
        let revision = snapshot.retain_revision();
        if plan
            .expected_project_revision
            .as_deref()
            .is_some_and(|expected| expected != revision.project_revision())
        {
            return Err(vec![error("codec plan authored Project revision is stale")]);
        }
        derive_json_codec_plan(&revision, &entries)
    })
    .map_err(|errors| report(&errors))?;
    let mut encoded = serde_json::to_vec(&bundle)
        .map_err(|e| report(&[error(format!("cannot encode checked codec bundle: {e}"))]))?;
    encoded.push(b'\n');
    publish_new(&output, &encoded).map_err(|e| report(&[e]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plan_json_is_closed_bounded_and_rejects_duplicate_fields() {
        let valid = br#"{"schema":"semaprax.json-codec-plan.v1","entries":[{"source":"src/schema.spx","record":"app.row","profile":"flat-scalars.v1"}]}"#;
        assert!(decode(valid).is_ok());
        for invalid in [
            br#"{"schema":"semaprax.json-codec-plan.v1","entries":[],"authority":true}"#.as_slice(),
            br#"{"schema":"semaprax.json-codec-plan.v1","schema":"semaprax.json-codec-plan.v1","entries":[]}"#.as_slice(),
            br#"{"schema":"semaprax.json-codec-plan.v1","entries":[{"source":"x","record":"r","profile":"flat-scalars.v1","unknown":1}]}"#.as_slice(),
        ] { assert!(decode(invalid).is_err()); }
        assert!(decode(&vec![b' '; MAX_PLAN_BYTES as usize + 1]).is_err());
    }
}
