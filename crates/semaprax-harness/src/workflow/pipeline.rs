//! The development pipeline. One lineage and one snapshot revision bind every
//! step: authenticate snapshot -> diagnose -> context -> budget -> route ->
//! generate -> validate -> check -> present -> publish. Publication happens
//! only through the compiler's route under a preexisting host policy.

use super::checks::CheckSpec;
use super::compiler::{CandidatePreview, CompilerDiagnostic, CompilerService, PublishError};
use super::composition::Composition;
use super::journal::Journal;
use super::lineage::Lineage;
use super::policy::{check_protected_facts, ApplyPolicy, REQUIREMENTS};
use super::report::{ProviderUse, Report};
use super::snapshot::Snapshot;
use super::stages::*;
use crate::decision::{
    decide, Budget, Confidentiality, ConfiguredProvider, DecisionInvoker, Destination,
    EnablementGate, LatencyClass, ModelPlan, ProviderMode, ProviderProfile, RouteContext,
    RouteInputs, RoutePolicy, RouteRequest, TaskFamily, TaskFeatures,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::{canonical, sha256_plain};
use crate::observe::{
    Availability, Observation, Observer, Outcome as ObsOutcome, Role, Stage, TokenCount,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
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
}

/// Skill prompt chosen for this task and its model-visible size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillPromptUse {
    pub text: String,
    pub model_visible_bytes: usize,
    pub loaded: Vec<String>,
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

struct Ctx<'a> {
    cfg: &'a RunConfig,
    compiler: &'a dyn CompilerService,
    lineage: &'a Lineage,
    observer: &'a mut Observer,
}

impl Ctx<'_> {
    #[allow(clippy::too_many_arguments)]
    fn observe(
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
    fn observe_sized(
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
    };
    let result = drive(&mut cx, &mut stages, &mut report);
    if let Err(e) = result {
        report.status = match e.code {
            "SPX-HPD050" => "rejected",
            "SPX-HPD062" | "SPX-HPD071" | "SPX-HPD072" => "uncertain",
            _ => "refused",
        };
        report.refusals.push(e);
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

fn step(r: &mut Report, name: &str, outcome: &str) {
    r.steps.push((name.into(), outcome.into()));
}

fn drive(cx: &mut Ctx, st: &mut Stages, r: &mut Report) -> HarnessResult<()> {
    let cfg = cx.cfg;
    let root = cfg.snapshot.root.clone();
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

    // 2. diagnose with the real compiler.
    let started = Instant::now();
    let check = cx.compiler.check(&root)?;
    r.diagnostics = check.diagnostics.clone();
    let mut seed = cfg.task.seed.clone();
    if !check.ok {
        step(r, "diagnose", "check failed");
        r.status = "diagnosed";
        r.notes.push("the base project does not verify; candidate operations need a verified base, so no repair was attempted".into());
        return Ok(());
    }
    let compiler_revision = check
        .revision
        .clone()
        .expect("verified check has a revision");
    r.compiler_revision = Some(compiler_revision.clone());
    let test = cx.compiler.test(&root)?;
    if test.passed {
        step(r, "diagnose", "clean");
        r.status = "no-repair-needed";
        return Ok(());
    }
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
    step(
        r,
        "diagnose",
        &format!("test failure in {}", seed.as_deref().unwrap_or("?")),
    );
    let diag_text = r
        .diagnostics
        .iter()
        .map(|x| format!("{}: {}", x.code, x.message))
        .collect::<Vec<_>>()
        .join("\n");
    let diag_view = st.command.view("diagnostics", &diag_text, 4096);
    cx.observe(
        "semaprax/compiler",
        "compiler.service",
        Stage::ContextSelect,
        Role::Local,
        Availability::Available,
        true,
        started,
    );

    // 3. context: native first; external only when needed.
    cfg.snapshot.verify_current()?;
    let budget = cfg.context_max_bytes;
    let creq = ContextRequest {
        lineage: cx.lineage,
        project: root.clone(),
        seed: seed.as_deref(),
        query: diag_text.chars().take(256).collect(),
        max_bytes: budget,
        external: cfg.task.external_context,
    };
    let started = Instant::now();
    let native = st.native.collect(&creq);
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
    let needs_external = match cfg.task.external_context {
        ExternalContext::Never => false,
        ExternalContext::Always => true,
        ExternalContext::WhenNeeded => !native_complete,
    };
    let mut packets: Vec<ContextPacket> = Vec::new();
    if let Ok(p) = native {
        packets.push(p);
    } else if let Err(e) = native {
        r.notes
            .push(format!("native context unavailable: {}", failure_text(&e)));
    }
    if needs_external {
        match st.external.as_mut() {
            Some(ext) => {
                let started = Instant::now();
                let got = ext.collect(&creq);
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
                match got {
                    Ok(p) => packets.push(p),
                    Err(e) => r.notes.push(format!(
                        "external context provider failed, using builtin only: {}",
                        failure_text(&e)
                    )),
                }
            }
            None => r
                .notes
                .push("external context was wanted but only builtin providers are selected".into()),
        }
    } else {
        r.notes
            .push("native context sufficient: no external provider call".into());
    }
    step(r, "context", &format!("{} packet(s)", packets.len()));

    // 4. final context budget. Native items are protected; external items fill the rest.
    let mut kept: Vec<ContextItem> = Vec::new();
    let (mut used, mut dropped) = (0usize, 0usize);
    for p in &packets {
        for it in &p.items {
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
    r.context = json!({"budget_bytes": budget, "used_bytes": used, "items": kept.len(), "dropped_external_items": dropped,
                       "providers": packets.iter().map(|p| p.provider.clone()).collect::<Vec<_>>()});
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

    // 5. route (policy first; rules only, zero router calls).
    let started = Instant::now();
    let (route_json, model) = route(cx, &cfg.task, used, st.decision.as_mut())?;
    let used_provider = route_json["provider"].as_str().unwrap_or("").to_string();
    let fell_back = route_json["source"]
        .as_str()
        .is_some_and(|s| s.starts_with("Fallback"));
    r.route = route_json;
    cx.observe(
        &used_provider,
        "decision.evaluate",
        Stage::Decision,
        Role::Local,
        if fell_back {
            Availability::Fallback
        } else {
            Availability::Available
        },
        true,
        started,
    );
    step(r, "route", &model);

    // 6. generate a proposal.
    cfg.snapshot.verify_current()?;
    let mut prompt = json!({
        "schema": "semaprax.harness-prompt.v1", "revision": compiler_revision, "goal": cfg.task.goal,
        "seed": seed, "diagnostics": diag_view, "intents": INTENT_KINDS,
        "context": kept.iter().map(|i| json!({"label": i.label, "provenance": i.provenance, "text": i.text})).collect::<Vec<_>>(),
    });
    if let Some(sp) = &cfg.skill_prompt {
        // Quoted data below host and compiler authority (framed by the skill service).
        prompt["skills"] = json!(sp.text);
    }
    let bytes = generate(cx, st, &mut journal, prompt, model, r)?;
    let proposal = parse_proposal(&bytes)?;
    if let Some(c) = proposal.claims.as_object() {
        r.ignored_claims = c.keys().cloned().collect();
    }
    step(r, "proposal", &proposal.kind);

    // 7. validate through the compiler's candidate operation, same revision.
    cfg.snapshot.verify_current()?;
    let change = change_bytes(&compiler_revision, &proposal.intent);
    let preview = cx.compiler.candidate_preview(&root, &change)?;
    check_protected_facts(&root, &compiler_revision, &proposal.kind, &preview)?;
    r.candidate = json!({"intent": proposal.kind, "base_revision": preview.base_revision, "candidate_revision": preview.candidate_revision,
                         "changed_files": preview.source_changes.iter().map(|c| c.path.clone()).collect::<Vec<_>>(),
                         "preview_digest": preview.digest});
    step(r, "validate", "candidate admitted by the compiler");

    // 8. authorized checks on the candidate: the verdict is the compiler's.
    let checks = candidate_checks(cx, st.command, &preview, r)?;
    r.checks = checks;
    step(r, "check", "candidate verified and tests passed");

    // 9. present + approval requirement.
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
    let Some(policy) = cfg
        .apply_policy
        .as_ref()
        .filter(|p| p.permits(&proposal.kind))
    else {
        r.status = "approved-candidate-ready";
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
        &capsule,
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

fn route(
    cx: &mut Ctx,
    task: &Task,
    context_bytes: usize,
    decision: Option<&mut DecisionStage>,
) -> HarnessResult<(Value, String)> {
    let family = TaskFamily::parse(&task.family).ok_or_else(|| {
        d(
            "SPX-HPD081",
            format!("unknown task_family `{}`", task.family),
        )
    })?;
    let features = TaskFeatures {
        task_family: family,
        estimated_context_tokens: (context_bytes / 4) as u64,
        requires_structured_output: true,
        requires_tools: false,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Interactive,
    };
    let catalog = match (&task.models, &cx.cfg.model_plans) {
        (Some(m), _) => RouteRequest::catalog_from_json(m)?,
        (None, Some(p)) => p.clone(),
        (None, None) => vec![ModelPlan {
            id: "workflow-default".into(),
            destination: Destination::Local,
            structured_output: true,
            tools: false,
            max_context: 1_000_000,
            est_cost_micros: 0,
            est_latency_ms: 1000,
            strength_rank: 1,
        }],
    };
    for m in &catalog {
        crate::endpoint::check_policy(
            cx.cfg.endpoint_policy,
            &crate::endpoint::AttemptOwnership::direct(),
            &m.destination,
        )?;
    }
    let request = RouteRequest::new(
        features,
        catalog,
        Budget {
            max_cost_micros: 1_000_000,
            max_latency_ms: 60_000,
            max_router_calls: u32::from(decision.is_some()),
        },
    )?;
    let inputs = RouteInputs {
        request,
        policy: RoutePolicy::default(),
    };
    let rctx = RouteContext {
        project: cx.lineage.project.clone(),
        lock_digest: cx.lineage.lock_digest.clone(),
        invocation_id: format!("route-{}", cx.lineage.id),
        lineage_id: cx.lineage.id.clone(),
        router_lineage: vec![],
        router_calls_used: 0,
        router_ms_used: 0,
    };
    let live = inputs.clone();
    let mut configured = decision.map(|d| ConfiguredProvider {
        profile: d.profile.clone(),
        invoker: &mut *d.invoker,
        mode: d.mode,
        gate: d.gate.clone(),
    });
    let dec = decide(
        &inputs,
        &rctx,
        configured.as_mut(),
        &move || live.clone(),
        None,
    )?;
    Ok((
        json!({"choice": dec.choice, "provider": dec.provider_id, "router_calls": dec.router_calls,
               "status": dec.provider_status, "source": format!("{:?}", dec.source)}),
        dec.choice,
    ))
}

fn generate(
    cx: &mut Ctx,
    st: &mut Stages,
    journal: &mut Journal,
    prompt: Value,
    model: String,
    r: &mut Report,
) -> HarnessResult<Vec<u8>> {
    let side = st.proposer.side_effecting();
    let cache = cx
        .cfg
        .cache_dir
        .join(format!("{}.proposal.json", cx.lineage.id));
    if side {
        if matches!(
            journal.state("generate").map(|x| x.state.as_str()),
            Some("done")
        ) {
            if let Ok(b) = std::fs::read(&cache) {
                r.notes.push(
                    "proposal reused from the journal; the model was not invoked again".into(),
                );
                return Ok(b);
            }
        }
        if journal.unfinished("generate")
            || matches!(
                journal.state("generate").map(|x| x.state.as_str()),
                Some("uncertain")
            )
        {
            return Err(d("SPX-HPD072", "uncertain: a model generation in this lineage began without a recorded result; it is not replayed, supply --proposal or change the task"));
        }
        journal.append("generate", "begin", json!({"provider": st.proposer.id()}))?;
    }
    let started = Instant::now();
    let req = ProposalRequest {
        lineage: cx.lineage,
        prompt,
        model,
    };
    let got = st.proposer.propose(&req);
    let prompt_bytes = crate::json::canonical(&req.prompt).len() as u64;
    cx.observe_sized(
        &st.proposer.id(),
        "model.generate",
        Stage::Generation,
        Role::Incurred,
        if got.is_ok() {
            Availability::Available
        } else {
            Availability::Unavailable
        },
        got.is_ok(),
        started,
        (None, Some(prompt_bytes), false),
    );
    match got {
        Ok(b) => {
            if side {
                std::fs::write(&cache, &b)
                    .map_err(|e| d("SPX-HPD070", format!("proposal cache: {e}")))?;
                journal.append("generate", "done", json!({"digest": sha256_plain(&b)}))?;
            }
            Ok(b)
        }
        Err(StageFailure::Uncertain(x)) => {
            journal.append("generate", "uncertain", json!({}))?;
            Err(d(
                "SPX-HPD072",
                format!(
                    "uncertain: model outcome unknown, not retried ({})",
                    x.message
                ),
            ))
        }
        Err(StageFailure::Refused(x)) => {
            if side {
                journal.append("generate", "refused", json!({"code": x.code}))?;
            }
            Err(x)
        }
        Err(StageFailure::Unavailable(x)) => {
            if side {
                journal.append("generate", "refused", json!({"code": x.code}))?;
            }
            Err(d(
                "SPX-HPD090",
                format!("no proposal available: {} {}", x.code, x.message),
            ))
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

fn copy_tree(from: &Path, to: &Path, depth: usize) -> HarnessResult<()> {
    let io = |e: std::io::Error| d("SPX-HPD070", format!("scratch copy: {e}"));
    std::fs::create_dir_all(to).map_err(io)?;
    for e in std::fs::read_dir(from).map_err(io)? {
        let e = e.map_err(io)?;
        let name = e.file_name().to_string_lossy().into_owned();
        let t = e.file_type().map_err(io)?;
        if t.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" || depth > 16 {
                continue;
            }
            copy_tree(&e.path(), &to.join(&name), depth + 1)?;
        } else if t.is_file() && !name.starts_with("semaprax.harness") {
            std::fs::copy(e.path(), to.join(&name)).map_err(io)?;
        }
    }
    Ok(())
}

/// Materialize the compiler-produced candidate sources in a private scratch
/// copy (never the project) and have the compiler check and test it.
fn candidate_checks(
    cx: &mut Ctx,
    command: &mut dyn CommandStage,
    preview: &CandidatePreview,
    r: &mut Report,
) -> HarnessResult<Value> {
    let scratch = cx.cfg.cache_dir.join(format!("scratch-{}", cx.lineage.id));
    let _ = std::fs::remove_dir_all(&scratch);
    copy_tree(&cx.cfg.snapshot.root, &scratch, 0)?;
    for c in &preview.source_changes {
        std::fs::write(scratch.join(&c.path), &c.replacement_source)
            .map_err(|e| d("SPX-HPD070", format!("scratch write: {e}")))?;
    }
    let scratch = scratch
        .canonicalize()
        .map_err(|e| d("SPX-HPD070", format!("scratch: {e}")))?;
    let out = (|| {
        let check = cx.compiler.check(&scratch)?;
        if !check.ok {
            let first = check
                .diagnostics
                .first()
                .map(|x| format!("{} {}", x.code, x.message))
                .unwrap_or_default();
            return Err(d(
                "SPX-HPD050",
                format!("candidate rejected: the compiler's check failed ({first})"),
            ));
        }
        if check.revision.as_deref() != Some(preview.candidate_revision.as_str()) {
            return Err(d(
                "SPX-HPD041",
                "candidate check revision differs from the previewed candidate revision",
            ));
        }
        let test = cx.compiler.test(&scratch)?;
        if test.project_revision != preview.candidate_revision {
            return Err(d(
                "SPX-HPD041",
                "candidate test revision differs from the previewed candidate revision",
            ));
        }
        if !test.passed {
            r.checks = json!({"check": "verified", "tests": "failed", "report_digest": test.report_digest});
            return Err(d(
                "SPX-HPD050",
                format!(
                    "candidate rejected: tests failed ({})",
                    test.failure.clone().unwrap_or_else(|| test.outcome.clone())
                ),
            ));
        }
        let mut runs: Vec<Value> = Vec::new();
        if !cx.cfg.checks.is_empty() {
            // Authorized checks see the project's own configuration (scope, mode).
            let cfg_file = cx
                .cfg
                .snapshot
                .root
                .join(crate::profile::config::CONFIG_FILE);
            if cfg_file.is_file() {
                let _ = std::fs::copy(&cfg_file, scratch.join(crate::profile::config::CONFIG_FILE));
            }
            for c in &cx.cfg.checks {
                let run = match command.run_check(c, &scratch, &mut *cx.observer) {
                    None => {
                        return Err(d(
                            "SPX-HPD051",
                            format!("authorized check `{}` cannot run: the command stage executes no checks", c.name),
                        ))
                    }
                    Some(Err(e)) => {
                        return Err(d(
                            "SPX-HPD051",
                            format!("authorized check `{}` was refused: {} {}", c.name, e.code, e.message),
                        ))
                    }
                    Some(Ok(run)) => run,
                };
                runs.push(run.to_json());
                if !run.passed {
                    r.checks = json!({"check": "verified", "tests": "passed", "commands": runs});
                    return Err(d(
                        "SPX-HPD050",
                        format!(
                            "candidate rejected: authorized check `{}` failed ({})",
                            c.name, run.status
                        ),
                    ));
                }
            }
        }
        Ok(
            json!({"check": "verified", "tests": "passed", "candidate_revision": preview.candidate_revision, "report_digest": test.report_digest, "commands": runs}),
        )
    })();
    let _ = std::fs::remove_dir_all(&scratch);
    out
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
