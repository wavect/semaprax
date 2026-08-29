use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::bounded_output;
use crate::diagnostic::Diagnostic;

use super::{
    consistency_error, limit_error, report_quote_json as quote_json, PackageReportV2Options,
    MAX_OUTPUT_BYTES, MAX_SOURCE_BYTES, SCHEMA,
};

pub(super) const SOURCE_SCHEMA: &str = "semaprax.canonical-source.v1";
pub(super) const SOURCE_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report-v2.source.v1\0";
pub(super) const PAYLOAD_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report-v2.payload.v1\0";
pub(super) const CONTRACT_DIGEST_DOMAIN: &[u8] = b"semaprax.package-report-v2.contract-fact.v1\0";

macro_rules! bf {
    ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) };
}

pub(super) struct ParsedSubject {
    pub(super) source: String,
    pub(super) options: PackageReportV2Options,
}

pub(super) fn parse_subject(envelope: &str) -> Result<ParsedSubject, Diagnostic> {
    parse_subject_impl(envelope, false)
}

pub(super) fn parse_subject_for_resolution(envelope: &str) -> Result<ParsedSubject, Diagnostic> {
    parse_subject_impl(envelope, true)
}

fn parse_subject_impl(
    envelope: &str,
    preserve_bound_diagnostic: bool,
) -> Result<ParsedSubject, Diagnostic> {
    if envelope.len() > MAX_OUTPUT_BYTES {
        return Err(if preserve_bound_diagnostic {
            limit_error("v2 envelope exceeds the frozen output bound")
        } else {
            consistency_error(
                "v2 envelope must be bounded compact UTF-8 without BOM, CR, or terminal LF",
            )
        });
    }
    if envelope.is_empty()
        || envelope.starts_with('\u{feff}')
        || envelope.ends_with('\n')
        || envelope.contains('\r')
    {
        return Err(consistency_error(
            "v2 envelope must be bounded compact UTF-8 without BOM, CR, or terminal LF",
        ));
    }
    let value: Value = serde_json::from_str(envelope)
        .map_err(|error| consistency_error(bf!("v2 envelope is not valid JSON: {error}")))?;
    let object = value
        .as_object()
        .ok_or_else(|| consistency_error("v2 envelope must be an object"))?;
    if object.len() != 4
        || !object.contains_key("schema")
        || !object.contains_key("digest")
        || !object.contains_key("bytes")
        || !object.contains_key("payload")
    {
        return Err(consistency_error(
            "v2 envelope must contain exactly schema,digest,bytes,payload",
        ));
    }
    if object["schema"].as_str() != Some(SCHEMA) {
        return Err(consistency_error(bf!(
            "v2 envelope schema must be {SCHEMA}"
        )));
    }
    let declared_digest = object["digest"]
        .as_str()
        .ok_or_else(|| consistency_error("v2 envelope digest must be a string"))?;
    let declared_bytes = object["bytes"]
        .as_u64()
        .ok_or_else(|| consistency_error("v2 envelope bytes must be an unsigned integer"))?;
    const PAYLOAD_MARKER: &str = "\"payload\":";
    let offset = envelope
        .find(PAYLOAD_MARKER)
        .ok_or_else(|| consistency_error("v2 envelope payload member is missing"))?
        + PAYLOAD_MARKER.len();
    if !envelope.ends_with('}') {
        return Err(consistency_error("v2 envelope must end with `}`"));
    }
    let payload = &envelope[offset..envelope.len() - 1];
    if !payload.starts_with('{') || !payload.ends_with('}') {
        return Err(consistency_error("v2 payload must be an object"));
    }
    if declared_bytes != payload.len() as u64 {
        return Err(consistency_error(
            "v2 payload byte count does not match its exact bytes",
        ));
    }
    if declared_digest != domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes()) {
        return Err(consistency_error(
            "v2 payload digest does not match its exact bytes",
        ));
    }
    let payload_value = object["payload"]
        .as_object()
        .ok_or_else(|| consistency_error("v2 payload must decode as an object"))?;
    if payload_value.get("schema").and_then(Value::as_str) != Some(SCHEMA) {
        return Err(consistency_error(bf!("v2 payload schema must be {SCHEMA}")));
    }
    let limits = payload_value
        .get("limits")
        .and_then(Value::as_object)
        .ok_or_else(|| consistency_error("v2 limits must be an object"))?;
    let requested_max_bytes = limits
        .get("requested_max_bytes")
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            consistency_error("v2 requested_max_bytes must be a host-sized unsigned integer")
        })?;
    let options = PackageReportV2Options::new(requested_max_bytes).map_err(|_| {
        if preserve_bound_diagnostic {
            limit_error("v2 requested_max_bytes is outside the frozen option range")
        } else {
            consistency_error("v2 requested_max_bytes is outside the frozen option range")
        }
    })?;
    let source = payload_value
        .get("source")
        .and_then(Value::as_object)
        .ok_or_else(|| consistency_error("v2 source subject must be an object"))?;
    if source.get("schema").and_then(Value::as_str) != Some(SOURCE_SCHEMA) {
        return Err(consistency_error(bf!(
            "v2 source subject schema must be {SOURCE_SCHEMA}"
        )));
    }
    let source_text = source
        .get("text")
        .and_then(Value::as_str)
        .ok_or_else(|| consistency_error("v2 source subject text must be a string"))?;
    if source_text.len() > MAX_SOURCE_BYTES {
        return Err(if preserve_bound_diagnostic {
            limit_error(bf!("v2 canonical source exceeds {MAX_SOURCE_BYTES} bytes"))
        } else {
            consistency_error(bf!("v2 canonical source exceeds {MAX_SOURCE_BYTES} bytes"))
        });
    }
    Ok(ParsedSubject {
        source: source_text.to_owned(),
        options,
    })
}

pub(super) fn render_envelope(payload: &str) -> String {
    bf!(
        "{{\"schema\":{},\"digest\":{},\"bytes\":{},\"payload\":{}}}",
        quote_json(SCHEMA),
        quote_json(&domain_digest(PAYLOAD_DIGEST_DOMAIN, payload.as_bytes())),
        payload.len(),
        payload
    )
}

pub(super) fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    bf!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}
