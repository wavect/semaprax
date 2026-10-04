//! Protocol-aware usage normalization. Provider-native usage objects are the
//! only input; the host, not the adapter or the model text, produces the
//! normalized categories. A category the protocol did not report stays `None`
//! (unknown), never zero.

use crate::endpoint::Protocol;
use serde_json::{json, Map, Value};

/// Normalized token categories of one request. `output` includes `reasoning`
/// (a subset detail, never another billable copy); `cache_write` includes
/// `cache_write_1h`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Whole input as the protocol makes it known (inclusive of the cache
    /// categories); present even when the cache split is unknown.
    pub input_total: Option<u64>,
    pub uncached_input: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    pub cache_write_1h: Option<u64>,
    pub output: Option<u64>,
    pub reasoning: Option<u64>,
}

impl Usage {
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
    pub fn to_json(&self) -> Value {
        let f = |o: Option<u64>| o.map_or(json!("unknown"), |n| json!(n));
        json!({"input_total": f(self.input_total), "uncached_input": f(self.uncached_input), "cache_read": f(self.cache_read),
               "cache_write": f(self.cache_write), "cache_write_1h": f(self.cache_write_1h),
               "output": f(self.output), "reasoning": f(self.reasoning)})
    }
    pub fn from_json(v: &Value) -> Self {
        let g = |k: &str| v.get(k).and_then(Value::as_u64);
        Self {
            input_total: g("input_total"),
            uncached_input: g("uncached_input"),
            cache_read: g("cache_read"),
            cache_write: g("cache_write"),
            cache_write_1h: g("cache_write_1h"),
            output: g("output"),
            reasoning: g("reasoning"),
        }
    }
}

fn first(u: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|k| u.get(*k).and_then(Value::as_u64))
}

fn nested(u: &Value, objs: &[&str], key: &str) -> Option<u64> {
    objs.iter()
        .find_map(|o| u.get(*o).and_then(|d| d.get(key)).and_then(Value::as_u64))
}

/// Normalize one provider-native usage object.
///
/// OpenAI (Responses `input_tokens`, Chat `prompt_tokens`): the input figure
/// is inclusive of cached reads and the output figure inclusive of reasoning.
/// Anthropic: `input_tokens` excludes cache reads and cache creation, which
/// are reported beside it; output includes thinking. OpenAI has no cache-write
/// category, so it is a known zero there.
pub fn normalize(protocol: Protocol, native: &Value) -> Usage {
    if !native.is_object() {
        return Usage::default();
    }
    match protocol {
        Protocol::AnthropicMessages => {
            let w5 = nested(native, &["cache_creation"], "ephemeral_5m_input_tokens");
            let w1 = nested(native, &["cache_creation"], "ephemeral_1h_input_tokens");
            let split = match (w5, w1) {
                (Some(a), Some(b)) => a.checked_add(b),
                (None, Some(b)) => Some(b),
                (Some(a), None) => Some(a),
                _ => None,
            };
            let uncached = first(native, &["input_tokens"]);
            let read = first(native, &["cache_read_input_tokens"]);
            let write = first(native, &["cache_creation_input_tokens"]).or(split);
            // Exclusive protocol: the whole input is the sum of every part.
            let total = uncached
                .zip(read)
                .zip(write)
                .and_then(|((a, b), c)| a.checked_add(b)?.checked_add(c));
            Usage {
                input_total: total,
                uncached_input: uncached,
                cache_read: read,
                cache_write: write,
                cache_write_1h: w1,
                output: first(native, &["output_tokens"]),
                reasoning: None,
            }
        }
        Protocol::Responses | Protocol::ChatCompletions => {
            let input = first(native, &["input_tokens", "prompt_tokens"]);
            let cached = nested(
                native,
                &["input_tokens_details", "prompt_tokens_details"],
                "cached_tokens",
            );
            let output = first(native, &["output_tokens", "completion_tokens"]);
            let reasoning = nested(
                native,
                &["output_tokens_details", "completion_tokens_details"],
                "reasoning_tokens",
            );
            Usage {
                input_total: input,
                // An inconsistent report (cached above the inclusive input) is
                // not repaired: the uncached figure stays unknown.
                uncached_input: match (input, cached) {
                    (Some(i), Some(c)) => i.checked_sub(c),
                    _ => None,
                },
                cache_read: cached,
                cache_write: input.map(|_| 0),
                cache_write_1h: None,
                output,
                reasoning: match (reasoning, output) {
                    (Some(r), Some(o)) if r > o => None,
                    (r, _) => r,
                },
            }
        }
    }
}

/// Recursive object merge: later keys win, absent keys are retained, so a
/// partial terminal update (output only) never erases an earlier input count
/// and a repeated cumulative snapshot is idempotent.
pub fn merge_native(into: &mut Value, update: &Value) {
    match (into.as_object_mut(), update.as_object()) {
        (Some(a), Some(b)) => {
            for (k, v) in b {
                if v.is_null() {
                    continue;
                }
                match a.get_mut(k) {
                    Some(slot) if slot.is_object() && v.is_object() => merge_native(slot, v),
                    _ => {
                        a.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        _ => {
            if update.is_object() {
                *into = update.clone();
            }
        }
    }
}

/// The usage object inside one stream event: `usage`, `response.usage`
/// (Responses), or `message.usage` (Anthropic `message_start`).
pub fn event_usage(ev: &Value) -> Option<&Value> {
    ev.get("usage")
        .or_else(|| ev.pointer("/response/usage"))
        .or_else(|| ev.pointer("/message/usage"))
        .filter(|u| u.is_object())
}

/// Merge usage updates of one streamed reply into one native object.
pub fn merge_stream<'a>(updates: impl IntoIterator<Item = &'a Value>) -> Value {
    let mut acc = json!({});
    for u in updates {
        merge_native(&mut acc, u);
    }
    acc
}

/// Keep only non-negative integer leaves (bounded depth): enough to audit the
/// normalization, never text, prompts or secrets.
pub fn numeric_only(v: &Value, depth: u8) -> Value {
    let Some(o) = v.as_object() else {
        return Value::Null;
    };
    let mut out = Map::new();
    for (k, x) in o.iter().take(64) {
        if x.is_u64() {
            out.insert(k.chars().take(64).collect(), x.clone());
        } else if x.is_object() && depth > 0 {
            let n = numeric_only(x, depth - 1);
            if n.as_object().is_some_and(|m| !m.is_empty()) {
                out.insert(k.chars().take(64).collect(), n);
            }
        }
    }
    Value::Object(out)
}
