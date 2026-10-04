//! Delivered-to-model measurement of one command view (HN-12). The count is of
//! the text the model is actually shown (view text plus its recovery reference),
//! never of provider-reported gain. With no named tokenizer the counts are
//! unavailable and no token saving is ever claimed.

use crate::observe::Tokenizer;
use serde_json::{json, Value};
use std::rc::Rc;

/// A named tokenizer lent to one execution (shared with the HN-11 helper's kind).
#[derive(Clone)]
pub struct ViewTokenizer(pub Rc<dyn Tokenizer>);

impl std::fmt::Debug for ViewTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ViewTokenizer({})", self.0.name())
    }
}

impl ViewTokenizer {
    pub fn count(&self, text: &str) -> Option<u64> {
        self.0.try_count(text).ok().map(|n| n as u64)
    }
}

pub fn count(t: Option<&ViewTokenizer>, text: &str) -> Option<u64> {
    t.and_then(|t| t.count(text))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measurement {
    pub decision: &'static str,
    /// `tokens` (named tokenizer) or `bytes-only` (no claim of token savings).
    pub basis: &'static str,
    pub tokenizer: Option<(String, String)>,
    pub raw_bytes: u64,
    pub delivered_bytes: u64,
    pub raw_tokens: Option<u64>,
    pub delivered_tokens: Option<u64>,
    /// What the provider's own view would have cost when it was rejected.
    pub rejected_view_tokens: Option<u64>,
    /// Wall time spent consulting the provider (overhead of the transform).
    pub overhead_ms: u64,
}

impl Measurement {
    /// Signed token saving; `None` whenever either count is unavailable.
    pub fn saved_tokens(&self) -> Option<i64> {
        Some(self.raw_tokens? as i64 - self.delivered_tokens? as i64)
    }
    pub fn to_json(&self) -> Value {
        json!({"decision": self.decision, "basis": self.basis,
               "tokenizer": self.tokenizer.as_ref().map(|(n, f)| json!({"name": n, "fingerprint": f})),
               "raw_bytes": self.raw_bytes, "delivered_bytes": self.delivered_bytes,
               "raw_tokens": self.raw_tokens, "delivered_tokens": self.delivered_tokens,
               "saved_tokens": self.saved_tokens(), "rejected_view_tokens": self.rejected_view_tokens,
               "overhead_ms": self.overhead_ms})
    }
}

/// Text the model sees for a view: the body plus its recovery reference.
pub fn delivered_text(text: &str, recovery: Option<&str>) -> String {
    match recovery {
        Some(h) => format!("{text}\n[raw recoverable: {h}]"),
        None => text.to_string(),
    }
}
