//! Model clients for a profile campaign and the per-attempt record.
//!
//! `ProductionClient` sends every request through the production harness model
//! path: a `ProposalStage` (the `HostModel` adapter contract), the host's
//! request budget (named-tokenizer count and admission), typed provider
//! receipts, the price book and `Observer` events. `RawClient` wraps the
//! benchmark's own `ModelClient` loop and is labelled as such; it is never wire
//! evidence for `HostModel` changes. Both record every dispatched attempt with
//! usage, cost (unknown kept unknown), pins, latency and cache state.

use super::cache_state::{CacheState, CacheTracker};
use super::model::{Generation, ModelClient, ModelError};
use crate::json::sha256_plain;
use crate::observe::{
    Availability, CacheState as ObsCache, Observation, Observer, ObserverLimits,
    Outcome as ObsOutcome, Role, Stage, TokenCount,
};
use crate::receipt::{GenerationControls, PriceBook, Support, Usage};
use crate::workflow::budget::{request_text, RequestCount};
use crate::workflow::context_target::CostMeter;
use crate::workflow::feedback::{project, FeedbackPolicy};
use crate::workflow::generation::{GenerationPolicy, ResponseShape};
use crate::workflow::lineage::Lineage;
use crate::workflow::prompt_render::{self, PromptRenderer};
use crate::workflow::stages::{ProposalRequest, ProposalStage, StageFailure};
use serde_json::{json, Value};
use std::sync::Mutex;
use std::time::Instant;

pub const PATH_PRODUCTION: &str = "production-harness";
pub const PATH_RAW: &str = "raw-model-loop";

/// One dispatched (or refused-before-dispatch) model attempt.
#[derive(Clone, Debug)]
pub struct Attempt {
    /// `router`, `generator` or `recovery`.
    pub role: &'static str,
    pub dispatched: bool,
    pub ok: bool,
    /// Model identity the provider reported (a pin), when known.
    pub model_pin: Option<String>,
    pub usage: Option<Usage>,
    /// Cost in micro-units; `None` is unknown, never zero.
    pub cost_micros: Option<u64>,
    /// `provider`, `estimate`, `non_billed` or `unknown`.
    pub cost_basis: &'static str,
    pub cache: CacheState,
    pub finish: String,
    pub latency_ms: u64,
    pub request_bytes: u64,
    pub request_tokens: Option<u64>,
    /// Reusable-prefix identity of an ordered-rendered request (`None`: canonical request).
    pub prefix_identity: Option<String>,
}

impl Attempt {
    pub fn to_json(&self) -> Value {
        json!({"role": self.role, "dispatched": self.dispatched, "ok": self.ok,
               "model_pin": self.model_pin.clone().unwrap_or_else(|| "unknown".into()),
               "usage": self.usage.map_or(Value::Null, |u| u.to_json()),
               "cost_micros": self.cost_micros.map_or(json!("unknown"), |n| json!(n)),
               "cost_basis": self.cost_basis, "cache_state": self.cache.as_str(),
               "finish": self.finish, "latency_ms": self.latency_ms,
               "request_bytes": self.request_bytes, "request_tokens": self.request_tokens,
               "prefix_identity": self.prefix_identity})
    }
}

/// A client for one trial: the model plus the attempt log it kept.
pub trait TrialClient: Send + Sync {
    fn path(&self) -> &'static str;
    fn model(&self) -> &dyn ModelClient;
    fn take_attempts(&self) -> Vec<Attempt>;
    /// Observation events recorded for the trial (production path only).
    fn observations(&self) -> Vec<Value> {
        vec![]
    }
    /// Reports of overlays applied inside the client (feedback projections).
    fn overlay_reports(&self) -> Vec<Value> {
        vec![]
    }
}

fn role_of(prompt: &str) -> &'static str {
    if prompt.contains("## Grader result: FAILED") {
        "recovery"
    } else {
        "generator"
    }
}

fn micros(usd: f64) -> u64 {
    (usd * 1e6).round().max(0.0) as u64
}

/// The benchmark's raw `ModelClient` loop with attempt recording.
pub struct RawClient<'a> {
    pub inner: &'a dyn ModelClient,
    pub tracker: &'a CacheTracker,
    pub model_id: String,
    pub billed: bool,
    log: Mutex<Vec<Attempt>>,
}

impl<'a> RawClient<'a> {
    pub fn new(
        inner: &'a dyn ModelClient,
        tracker: &'a CacheTracker,
        model_id: &str,
        billed: bool,
    ) -> Self {
        Self {
            inner,
            tracker,
            model_id: model_id.into(),
            billed,
            log: Mutex::default(),
        }
    }
}

impl ModelClient for RawClient<'_> {
    fn generate(&self, prompt: &str, seed: u64) -> Result<Generation, ModelError> {
        let role = role_of(prompt);
        let t0 = Instant::now();
        let r = self.inner.generate(prompt, seed);
        let mut a = Attempt {
            role,
            dispatched: !matches!(r, Err(ModelError::Budget(_))),
            ok: r.is_ok(),
            model_pin: Some(self.model_id.clone()),
            usage: None,
            cost_micros: None,
            cost_basis: "unknown",
            cache: CacheState::Unknown,
            finish: if r.is_ok() { "complete" } else { "failed" }.into(),
            latency_ms: t0.elapsed().as_millis() as u64,
            request_bytes: prompt.len() as u64,
            request_tokens: None,
            prefix_identity: None,
        };
        if let Ok(g) = &r {
            let known = [g.provider_in, g.cache_read, g.cache_write];
            let u = Usage {
                uncached_input: g.provider_in,
                cache_read: g.cache_read,
                cache_write: g.cache_write,
                output: g.provider_out,
                input_total: known
                    .iter()
                    .all(Option::is_some)
                    .then(|| known.iter().flatten().sum()),
                ..Usage::default()
            };
            a.latency_ms = g.latency_ms;
            a.cache = self.tracker.classify(&self.model_id, &u);
            a.usage = Some(u);
            match (g.cost_usd, self.billed) {
                (Some(c), _) => {
                    a.cost_micros = Some(micros(c));
                    a.cost_basis = "provider";
                }
                (None, false) => {
                    a.cost_micros = Some(0);
                    a.cost_basis = "non_billed";
                }
                (None, true) => {}
            }
        } else if !self.billed {
            // A failed local call still billed nothing.
            a.cost_micros = Some(0);
            a.cost_basis = "non_billed";
        }
        self.log.lock().expect("attempt log").push(a);
        r
    }
}

impl TrialClient for RawClient<'_> {
    fn path(&self) -> &'static str {
        PATH_RAW
    }
    fn model(&self) -> &dyn ModelClient {
        self
    }
    fn take_attempts(&self) -> Vec<Attempt> {
        std::mem::take(&mut *self.log.lock().expect("attempt log"))
    }
}

/// Maximum request the production admission allows (tokens or, with no
/// tokenizer mapped, the UTF-8 byte upper bound), output reserve included.
pub struct Admission {
    pub max_request_tokens: u64,
    pub protocol_overhead_tokens: u64,
}

/// Counts one serialized request for a model: the host's `RequestBudget::count`
/// (named tokenizer, else UTF-8 bytes). Built by the caller so non-thread-safe
/// tokenizer state stays out of the client.
pub type Counter<'a> = Box<dyn Fn(&str, &str) -> RequestCount + Send + Sync + 'a>;

/// Production-harness model path (see the module comment).
pub struct ProductionClient<'a> {
    stage: Mutex<Box<dyn ProposalStage + Send + 'a>>,
    lineage: Mutex<Lineage>,
    model: String,
    controls: GenerationControls,
    counter: Counter<'a>,
    prices: &'a PriceBook,
    admission: Admission,
    tracker: &'a CacheTracker,
    events: Mutex<Vec<Observation>>,
    log: Mutex<Vec<Attempt>>,
    renderer: PromptRenderer,
    cache_support: Support,
    overlays: Overlays,
    goal: Option<String>,
    /// Grader failures seen in the current step (oldest first).
    history: Mutex<Vec<Value>>,
    reports: Mutex<Vec<Value>>,
}

/// Opt-in cost policies the client applies on the wire.
#[derive(Clone, Debug, Default)]
pub struct Overlays {
    /// TC-02 tiers: the source-repair cap (app answers are whole-file edits).
    pub generation: Option<GenerationPolicy>,
    /// TC-06 feedback allowance for repair turns, in named tokens (bytes
    /// policy when the request is not measured in tokens).
    pub feedback_max_tokens: Option<u64>,
}

impl<'a> ProductionClient<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        stage: Box<dyn ProposalStage + Send + 'a>,
        lineage: Lineage,
        model: &str,
        controls: GenerationControls,
        counter: Counter<'a>,
        prices: &'a PriceBook,
        admission: Admission,
        tracker: &'a CacheTracker,
    ) -> Self {
        Self {
            stage: Mutex::new(stage),
            lineage: Mutex::new(lineage),
            model: model.into(),
            controls,
            counter,
            prices,
            admission,
            tracker,
            events: Mutex::default(),
            log: Mutex::default(),
            renderer: PromptRenderer::Canonical,
            cache_support: Support::Unknown,
            overlays: Overlays::default(),
            goal: None,
            history: Mutex::default(),
            reports: Mutex::default(),
        }
    }

    /// A `goal` member for fixture adapters that select behaviour from it
    /// (for example `MODE:receipt`); real adapters never need it.
    pub fn with_goal(mut self, g: Option<String>) -> Self {
        self.goal = g;
        self
    }

    pub fn with_overlays(mut self, o: Overlays) -> Self {
        self.overlays = o;
        self
    }

    fn controls_now(&self) -> GenerationControls {
        match &self.overlays.generation {
            Some(p) => {
                let reserve = p.reserve_for(
                    ResponseShape::SourceRepair,
                    self.controls.max_output_tokens.unwrap_or(4096),
                );
                p.controls(ResponseShape::SourceRepair, reserve)
            }
            None => self.controls.clone(),
        }
    }

    /// Replace the grader feedback of a retry prompt by the TC-06 projection of
    /// this step's failure history (current failure exact, history bounded).
    fn project_feedback(&self, prompt: &str) -> Option<String> {
        const OPEN: &str = "## Grader result: FAILED\n";
        const CLOSE: &str = "\n\nReply again, in the same block format";
        let max = self.overlays.feedback_max_tokens?;
        let Some(i) = prompt.find(OPEN) else {
            self.history.lock().expect("history").clear();
            return None;
        };
        let start = i + OPEN.len();
        let end = prompt[start..].find(CLOSE)? + start;
        let mut h = self.history.lock().expect("history");
        let n = h.len() as u64 + 1;
        h.push(json!({"attempt": n, "stage": "checks", "code": "GRADER",
                      "message": &prompt[start..end], "proposed": {"kind": "file-blocks"}}));
        let probe = (self.counter)(&self.model, "");
        let meter = match &probe.tokenizer {
            Some((name, _)) => {
                let m = self.model.clone();
                CostMeter::with_counter(
                    name,
                    Box::new(move |t| (self.counter)(&m, t).tokens.unwrap_or(t.len() as u64)),
                )
            }
            None => CostMeter::bytes(),
        };
        let policy = FeedbackPolicy::for_meter(Some(max), &meter);
        let p = project(&h, &policy, &meter).ok()?;
        self.reports.lock().expect("reports").push(p.report);
        Some(format!(
            "{}{}{}",
            &prompt[..start],
            crate::json::canonical(&Value::Array(p.entries)),
            &prompt[end..]
        ))
    }

    /// `[budget] prompt_renderer` and `model_prompt_cache` of the arm (TC-04).
    pub fn with_renderer(mut self, r: PromptRenderer, cache: Support) -> Self {
        self.renderer = r;
        self.cache_support = cache;
        self
    }

    fn observe(&self, id: &str, ok: bool, a: &Attempt, bytes: u64, rev: &str) {
        let mut o = Observation::new(
            &self.stage.lock().expect("stage").id(),
            "model.generate",
            Stage::Generation,
            Role::Incurred,
            id,
        );
        o.source_revision = rev.into();
        o.availability = if ok {
            Availability::Available
        } else {
            Availability::Unavailable
        };
        o.outcome = if ok {
            ObsOutcome::Ok
        } else {
            ObsOutcome::Failed
        };
        o.latency_ms = a.latency_ms;
        o.incurred = Some(TokenCount::bytes(bytes));
        o.upstream_model = a.model_pin.clone();
        o.cost.provider_billed = (a.cost_basis == "provider")
            .then_some(a.cost_micros)
            .flatten();
        o.estimated_cost = (a.cost_basis == "estimate")
            .then_some(a.cost_micros)
            .flatten();
        o.usage = a.usage;
        o.cache = match a.cache {
            CacheState::Warm => ObsCache::Hit,
            CacheState::Cold | CacheState::Expired => ObsCache::Miss,
            CacheState::Unknown => ObsCache::Bypass,
        };
        self.events.lock().expect("events").push(o);
    }
}

impl ProductionClient<'_> {
    fn generate_one(&self, prompt: &str) -> Result<Generation, ModelError> {
        let role = role_of(prompt);
        let projected = self.project_feedback(prompt);
        let prompt = projected.as_deref().unwrap_or(prompt);
        let controls = self.controls_now();
        let mut doc = match self.renderer {
            PromptRenderer::Canonical => {
                json!({"schema": "semaprax.harness-apptask-prompt.v1", "text": prompt})
            }
            PromptRenderer::OrderedV1 => ordered_doc(prompt),
        };
        if let Some(g) = &self.goal {
            doc["goal"] = json!(g);
        }
        let text = request_text(&doc);
        let count = (self.counter)(&self.model, &text);
        let required = count.admission_tokens()
            + self.admission.protocol_overhead_tokens
            + controls.max_output_tokens.unwrap_or(0);
        let mut a = Attempt {
            role,
            dispatched: false,
            ok: false,
            model_pin: None,
            usage: None,
            cost_micros: None,
            cost_basis: "unknown",
            cache: CacheState::Unknown,
            finish: "not_dispatched".into(),
            latency_ms: 0,
            request_bytes: count.bytes,
            request_tokens: count.tokens,
            prefix_identity: None,
        };
        let lineage = self.lineage.lock().expect("lineage");
        let rev = lineage.project.revision.clone();
        let id = lineage.next_invocation();
        if required > self.admission.max_request_tokens {
            drop(lineage);
            a.finish = "budget_refused".into();
            self.observe(&id, false, &a, count.bytes, &rev);
            self.log.lock().expect("attempt log").push(a);
            return Err(ModelError::Budget(format!(
                "request needs {required} tokens, admission allows {}",
                self.admission.max_request_tokens
            )));
        }
        if self.cache_support == Support::Supported {
            let b = prompt_render::PrefixBinding {
                provider: &self.stage.lock().expect("stage").id(),
                model: &self.model,
                project: &lineage.project.id,
                worktree: &lineage.project.worktree,
                lock_digest: &lineage.lock_digest,
            };
            a.prefix_identity = prompt_render::prefix_identity(&doc, &b);
        }
        let req = ProposalRequest {
            lineage: &lineage,
            prompt: doc,
            model: self.model.clone(),
            controls: controls.clone(),
        };
        let t0 = Instant::now();
        let (res, receipt) = self.stage.lock().expect("stage").propose_receipted(&req);
        drop(lineage);
        a.dispatched = true;
        a.latency_ms = t0.elapsed().as_millis() as u64;
        a.finish = receipt.finish.as_str();
        a.model_pin = receipt.model.clone();
        if receipt.unavailable.is_none() {
            a.cache = self.tracker.classify(&self.model, &receipt.usage);
            a.usage = Some(receipt.usage);
            if let Some(m) = receipt.provider_cost_micros {
                a.cost_micros = Some(m);
                a.cost_basis = "provider";
            } else {
                let est = self.prices.estimate(&self.model, &receipt.usage);
                if let Some(m) = est.micros {
                    a.cost_micros = Some(m);
                    a.cost_basis = if est.basis == "non_billed_source" {
                        "non_billed"
                    } else {
                        "estimate"
                    };
                }
            }
        }
        let out = match res {
            Ok(bytes) => {
                a.ok = true;
                Ok(bytes)
            }
            Err(f) => Err(f),
        };
        self.observe(&id, a.ok, &a, count.bytes, &rev);
        let u = a.usage.unwrap_or_default();
        let gen = out.map(|bytes| Generation {
            text: String::from_utf8_lossy(&bytes).into_owned(),
            provider_in: u.uncached_input,
            provider_out: u.output,
            cache_read: u.cache_read,
            cache_write: u.cache_write,
            cost_usd: a.cost_micros.map(|m| m as f64 / 1e6),
            latency_ms: a.latency_ms,
        });
        self.log.lock().expect("attempt log").push(a);
        gen.map_err(|f| {
            ModelError::Failed(match f {
                StageFailure::Unavailable(d)
                | StageFailure::Refused(d)
                | StageFailure::Uncertain(d) => format!("{}: {}", d.code, d.message),
            })
        })
    }
}

impl ModelClient for ProductionClient<'_> {
    fn generate(&self, prompt: &str, _seed: u64) -> Result<Generation, ModelError> {
        self.generate_one(prompt)
    }
}

impl TrialClient for ProductionClient<'_> {
    fn path(&self) -> &'static str {
        PATH_PRODUCTION
    }
    fn model(&self) -> &dyn ModelClient {
        self
    }
    fn take_attempts(&self) -> Vec<Attempt> {
        std::mem::take(&mut *self.log.lock().expect("attempt log"))
    }
    fn overlay_reports(&self) -> Vec<Value> {
        self.reports.lock().expect("reports").clone()
    }
    /// The events, recorded through a real `Observer` (bounds-checked, sequenced).
    fn observations(&self) -> Vec<Value> {
        let mut obs = Observer::new(None, ObserverLimits::default());
        for e in self.events.lock().expect("events").iter() {
            obs.record(e.clone());
        }
        obs.events().iter().map(Observation::to_json).collect()
    }
}

/// Digest pin of an opaque text (profile, tool list); never the text itself.
pub fn pin(text: &str) -> String {
    sha256_plain(text.as_bytes())
}

/// The apptask prompt as ordered-v1 segments: `host` (skill text and protocol),
/// `task` (project context) form the reusable prefix; `live` (task request,
/// failing output, retry feedback) is the mutable suffix. The text is the
/// same bytes in a stable order, so nothing the model sees is dropped.
pub fn ordered_doc(prompt: &str) -> Value {
    let (head, live) = match prompt.split_once("\n\n## Task\n") {
        Some((h, l)) => (h, format!("\n\n## Task\n{l}")),
        None => (prompt, String::new()),
    };
    let (host, task) = match head.split_once("\n\n## Project\n") {
        Some((h, t)) => (h.to_string(), format!("\n\n## Project\n{t}")),
        None => (head.to_string(), String::new()),
    };
    json!({"schema": prompt_render::SCHEMA, "renderer": prompt_render::RENDERER_VERSION,
           "skill_ids": [],
           "segments": [{"id": "host", "role": "stable-instructions", "text": host},
                        {"id": "task", "role": "task-context", "text": task},
                        {"id": "live", "role": "mutable-suffix", "text": live}]})
}
