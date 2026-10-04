//! Export of harness observations as `semaprax.token-observation.v1` rows, the
//! input of `scripts/token_report.py session`. Metadata only: bytes, digests,
//! counts and identities; never payload text. A stage that measured a named
//! tokenizer on both sides of a transform becomes a paired row (`tokens` and
//! `baselineTokens`); byte-only sizes stay `tokenizer_unavailable` and are
//! never presented as tokens.

use super::event::{Observation, Outcome, Role, TokenCount, TokenizerId};
use serde_json::{json, Value};

pub const ROW_SCHEMA: &str = "semaprax.token-observation.v1";

fn named(c: &Option<TokenCount>) -> Option<(&str, &str, u64)> {
    match c {
        Some(TokenCount {
            tokenizer: TokenizerId::Named { name, fingerprint },
            value,
        }) => Some((name, fingerprint, *value)),
        _ => None,
    }
}

fn bytes(c: &Option<TokenCount>) -> Option<u64> {
    match c {
        Some(TokenCount {
            tokenizer: TokenizerId::ByteOnly,
            value,
        }) => Some(*value),
        _ => None,
    }
}

/// One row per observation. `session` names the session; rows are ordered by `seq`.
pub fn rows(events: &[Observation], session: &str) -> Vec<Value> {
    events.iter().map(|e| row(e, session)).collect()
}

fn row(e: &Observation, session: &str) -> Value {
    // The measured side: `after` for a transform, `incurred` for a request.
    let (measured, baseline) = match e.role {
        Role::Transform => (&e.after, &e.before),
        _ => (&e.incurred, &None),
    };
    let tok = named(measured);
    let base = named(baseline).filter(|b| tok.is_some_and(|t| t.0 == b.0 && t.1 == b.1));
    let status = match (tok, base, measured.is_some()) {
        (Some(_), Some(_), _) => "measured",
        (Some(_), None, _) => "baseline_unavailable",
        (None, _, true) => "tokenizer_unavailable",
        (None, _, false) => "incomplete",
    };
    let size = bytes(measured)
        .or_else(|| bytes(&e.after))
        .or_else(|| bytes(&e.incurred));
    json!({
        "schema": ROW_SCHEMA,
        "eventId": format!("{}#{}", e.invocation_id, e.seq),
        "sessionId": session,
        "attemptSequence": 0,
        "deliverySequence": e.seq,
        "method": format!("{}:{}", e.capability, e.provider),
        "boundary": e.stage.as_str(),
        "subjectRevision": e.source_revision,
        "outcome": match e.outcome { Outcome::Ok => "success", Outcome::Failed => "error" },
        "status": status,
        "bytes": size,
        "digest": e.after_digest,
        "tokenizer": tok.map(|t| t.0),
        "tokenizerFingerprint": tok.map(|t| t.1),
        "tokens": tok.map(|t| t.2),
        "referenceKind": base.map(|_| "source_context"),
        "baselineTokens": base.map(|b| b.2),
    })
}

/// JSONL text of the rows (one canonical object per line).
pub fn to_jsonl(events: &[Observation], session: &str) -> String {
    let mut out = String::new();
    for r in rows(events, session) {
        out.push_str(&crate::json::canonical(&r));
        out.push('\n');
    }
    out
}
