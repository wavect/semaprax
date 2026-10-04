//! Model access and the spend ledger. Real trials go to a loopback endpoint that
//! speaks the Ollama `/api/generate` shape: Ollama itself (small model) or a
//! loopback shim in front of the Claude Code CLI (larger model). The ledger
//! refuses a call that could cross the cap and counts the provider's own
//! reported cost; it is the authority the campaign obeys, and the shim keeps a
//! second, independent ledger.

use crate::bench::pilot::http_with;
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Debug, Default)]
pub struct Generation {
    pub text: String,
    /// Provider-reported usage; `None` is unavailable, never zero.
    pub provider_in: Option<u64>,
    pub provider_out: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_write: Option<u64>,
    /// Provider-reported cost in USD; `None` for a local model (no billing).
    pub cost_usd: Option<f64>,
    pub latency_ms: u64,
}

#[derive(Clone, Debug)]
pub enum ModelError {
    /// The ledger refused the call: nothing was sent.
    Budget(String),
    /// Transport or provider failure (a failed attempt, still recorded).
    Failed(String),
}

pub trait ModelClient: Send + Sync {
    fn generate(&self, prompt: &str, seed: u64) -> Result<Generation, ModelError>;
}

#[derive(Clone, Debug)]
pub struct HttpModel {
    pub addr: String,
    pub name: String,
    pub temperature: f64,
    pub num_ctx: u32,
    pub num_predict: u32,
    pub timeout: Duration,
}

impl ModelClient for HttpModel {
    fn generate(&self, prompt: &str, seed: u64) -> Result<Generation, ModelError> {
        if !(self.addr.starts_with("127.0.0.1:") || self.addr.starts_with("localhost:")) {
            return Err(ModelError::Failed("model endpoint must be loopback".into()));
        }
        let body = json!({"model": self.name, "prompt": prompt, "stream": false,
            "options": {"temperature": self.temperature, "seed": seed, "num_ctx": self.num_ctx, "num_predict": self.num_predict}});
        let t0 = Instant::now();
        let raw = http_with(
            &self.addr,
            "POST",
            "/api/generate",
            &body.to_string(),
            self.timeout,
        )
        .map_err(ModelError::Failed)?;
        let v: Value = serde_json::from_str(&raw)
            .map_err(|e| ModelError::Failed(format!("bad response: {e}")))?;
        if v["refused"].as_bool() == Some(true) {
            return Err(ModelError::Budget(
                v["reason"]
                    .as_str()
                    .unwrap_or("shim ledger refused")
                    .to_string(),
            ));
        }
        if let Some(e) = v["error"].as_str() {
            return Err(ModelError::Failed(e.to_string()));
        }
        let u = &v["usage"];
        let provider_in = u["input_tokens"]
            .as_u64()
            .or_else(|| v["prompt_eval_count"].as_u64());
        Ok(Generation {
            text: v["response"].as_str().unwrap_or_default().to_string(),
            provider_in,
            provider_out: u["output_tokens"]
                .as_u64()
                .or_else(|| v["eval_count"].as_u64()),
            cache_read: u["cache_read_input_tokens"].as_u64(),
            cache_write: u["cache_creation_input_tokens"].as_u64(),
            cost_usd: v["cost_usd"].as_f64(),
            latency_ms: t0.elapsed().as_millis() as u64,
        })
    }
}

/// Deterministic model for tests: answers by calling a closure.
pub struct Scripted<F: Fn(&str, u64) -> Result<Generation, ModelError> + Send + Sync>(pub F);

impl<F: Fn(&str, u64) -> Result<Generation, ModelError> + Send + Sync> ModelClient for Scripted<F> {
    fn generate(&self, prompt: &str, seed: u64) -> Result<Generation, ModelError> {
        (self.0)(prompt, seed)
    }
}

// ---- ledger ----

#[derive(Debug, Default)]
struct LedgerState {
    spent: f64,
    reserved: f64,
    calls: u64,
    refused: u64,
    max_call: f64,
}

/// Hard call+cost ledger. `reserve` takes a ceiling for one call before it is
/// sent and refuses when `spent + reserved + ceiling` would pass the cap, so
/// parallel in-flight calls cannot jointly overshoot. `settle` replaces the
/// reservation with the provider-reported cost.
pub struct SpendLedger {
    pub cap_usd: f64,
    pub max_calls: u64,
    /// Ceiling used for a call before any cost was observed.
    pub initial_ceiling_usd: f64,
    state: Mutex<LedgerState>,
    path: Option<std::path::PathBuf>,
}

pub struct Reservation(f64);

impl SpendLedger {
    pub fn new(cap_usd: f64, max_calls: u64, initial_ceiling_usd: f64) -> Self {
        Self {
            cap_usd,
            max_calls,
            initial_ceiling_usd,
            state: Mutex::new(LedgerState::default()),
            path: None,
        }
    }

    /// Persist after every settlement and start from the recorded totals.
    pub fn with_file(mut self, path: &std::path::Path) -> Self {
        if let Ok(t) = std::fs::read_to_string(path) {
            if let Ok(v) = serde_json::from_str::<Value>(&t) {
                let mut s = self.state.lock().expect("fresh lock");
                s.spent = v["spent_usd"].as_f64().unwrap_or(0.0);
                s.calls = v["calls"].as_u64().unwrap_or(0);
                s.refused = v["refused_calls"].as_u64().unwrap_or(0);
                s.max_call = v["max_call_usd"].as_f64().unwrap_or(0.0);
            }
        }
        self.path = Some(path.to_path_buf());
        self
    }

    fn ceiling(&self, s: &LedgerState) -> f64 {
        (s.max_call * 2.0).max(self.initial_ceiling_usd)
    }

    pub fn reserve(&self) -> Result<Reservation, String> {
        let mut s = self
            .state
            .lock()
            .map_err(|_| "ledger poisoned".to_string())?;
        let c = self.ceiling(&s);
        if s.calls >= self.max_calls {
            s.refused += 1;
            return Err(format!("call cap {} reached", self.max_calls));
        }
        if s.spent + s.reserved + c > self.cap_usd {
            s.refused += 1;
            return Err(format!(
                "spend cap USD {:.2} would be crossed (spent {:.4}, call ceiling {:.4})",
                self.cap_usd, s.spent, c
            ));
        }
        s.reserved += c;
        Ok(Reservation(c))
    }

    /// Record the provider-reported cost (`None`: a failed call with no report is
    /// charged its whole reservation, never zero).
    pub fn settle(&self, r: Reservation, cost: Option<f64>) {
        let Ok(mut s) = self.state.lock() else { return };
        s.reserved = (s.reserved - r.0).max(0.0);
        let c = cost.unwrap_or(r.0);
        s.spent += c;
        s.calls += 1;
        s.max_call = s.max_call.max(c);
        if let Some(p) = &self.path {
            let _ = std::fs::write(
                p,
                format!(
                    "{}\n",
                    serde_json::to_string_pretty(&Self::json_of(&s, self.cap_usd, self.max_calls))
                        .unwrap_or_default()
                ),
            );
        }
    }

    fn json_of(s: &LedgerState, cap: f64, max_calls: u64) -> Value {
        json!({"cap_usd": cap, "spent_usd": (s.spent * 1e6).round() / 1e6, "calls": s.calls, "max_calls": max_calls,
               "refused_calls": s.refused, "max_call_usd": s.max_call})
    }

    pub fn snapshot(&self) -> Value {
        let s = self.state.lock().expect("ledger");
        Self::json_of(&s, self.cap_usd, self.max_calls)
    }

    pub fn spent(&self) -> f64 {
        self.state.lock().map(|s| s.spent).unwrap_or(f64::MAX)
    }
}

/// A client that charges every call to a ledger and refuses past the cap.
pub struct Metered<'a> {
    pub inner: &'a dyn ModelClient,
    pub ledger: &'a SpendLedger,
    /// Local models have no billing: they still count calls but are never charged.
    pub billed: bool,
}

impl ModelClient for Metered<'_> {
    fn generate(&self, prompt: &str, seed: u64) -> Result<Generation, ModelError> {
        if !self.billed {
            return self.inner.generate(prompt, seed);
        }
        let r = self.ledger.reserve().map_err(ModelError::Budget)?;
        match self.inner.generate(prompt, seed) {
            Ok(g) => {
                self.ledger.settle(r, g.cost_usd);
                Ok(g)
            }
            Err(e) => {
                // A failed call may still have been billed upstream: charge the reservation.
                self.ledger.settle(r, None);
                Err(e)
            }
        }
    }
}
