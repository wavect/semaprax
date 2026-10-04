//! `run <project> [--task f] [--proposal f] [--apply-policy f] [--disable]
//! [--compiler p] [--python p] [--node p] [--json]`.

use super::broker_stage::BrokerContext;
use super::checks::{CheckSpec, HostCommandChecks};
use super::compiler::{CompilerService, SubprocessCompiler};
use super::composition::Composition;
use super::pipeline::{provider_rows, run, RunConfig, Stages};
use super::pipeline::{DecisionStage, SkillPromptUse};
use super::policy::ApplyPolicy;
use super::snapshot::Snapshot;
use super::stages::*;
use crate::cli::{Environment, Outcome};
use crate::contract::{CapabilityKind, Runtime};
use crate::decision::{
    DecisionTask, EnablementGate, HostDecisionInvoker, ProviderMode, ProviderProfile,
};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::endpoint::{check_policy, Catalog as EndpointCatalog, EndpointPolicy};
use crate::host::grant::Grant;
use crate::host::{AdapterManager, HostConfig, IsolationRequest, LaunchSpec, NetworkPolicy};
use crate::observe::JsonlFileSink;
use crate::observe::{Observer, ObserverLimits};
use crate::profile::{self, lock, BindingState, HarnessConfig, ResolvedLaunch};
use crate::skills::cli_defaults::project_id;
use crate::skills::defaults::{DefaultSkills, TaskInput};
use crate::skills::{task_tags, ApprovedRoot, SkillCatalogConfig, SkillService};
use std::path::{Path, PathBuf};

fn usage(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPD080", msg)
}

#[derive(Default)]
struct Args {
    project: Option<String>,
    task: Option<String>,
    proposal: Option<String>,
    policy: Option<String>,
    compiler: Option<String>,
    python: Option<String>,
    node: Option<String>,
    observations: Option<String>,
    tokenizer_python: Option<String>,
    tokenizer_script: Option<String>,
    tokenizer_cache: Option<String>,
    tokenizers: Vec<String>,
    cancel_file: Option<String>,
    disable: bool,
    json: bool,
}

fn parse(args: &[String]) -> HarnessResult<Args> {
    let mut a = Args::default();
    let mut it = args.iter();
    while let Some(x) = it.next() {
        let mut val = |name: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| usage(format!("`{name}` needs a value")))
        };
        match x.as_str() {
            "--task" => a.task = Some(val("--task")?),
            "--proposal" => a.proposal = Some(val("--proposal")?),
            "--apply-policy" => a.policy = Some(val("--apply-policy")?),
            "--compiler" => a.compiler = Some(val("--compiler")?),
            "--python" => a.python = Some(val("--python")?),
            "--node" => a.node = Some(val("--node")?),
            "--observations" => a.observations = Some(val("--observations")?),
            "--tokenizer-python" => a.tokenizer_python = Some(val("--tokenizer-python")?),
            "--tokenizer-script" => a.tokenizer_script = Some(val("--tokenizer-script")?),
            "--tokenizer-cache" => a.tokenizer_cache = Some(val("--tokenizer-cache")?),
            "--cancel-file" => a.cancel_file = Some(val("--cancel-file")?),
            "--tokenizer" => a.tokenizers.push(val("--tokenizer")?),
            "--disable" => a.disable = true,
            "--json" => a.json = true,
            o if o.starts_with("--") => return Err(usage(format!("unknown option `{o}`"))),
            p if a.project.is_none() => a.project = Some(p.into()),
            _ => return Err(usage("expected exactly one project")),
        }
    }
    Ok(a)
}

pub fn cli_run(args: &[String], env: &Environment) -> Outcome {
    match parse(args).and_then(|a| execute(a, env)) {
        Ok(o) => o,
        Err(e) if e.code == "SPX-HPD080" => Outcome::usage(format!("run: {}", e.message)),
        Err(e) => Outcome::refused(&e),
    }
}

fn abs(env: &Environment, p: &str) -> PathBuf {
    let p = Path::new(p);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        env.cwd.join(p)
    }
}

fn execute(a: Args, env: &Environment) -> HarnessResult<Outcome> {
    let project = abs(
        env,
        a.project
            .as_deref()
            .ok_or_else(|| usage("missing project"))?,
    );
    let exe = match (&a.compiler, &env.compiler) {
        (Some(c), _) => abs(env, c),
        (None, Some(c)) => c.clone(),
        _ => {
            return Err(HarnessDiagnostic::new(
                "SPX-HPD001",
                "no compiler: pass --compiler or set SEMAPRAX_COMPILER",
            ))
        }
    };
    let snapshot = Snapshot::capture(&project)?;
    let home = env
        .harness_home
        .clone()
        .ok_or_else(|| usage("SEMAPRAX_HARNESS_HOME is not set"))?;
    let cache = home
        .join("cache/workflow")
        .join(&snapshot.project_id["sha256:".len()..][..16]);
    std::fs::create_dir_all(&cache)
        .map_err(|e| HarnessDiagnostic::new("SPX-HPD070", format!("cache: {e}")))?;
    let compiler_path = exe.clone();
    let compiler = SubprocessCompiler::new(exe, cache.join("compiler"))?;
    let mut opts = a_to_opts(&a, env);
    opts.compiler = Some(compiler_path);
    run_with(&opts, env, snapshot, cache, &compiler)
}

/// Options after argument parsing (also usable by embedders and tests).
pub struct RunOptions {
    pub task: Option<PathBuf>,
    pub proposal: Option<PathBuf>,
    pub apply_policy: Option<PathBuf>,
    pub python: Option<PathBuf>,
    pub node: Option<PathBuf>,
    /// Compiler executable (needed to compose the context broker).
    pub compiler: Option<PathBuf>,
    /// JSONL file receiving one metadata-only observation per stage.
    pub observations: Option<PathBuf>,
    /// Named-tokenizer helper (HN-11): python, `scripts/harness_tokenize.py`, the
    /// local tiktoken cache directory and the encodings to start. Without it
    /// token counts are `unknown` and admission uses the byte upper bound.
    pub tokenizer_python: Option<PathBuf>,
    pub tokenizer_script: Option<PathBuf>,
    pub tokenizer_cache: Option<PathBuf>,
    pub tokenizers: Vec<String>,
    /// Cooperative cancellation for sessions.
    pub cancel: Option<super::session::CancelFlag>,
    /// Polled path: once it exists the session is cancelled (no signal handling:
    /// that needs `unsafe`, which this crate forbids).
    pub cancel_file: Option<PathBuf>,
    pub disable: bool,
    pub json: bool,
}

fn a_to_opts(a: &Args, env: &Environment) -> RunOptions {
    RunOptions {
        task: a.task.as_deref().map(|p| abs(env, p)),
        proposal: a.proposal.as_deref().map(|p| abs(env, p)),
        apply_policy: a.policy.as_deref().map(|p| abs(env, p)),
        // Flags only: a recorded `adopt --runtime` and then HARNESS_* follow them.
        python: a.python.as_deref().map(|p| abs(env, p)),
        node: a.node.as_deref().map(|p| abs(env, p)),
        compiler: None,
        observations: a.observations.as_deref().map(|p| abs(env, p)),
        tokenizer_python: a.tokenizer_python.as_deref().map(|p| abs(env, p)),
        tokenizer_script: a.tokenizer_script.as_deref().map(|p| abs(env, p)),
        tokenizer_cache: a.tokenizer_cache.as_deref().map(|p| abs(env, p)),
        tokenizers: a.tokenizers.clone(),
        cancel: None,
        cancel_file: a.cancel_file.as_deref().map(|p| abs(env, p)),
        disable: a.disable,
        json: a.json,
    }
}

/// Resolve the profile, wire the stages and run the pipeline with `compiler`.
pub fn run_with(
    o: &RunOptions,
    env: &Environment,
    snapshot: Snapshot,
    cache: PathBuf,
    compiler: &dyn CompilerService,
) -> HarnessResult<Outcome> {
    let config = HarnessConfig::load(&snapshot.root)?;
    let disabled = o.disable || !config.profile_enabled;
    let mut notes = Vec::new();
    if disabled {
        notes.push("provider profile disabled: builtin providers only, no external calls".into());
    }
    let resolution = if disabled {
        None
    } else {
        Some(profile::resolve_project(env, &snapshot.root)?)
    };
    let mut lock_digest = "disabled".to_string();
    if let Some(res) = &resolution {
        if let Some(e) = res.unmet.first() {
            return Err(e.clone());
        }
        match lock::load(&snapshot.root)? {
            Some(l) => lock::verify_frozen(&l, &res.profile)?,
            None => lock::write(&snapshot.root, &res.profile)?,
        }
        lock_digest = res.profile.lock_digest();
    }
    let task = match &o.task {
        Some(p) => Task::parse(&std::fs::read(p).map_err(|e| {
            HarnessDiagnostic::new("SPX-HPD081", format!("task {}: {e}", p.display()))
        })?)?,
        None => Task::default(),
    };
    let apply_policy = match &o.apply_policy {
        Some(p) => Some(ApplyPolicy::load(p, &snapshot.root)?),
        None => None,
    };
    let composition = Composition::from_profile(resolution.as_ref(), disabled, vec![], &[])?;

    // Provider stages through the adapter host.
    let manager = AdapterManager::new(HostConfig::default());
    let launch_for = |kind| {
        resolution
            .as_ref()
            .filter(|r| {
                r.profile
                    .binding(kind)
                    .is_some_and(|b| b.state == BindingState::Selected)
            })
            .and_then(|r| r.launches.get(&kind))
    };
    // Runtimes: explicit flag, then the adopted `--runtime`, then HARNESS_*.
    let rt_env = |l: &ResolvedLaunch| {
        let mut e = env.clone();
        let rt = l.descriptor.runtime;
        let flag = match rt {
            Runtime::Python => o.python.as_deref(),
            Runtime::Node => o.node.as_deref(),
            _ => None,
        };
        if let (Some(var), Some(p)) = (
            crate::profile::runtime::env_var(rt),
            crate::profile::runtime::pick(rt, flag, l.runtime.as_deref(), None, env),
        ) {
            e.vars.insert(var.into(), p.to_string_lossy().into_owned());
        }
        e
    };
    let mut broker_ctx: Option<BrokerContext> = None;
    if let (Some(l), Some(res), Some(exe)) = (
        launch_for(CapabilityKind::ContextRepository),
        resolution.as_ref(),
        o.compiler.clone().or_else(|| env.compiler.clone()),
    ) {
        match BrokerContext::new(
            exe,
            l.clone(),
            rt_env(l),
            res.profile.lock_digest(),
            res.profile.config_digest.clone(),
            config.capability(CapabilityKind::ContextRepository).scope,
        ) {
            Ok(b) => broker_ctx = Some(b),
            Err(e) => notes.push(format!(
                "context provider `{}` not composed, builtin used: {}",
                l.provider_id, e.message
            )),
        }
    }
    let mut model: Option<HostModel> = None;
    if o.proposal.is_none() {
        if let Some(l) = launch_for(CapabilityKind::ModelGenerate) {
            match start(&manager, l, &snapshot, &cache, o, env) {
                Ok(h) => {
                    model = Some(HostModel::new(
                        h,
                        l.grant.clone(),
                        env.clone(),
                        l.provider_id.clone(),
                    ))
                }
                Err(e) => notes.push(format!(
                    "model provider `{}` not started: {}",
                    l.provider_id, e.message
                )),
            }
        }
    }
    // External decision provider: consulted only when explicit or gate-passed.
    let mut decision_inv: Option<HostDecisionInvoker> = None;
    let mut decision_meta: Option<(ProviderProfile, ProviderMode)> = None;
    if let Some(l) = launch_for(CapabilityKind::DecisionEvaluate) {
        match start(&manager, l, &snapshot, &cache, o, env) {
            Ok(h) => {
                decision_inv = Some(HostDecisionInvoker::new(h));
                let explicit = config
                    .capability(CapabilityKind::DecisionEvaluate)
                    .provider
                    .is_some();
                decision_meta = Some((
                    ProviderProfile {
                        provider_id: l.provider_id.clone(),
                        model_id: l.provider_id.clone(),
                        checkpoint: l.descriptor.adapter_version.clone(),
                        min_confidence: None,
                        max_context_tokens: None,
                        supported_families: None,
                    },
                    if explicit {
                        ProviderMode::Explicit
                    } else {
                        ProviderMode::Auto
                    },
                ));
            }
            Err(e) => notes.push(format!(
                "decision provider `{}` not started, rules decide: {}",
                l.provider_id, e.message
            )),
        }
    }
    let mut scripted = match &o.proposal {
        Some(p) => ScriptedProposer::from_file(p.clone()),
        None => ScriptedProposer::empty(),
    };

    // `[model]`: project guarantees against the machine-local logical binding.
    let endpoint_policy = EndpointPolicy {
        local_only: config.model.local_only,
        strict_one_attempt: config.model.strict_one_attempt,
    };
    let mut model_plans = None;
    if let Some(id) = &config.model.logical {
        let home = env
            .harness_home
            .clone()
            .ok_or_else(|| usage("SEMAPRAX_HARNESS_HOME is not set"))?;
        let catalog = EndpointCatalog::load(&home)?;
        let binding = catalog.bindings.get(id).ok_or_else(|| {
            HarnessDiagnostic::new(
                "SPX-HPL034",
                format!("logical model `{id}` is not bound; run `semaprax harness endpoints bind`"),
            )
        })?;
        let endpoint = catalog.endpoints.get(&binding.endpoint_id).ok_or_else(|| {
            HarnessDiagnostic::new(
                "SPX-HPL034",
                format!("endpoint `{}` is no longer adopted", binding.endpoint_id),
            )
        })?;
        check_policy(endpoint_policy, &endpoint.ownership, &binding.destination)?;
        model_plans = Some(vec![binding.to_model_plan(0, 1000)]);
    }

    // Authorized checks and the skill prompt.
    let checks: Vec<CheckSpec> = config
        .workflow
        .checks
        .iter()
        .map(|(name, argv)| CheckSpec {
            name: name.clone(),
            argv: argv.clone(),
        })
        .collect();
    let (skill_prompt, official_use) = skill_prompt(&config, env, &task, disabled, &snapshot.root);
    let mut budget = super::budget::BudgetConfig::default();
    if let (Some(py), Some(script)) = (&o.tokenizer_python, &o.tokenizer_script) {
        let mut tenv = std::collections::BTreeMap::new();
        tenv.insert("PATH".to_string(), "/usr/bin:/bin".to_string());
        if let Some(c) = &o.tokenizer_cache {
            tenv.insert(
                "TIKTOKEN_CACHE_DIR".to_string(),
                c.to_string_lossy().into_owned(),
            );
        }
        for name in &o.tokenizers {
            let args = vec![script.to_string_lossy().into_owned(), name.clone()];
            match crate::observe::ExternalTokenizer::spawn(py, &args, &tenv) {
                Ok(t) => budget.tokenizers.add(Box::new(t)),
                Err(e) => notes.push(format!(
                    "tokenizer `{name}` unavailable ({}); counts are unknown, admission uses the utf8-bytes upper bound",
                    e.message
                )),
            }
        }
    }

    let cfg = RunConfig {
        context_max_bytes: config.budget.context_max_bytes as usize,
        providers: provider_rows(resolution.as_ref().map(|r| &r.profile), disabled),
        snapshot,
        task,
        cache_dir: cache.clone(),
        lock_digest,
        composition,
        apply_policy,
        checks,
        skill_prompt,
        endpoint_policy,
        model_plans,
        notes,
        budget,
        cancel: o.cancel.clone().or_else(|| {
            o.cancel_file.as_ref().map(|p| {
                let flag = super::session::CancelFlag::default();
                let (f, path) = (flag.clone(), p.clone());
                if path.exists() {
                    f.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                std::thread::spawn(move || {
                    // Ends with the process or once the flag fires (a run is short-lived).
                    for _ in 0..72_000 {
                        if path.exists() {
                            f.store(true, std::sync::atomic::Ordering::SeqCst);
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                });
                flag
            })
        }),
    };
    let sink: Option<Box<dyn crate::observe::Sink>> = match &o.observations {
        Some(p) => Some(Box::new(JsonlFileSink::create(p, 16 << 20).map_err(
            |e| {
                HarnessDiagnostic::new(
                    "SPX-HPD070",
                    format!(
                        "observation file {}: {e} (it must not exist yet)",
                        p.display()
                    ),
                )
            },
        )?)),
        None => None,
    };
    let mut native = NativeContext::new(compiler);
    let mut view_checks = HostCommandChecks::new({
        let mut e = env.clone();
        for (k, v) in [("HARNESS_PYTHON", &o.python), ("HARNESS_NODE", &o.node)] {
            if let Some(p) = v {
                e.vars
                    .entry(k.into())
                    .or_insert_with(|| p.to_string_lossy().into_owned());
            }
        }
        e
    });
    view_checks.raw = disabled;
    let mut raw_view = RawCommandView;
    let mut observer = Observer::new(sink, ObserverLimits::default());
    let proposer: &mut dyn ProposalStage = match model.as_mut() {
        Some(m) => m,
        None => &mut scripted,
    };
    let command: &mut dyn CommandStage = if cfg.checks.is_empty() {
        &mut raw_view
    } else {
        &mut view_checks
    };
    let native_stage: &mut dyn ContextStage = match broker_ctx.as_mut() {
        Some(b) => b,
        None => &mut native,
    };
    let decision = match (decision_inv.as_mut(), decision_meta) {
        (Some(inv), Some((profile, mode))) => Some(DecisionStage {
            gate: EnablementGate::not_evaluated(
                DecisionTask::ModelRoute.id(),
                &profile.provider_id,
            ),
            invoker: inv,
            profile,
            mode,
        }),
        _ => None,
    };
    let stages = Stages {
        decision,
        native: native_stage,
        external: None,
        proposer,
        command,
    };
    let report = run(&cfg, compiler, stages, &mut observer);
    manager.shutdown_all();
    observer.finish();
    // Applied-to-model only once a proposal request was actually made.
    if let Some((mut ds, ids)) = official_use {
        if report.steps.iter().any(|(k, _)| k == "proposal") {
            let _ = ds.mark_applied(&ids);
        }
    }
    let lines: Vec<String> = observer
        .events()
        .iter()
        .map(|e| e.to_json().to_string())
        .collect();
    let _ = std::fs::write(
        cache.join(format!("{}.observations.jsonl", report.lineage)),
        lines.join("\n") + "\n",
    );
    let stdout = if o.json {
        format!("{}\n", crate::json::canonical(&report.to_json()))
    } else {
        report.to_text()
    };
    Ok(Outcome {
        code: report.exit_code(),
        stdout,
        stderr: String::new(),
    })
}

/// The skill prompt for this task: the official default skills (embedded,
/// selected deterministically for the task family, no adopt step) followed by
/// approved-root skills that do not shadow an official id. Returns the
/// default-skill state so the run can record `applied` after the request.
fn skill_prompt(
    config: &HarnessConfig,
    env: &Environment,
    task: &Task,
    disabled: bool,
    project: &Path,
) -> (Option<SkillPromptUse>, Option<(DefaultSkills, Vec<String>)>) {
    if disabled || !config.skills.enabled {
        return (None, None);
    }
    if config.capability(CapabilityKind::SkillCatalog).mode == crate::profile::Mode::Disabled {
        return (None, None);
    }
    let budget = config.skills.max_bytes as usize;
    let mut text = String::new();
    let mut loaded: Vec<String> = Vec::new();
    let mut official = None;
    let mut defaults =
        DefaultSkills::embedded(env.harness_home.clone(), &project_id(project), "default")
            .and_then(|d| d.with_project_prefs(config.skills.prefs.clone()))
            .ok();
    if let Some(ds) = defaults.as_mut() {
        let input = TaskInput {
            family: &task.family,
            instruction: Some(&task.goal),
        };
        if let Ok(sel) = ds.select_for_task(&input, budget) {
            let mut ids = Vec::new();
            for r in sel.reports.iter().filter(|r| r.loaded) {
                loaded.push(format!(
                    "{}@{}:{}:{}",
                    r.id,
                    r.version,
                    r.mode,
                    r.locked_revision.as_deref().unwrap_or("")
                ));
                ids.push(r.id.clone());
            }
            text = sel.text;
            official = Some(ids);
        }
    }
    if let Ok(state) = crate::profile::LocalState::load(env) {
        let roots: Vec<ApprovedRoot> = state
            .skill_roots
            .iter()
            .map(|r| ApprovedRoot {
                path: r.path.clone(),
                origin: r.origin.clone(),
                approved_digest: None,
            })
            .collect();
        let roots = match defaults.as_ref() {
            Some(ds) => ds.refuse_shadowing(&roots).0,
            None => roots,
        };
        if !roots.is_empty() {
            let mut svc = SkillService::new(
                roots,
                SkillCatalogConfig {
                    enabled: true,
                    select: config.skills.select.clone(),
                    max_bytes: budget.saturating_sub(text.len()),
                    ..Default::default()
                },
            );
            let p = svc.render_prompt(&task_tags(&task.family));
            text.push_str(&p.text);
            loaded.extend(p.loaded.iter().map(|(n, _)| n.clone()));
        }
    }
    if text.is_empty() {
        return (None, None);
    }
    let used = defaults.zip(official);
    (
        Some(SkillPromptUse {
            model_visible_bytes: text.len(),
            loaded,
            text,
        }),
        used,
    )
}

fn start(
    manager: &AdapterManager,
    l: &ResolvedLaunch,
    snap: &Snapshot,
    cache: &Path,
    o: &RunOptions,
    env: &Environment,
) -> HarnessResult<std::sync::Arc<crate::host::AdapterHandle>> {
    let flag = match l.descriptor.runtime {
        Runtime::Python => o.python.as_deref(),
        Runtime::Node => o.node.as_deref(),
        _ => None,
    };
    let runtime = crate::profile::runtime::require(
        "SPX-HPD091",
        &l.provider_id,
        l.descriptor.runtime,
        flag,
        l.runtime.as_deref(),
        None,
        env,
    )?;
    let dir = cache.join("adapters").join(l.provider_id.replace('/', "_"));
    let (cache_dir, retention) = (dir.join("cache"), dir.join("retention"));
    for p in [&cache_dir, &retention] {
        std::fs::create_dir_all(p)
            .map_err(|e| HarnessDiagnostic::new("SPX-HPD070", format!("{}: {e}", p.display())))?;
    }
    let isolation = if matches!(
        l.descriptor.support.isolation.as_str(),
        "restricted" | "os-enforced"
    ) {
        IsolationRequest::Restricted {
            allow_read: vec![snap.root.clone()],
            allow_write: vec![],
            network: NetworkPolicy::Deny,
        }
    } else {
        IsolationRequest::None
    };
    let grant: Grant = l.grant.clone();
    manager.prepare(
        &snap.project_id,
        LaunchSpec {
            descriptor: l.descriptor.clone(),
            descriptor_dir: l
                .descriptor_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default(),
            runtime_executable: runtime,
            upstream_executable: l.upstream_path.clone(),
            grant,
            project_root: snap.root.clone(),
            cache_dir: cache_dir.canonicalize().unwrap_or(cache_dir),
            retention_dir: retention.canonicalize().unwrap_or(retention),
            isolation,
            forward_env: Default::default(),
        },
    )
}
