//! One request-budget calculation over the exact serialized model-visible
//! request (HN-11, `docs/HARNESS-WORKFLOW-V1.md`). Counts use the selected
//! model's explicitly mapped tokenizer; without one the count is `unknown` and
//! the host policy admits against the UTF-8 byte length, an upper bound for any
//! byte-level tokenizer, never labelled as measured tokens. No call is made
//! to estimate tokens.

use super::stages::Task;
use crate::decision::ModelPlan;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::canonical;
use crate::observe::TokenCount;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub use super::tokenizers::{CountCache, TokenizerSet, BUILTIN_TOKENIZER_NAMES as TOKENIZER_NAMES};

/// Explicit model-id prefix to tokenizer data (longest prefix wins). Anything
/// not listed is `unknown`; a name is never guessed.
pub const DEFAULT_MODEL_TOKENIZERS: &[(&str, &str)] = &[
    ("gpt-3.5", "cl100k_base"),
    ("gpt-4-", "cl100k_base"),
    ("gpt-4", "cl100k_base"),
    ("gpt-4o", "o200k_base"),
    ("gpt-4.1", "o200k_base"),
    ("gpt-5", "o200k_base"),
    ("o1", "o200k_base"),
    ("o3", "o200k_base"),
    ("o4", "o200k_base"),
    ("openai/gpt-4o", "o200k_base"),
    ("openai/gpt-4.1", "o200k_base"),
    ("openai/gpt-5", "o200k_base"),
];

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelTokenizerMap(BTreeMap<String, String>);

impl Default for ModelTokenizerMap {
    fn default() -> Self {
        Self(
            DEFAULT_MODEL_TOKENIZERS
                .iter()
                .map(|(p, t)| (p.to_string(), t.to_string()))
                .collect(),
        )
    }
}

impl ModelTokenizerMap {
    pub fn empty() -> Self {
        Self(BTreeMap::new())
    }
    pub fn with(mut self, prefix: &str, tokenizer: &str) -> Self {
        self.0.insert(prefix.into(), tokenizer.into());
        self
    }
    /// Task-supplied overrides (`tokenizer_map`: prefix to supported name).
    pub fn from_json(v: &Value) -> HarnessResult<Self> {
        Self::from_json_with(v, &TokenizerSet::default())
    }
    /// As `from_json`, also admitting names the host approved in `set`.
    pub fn from_json_with(v: &Value, set: &TokenizerSet) -> HarnessResult<Self> {
        let m = v
            .as_object()
            .ok_or_else(|| d("SPX-HPD081", "`tokenizer_map` must be an object"))?;
        let mut out = Self::default();
        for (k, t) in m {
            let t = t
                .as_str()
                .filter(|t| set.is_approved_name(t))
                .ok_or_else(|| {
                    d(
                        "SPX-HPD081",
                        format!("`tokenizer_map.{k}` must name a built-in {TOKENIZER_NAMES:?} or host-approved tokenizer"),
                    )
                })?;
            out.0.insert(k.clone(), t.into());
        }
        Ok(out)
    }
    pub fn tokenizer_for(&self, model_id: &str) -> Option<&str> {
        self.0
            .iter()
            .filter(|(p, _)| model_id.starts_with(p.as_str()))
            .max_by_key(|(p, _)| p.len())
            .map(|(_, t)| t.as_str())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetPolicy {
    pub output_reserve_tokens: u64,
    pub protocol_overhead_tokens: u64,
    /// Whole-task limit on reserved tokens (inputs plus output reserves).
    pub max_task_tokens: Option<u64>,
    pub max_task_cost_micros: Option<u64>,
}

impl Default for BudgetPolicy {
    fn default() -> Self {
        Self {
            output_reserve_tokens: 4096,
            protocol_overhead_tokens: 256,
            max_task_tokens: None,
            max_task_cost_micros: None,
        }
    }
}

/// Exact count of one serialized request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestCount {
    pub bytes: u64,
    /// Named-tokenizer tokens; `None` is unknown (never zero).
    pub tokens: Option<u64>,
    pub tokenizer: Option<(String, String)>,
    pub note: Option<String>,
}

impl RequestCount {
    pub fn measured(&self) -> bool {
        self.tokens.is_some()
    }
    /// Admission figure: measured tokens, else the byte upper bound.
    pub fn admission_tokens(&self) -> u64 {
        self.tokens.unwrap_or(self.bytes)
    }
    pub fn token_count(&self) -> TokenCount {
        match (&self.tokenizer, self.tokens) {
            (Some((n, f)), Some(t)) => TokenCount::named(n, f, t),
            _ => TokenCount::bytes(self.bytes),
        }
    }
    pub fn to_json(&self) -> Value {
        let tok = match &self.tokenizer {
            Some((n, f)) if self.tokens.is_some() => {
                json!({"kind": "named", "name": n, "fingerprint": f})
            }
            _ => json!({"kind": "unknown", "policy": "utf8-bytes-upper-bound",
                        "reason": self.note.clone().unwrap_or_else(|| "no tokenizer mapped for the model".into())}),
        };
        json!({"tokenizer": tok, "request_bytes": self.bytes, "request_tokens": self.tokens,
               "measured": self.measured(), "admission_tokens": self.admission_tokens(),
               "admission_basis": if self.measured() { "named-tokens" } else { "utf8-bytes-upper-bound" }})
    }
}

/// The exact text sent to the model for `prompt` (what is counted).
pub fn request_text(prompt: &Value) -> String {
    super::prompt_render::rendered_text(prompt).unwrap_or_else(|| canonical(prompt))
}

/// Result of fitting one request to one model.
#[derive(Clone, Debug)]
pub struct Fit {
    pub model: String,
    pub prompt: Value,
    pub count: RequestCount,
    pub required_tokens: u64,
    pub max_context: u64,
    pub fits: bool,
    /// Output tokens reserved by this fit; the cap sent with the request.
    pub output_reserve: u64,
    pub dropped: Vec<String>,
    /// Required tokens with every optional item dropped.
    pub floor_tokens: u64,
}

impl Fit {
    pub fn to_json(&self, policy: &BudgetPolicy) -> Value {
        let mut v = self.count.to_json();
        v["model"] = json!(self.model);
        v["protocol_overhead_tokens"] = json!(policy.protocol_overhead_tokens);
        v["output_reserve_tokens"] = json!(policy.output_reserve_tokens);
        v["required_tokens"] = json!(self.required_tokens);
        v["model_max_context"] = json!(self.max_context);
        v["fits"] = json!(self.fits);
        v["dropped_optional"] = json!(self.dropped);
        v
    }
}

/// Host-level budget configuration held by the run (task v2 may override
/// the policy and the model-to-tokenizer map; it never supplies a tokenizer).
#[derive(Default)]
pub struct BudgetConfig {
    pub policy: BudgetPolicy,
    pub map: ModelTokenizerMap,
    pub tokenizers: TokenizerSet,
    pub cache: CountCache,
    /// Opt-in repair-feedback allowance in named tokens (TC-06); `None` keeps
    /// the labelled byte policy.
    pub feedback_max_tokens: Option<u64>,
    /// Host-owned output-cap tiers and reasoning controls (TC-02; opt-in).
    pub generation: super::generation::GenerationPolicy,
    /// Host-configured price records for local cost estimates (TC-01).
    pub prices: crate::receipt::PriceBook,
}

impl BudgetConfig {
    pub fn for_task(&self, task: &Task) -> RequestBudget<'_> {
        RequestBudget {
            policy: task.budget.clone().unwrap_or_else(|| self.policy.clone()),
            map: task
                .tokenizer_map
                .clone()
                .unwrap_or_else(|| self.map.clone()),
            tokenizers: &self.tokenizers,
            cache: &self.cache,
        }
    }
}

pub struct RequestBudget<'a> {
    pub policy: BudgetPolicy,
    pub map: ModelTokenizerMap,
    pub tokenizers: &'a TokenizerSet,
    pub cache: &'a CountCache,
}

impl RequestBudget<'_> {
    /// Count `text` for `model_id` with its mapped tokenizer, if one is supplied.
    pub fn count(&self, model_id: &str, text: &str) -> RequestCount {
        let bytes = text.len() as u64;
        let unknown = |note: &str| RequestCount {
            bytes,
            tokens: None,
            tokenizer: None,
            note: Some(note.into()),
        };
        let Some(name) = self.map.tokenizer_for(model_id) else {
            return unknown("no tokenizer is mapped for the model");
        };
        let Some(t) = self.tokenizers.get(name) else {
            return unknown(&format!("tokenizer `{name}` is not provisioned"));
        };
        let Some(ap) = self.tokenizers.approval(name) else {
            return unknown(&format!("tokenizer `{name}` is not approved"));
        };
        if ap.fingerprint != t.fingerprint() {
            return unknown(&format!(
                "tokenizer `{name}` fingerprint differs from approval"
            ));
        }
        let counted = self
            .cache
            .count_with(t.name(), t.fingerprint(), &ap.semantics, text, || {
                t.try_count(text).map(|n| n as u64)
            });
        match counted {
            Ok(n) => RequestCount {
                bytes,
                tokens: Some(n),
                tokenizer: Some((t.name().into(), t.fingerprint().into())),
                note: None,
            },
            Err(e) => unknown(&format!("tokenizer `{name}` failed: {}", e.message)),
        }
    }

    fn required(&self, count: &RequestCount) -> u64 {
        count.admission_tokens()
            + self.policy.protocol_overhead_tokens
            + self.policy.output_reserve_tokens
    }

    /// Fit the request for `model`: drop optional material in `optional` order
    /// (each item whole) until the exact serialized request fits. Protected
    /// content is never truncated; `fits` is false when it alone cannot fit.
    pub fn fit(
        &self,
        model: &ModelPlan,
        optional: &[String],
        build: &dyn Fn(&BTreeSet<String>) -> Value,
    ) -> Fit {
        let mut dropped = BTreeSet::new();
        let mut order = Vec::new();
        let mut prompt = build(&dropped);
        let mut count = self.count(&model.id, &request_text(&prompt));
        let mut next = optional.iter();
        while self.required(&count) > model.max_context {
            let Some(item) = next.next() else { break };
            dropped.insert(item.clone());
            order.push(item.clone());
            prompt = build(&dropped);
            count = self.count(&model.id, &request_text(&prompt));
        }
        let floor = if order.len() == optional.len() {
            self.required(&count)
        } else {
            let all: BTreeSet<String> = optional.iter().cloned().collect();
            self.required(&self.count(&model.id, &request_text(&build(&all))))
        };
        let required_tokens = self.required(&count);
        Fit {
            model: model.id.clone(),
            prompt,
            fits: required_tokens <= model.max_context,
            output_reserve: self.policy.output_reserve_tokens,
            required_tokens,
            max_context: model.max_context,
            dropped: order,
            floor_tokens: floor,
            count,
        }
    }

    /// Smallest protected-only requirement across the catalog: the honest
    /// routing estimate (a model is only excluded when even that cannot fit).
    pub fn floor_estimate(
        &self,
        catalog: &[ModelPlan],
        optional: &[String],
        build: &dyn Fn(&BTreeSet<String>) -> Value,
    ) -> u64 {
        let all: BTreeSet<String> = optional.iter().cloned().collect();
        let prompt = build(&all);
        let text = request_text(&prompt);
        catalog
            .iter()
            .map(|m| self.required(&self.count(&m.id, &text)))
            .min()
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedgerEntry {
    pub label: String,
    /// `generation`, `router`, ...
    pub kind: String,
    pub count: RequestCount,
    pub output_reserve: u64,
    pub cost_micros: u64,
}

/// Per-task reservations across attempts and router/decision calls.
#[derive(Default)]
pub struct TaskLedger {
    pub entries: Vec<LedgerEntry>,
}

impl TaskLedger {
    pub fn reserved_tokens(&self) -> u64 {
        self.entries
            .iter()
            .map(|e| e.count.admission_tokens() + e.output_reserve)
            .sum()
    }
    pub fn reserved_cost(&self) -> u64 {
        self.entries.iter().map(|e| e.cost_micros).sum()
    }
    /// Refuse (`SPX-HPD101`) when this call would exceed a whole-task limit;
    /// otherwise record it. Called before the call starts.
    pub fn reserve(&mut self, policy: &BudgetPolicy, e: LedgerEntry) -> HarnessResult<()> {
        let add = e.count.admission_tokens() + e.output_reserve;
        if let Some(max) = policy.max_task_tokens {
            if self.reserved_tokens() + add > max {
                return Err(d(
                    "SPX-HPD101",
                    format!(
                        "task token budget exhausted: {} reserved + {add} for `{}` exceeds {max}",
                        self.reserved_tokens(),
                        e.label
                    ),
                ));
            }
        }
        if let Some(max) = policy.max_task_cost_micros {
            if self.reserved_cost() + e.cost_micros > max {
                return Err(d(
                    "SPX-HPD101",
                    format!("task cost budget exhausted for `{}`", e.label),
                ));
            }
        }
        self.entries.push(e);
        Ok(())
    }
    /// Counts per tokenizer identity; named and upper-bound figures never mix.
    pub fn to_json(&self) -> Value {
        let mut named: BTreeMap<String, u64> = BTreeMap::new();
        let mut bound = 0u64;
        for e in &self.entries {
            match (&e.count.tokenizer, e.count.tokens) {
                (Some((n, f)), Some(t)) => *named.entry(format!("{n}@{f}")).or_default() += t,
                _ => bound += e.count.bytes,
            }
        }
        json!({"calls": self.entries.len(), "reserved_tokens": self.reserved_tokens(),
               "named_input_tokens": named, "unknown_tokenizer_upper_bound_bytes": bound,
               "entries": self.entries.iter().map(|e| json!({"label": e.label, "kind": e.kind,
                    "request": e.count.to_json(), "output_reserve": e.output_reserve})).collect::<Vec<_>>()})
    }
}
