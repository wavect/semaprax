//! The development pipeline. One lineage and one snapshot revision bind every
//! step: authenticate snapshot -> diagnose -> context -> budget -> route ->
//! generate -> validate -> check -> present -> publish. Publication happens
//! only through the compiler's route under a preexisting host policy.

use super::attempt::{self, PromptCtx};
use super::budget::{BudgetConfig, RequestCount, TaskLedger};
use super::checks::CheckSpec;
use super::compiler::{CompilerDiagnostic, CompilerService, PublishError};
use super::composition::Composition;
use super::journal::Journal;
use super::lineage::Lineage;
use super::policy::{ApplyPolicy, REQUIREMENTS};
use super::report::{ProviderUse, Report};
use super::snapshot::Snapshot;
use super::stages::*;
use crate::context::plan;
use crate::decision::{DecisionInvoker, EnablementGate, ModelPlan, ProviderMode, ProviderProfile};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{canonical, sha256_plain};
use crate::observe::{
    Availability, Observation, Observer, Outcome as ObsOutcome, Role, Stage, TokenCount,
};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::time::Instant;

pub struct RunConfig {
    pub snapshot: Snapshot,
    pub task: Task,
    pub context_max_bytes: usize,
    pub cache_dir: PathBuf,
    pub lock_digest: String,
    pub providers: Vec<ProviderUse>,
    pub composition: Composition,
    pub apply_policy: Option<ApplyPolicy>,
    /// Authorized checks run on the candidate after the compiler's own checks.
    pub checks: Vec<CheckSpec>,
    /// Rendered skill prompt included in the proposal request (HP-13).
    pub skill_prompt: Option<SkillPromptUse>,
    /// `[model]` guarantees; enforced on every candidate model of the route.
    pub endpoint_policy: crate::endpoint::EndpointPolicy,
    /// Plans from a machine-local logical model binding (`[model] logical`).
    pub model_plans: Option<Vec<ModelPlan>>,
    /// Setup-time notes (for example a provider that could not be launched).
    pub notes: Vec<String>,
    /// Request budget: policy, model-to-tokenizer map and supplied tokenizers (HN-11).
    pub budget: BudgetConfig,
    /// Cooperative cancellation for a session (checked between steps).
    pub cancel: Option<super::session::CancelFlag>,
    /// `[routing]`, evidence registry and session lock (HN-16).
    pub routing: super::routing::RoutingWiring,
}

/// Skill prompt chosen for this task and its model-visible size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillPromptUse {
    pub text: String,
    pub model_visible_bytes: usize,
    pub loaded: Vec<String>,
    /// Host-side cost report (TC-08): reasons, snapshot vs rendered bytes.
    pub cost_report: Option<serde_json::Value>,
}

/// An external `decision.evaluate` provider offered to the router. The router
/// consults it only when its mode is explicit or its enablement gate passed.
pub struct DecisionStage<'a> {
    pub invoker: &'a mut dyn DecisionInvoker,
    pub profile: ProviderProfile,
    pub mode: ProviderMode,
    pub gate: EnablementGate,
}

pub struct Stages<'a> {
    pub decision: Option<DecisionStage<'a>>,
    pub native: &'a mut dyn ContextStage,
    pub external: Option<&'a mut dyn ContextStage>,
    pub proposer: &'a mut dyn ProposalStage,
    pub command: &'a mut dyn CommandStage,
}

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

pub(super) struct Ctx<'a> {
    pub(super) cfg: &'a RunConfig,
    pub(super) compiler: &'a dyn CompilerService,
    pub(super) lineage: &'a Lineage,
    pub(super) observer: &'a mut Observer,
    /// Whole-task reservations across attempts and router calls.
    pub(super) ledger: TaskLedger,
    pub(super) started: Instant,
}

impl Ctx<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe(
        &mut self,
        provider: &str,
        capability: &str,
        stage: Stage,
        role: Role,
        avail: Availability,
        ok: bool,
        started: Instant,
    ) {
        self.observe_sized(
            provider,
            capability,
            stage,
            role,
            avail,
            ok,
            started,
            (None, None, false),
        );
    }

    /// Record one metadata-only event; `sizes` are exact byte counts
    /// (before, after/incurred, model-visible) at the measurement boundary.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn observe_sized(
        &mut self,
        provider: &str,
        capability: &str,
        stage: Stage,
        role: Role,
        avail: Availability,
        ok: bool,
        started: Instant,
        sizes: (Option<u64>, Option<u64>, bool),
    ) {
        let mut o = Observation::new(
            provider,
            capability,
            stage,
            role,
            &format!("{}-{}", self.lineage.id, stage.as_str()),
        );
        o.source_revision = self.lineage.project.revision.clone();
        o.config_revision = self.lineage.lock_digest.clone();
        o.availability = avail;
        o.outcome = if ok {
            ObsOutcome::Ok
        } else {
            ObsOutcome::Failed
        };
        o.latency_ms = started.elapsed().as_millis() as u64;
        if role == Role::Transform {
            o.payload_id = Some(self.lineage.id.clone());
            o.before = sizes.0.map(TokenCount::bytes);
            o.after = sizes.1.map(TokenCount::bytes);
            o.model_visible = sizes.2;
        } else {
            o.incurred = sizes.1.map(TokenCount::bytes);
        }
        self.observer.record(o);
    }

    /// An incurred request measured at its exact serialized boundary: named
    /// tokens when a tokenizer is mapped, otherwise bytes (`tokenizer_unavailable`).
    pub(super) fn observe_incurred_at(
        &mut self,
        provider: &str,
        capability: &str,
        stage: Stage,
        ok: bool,
        started: Instant,
        count: &RequestCount,
    ) {
        let mut o = Observation::new(
            provider,
            capability,
            stage,
            Role::Incurred,
            &format!(
                "{}-{}-{}",
                self.lineage.id,
                stage.as_str(),
                self.ledger.entries.len()
            ),
        );
        o.source_revision = self.lineage.project.revision.clone();
        o.config_revision = self.lineage.lock_digest.clone();
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
        o.latency_ms = started.elapsed().as_millis() as u64;
        o.incurred = Some(count.token_count());
        self.observer.record(o);
    }

    pub(super) fn observe_incurred(
        &mut self,
        provider: &str,
        capability: &str,
        stage: Stage,
        count: &RequestCount,
    ) {
        self.observe_incurred_at(provider, capability, stage, true, Instant::now(), count);
    }
}

/// Run the pipeline once and return its report (never panics on refusal).
pub fn run(
    cfg: &RunConfig,
    compiler: &dyn CompilerService,
    mut stages: Stages,
    observer: &mut Observer,
) -> Report {
    let lineage = Lineage::new(cfg.snapshot.binding(), &cfg.lock_digest, &cfg.task.digest());
    let mut report = Report::new(&lineage.id, &cfg.snapshot.revision);
    report.providers = cfg.providers.clone();
    report.composition = cfg.composition.to_json();
    report.notes = cfg.notes.clone();
    let mut cx = Ctx {
        cfg,
        compiler,
        lineage: &lineage,
        observer,
        ledger: TaskLedger::default(),
        started: Instant::now(),
    };
    let result = drive(&mut cx, &mut stages, &mut report);
    if let Err(e) = result {
        report.status = match e.code {
            "SPX-HPD050" => "rejected",
            "SPX-HPD062" | "SPX-HPD071" | "SPX-HPD072" => "uncertain",
            "SPX-HPD092" if cfg.task.schema_version == 2 => "unsupported-goal",
            "SPX-HPD111" => "exhausted",
            "SPX-HPD112" => "no-progress",
            "SPX-HPD113" => "cancelled",
            _ => "refused",
        };
        report.refusals.push(e);
    }
    if !cx.ledger.entries.is_empty() {
        if report.context.is_null() {
            report.context = json!({});
        }
        report.context["task_ledger"] = cx.ledger.to_json();
    }
    report.compiler_commands = compiler.commands();
    let ext = stages.external.as_ref().map(|e| (e.id(), e.calls()));
    let nat = (stages.native.id(), stages.native.calls());
    let dec = stages
        .decision
        .as_ref()
        .map(|d| d.profile.provider_id.clone());
    let router_calls = report.route["router_calls"].as_u64().unwrap_or(0) as u32;
    let prop = (stages.proposer.id(), stages.proposer.calls());
    for p in &mut report.providers {
        if let Some((id, n)) = &ext {
            if &p.provider == id {
                p.invoked = *n;
            }
        }
        if dec.as_deref() == Some(p.provider.as_str()) {
            p.invoked = router_calls;
        }
        if p.provider == nat.0 && p.provider != NATIVE_CONTEXT_ID {
            p.invoked = nat.1;
        }
        if p.provider == prop.0 {
            p.invoked = prop.1;
        }
    }
    report.external_calls = report.providers.iter().map(|p| p.invoked).sum();
    report
}

pub(super) fn step(r: &mut Report, name: &str, outcome: &str) {
    r.steps.push((name.into(), outcome.into()));
}

fn drive(cx: &mut Ctx, st: &mut Stages, r: &mut Report) -> HarnessResult<()> {
    let cfg = cx.cfg;
    let task = &cfg.task;
    let v2 = task.schema_version == 2;
    let root = cfg.snapshot.root.clone();
    if v2 {
        r.schema_version = 2;
        r.task = task.summary_json();
    }
    let mut journal = Journal::open(&cfg.cache_dir, &cx.lineage.id)?;

    // Resume rules: never replay a publication or an unfinished side effect.
    match journal
        .state("publish")
        .map(|x| (x.state.clone(), x.detail.clone()))
    {
        Some((s, detail)) if s == "done" => {
            r.status = "published";
            r.publication = detail;
            r.notes
                .push("already published in this lineage; nothing was replayed".into());
            step(r, "resume", "published");
            return Ok(());
        }
        Some((s, _)) if s == "begin" || s == "uncertain" => {
            step(r, "resume", "uncertain");
            return Err(d("SPX-HPD071", "uncertain: manual reconciliation required; a publication in this lineage may have occurred, inspect the reference and do not retry blindly"));
        }
        _ => {}
    }

    // 1. authenticate snapshot.
    cfg.snapshot.verify_current()?;
    step(r, "snapshot", "authenticated");

    // 2. diagnose with the real compiler. Baseline health is a precondition,
    // not the task's completion.
    let started = Instant::now();
    let check = cx.compiler.check(&root)?;
    r.diagnostics = check.diagnostics.clone();
    let mut seed = task.seed.clone();
    if !check.ok {
        step(r, "diagnose", "check failed");
        if v2 && task.session.is_some() && task.mode != TaskMode::Plan {
            return super::session::repair_unverified(cx, st, &mut journal, r);
        }
        r.status = "diagnosed";
        r.notes.push("the base project does not verify; candidate operations need a verified base, so no repair was attempted".into());
        if v2 {
            r.notes.push("a `session` block in the task enables the isolated scratch source-repair path for an unverified baseline".into());
        }
        return Ok(());
    }
    let compiler_revision = check
        .revision
        .clone()
        .expect("verified check has a revision");
    r.compiler_revision = Some(compiler_revision.clone());
    let test = cx.compiler.test(&root)?;
    let green = test.passed;
    if green && task.mode == TaskMode::Repair {
        step(r, "diagnose", "clean");
        if v2 {
            r.status = "unchanged-repair-baseline";
            r.notes.push("repair mode on a healthy baseline: no model call; use mode `change` for an intentional change".into());
        } else {
            r.status = "no-repair-needed";
            if task.goal != Task::default().goal {
                r.notes.push("a task goal was supplied but the legacy v1 task is a repair task; use `semaprax.harness-task.v2` with mode `change` for an intentional change".into());
            }
        }
        return Ok(());
    }
    if !green {
        if let Some(f) = &test.failure {
            r.diagnostics.push(CompilerDiagnostic {
                code: "test-failure".into(),
                message: f.clone(),
                path: None,
                line: None,
            });
        }
        if seed.is_none() {
            seed = test.failing_function.clone();
        }
        if task.mode != TaskMode::Repair && task.session.is_none() {
            step(r, "diagnose", "baseline tests fail");
            r.status = "diagnosed";
            r.notes.push("a change or plan task needs a green baseline; baseline tests fail, so run a repair task (or a session) first".into());
            return Ok(());
        }
        step(
            r,
            "diagnose",
            &format!("test failure in {}", seed.as_deref().unwrap_or("?")),
        );
    } else {
        step(r, "diagnose", "baseline healthy; explicit task proceeds");
    }
    // Installed compiler operations (HN-01): advertised, never assumed.
    let ops: Vec<String> = if v2 {
        let ops = cx.compiler.supported_intents(&root, &compiler_revision)?;
        r.operations = json!({"source": "installed compiler", "kinds": ops});
        if let Some(op) = &task.operation {
            if !ops.contains(op) {
                return Err(d(
                    "SPX-HPD092",
                    format!("unsupported goal: operation `{op}` is not admitted by the installed compiler; installed operations: {}", ops.join(", ")),
                ));
            }
        }
        if ops.is_empty() {
            return Err(d(
                "SPX-HPD092",
                "unsupported goal: the installed compiler admits no candidate operation",
            ));
        }
        ops
    } else {
        INTENT_KINDS.iter().map(|s| s.to_string()).collect()
    };
    if v2 && task.session.is_some() && task.mode == TaskMode::Change {
        return super::session::run_session(cx, st, &mut journal, r, &compiler_revision, seed, ops);
    }
    let diag_text = r
        .diagnostics
        .iter()
        .map(|x| format!("{}: {}", x.code, x.message))
        .collect::<Vec<_>>()
        .join("\n");
    let diag_view = if diag_text.is_empty() {
        String::new()
    } else {
        st.command.view("diagnostics", &diag_text, 4096)
    };
    cx.observe(
        "semaprax/compiler",
        "compiler.service",
        Stage::ContextSelect,
        Role::Local,
        Availability::Available,
        true,
        started,
    );
    let query = if diag_text.is_empty() {
        task.goal.chars().take(256).collect()
    } else {
        diag_text.chars().take(256).collect()
    };
    let (kept, _used) = gather_context(cx, st, r, &root, &seed, query)?;

    // 5-6. route, fit the exact request to the model, generate a proposal.
    cfg.snapshot.verify_current()?;
    let pc = PromptCtx {
        revision: &compiler_revision,
        seed: seed.as_deref(),
        diag_view: &diag_view,
        kept: &kept,
        ops: &ops,
        feedback: &[],
        attempt: 1,
        scratch_repair: false,
    };
    let proposal = match attempt::propose_step(cx, st, &mut journal, r, &pc, "generate") {
        Err(e) if e.code == "SPX-HPD090" && task.mode == TaskMode::Plan => {
            step(r, "route", "plan");
            r.status = "planned";
            r.notes.push(format!("plan without a proposal source: {}; context and installed operations are reported only", e.message));
            return Ok(());
        }
        other => other?,
    };
    #[allow(clippy::unnecessary_to_owned)]
    step(
        r,
        "route",
        // Owned copy: `r` is also the first (mutable) argument.
        &r.route["choice"].as_str().unwrap_or("").to_string(),
    );
    attempt::require_intent(&proposal, &ops)?;
    step(r, "proposal", &proposal.kind);

    // 7. validate through the compiler's candidate operation, same revision.
    cfg.snapshot.verify_current()?;
    let (change, preview) = attempt::validate_step(cx, &root, &compiler_revision, &proposal, r)?;
    step(r, "validate", "candidate admitted by the compiler");
    if task.mode == TaskMode::Plan {
        r.status = "planned";
        r.notes.push("read-only plan: the compiler admitted the change; no candidate was checked, exported or published".into());
        return Ok(());
    }

    // 8. authorized checks on the candidate: the verdict is the compiler's.
    let checks = attempt::candidate_checks(cx, st.command, &root, "1", &preview, r, true)?;
    r.checks = checks;
    step(r, "check", "candidate verified and tests passed");

    // 9. present + approval requirement; 10. publish only under a host policy.
    cfg.snapshot.verify_current()?;
    let capsule = cx.compiler.candidate_export(&root, &change)?;
    if capsule.base_revision != compiler_revision
        || capsule.candidate_project_revision != preview.candidate_revision
    {
        return Err(d(
            "SPX-HPD041",
            "exported capsule is bound to a different revision than the validated candidate",
        ));
    }
    present_and_publish(cx, &mut journal, r, &capsule, &proposal.kind)
}

/// Native context first; external only when needed; then the final byte budget.
pub(super) fn gather_context(
    cx: &mut Ctx,
    st: &mut Stages,
    r: &mut Report,
    root: &std::path::Path,
    seed: &Option<String>,
    query: String,
) -> HarnessResult<(Vec<ContextItem>, usize)> {
    let cfg = cx.cfg;
    let root = root.to_path_buf();
    if root == cfg.snapshot.root {
        cfg.snapshot.verify_current()?;
    }
    let budget = cfg.context_max_bytes;
    let creq = ContextRequest {
        lineage: cx.lineage,
        project: root.clone(),
        seed: seed.as_deref(),
        query,
        max_bytes: budget,
        external: cfg.task.external_context,
    };
    // A plan-capable native stage (the broker) is also the repository provider
    // slot: its native step must not itself call the provider.
    let combined = st.external.is_none() && st.native.plans();
    let native_req = ContextRequest {
        lineage: cx.lineage,
        project: root.clone(),
        seed: seed.as_deref(),
        query: creq.query.clone(),
        max_bytes: budget,
        external: if combined {
            ExternalContext::Never
        } else {
            cfg.task.external_context
        },
    };
    let started = Instant::now();
    let native = st.native.collect(&native_req);
    if let Some(n) = st.native.take_note() {
        r.notes.push(n);
    }
    cx.observe(
        &st.native.id(),
        "context.repository",
        Stage::ContextSelect,
        Role::Local,
        if native.is_ok() {
            Availability::Available
        } else {
            Availability::Unavailable
        },
        native.is_ok(),
        started,
    );
    let native_complete = native.as_ref().map(|p| p.complete).unwrap_or(false);
    // HN-13: evidence needs come from the task, not from graph completeness.
    let diags: Vec<(String, String)> = r
        .diagnostics
        .iter()
        .map(|x| (x.code.clone(), x.message.clone()))
        .collect();
    let rp = plan::plan(&root, &cfg.task.goal, seed.as_deref(), &diags);
    let needs_external = match cfg.task.external_context {
        ExternalContext::Never => false,
        ExternalContext::Always => true,
        ExternalContext::WhenNeeded => {
            rp.needs.needs_provider() || (!native_complete && !rp.needs.spx_local())
        }
    };
    let mut packets: Vec<ContextPacket> = Vec::new();
    let mut unknowns: Vec<String> = Vec::new();
    let mut stage_report: Option<Value> = None;
    if let Ok(p) = native {
        packets.push(p);
    } else if let Err(e) = native {
        r.notes
            .push(format!("native context unavailable: {}", failure_text(&e)));
    }
    if needs_external {
        match plan_stage(st) {
            Some(ext) => {
                let started = Instant::now();
                let got = match &rp.initial {
                    Some(step) => ext.collect_planned(&creq, step),
                    None => ext.collect(&creq),
                };
                cx.observe(
                    &ext.id(),
                    "context.repository",
                    Stage::ContextSelect,
                    Role::Transform,
                    if got.is_ok() {
                        Availability::Available
                    } else {
                        Availability::Fallback
                    },
                    got.is_ok(),
                    started,
                );
                stage_report = ext.take_plan_report();
                match got {
                    Ok(p) => packets.push(p),
                    Err(e) => {
                        unknowns.push(format!(
                            "repository provider unavailable: {}; the task's foreign evidence was not retrieved",
                            failure_text(&e)
                        ));
                        r.notes.push(format!(
                            "external context provider failed, using builtin only: {}",
                            failure_text(&e)
                        ))
                    }
                }
            }
            None => {
                unknowns.push("no repository provider is selected: foreign-language evidence was not retrieved".into());
                r.notes.push(
                    "external context was wanted but only builtin providers are selected".into(),
                )
            }
        }
    } else {
        if rp.needs.needs_provider() {
            unknowns.push("the task names foreign-language or configuration evidence but external context is `never`".into());
        }
        r.notes.push(if rp.needs.spx_local() {
            "native-only task: no external provider call".to_string()
        } else {
            "native context sufficient: no external provider call".to_string()
        });
    }
    step(r, "context", &format!("{} packet(s)", packets.len()));

    // 4. final context budget. Native items are protected; external items fill the rest.
    let mut kept: Vec<ContextItem> = Vec::new();
    let (mut used, mut dropped) = (0usize, 0usize);
    for p in &packets {
        for it in &p.items {
            if kept.contains(it) {
                continue; // exact slice and provenance already present
            }
            let native_packet = it.provenance == super::broker_stage::COMPILER_VERIFIED;
            if used + it.bytes() <= budget {
                used += it.bytes();
                kept.push(it.clone());
            } else if native_packet {
                return Err(d("SPX-HPD020", format!("native compiler context ({} bytes) exceeds the context budget of {budget} bytes", it.bytes())));
            } else {
                dropped += 1;
            }
        }
    }
    // A plan-capable stage answers both the native and the provider step: one name.
    let mut provider_names: Vec<String> = Vec::new();
    for p in &packets {
        if !provider_names.contains(&p.provider) {
            provider_names.push(p.provider.clone());
        }
    }
    r.context = json!({"budget_bytes": budget, "used_bytes": used, "items": kept.len(), "dropped_external_items": dropped,
                       "providers": provider_names});
    if dropped > 0 {
        unknowns.push(format!(
            "{dropped} external item(s) dropped by the context budget"
        ));
    }
    let mut plan_json = json!({"schema": plan::PLAN_SCHEMA, "needs": rp.needs.to_json(),
        "planned_steps": rp.initial.iter().map(|s| s.to_json()).collect::<Vec<_>>(),
        "provider_consulted": needs_external && (st.external.is_some() || st.native.plans())});
    if let Some(sr) = stage_report {
        plan_json["retrieval"] = sr;
    }
    unknowns.extend(
        plan_json["retrieval"]["unknowns"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|u| u.as_str().map(str::to_string)),
    );
    plan_json["unknowns"] = json!(unknowns);
    r.context["plan"] = plan_json;
    step(r, "budget", &format!("{used}/{budget} bytes"));
    // Skills: the exact model-visible prompt block, counted at its boundary.
    if let Some(sp) = &cfg.skill_prompt {
        cx.observe_sized(
            "semaprax/plain-skills",
            "skill.catalog",
            Stage::SkillCatalog,
            Role::Transform,
            Availability::Available,
            true,
            Instant::now(),
            (None, Some(sp.model_visible_bytes as u64), true),
        );
        r.context["skills"] =
            json!({"loaded": sp.loaded, "model_visible_bytes": sp.model_visible_bytes});
        if let Some(c) = &sp.cost_report {
            r.context["skills"]["cost"] = c.clone();
        }
        step(
            r,
            "skills",
            &format!(
                "{} skill(s), {} bytes",
                sp.loaded.len(),
                sp.model_visible_bytes
            ),
        );
    }
    cx.observe_sized(
        "semaprax/context-budget",
        "context.repository",
        Stage::ContextSelect,
        Role::Transform,
        Availability::Available,
        true,
        Instant::now(),
        (
            Some(
                packets
                    .iter()
                    .flat_map(|p| p.items.iter())
                    .map(|i| i.bytes() as u64)
                    .sum(),
            ),
            Some(used as u64),
            false,
        ),
    );

    Ok((kept, used))
}

/// The stage serving provider plans: the external slot, else a plan-capable native stage.
pub(super) fn plan_stage<'a, 'b>(
    st: &'a mut Stages<'b>,
) -> Option<&'a mut (dyn ContextStage + 'a)> {
    if let Some(e) = st.external.as_mut() {
        return Some(&mut **e);
    }
    if st.native.plans() {
        return Some(&mut *st.native);
    }
    None
}

/// Merge provider items into `kept` under the context budget: exact duplicates
/// are skipped, items that do not fit are counted (never truncated).
fn merge_fitting(kept: &mut Vec<ContextItem>, items: Vec<ContextItem>, budget: usize) -> usize {
    let mut used: usize = kept.iter().map(ContextItem::bytes).sum();
    let mut dropped = 0;
    for it in items {
        if kept.contains(&it) {
            continue;
        }
        if used + it.bytes() <= budget {
            used += it.bytes();
            kept.push(it);
        } else {
            dropped += 1;
        }
    }
    dropped
}

/// HN-13: after a failed candidate, one focused follow-up retrieval named by the
/// failure's own identifiers. Returns whether new material was added; a stage
/// without a plan, a spent call bound or a failure that names nothing new makes
/// no call. Provider trouble is reported, never fatal.
pub(super) fn follow_up_context(
    cx: &mut Ctx,
    st: &mut Stages,
    r: &mut Report,
    root: &std::path::Path,
    seed: &Option<String>,
    failure: &str,
    kept: &mut Vec<ContextItem>,
) -> HarnessResult<bool> {
    let cfg = cx.cfg;
    let creq = ContextRequest {
        lineage: cx.lineage,
        project: root.to_path_buf(),
        seed: seed.as_deref(),
        query: String::new(),
        max_bytes: cfg.context_max_bytes,
        external: cfg.task.external_context,
    };
    let Some(ext) = plan_stage(st) else {
        return Ok(false);
    };
    let started = Instant::now();
    let got = ext.follow_up(&creq, failure);
    cx.observe(
        &ext.id(),
        "context.repository",
        Stage::ContextSelect,
        Role::Transform,
        if got.is_ok() {
            Availability::Available
        } else {
            Availability::Fallback
        },
        got.is_ok(),
        started,
    );
    let report = ext.take_plan_report();
    let added = match got {
        Ok(Some(p)) => {
            let n = p.items.len();
            let dropped = merge_fitting(kept, p.items, cfg.context_max_bytes);
            r.context["plan"]["follow_up"] = json!({"added_items": n.saturating_sub(dropped), "dropped_items": dropped, "report": report});
            n > dropped
        }
        Ok(None) => {
            r.context["plan"]["follow_up"] =
                json!({"added_items": 0, "reason": "no new identifiers or call bound spent"});
            false
        }
        Err(e) => {
            r.notes
                .push(format!("context follow-up failed: {}", failure_text(&e)));
            r.context["plan"]["follow_up"] = json!({"added_items": 0, "reason": "provider failed"});
            false
        }
    };
    step(r, "context-follow-up", if added { "added" } else { "none" });
    Ok(added)
}

/// HN-13: expand one continuation handle from the last collection into `kept`
/// without a provider call. Stale or unknown handles are refused.
pub(super) fn expand_context(
    cx: &mut Ctx,
    st: &mut Stages,
    root: &std::path::Path,
    handle: &str,
    kept: &mut Vec<ContextItem>,
) -> HarnessResult<usize> {
    let cfg = cx.cfg;
    let creq = ContextRequest {
        lineage: cx.lineage,
        project: root.to_path_buf(),
        seed: None,
        query: String::new(),
        max_bytes: cfg.context_max_bytes,
        external: cfg.task.external_context,
    };
    let Some(ext) = plan_stage(st) else {
        return Err(d(
            "SPX-HPD130",
            "no context stage issued continuation handles",
        ));
    };
    let p = ext.expand(&creq, handle).map_err(|e| match e {
        StageFailure::Refused(x) | StageFailure::Unavailable(x) | StageFailure::Uncertain(x) => x,
    })?;
    let n = p.items.len();
    let dropped = merge_fitting(kept, p.items, cfg.context_max_bytes);
    if dropped > 0 {
        return Err(d(
            "SPX-HPD131",
            "continuation slice does not fit the remaining context budget",
        ));
    }
    Ok(n)
}

/// Export bookkeeping, approval requirement and (only under a host policy) publication.
pub(super) fn present_and_publish(
    cx: &mut Ctx,
    journal: &mut Journal,
    r: &mut Report,
    capsule: &super::compiler::Capsule,
    kind: &str,
) -> HarnessResult<()> {
    let cfg = cx.cfg;
    let root = cfg.snapshot.root.clone();
    let capsule_path = cfg
        .cache_dir
        .join(format!("{}.capsule.json", cx.lineage.id));
    std::fs::write(&capsule_path, &capsule.bytes)
        .map_err(|e| d("SPX-HPD070", format!("capsule: {e}")))?;
    r.approval = json!({"required": true, "candidate_digest": capsule.candidate_digest,
        "capsule_digest": sha256_plain(&capsule.bytes),
        "publish_with": "semaprax project-candidate-git-publish <manifest> <capsule> <candidate_digest> <host-policy.json>"});
    step(r, "present", "approval required");

    // 10. publish only under an explicit preexisting host policy.
    let Some(policy) = cfg.apply_policy.as_ref().filter(|p| p.permits(kind)) else {
        r.status = if r.schema_version == 2 {
            "candidate-ready"
        } else {
            "approved-candidate-ready"
        };
        r.notes.push(
            "no apply policy permits publication; stopped at approved-candidate-ready".into(),
        );
        return Ok(());
    };
    cfg.snapshot.verify_current()?;
    journal.append(
        "publish",
        "begin",
        json!({"candidate_digest": capsule.candidate_digest, "policy": policy.digest}),
    )?;
    match cx.compiler.publish(
        &root,
        capsule,
        &capsule.candidate_digest,
        &policy.publication_policy,
    ) {
        Ok(receipt) => {
            let detail = json!({"published_commit": receipt.published_commit, "reference": receipt.reference, "candidate_digest": capsule.candidate_digest});
            journal.append("publish", "done", detail.clone())?;
            r.publication = detail;
            r.status = "published";
            step(r, "publish", "published through the compiler route");
            Ok(())
        }
        Err(PublishError::Refused(c)) => {
            journal.append("publish", "refused", json!({"code": c.code}))?;
            Err(d(
                "SPX-HPD061",
                format!(
                    "publication refused by the compiler: {} {}",
                    c.code, c.message
                ),
            ))
        }
        Err(PublishError::Uncertain(m)) => {
            journal.append("publish", "uncertain", json!({}))?;
            r.publication = json!({"state": "uncertain"});
            Err(d(
                "SPX-HPD062",
                format!(
                    "uncertain: manual reconciliation required; not retried ({})",
                    m.lines().next().unwrap_or("")
                ),
            ))
        }
    }
}

fn failure_text(e: &StageFailure) -> String {
    match e {
        StageFailure::Unavailable(x) | StageFailure::Refused(x) | StageFailure::Uncertain(x) => {
            format!("{} {}", x.code, x.message)
        }
    }
}

/// Canonical change bytes: the host fixes schema, base revision and the full
/// requirement inventory; only the intent comes from the proposal.
pub fn change_bytes(base_revision: &str, intent: &Value) -> Vec<u8> {
    let doc = json!({"schema": "semaprax.semantic-change.v1", "base_revision": base_revision, "intent": intent, "requirements": REQUIREMENTS});
    let mut s = canonical(&doc);
    s.push('\n');
    s.into_bytes()
}

/// Profile bindings -> provider usage rows (invocation counts filled later).
pub fn provider_rows(
    profile: Option<&crate::profile::ResolvedProfile>,
    disabled: bool,
) -> Vec<ProviderUse> {
    let Some(p) = profile else {
        return crate::contract::CapabilityKind::ALL
            .iter()
            .map(|k| ProviderUse {
                capability: k.as_str().into(),
                provider: "-".into(),
                state: "disabled".into(),
                invoked: 0,
            })
            .collect();
    };
    p.bindings
        .iter()
        .map(|b| ProviderUse {
            capability: b.kind.as_str().into(),
            provider: if b.provider_id.is_empty() {
                "-".into()
            } else {
                b.provider_id.clone()
            },
            state: if disabled {
                "disabled".into()
            } else {
                b.state.as_str().into()
            },
            invoked: 0,
        })
        .collect()
}
