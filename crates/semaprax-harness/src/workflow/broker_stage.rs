//! Context stage backed by [`crate::context::Broker`]: compiler-verified native
//! facts plus structural items from the selected external `context.repository`
//! provider, fitted under one byte budget. Used only when the resolved profile
//! selects an external provider; otherwise the plain native stage runs.
//!
//! HN-13: a planned collection asks the provider one task-derived question
//! (plus at most one unmet-need retry and one failed-candidate follow-up),
//! never recurses, keeps native facts when the provider fails and reports what
//! stays unknown.

use super::stages::*;
use crate::context::cache::{system_clock, CacheConfig, ResultCache};
use crate::context::plan::{self, PlanStep, MAX_PROVIDER_CALLS};
use crate::context::{
    Broker, BrokerOutput, BrokerRequest, ExternalSource, HostExternal, NativeContextSource,
    SubprocessNative,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use crate::profile::ResolvedLaunch;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::PathBuf;

/// Provenance label of compiler facts (also what the byte budget protects).
pub const COMPILER_VERIFIED: &str = "compiler-verified";
const MAX_HANDLES: usize = 8;

/// Retrieval facts of the last collection, surfaced through `take_plan_report`.
#[derive(Default)]
struct PlanReport {
    steps: Vec<Value>,
    invocations: u32,
    cache_hits: u32,
    omitted: usize,
    exhaustive: bool,
    definitive_absence: bool,
    references: bool,
    coverage_complete: bool,
    unknowns: Vec<String>,
}

pub struct BrokerContext {
    id: String,
    native_only: Broker,
    full: Broker,
    calls: u32,
    note: Option<String>,
    report: PlanReport,
    /// Provider queries spent for the current task.
    planned_calls: u32,
    follow_ups: u32,
    last_step: Option<PlanStep>,
    /// Continuation handles issued by the last collection.
    handles: BTreeSet<String>,
    seen: BTreeSet<(String, String, String)>,
}

fn subprocess(c: &std::path::Path) -> Option<Box<dyn NativeContextSource>> {
    Some(Box::new(SubprocessNative::new(c.to_path_buf())))
}

impl BrokerContext {
    /// `launch` is the resolved external provider; `compiler` the service executable.
    pub fn new(
        compiler: PathBuf,
        launch: ResolvedLaunch,
        env: crate::cli::Environment,
        lock_digest: String,
        config_digest: String,
        scope: Vec<String>,
    ) -> HarnessResult<Self> {
        Self::new_with_config(
            compiler,
            launch,
            env,
            lock_digest,
            config_digest,
            scope,
            Default::default(),
        )
    }

    /// As [`Self::new`], forwarding validated adapter config (`SEMAPRAX_HARNESS_CFG_*`).
    pub fn new_with_config(
        compiler: PathBuf,
        launch: ResolvedLaunch,
        env: crate::cli::Environment,
        lock_digest: String,
        config_digest: String,
        scope: Vec<String>,
        config_env: std::collections::BTreeMap<String, String>,
    ) -> HarnessResult<Self> {
        let id = launch.provider_id.clone();
        let ext = HostExternal::new(launch, env, lock_digest, config_digest, scope)
            .with_config_env(config_env);
        Self::with_sources(
            subprocess(&compiler),
            subprocess(&compiler),
            Box::new(ext),
            id,
            None,
        )
    }

    /// Cache provider answers under `root` (the CLI uses `<harness home>/cache/context`).
    /// Keys bind the provider lock, configuration, working-tree content and
    /// worktree; a hit is not a provider invocation. Opt-in: without it every
    /// collection queries the provider.
    pub fn with_cache(mut self, root: PathBuf) -> Self {
        let c = ResultCache::new(root, CacheConfig::default(), system_clock());
        self.full.set_cache(Some(c));
        self
    }

    /// Compose from explicit sources (tests substitute counting providers).
    pub fn with_sources(
        native_full: Option<Box<dyn NativeContextSource>>,
        native_only: Option<Box<dyn NativeContextSource>>,
        external: Box<dyn ExternalSource>,
        id: String,
        cache: Option<ResultCache>,
    ) -> HarnessResult<Self> {
        let mut full = Broker::new(native_full, cache);
        full.add_provider(external)?;
        Ok(Self {
            id,
            native_only: Broker::new(native_only, None),
            full,
            calls: 0,
            note: None,
            report: PlanReport::default(),
            planned_calls: 0,
            follow_ups: 0,
            last_step: None,
            handles: BTreeSet::new(),
            seen: BTreeSet::new(),
        })
    }

    fn broker_request(&self, req: &ContextRequest, step: Option<&PlanStep>) -> BrokerRequest {
        let query = match step {
            Some(s) => s.query.clone(),
            None => req
                .seed
                .map(str::to_string)
                .unwrap_or_else(|| req.query.clone()),
        };
        let mut b = BrokerRequest::new(&query, req.max_bytes.max(2048));
        b.symbol = req
            .seed
            .map(str::to_string)
            .or_else(|| step.and_then(|s| s.symbol.clone()));
        if let Some(s) = step {
            b.max_items = s.max_items;
            b.references = s.references;
            b.exhaustive = s.exhaustive;
            if s.references {
                // Reference discovery keeps every item so coverage can be judged.
                b.symbol = s.symbol.clone();
            } else {
                // The compiler owns `.spx`; the provider is asked only for the rest.
                b.exclude_extensions = vec!["spx".into()];
            }
        }
        b
    }

    /// One provider-backed broker call. A provider failure keeps native facts,
    /// marks them incomplete and records what is unknown; it never recurses.
    fn run_full(
        &mut self,
        req: &ContextRequest,
        breq: &BrokerRequest,
        what: &str,
    ) -> Result<(BrokerOutput, bool), HarnessDiagnostic> {
        self.planned_calls += 1;
        match self.full.context(&req.project, breq) {
            Ok(mut o) => {
                if o.cache_hits.values().any(|h| *h) {
                    self.report.cache_hits += 1;
                } else {
                    self.calls += 1;
                    self.report.invocations += 1;
                }
                // The broker absorbs an unusable provider into its report; a
                // task must still see it as a failure, not as "nothing found".
                let v: Value = serde_json::from_str(&o.rendered).unwrap_or(Value::Null);
                let bad = v["providers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|p| matches!(p["status"].as_str(), Some("unavailable" | "stale")));
                if let Some(p) = bad {
                    let d0 = &p["diagnostics"][0];
                    let code = d0["code"].as_str().unwrap_or("SPX-HPE030");
                    self.note = Some(format!(
                        "external context provider failed, compiler facts only: {code} {}",
                        d0["message"].as_str().unwrap_or("provider unavailable")
                    ));
                    self.report.unknowns.push(format!(
                        "repository provider failed ({code}): {what} not retrieved; compiler facts are valid but say nothing about it"
                    ));
                    o.native.iter_mut().for_each(|n| n.complete = false);
                    return Ok((o, false));
                }
                Ok((o, true))
            }
            Err(e) => {
                self.calls += 1;
                self.report.invocations += 1;
                self.note = Some(format!(
                    "external context provider failed, compiler facts only: {} {}",
                    e.code, e.message
                ));
                self.report.unknowns.push(format!(
                    "repository provider failed ({}): {what} not retrieved; compiler facts are valid but say nothing about it",
                    e.code
                ));
                self.native_only.context(&req.project, breq).map(|mut o| {
                    o.native.iter_mut().for_each(|n| n.complete = false);
                    (o, false)
                })
            }
        }
    }

    fn start(&mut self) {
        self.report = PlanReport::default();
        self.planned_calls = 0;
        self.follow_ups = 0;
        self.last_step = None;
        self.handles.clear();
        self.seen.clear();
    }

    fn absorb(&mut self, out: &BrokerOutput, step: Option<&PlanStep>, ok: bool, kind: &str) {
        let v: Value = serde_json::from_str(&out.rendered).unwrap_or(Value::Null);
        for h in v["omitted"]["handles"].as_array().into_iter().flatten() {
            if let (Some(h), true) = (h["handle"].as_str(), self.handles.len() < MAX_HANDLES) {
                self.handles.insert(h.to_string());
            }
        }
        self.report.omitted += out.omitted;
        self.report.exhaustive = out.exhaustive;
        self.report.definitive_absence = out.definitive_absence;
        self.report.coverage_complete = v["coverage"]["complete"].as_bool().unwrap_or(false);
        if out.omitted > 0 {
            self.report.unknowns.push(format!(
                "{} provider item(s) omitted by the byte budget; continuation handles carry them",
                out.omitted
            ));
        }
        if let Some(s) = step {
            self.report.references |= s.references;
            let mut j = s.to_json();
            j["kind"] = json!(kind);
            j["provider_ok"] = json!(ok);
            j["items"] = json!(out.external.len());
            j["cache_hit"] = json!(out.cache_hits.values().any(|h| *h));
            self.report.steps.push(j);
        }
    }

    fn packet(
        &mut self,
        out: &BrokerOutput,
        extra: &[crate::context::ContextItem],
        external: bool,
    ) -> ContextPacket {
        let mut items = Vec::new();
        let all = out
            .native
            .iter()
            .map(|n| (n, true))
            .chain(out.external.iter().chain(extra.iter()).map(|e| (e, false)));
        for (it, native) in all {
            let label = format!("{}:{}-{}", it.path, it.span.start_line, it.span.end_line);
            let mut text = it.text.clone().unwrap_or_else(|| {
                format!("structural item {} digest {}", it.provider_id, it.digest)
            });
            for e in &it.edges {
                text.push_str(&format!("\nedge {} -> {}", e.relation, e.target_path));
                if let Some(r) = &e.resolution {
                    text.push_str(&format!(" ({r})"));
                }
            }
            let provenance = if native {
                COMPILER_VERIFIED.to_string()
            } else {
                // Provider output is a hint, never compiler verification.
                format!("external:{}", it.provenance.as_str())
            };
            // Exact slice + provenance: identical material is sent once.
            let key = (
                label.clone(),
                provenance.clone(),
                sha256_plain(text.as_bytes()),
            );
            if !self.seen.insert(key) {
                continue;
            }
            items.push(ContextItem {
                label,
                provenance,
                text,
            });
        }
        let native_complete = !out.native.is_empty() && out.native.iter().all(|n| n.complete);
        ContextPacket {
            provider: self.id.clone(),
            items,
            complete: native_complete || external,
        }
    }

    fn satisfied(items: &[crate::context::ContextItem], want: &[String]) -> bool {
        items.iter().any(|i| {
            !i.path.ends_with(".spx")
                && (want.is_empty() || want.iter().any(|w| w.eq_ignore_ascii_case(&i.language)))
        })
    }
}

impl ContextStage for BrokerContext {
    fn id(&self) -> String {
        self.id.clone()
    }

    fn plans(&self) -> bool {
        true
    }

    fn collect(&mut self, req: &ContextRequest) -> Result<ContextPacket, StageFailure> {
        self.start();
        let breq = self.broker_request(req, None);
        let native_complete =
            |o: &BrokerOutput| !o.native.is_empty() && o.native.iter().all(|n| n.complete);
        let mut out = None;
        let mut external = false;
        match req.external {
            ExternalContext::Never => {
                out = Some(self.native_only.context(&req.project, &breq));
            }
            ExternalContext::WhenNeeded => {
                let first = self.native_only.context(&req.project, &breq);
                if matches!(&first, Ok(o) if native_complete(o)) {
                    out = Some(first);
                }
            }
            ExternalContext::Always => {}
        }
        let out = match out {
            Some(o) => o,
            None => self
                .run_full(req, &breq, "repository discovery")
                .map(|(o, ok)| {
                    external = ok;
                    o
                }),
        };
        let out = out.map_err(StageFailure::Unavailable)?;
        self.absorb(&out, None, external, "legacy");
        Ok(self.packet(&out, &[], external))
    }

    fn collect_planned(
        &mut self,
        req: &ContextRequest,
        step: &PlanStep,
    ) -> Result<ContextPacket, StageFailure> {
        self.start();
        let breq = self.broker_request(req, Some(step));
        let (out, ok) = self
            .run_full(
                req,
                &breq,
                "the task's foreign-language or configuration evidence",
            )
            .map_err(StageFailure::Unavailable)?;
        self.absorb(&out, Some(step), ok, "initial");
        self.last_step = Some(step.clone());
        let mut extra: Vec<crate::context::ContextItem> = Vec::new();
        let mut external = ok;
        // The need is concrete but the ranked page did not surface it: one
        // goal-worded retry, then an explicit unknown. Never a full dump.
        if ok && !step.references && !Self::satisfied(&out.external, &step.want_languages) {
            if let Some(fq) = step.fallback_query.as_ref().filter(|q| **q != step.query) {
                let mut f = step.clone();
                f.label = "follow-up";
                f.query = fq.clone();
                f.fallback_query = None;
                let b2 = self.broker_request(req, Some(&f));
                let (o2, ok2) = self
                    .run_full(req, &b2, "the unmet task need")
                    .map_err(StageFailure::Unavailable)?;
                self.absorb(&o2, Some(&f), ok2, "unmet-need");
                external &= ok2;
                extra = o2.external;
            }
        }
        if ok && !step.references {
            let all = [out.external.clone(), extra.clone()].concat();
            if !Self::satisfied(&all, &step.want_languages) {
                self.report.unknowns.push(
                    "no foreign-boundary item was retrieved; ranked search cannot prove that none exists"
                        .into(),
                );
            }
        }
        if step.references && ok && !out.exhaustive {
            self.report.unknowns.push(
                "exhaustive reference coverage declined: the provider did not establish scoped complete coverage"
                    .into(),
            );
        }
        Ok(self.packet(&out, &extra, external))
    }

    fn follow_up(
        &mut self,
        req: &ContextRequest,
        failure: &str,
    ) -> Result<Option<ContextPacket>, StageFailure> {
        let Some(prior) = self.last_step.clone() else {
            return Ok(None);
        };
        if self.follow_ups >= 1 || self.planned_calls >= MAX_PROVIDER_CALLS {
            return Ok(None);
        }
        let Some(step) = plan::follow_up_step(&prior, failure) else {
            return Ok(None);
        };
        self.follow_ups += 1;
        let breq = self.broker_request(req, Some(&step));
        let (out, ok) = self
            .run_full(req, &breq, "the failed candidate's subject")
            .map_err(StageFailure::Unavailable)?;
        self.absorb(&out, Some(&step), ok, "failed-candidate");
        let mut p = self.packet(&out, &[], ok);
        // Native facts were already delivered; only new provider material is added.
        p.items.retain(|i| i.provenance != COMPILER_VERIFIED);
        Ok(Some(p))
    }

    fn expand(
        &mut self,
        req: &ContextRequest,
        handle: &str,
    ) -> Result<ContextPacket, StageFailure> {
        let refuse =
            |m: &str| StageFailure::Refused(HarnessDiagnostic::new("SPX-HPD130", m.to_string()));
        if !self.handles.contains(handle) {
            return Err(refuse(
                "unknown continuation handle: it was not issued by this stage's last collection",
            ));
        }
        let h =
            plan::parse_handle(handle).ok_or_else(|| refuse("malformed continuation handle"))?;
        let text =
            plan::expand_handle(&req.project, &h, req.max_bytes).map_err(StageFailure::Refused)?;
        let label = format!("{}:{}-{}", h.path, h.start, h.end);
        let key = (
            label.clone(),
            "external:inferred".to_string(),
            sha256_plain(text.as_bytes()),
        );
        let items = if self.seen.insert(key) {
            vec![ContextItem {
                label,
                provenance: "external:inferred".into(),
                text,
            }]
        } else {
            vec![]
        };
        Ok(ContextPacket {
            provider: self.id.clone(),
            items,
            complete: false,
        })
    }

    fn calls(&self) -> u32 {
        self.calls
    }

    fn take_note(&mut self) -> Option<String> {
        self.note.take()
    }

    fn take_plan_report(&mut self) -> Option<Value> {
        let r = std::mem::take(&mut self.report);
        Some(json!({
            "steps": r.steps, "provider_invocations": r.invocations, "cache_hits": r.cache_hits,
            "max_provider_calls": MAX_PROVIDER_CALLS, "omitted_items": r.omitted,
            // Ranked provider output is top-N unless the provider proved scoped coverage.
            "ranked": !r.exhaustive, "exhaustive": r.exhaustive,
            "absence_provable": r.definitive_absence, "reference_query": r.references,
            "coverage_complete": r.coverage_complete,
            "continuation": self.handles.iter().cloned().collect::<Vec<_>>(),
            "unknowns": r.unknowns,
        }))
    }
}
