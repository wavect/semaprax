//! `semaprax harness setup` (HN-07): one reviewed action that adopts and trusts
//! the user's existing RTK / Graft / Graphify, records the interpreter, and
//! writes the project profile. Idempotent; machine paths and trust stay in the
//! machine-local state; the project file carries only provider ids.
//!
//! Diagnostics: `SPX-HPB050` usage (shared), `060` required provider
//! unavailable, `061` conflicting selection, `062` project path problem,
//! `063` bundled adapter store problem.

use super::adopt::{adopt, set_runtime, AdoptOptions};
use super::config::{HarnessConfig, CONFIG_FILE};
use super::installations::{Installation, LocalState};
use super::setup_find::{is_tool, Evaluation, Finder, Ready};
use crate::assets;
use crate::cli::{Environment, Outcome};
use crate::contract::{CapabilityKind, Descriptor};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const SETUP_SCHEMA: &str = "semaprax.harness-setup.v1";

fn d(code: &'static str, m: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, m)
}
fn usage(m: impl Into<String>) -> HarnessDiagnostic {
    d("SPX-HPB050", m)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Preset {
    Native,
    LocalEfficient,
}

struct Opts {
    project: PathBuf,
    path_dirs: Vec<PathBuf>,
    tools: BTreeMap<String, PathBuf>,
    preset: Preset,
    provider: Option<String>,
    require: Vec<String>,
    dry_run: bool,
    yes: bool,
    json: bool,
}

fn abs(what: &str, v: &str) -> HarnessResult<PathBuf> {
    let p = PathBuf::from(v);
    if p.is_absolute() {
        Ok(p)
    } else {
        Err(usage(format!(
            "{what} must be an absolute path; PATH and the current directory are never searched"
        )))
    }
}

fn parse(args: &[String], env: &Environment) -> HarnessResult<Opts> {
    let mut o = Opts {
        project: env.cwd.clone(),
        path_dirs: Vec::new(),
        tools: BTreeMap::new(),
        preset: Preset::Native,
        provider: None,
        require: Vec::new(),
        dry_run: false,
        yes: false,
        json: false,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |w: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| usage(format!("`{w}` needs a value")))
        };
        match a.as_str() {
            "--project" => o.project = env.cwd.join(val("--project")?),
            "--path-dirs" => {
                for dir in val("--path-dirs")?.split(':').filter(|s| !s.is_empty()) {
                    o.path_dirs.push(abs("--path-dirs entries", dir)?);
                }
            }
            "--tool" => {
                let kv = val("--tool")?;
                let (k, v) = kv
                    .split_once('=')
                    .ok_or_else(|| usage("--tool needs NAME=<absolute path>"))?;
                if !is_tool(k) {
                    return Err(usage(format!(
                        "unknown tool `{k}` (rtk, graft, graphify, node, python)"
                    )));
                }
                o.tools.insert(k.to_string(), abs("--tool", v)?);
            }
            "--preset" => {
                o.preset = match val("--preset")?.as_str() {
                    "native" => Preset::Native,
                    "local-efficient" => Preset::LocalEfficient,
                    x => {
                        return Err(usage(format!(
                            "unknown preset `{x}` (native, local-efficient)"
                        )))
                    }
                }
            }
            "--provider" => {
                let v = val("--provider")?;
                if !["graft", "graphify", "none"].contains(&v.as_str()) {
                    return Err(usage("--provider is graft, graphify or none"));
                }
                o.provider = Some(v);
            }
            "--require" => {
                let v = val("--require")?;
                if !["graft", "graphify", "rtk"].contains(&v.as_str()) {
                    return Err(usage("--require is graft, graphify or rtk"));
                }
                o.require.push(v);
            }
            "--dry-run" => o.dry_run = true,
            "--yes" => o.yes = true,
            "--json" => o.json = true,
            x => return Err(usage(format!("unexpected argument `{x}`"))),
        }
    }
    if o.dry_run && o.yes {
        return Err(usage("--dry-run and --yes are mutually exclusive"));
    }
    Ok(o)
}

/// One provider's plan row.
struct Row {
    short: &'static str,
    kind: CapabilityKind,
    desc: Descriptor,
    eval: Evaluation,
    chosen: bool,
    /// `adopt` | `trust` | `current` | `kept` (existing adoption of the same id).
    action: &'static str,
}

fn descriptor_of(short: &str) -> Descriptor {
    let (_, dir) = assets::PROVIDERS
        .iter()
        .find(|(n, _)| *n == short)
        .expect("shipped provider");
    let rel = format!("{dir}/harness-provider.json");
    let (_, bytes) = assets::files()
        .iter()
        .find(|(p, _)| *p == rel)
        .expect("embedded descriptor");
    Descriptor::parse(bytes).expect("embedded descriptors are valid")
}

fn kind_of(short: &str) -> CapabilityKind {
    if short == "rtk" {
        CapabilityKind::CommandView
    } else {
        CapabilityKind::ContextRepository
    }
}

fn pin_of(rows: &[Row], kind: CapabilityKind) -> String {
    rows.iter()
        .find(|r| r.chosen && r.kind == kind)
        .map(|r| r.desc.provider_id.clone())
        .or_else(|| super::builtin::provider_for(kind).map(str::to_string))
        .unwrap_or_default()
}

fn profile_toml(rows: &[Row]) -> String {
    format!(
        "schema = \"semaprax.harness-config.v1\"\n\n[profile]\nenabled = true\n\n[skills]\nenabled = true\nofficial = true\n\n[capability.\"context.repository\"]\nmode = \"auto\"\nprovider = \"{}\"\n\n[capability.\"command.view\"]\nmode = \"auto\"\nprovider = \"{}\"\n",
        pin_of(rows, CapabilityKind::ContextRepository),
        pin_of(rows, CapabilityKind::CommandView),
    )
}

/// Is the machine-local state already exactly what setup would write?
fn is_current(
    state: &LocalState,
    id: &str,
    files_dir: &Path,
    short: &str,
    ready: &Ready,
    kind: CapabilityKind,
) -> bool {
    let Some(inst) = state.installations.get(id) else {
        return false;
    };
    let want = assets::descriptor_in(files_dir, short).unwrap_or_default();
    let Some(up) = &inst.upstream else {
        return false;
    };
    if inst.descriptor_path != want
        || up.path != ready.upstream
        || up.digest != ready.upstream_digest
        || !up.compatible
        || inst.runtime != ready.runtime
        || state.preferences.get(&kind).map(String::as_str) != Some(id)
    {
        return false;
    }
    let Ok(insp) = inst.inspect() else {
        return false;
    };
    let c = &insp.current;
    c.descriptor_digest == inst.descriptor_digest
        && c.entry_digest == inst.entry_digest
        && c.upstream_digest.as_deref() == Some(up.digest.as_str())
        && state.trust.get(id).is_some_and(|t| {
            t.descriptor_digest == c.descriptor_digest
                && t.entry_digest == c.entry_digest
                && t.upstream_digest == c.upstream_digest
        })
}

/// An adoption of the same provider id from another, still-readable descriptor.
fn foreign_adoption<'a>(
    state: &'a LocalState,
    id: &str,
    files_dir: &Path,
    short: &str,
) -> Option<&'a Installation> {
    let inst = state.installations.get(id)?;
    let want = assets::descriptor_in(files_dir, short)?;
    (inst.descriptor_path != want && inst.inspect().is_ok()).then_some(inst)
}

pub fn setup_verb(args: &[String], env: &Environment) -> Outcome {
    match parse(args, env).and_then(|o| run(&o, env)) {
        Ok(o) => o,
        Err(e) if e.code == "SPX-HPB050" => Outcome::usage(e.message),
        Err(e) => Outcome::refused(&e),
    }
}

fn run(o: &Opts, env: &Environment) -> HarnessResult<Outcome> {
    let home = env.harness_home.clone().ok_or_else(|| {
        d(
            "SPX-HPB020",
            "no harness home; set SEMAPRAX_HARNESS_HOME or HOME",
        )
    })?;
    let project = o.project.canonicalize().map_err(|e| {
        d(
            "SPX-HPB062",
            format!("project {} is unreadable: {e}", o.project.display()),
        )
    })?;
    if o.provider.as_deref().is_some_and(|p| p != "none")
        && o.require
            .iter()
            .any(|r| r != "rtk" && Some(r) != o.provider.as_ref())
    {
        return Err(d(
            "SPX-HPB061",
            "one repository provider per project: --provider and --require name different graphs",
        ));
    }
    if o.require.iter().any(|r| r == "graft") && o.require.iter().any(|r| r == "graphify") {
        return Err(d(
            "SPX-HPB061",
            "one repository provider per project: never require both graft and graphify",
        ));
    }
    let scratch = env
        .vars
        .get("TMPDIR")
        .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
        .join("semaprax-setup-probe");
    let mut path_dirs = Vec::new();
    let mut notes = Vec::new();
    for dir in &o.path_dirs {
        match dir.canonicalize() {
            Ok(c) if c.starts_with(&project) => notes.push(format!(
                "{} is inside the project and was not searched",
                dir.display()
            )),
            Ok(c) => path_dirs.push(c),
            Err(_) => notes.push(format!("{} does not exist; skipped", dir.display())),
        }
    }
    let mut finder = Finder {
        project: project.clone(),
        path_dirs,
        explicit: o.tools.clone(),
        scratch: scratch.clone(),
        notes: Vec::new(),
    };
    let node = finder.interpreter("node");
    let python = finder.interpreter("python");
    let state = LocalState::load(env)?;
    let home_c = home.canonicalize().unwrap_or_else(|_| home.clone());
    let files_dir = assets::expected_files_dir(&home_c)?;

    // Which shipped providers does the preset want?
    let wanted_repo: Option<&str> = match (&o.provider, o.preset) {
        (Some(p), _) if p == "none" => None,
        (Some(p), _) => Some(if p == "graft" { "graft" } else { "graphify" }),
        _ => o.require.iter().find(|r| *r != "rtk").map(String::as_str),
    };
    let efficient = o.preset == Preset::LocalEfficient;
    let mut rows: Vec<Row> = Vec::new();
    for short in ["graft", "graphify", "rtk"] {
        let desc = descriptor_of(short);
        let eval = finder.evaluate(short, &desc, &node, &python);
        rows.push(Row {
            short,
            kind: kind_of(short),
            desc,
            eval,
            chosen: false,
            action: "none",
        });
    }
    let ready = |s: &str, rows: &[Row]| {
        rows.iter()
            .any(|r| r.short == s && matches!(r.eval, Evaluation::Ready(_)))
    };
    let repo_pick = match wanted_repo {
        Some(p) => Some(p),
        None if efficient => ["graft", "graphify"].into_iter().find(|s| ready(s, &rows)),
        None => None,
    };
    let rtk_pick = o.require.iter().any(|r| r == "rtk") || efficient;
    for r in rows.iter_mut() {
        r.chosen = Some(r.short) == repo_pick || (r.short == "rtk" && rtk_pick);
    }

    // Required (explicitly asked-for) providers must be usable.
    let mut missing = Vec::new();
    for r in &rows {
        let asked =
            o.require.iter().any(|x| x == r.short) || Some(r.short) == o.provider.as_deref();
        if asked {
            if let Evaluation::Unavailable(why) = &r.eval {
                missing.push(format!("{}: {why}", r.short));
            }
        }
    }
    if !missing.is_empty() {
        return Err(d(
            "SPX-HPB060",
            format!(
                "a required provider is unavailable; nothing was changed. {}{}",
                missing.join(" | "),
                finder
                    .notes
                    .iter()
                    .map(|n| format!(" | note: {n}"))
                    .collect::<String>()
            ),
        ));
    }

    // Drop optional picks that are unavailable (builtin fallback) and decide actions.
    let mut changes: Vec<String> = Vec::new();
    for r in rows.iter_mut() {
        if !r.chosen {
            continue;
        }
        let Evaluation::Unavailable(_) = &r.eval else {
            let Evaluation::Ready(rd) = &r.eval else {
                unreachable!()
            };
            let id = r.desc.provider_id.clone();
            r.action = if is_current(&state, &id, &files_dir, r.short, rd, r.kind) {
                "current"
            } else if foreign_adoption(&state, &id, &files_dir, r.short).is_some() {
                "kept"
            } else {
                "adopt"
            };
            continue;
        };
        r.chosen = false;
    }
    let want_toml = profile_toml(&rows);
    let cfg_path = project.join(CONFIG_FILE);
    let (cfg_action, cfg_note) = if cfg_path.exists() {
        let ok = HarnessConfig::load(&project).is_ok_and(|c| {
            [
                CapabilityKind::ContextRepository,
                CapabilityKind::CommandView,
            ]
            .iter()
            .all(|k| c.capability(*k).provider.as_deref() == Some(pin_of(&rows, *k).as_str()))
        });
        if ok {
            ("current", String::new())
        } else {
            ("kept", format!("existing {CONFIG_FILE} differs from the setup profile and was left untouched; the suggested content is in the report"))
        }
    } else {
        ("create", String::new())
    };
    let pending = rows.iter().any(|r| r.chosen && r.action == "adopt") || cfg_action == "create";
    let apply = o.yes && !o.dry_run;

    if apply {
        if rows.iter().any(|r| r.chosen && r.action == "adopt") {
            let snap = assets::materialize(&home)?;
            let mut aenv = env.clone();
            for r in rows.iter_mut().filter(|r| r.chosen && r.action == "adopt") {
                let Evaluation::Ready(rd) = &r.eval else {
                    continue;
                };
                let desc_path = assets::descriptor_in(&snap.files_dir, r.short).expect("shipped");
                if let Some(rt) = &rd.runtime {
                    let var = super::runtime::env_var(r.desc.runtime).unwrap_or("HARNESS_NODE");
                    aenv.vars
                        .insert(var.into(), rt.to_string_lossy().into_owned());
                }
                let rep = adopt(
                    &aenv,
                    &desc_path,
                    &AdoptOptions {
                        upstream: Some(rd.upstream.clone()),
                        project: project.clone(),
                        allow_project_local: false,
                    },
                )?;
                if !rep
                    .installation
                    .upstream
                    .as_ref()
                    .is_some_and(|u| u.compatible)
                {
                    return Err(d(
                        "SPX-HPB060",
                        format!("{} was not identified as compatible at adoption", r.short),
                    ));
                }
                if let Some(rt) = &rd.runtime {
                    set_runtime(env, &r.desc.provider_id, rt, &project)?;
                }
                let t = super::cli::trust_verb(
                    &[
                        r.desc.provider_id.clone(),
                        "--prefer".into(),
                        r.kind.as_str().into(),
                    ],
                    env,
                );
                if t.code != 0 {
                    return Err(d(
                        "SPX-HPB060",
                        format!(
                            "trust of {} was refused: {}",
                            r.desc.provider_id,
                            t.stderr.trim()
                        ),
                    ));
                }
                changes.push(format!(
                    "adopted and trusted {} (runtime recorded; preferred for {})",
                    r.desc.provider_id,
                    r.kind.as_str()
                ));
            }
        }
        if cfg_action == "create" {
            std::fs::write(&cfg_path, &want_toml).map_err(|e| {
                d(
                    "SPX-HPB062",
                    format!("cannot write {}: {e}", cfg_path.display()),
                )
            })?;
            changes.push(format!("wrote {CONFIG_FILE}"));
        }
    }
    let noop = !pending;
    Ok(render(
        o,
        &rows,
        &finder.notes,
        &notes,
        &node,
        &python,
        cfg_action,
        &cfg_note,
        &want_toml,
        &changes,
        noop,
        apply,
    ))
}

fn tool_line(r: &Result<(PathBuf, String), String>) -> (Value, String) {
    match r {
        Ok((p, v)) => (
            json!({"path": p.to_string_lossy(), "version": v}),
            format!("{} ({v})", p.display()),
        ),
        Err(e) => (json!({"error": e}), format!("not found: {e}")),
    }
}

#[allow(clippy::too_many_arguments)]
fn render(
    o: &Opts,
    rows: &[Row],
    find_notes: &[String],
    notes: &[String],
    node: &Result<(PathBuf, String), String>,
    python: &Result<(PathBuf, String), String>,
    cfg_action: &str,
    cfg_note: &str,
    toml: &str,
    changes: &[String],
    noop: bool,
    applied: bool,
) -> Outcome {
    let mode = if applied {
        "applied"
    } else if o.dry_run {
        "dry-run"
    } else {
        "plan"
    };
    let preset = if o.preset == Preset::Native {
        "native"
    } else {
        "local-efficient"
    };
    let mut human = format!("harness setup ({mode}, preset {preset})\n");
    let (nj, nl) = tool_line(node);
    let (pj, pl) = tool_line(python);
    human.push_str(&format!("node: {nl}\npython: {pl}\n"));
    let mut provs = Vec::new();
    for r in rows {
        let (status, line) = match &r.eval {
            Evaluation::Ready(rd) => (
                "ready",
                format!(
                    "{} {} at {}",
                    r.desc.provider_id,
                    rd.version,
                    rd.upstream.display()
                ),
            ),
            Evaluation::Unavailable(w) => (
                "unavailable",
                format!("{} unavailable ({w}); builtin fallback", r.desc.provider_id),
            ),
        };
        let act = if r.chosen { r.action } else { "not-selected" };
        human.push_str(&format!("{}: {status}; {act}: {line}\n", r.short));
        provs.push(json!({
            "name": r.short, "provider_id": r.desc.provider_id, "status": status,
            "action": act, "detail": line, "capability": r.kind.as_str(),
            "upstream": match &r.eval { Evaluation::Ready(rd) => json!({"path": rd.upstream.to_string_lossy(), "version": rd.version}), _ => Value::Null },
        }));
    }
    for n in find_notes.iter().chain(notes) {
        human.push_str(&format!("note: {n}\n"));
    }
    human.push_str(&format!("project profile {CONFIG_FILE}: {cfg_action}\n"));
    if !cfg_note.is_empty() {
        human.push_str(&format!("note: {cfg_note}\n"));
    }
    let wants = rows
        .iter()
        .filter(|r| r.chosen)
        .map(|r| r.short)
        .collect::<Vec<_>>();
    let mut cmd = format!("semaprax harness setup --project . --preset {preset}");
    if let Some(g) = wants.iter().find(|s| **s != "rtk") {
        cmd.push_str(&format!(" --provider {g}"));
    }
    cmd.push_str(" --path-dirs <dirs holding your tools> --yes");
    let teammate = format!(
        "{cmd}\nneeds: {}",
        rows.iter()
            .filter(|r| r.chosen)
            .map(|r| format!(
                "{} {}",
                r.desc.upstream.as_ref().map_or("", |u| u.name.as_str()),
                r.desc
                    .upstream
                    .as_ref()
                    .map_or(String::new(), |u| u.versions.join("|"))
            ))
            .collect::<Vec<_>>()
            .join(", ")
    );
    if noop && !applied && changes.is_empty() {
        human.push_str("no changes needed; setup is already complete\n");
    } else if applied {
        for c in changes {
            human.push_str(&format!("changed: {c}\n"));
        }
        if changes.is_empty() {
            human.push_str("no changes; setup was already complete\n");
        }
    } else {
        human.push_str(if o.dry_run {
            "dry run: nothing was changed\n"
        } else {
            "plan only: re-run with --yes to apply\n"
        });
    }
    human.push_str(&format!(
        "teammate setup (machine-local paths differ per machine):\n  {}\n",
        teammate.replace('\n', "\n  ")
    ));
    human.push_str("a pinned managed install is the job of `semaprax harness updates`; setup never installs or downloads\n");
    let doc = json!({
        "schema": SETUP_SCHEMA, "mode": mode, "preset": preset, "noop": noop,
        "node": nj, "python": pj, "providers": provs,
        "project_profile": {"file": CONFIG_FILE, "action": cfg_action, "suggested": toml},
        "notes": find_notes.iter().chain(notes).collect::<Vec<_>>(),
        "changes": changes, "teammate_plan": teammate,
    });
    Outcome::ok(if o.json {
        format!("{}\n", json::canonical(&doc))
    } else {
        human
    })
}
