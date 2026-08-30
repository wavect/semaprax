//! Session-local rejected-intent association; never diagnostics on invalid HIR.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;

const REPORT_SCHEMA: &str = "semaprax.project-candidate-symbol-diagnostics.v1";
const CHUNK_SCHEMA: &str = "semaprax.image-symbol-diagnostics-chunk.v1";
const MAX_ATTEMPTS_CONSIDERED: usize = 16;
const MAX_REPAIR_DISCOVERIES: usize = 4;
const MAX_REPORT_BYTES: usize = 1024 * 1024;
const REPORT_DOMAIN: &[u8] = b"semaprax.project-candidate-symbol-diagnostics.report.v1\0";

pub(super) fn prepare(
    params: &Map<String, Value>,
    image: &ProjectSemanticImage,
    registry: &candidates::Registry,
) -> Result<Value, Vec<Diagnostic>> {
    if text(params, "image_revision") != image.image_digest() {
        return Err(failure(
            "SPX-G243",
            "symbol diagnostic image revision is stale",
        ));
    }
    let offset = number(params, "offset", 0);
    let chunk_bytes = number(params, "chunk_bytes", 16_384);
    let expected = params
        .get("expected_report_revision")
        .and_then(Value::as_str);
    if offset > 0 && expected.is_none() {
        return Err(failure(
            "SPX-G243",
            "symbol diagnostic continuation requires expected_report_revision",
        ));
    }
    if !(1024..=65_536).contains(&chunk_bytes) {
        return Err(failure(
            "SPX-G241",
            "symbol diagnostic chunk size is outside its bounds",
        ));
    }
    let candidate = registry.candidate(text(params, "candidate_revision"))?;
    let target = text(params, "target");
    let provenance = candidate.diagnostic_symbol(candidate.candidate_digest(), target)?;
    let mut considered = 0;
    let mut matched = Vec::new();
    for attempt in registry.retained_attempts() {
        considered += 1;
        if considered > MAX_ATTEMPTS_CONSIDERED {
            return Err(failure(
                "SPX-G242",
                "symbol diagnostic attempt inventory exceeds its bound",
            ));
        }
        if attempt.matches_symbol(candidate.candidate_digest(), target) {
            matched.push(attempt);
            if matched.len() > MAX_REPAIR_DISCOVERIES {
                // Reject before any repair apply. Never silently omit attempts
                // or advertise unvalidated repair availability to save work.
                return Err(failure("SPX-G242","symbol diagnostics exceed four matching repair discoveries; inspect individual attempts"));
            }
        }
    }
    let mut attempts = Vec::with_capacity(matched.len());
    let mut nested_bytes = 0usize;
    for attempt in &matched {
        let facts = attempt.symbol_diagnostics(
            attempt.attempt_digest(),
            candidate.candidate_digest(),
            target,
        )?;
        nested_bytes = nested_bytes.saturating_add(facts.len());
        if nested_bytes > MAX_REPORT_BYTES {
            return Err(failure(
                "SPX-G242",
                "symbol diagnostic facts exceed aggregate report capacity",
            ));
        }
        attempts.push(serde_json::from_str::<Value>(&facts).map_err(|_| {
            failure(
                "SPX-G241",
                "compiler symbol diagnostic facts are invalid JSON",
            )
        })?);
    }
    let report = render(json!({
        "schema":REPORT_SCHEMA,"image_revision":image.image_digest(),
        "current_project_revision":image.revision().project_revision(),
        "candidate_revision":candidate.candidate_digest(),
        "candidate_project_revision":candidate.revision().project_revision(),
        "target":target,"target_provenance":provenance,
        "candidate_state":"admitted","candidate_diagnostic_inventory":"not_retained",
        "scope":"session_retained_rejected_intents_with_exact_candidate_and_target",
        "availability":if matched.is_empty(){"no_matching_retained_rejected_attempts"}else{"matching_retained_rejected_attempts"},
        "attempts":attempts,"matching_attempt_count":matched.len(),
        "work":{"retained_attempts_considered":considered,"repair_catalog_evaluations":matched.len(),
            "repair_candidate_apply_upper_bound":matched.len()},
        "limits":{"retained_attempts_considered":MAX_ATTEMPTS_CONSIDERED,
            "repair_catalog_evaluations":MAX_REPAIR_DISCOVERIES,"report_bytes":MAX_REPORT_BYTES},
        "source_authority":false,"tests":"not_run",
        "nonclaims":["empty_scope_is_not_absence_of_all_diagnostics_or_warnings",
            "association_is_intent_target_not_diagnostic_causality",
            "diagnostic_spans_do_not_identify_verified_candidate_expressions",
            "no_invalid_source_or_checked_image","no_automatic_repair_or_repair_authority",
            "no_attempt_persistence_or_remapping_after_refresh"]
    }))?;
    let mut hash = Sha256::new();
    hash.update(REPORT_DOMAIN);
    hash.update((report.len() as u64).to_le_bytes());
    hash.update(report.as_bytes());
    let revision = format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()));
    if expected.is_some_and(|expected| expected != revision) {
        return Err(failure(
            "SPX-G243",
            "symbol diagnostic report changed; restart from offset zero",
        ));
    }
    if offset > report.len() || !report.is_char_boundary(offset) {
        return Err(failure(
            "SPX-G241",
            "symbol diagnostic offset is outside the exact UTF8 report",
        ));
    }
    let mut end = offset.saturating_add(chunk_bytes).min(report.len());
    while !report.is_char_boundary(end) {
        end -= 1;
    }
    if end == offset && offset < report.len() {
        return Err(failure(
            "SPX-G241",
            "symbol diagnostic chunk cannot hold the next UTF8 character",
        ));
    }
    Ok(json!({"schema":CHUNK_SCHEMA,"report_schema":REPORT_SCHEMA,
        "image_revision":image.image_digest(),"candidate_revision":candidate.candidate_digest(),
        "target":target,"report_revision":revision,"offset":offset,"total_bytes":report.len(),
        "chunk":&report[offset..end],"next_offset":(end<report.len()).then_some(end),
        "source_authority":false}))
}

fn render(mut value: Value) -> Result<String, Vec<Diagnostic>> {
    struct Sink(Vec<u8>);
    impl Write for Sink {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > MAX_REPORT_BYTES.saturating_sub(self.0.len()) {
                return Err(std::io::Error::other("symbol diagnostic report capacity"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    value.sort_all_objects();
    let mut sink = Sink(Vec::new());
    serde_json::to_writer(&mut sink, &value).map_err(|_| {
        failure(
            "SPX-G242",
            "symbol diagnostic report exceeds its byte bound",
        )
    })?;
    sink.write_all(b"\n").map_err(|_| {
        failure(
            "SPX-G242",
            "symbol diagnostic report exceeds its byte bound",
        )
    })?;
    String::from_utf8(sink.0)
        .map_err(|_| failure("SPX-G241", "symbol diagnostic report is not UTF8"))
}
