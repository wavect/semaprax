//! `run <project> [--task f] [--proposal f] [--apply-policy f] [--disable]
//! [--compiler p] [--python p] [--node p] [--json]`.

use super::compiler::{CompilerService, SubprocessCompiler};
use super::composition::Composition;
use super::pipeline::{provider_rows, run, RunConfig, Stages};
use super::policy::ApplyPolicy;
use super::snapshot::Snapshot;
use super::stages::*;
use crate::cli::{Environment, Outcome};
use crate::contract::{CapabilityKind, Runtime};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::host::grant::Grant;
use crate::host::{AdapterManager, HostConfig, IsolationRequest, LaunchSpec, NetworkPolicy};
use crate::observe::{Observer, ObserverLimits};
use crate::profile::{self, lock, BindingState, HarnessConfig, ResolvedLaunch};
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
    let compiler = SubprocessCompiler::new(exe, cache.join("compiler"))?;
    run_with(&a_to_opts(&a, env), env, snapshot, cache, &compiler)
}

/// Options after argument parsing (also usable by embedders and tests).
pub struct RunOptions {
    pub task: Option<PathBuf>,
    pub proposal: Option<PathBuf>,
    pub apply_policy: Option<PathBuf>,
    pub python: Option<PathBuf>,
    pub node: Option<PathBuf>,
    pub disable: bool,
    pub json: bool,
}

fn a_to_opts(a: &Args, env: &Environment) -> RunOptions {
    let var = |k: &str| env.vars.get(k).map(PathBuf::from);
    RunOptions {
        task: a.task.as_deref().map(|p| abs(env, p)),
        proposal: a.proposal.as_deref().map(|p| abs(env, p)),
        apply_policy: a.policy.as_deref().map(|p| abs(env, p)),
        python: a
            .python
            .as_deref()
            .map(|p| abs(env, p))
            .or_else(|| var("HARNESS_PYTHON")),
        node: a
            .node
            .as_deref()
            .map(|p| abs(env, p))
            .or_else(|| var("HARNESS_NODE")),
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
    let mut ext_ctx: Option<HostContext> = None;
    if let Some(l) = launch_for(CapabilityKind::ContextRepository) {
        match start(&manager, l, &snapshot, &cache, o) {
            Ok(h) => {
                ext_ctx = Some(HostContext::new(
                    h,
                    l.grant.clone(),
                    env.clone(),
                    l.provider_id.clone(),
                ))
            }
            Err(e) => notes.push(format!(
                "context provider `{}` not started, builtin used: {}",
                l.provider_id, e.message
            )),
        }
    }
    let mut model: Option<HostModel> = None;
    if o.proposal.is_none() {
        if let Some(l) = launch_for(CapabilityKind::ModelGenerate) {
            match start(&manager, l, &snapshot, &cache, o) {
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
    let mut scripted = match &o.proposal {
        Some(p) => ScriptedProposer::from_file(p.clone()),
        None => ScriptedProposer::empty(),
    };
    let cfg = RunConfig {
        context_max_bytes: config.budget.context_max_bytes as usize,
        providers: provider_rows(resolution.as_ref().map(|r| &r.profile), disabled),
        snapshot,
        task,
        cache_dir: cache.clone(),
        lock_digest,
        composition,
        apply_policy,
        notes,
    };
    let mut native = NativeContext::new(compiler);
    let mut view = RawCommandView;
    let mut observer = Observer::new(None, ObserverLimits::default());
    let proposer: &mut dyn ProposalStage = match model.as_mut() {
        Some(m) => m,
        None => &mut scripted,
    };
    let stages = Stages {
        native: &mut native,
        external: ext_ctx.as_mut().map(|c| c as &mut dyn ContextStage),
        proposer,
        command: &mut view,
    };
    let report = run(&cfg, compiler, stages, &mut observer);
    manager.shutdown_all();
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

fn start(
    manager: &AdapterManager,
    l: &ResolvedLaunch,
    snap: &Snapshot,
    cache: &Path,
    o: &RunOptions,
) -> HarnessResult<std::sync::Arc<crate::host::AdapterHandle>> {
    let runtime = match l.descriptor.runtime {
        Runtime::Python => Some(o.python.clone().ok_or_else(|| {
            HarnessDiagnostic::new(
                "SPX-HPD091",
                "python runtime path not provided (--python or HARNESS_PYTHON)",
            )
        })?),
        Runtime::Node => Some(o.node.clone().ok_or_else(|| {
            HarnessDiagnostic::new(
                "SPX-HPD091",
                "node runtime path not provided (--node or HARNESS_NODE)",
            )
        })?),
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
