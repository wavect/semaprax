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
        let pick = |flag: &Option<PathBuf>| flag.clone().or_else(|| l.runtime.clone());
        match l.descriptor.runtime {
            Runtime::Python => {
                if let Some(p) = pick(&o.python) {
                    e.vars
                        .insert("HARNESS_PYTHON".into(), p.to_string_lossy().into_owned());
                }
            }
            Runtime::Node => {
                if let Some(p) = pick(&o.node) {
                    e.vars
                        .insert("HARNESS_NODE".into(), p.to_string_lossy().into_owned());
                }
            }
            _ => {}
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
    let skill_prompt = skill_prompt(&config, env, &task, disabled);

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

/// The approved-root skill prompt for this task (machine-local roots only).
fn skill_prompt(
    config: &HarnessConfig,
    env: &Environment,
    task: &Task,
    disabled: bool,
) -> Option<SkillPromptUse> {
    if disabled || !config.skills.enabled {
        return None;
    }
    if config.capability(CapabilityKind::SkillCatalog).mode == crate::profile::Mode::Disabled {
        return None;
    }
    let state = crate::profile::LocalState::load(env).ok()?;
    if state.skill_roots.is_empty() {
        return None;
    }
    let roots = state
        .skill_roots
        .iter()
        .map(|r| ApprovedRoot {
            path: r.path.clone(),
            origin: r.origin.clone(),
            approved_digest: None,
        })
        .collect();
    let mut svc = SkillService::new(
        roots,
        SkillCatalogConfig {
            enabled: true,
            select: config.skills.select.clone(),
            max_bytes: config.skills.max_bytes as usize,
            ..Default::default()
        },
    );
    let p = svc.render_prompt(&task_tags(&task.family));
    if p.text.is_empty() {
        return None;
    }
    Some(SkillPromptUse {
        model_visible_bytes: p.model_visible_bytes,
        loaded: p.loaded.iter().map(|(n, _)| n.clone()).collect(),
        text: p.text,
    })
}

fn start(
    manager: &AdapterManager,
    l: &ResolvedLaunch,
    snap: &Snapshot,
    cache: &Path,
    o: &RunOptions,
    env: &Environment,
) -> HarnessResult<std::sync::Arc<crate::host::AdapterHandle>> {
    let var = |k: &str| env.vars.get(k).map(PathBuf::from);
    let runtime = match l.descriptor.runtime {
        Runtime::Python => Some(
            o.python
                .clone()
                .or_else(|| l.runtime.clone())
                .or_else(|| var("HARNESS_PYTHON"))
                .ok_or_else(|| {
                    HarnessDiagnostic::new(
                        "SPX-HPD091",
                        "python runtime path not provided (--python, `adopt --runtime`, or HARNESS_PYTHON)",
                    )
                })?,
        ),
        Runtime::Node => Some(
            o.node
                .clone()
                .or_else(|| l.runtime.clone())
                .or_else(|| var("HARNESS_NODE"))
                .ok_or_else(|| {
                    HarnessDiagnostic::new(
                        "SPX-HPD091",
                        "node runtime path not provided (--node, `adopt --runtime`, or HARNESS_NODE)",
                    )
                })?,
        ),
        _ => None,
    };
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
