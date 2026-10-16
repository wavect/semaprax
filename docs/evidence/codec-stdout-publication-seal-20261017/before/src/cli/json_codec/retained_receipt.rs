//! Additive single-codec receipt bundle route; old source publication is unchanged.
use super::*;
use semaprax::project::{with_authenticated_json_codec_receipt, JsonCodecPlanEntry};

pub(super) fn run(args: &[String], report: impl Fn(&[Diagnostic]) -> u8) -> Result<(), u8> {
    if args
        .iter()
        .filter(|arg| arg.as_str() == "--retained-receipt")
        .count()
        != 1
    {
        eprintln!("{USAGE} --retained-receipt");
        return Err(2);
    }
    let ordinary = args
        .iter()
        .filter(|arg| arg.as_str() != "--retained-receipt")
        .cloned()
        .collect::<Vec<_>>();
    // Reuse exact ordinary selector/bound parsing; plan/explain/unknown flags refuse.
    let options = parse(&ordinary)?;
    let project = super::super::project::resolve_positional(options.project);
    let entry = JsonCodecPlanEntry {
        source_path: options.source,
        record_id: options.record,
        profile: options.profile,
    };
    let receipt = with_authenticated_json_codec_receipt(&project, &[entry])
        .map_err(|errors| report(&errors))?;
    let mut bytes = serde_json::to_vec(&receipt)
        .map_err(|_| report(&[error("cannot encode retained codec receipt")]))?;
    bytes.push(b'\n');
    // A single complete JSON artifact avoids partial source/sidecar publication.
    // Publication success is the actual return value, not a prewritten receipt claim.
    publish_new(&options.output, &bytes).map_err(|failure| report(&[failure]))
}
#[cfg(test)]
#[path = "retained_receipt/tests.rs"]
mod tests;
