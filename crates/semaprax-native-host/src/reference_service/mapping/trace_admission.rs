//! Optional inbound trace metadata admission, before any route effects.
use super::{error, DecisionEngine, HttpExchange, PendingResponse};

pub(super) fn admit(
    decisions: &DecisionEngine<'_>,
    exchange: &HttpExchange,
) -> Result<(), PendingResponse> {
    let mut headers = exchange
        .headers
        .iter()
        .filter(|(name, _)| name.eq_ignore_ascii_case("traceparent"));
    let Some((_, value)) = headers.next() else {
        return Ok(());
    };
    if headers.next().is_some() {
        return Err(error(400, "trace_not_admitted", None));
    }
    let bytes = value.as_bytes();
    // Framing belongs to this fixed version-00 host profile. Field semantics
    // (lowercase hex, exact widths, nonzero IDs) belong to checked source.
    if bytes.len() != 55 || &bytes[..3] != b"00-" || bytes[35] != b'-' || bytes[52] != b'-' {
        return Err(error(400, "trace_not_admitted", None));
    }
    // These are public trace identifiers/flags, never held credential fields.
    // This classification is not secret-content scanning. No trace is created,
    // persisted, echoed, or propagated by admitting the incoming metadata.
    match decisions.trace_context_is_admitted(&bytes[3..35], &bytes[36..52], &bytes[53..55], false)
    {
        Ok(true) => Ok(()),
        Ok(false) => Err(error(400, "trace_not_admitted", None)),
        Err(_) => Err(error(500, "decision_failed", None)),
    }
}
