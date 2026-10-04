//! `skills` verb for the curated default skills (no `--root`):
//! `list|load <name|digest>|use <name> [mode]|status|off <name|all>`.
//! Flags: `--project ID --session ID --scope session|project|user --family F
//! --instruction TEXT --max-bytes N --resource PATH --json`.

use super::defaults::{DefaultSkills, StatusReport, TaskInput};
use super::modes::Scope;
use super::official::OfficialSet;
use super::{d, DEFAULT_MAX_BYTES};
use crate::cli::{Environment, Outcome};
use crate::json;
use serde_json::json;

/// Stable project id for the working directory (no path leaves the machine).
pub fn project_id(cwd: &std::path::Path) -> String {
    let h = json::sha256_plain(cwd.to_string_lossy().as_bytes());
    format!("p-{}", &h[7..23])
}

#[derive(Default)]
struct Opts {
    action: Option<String>,
    positional: Vec<String>,
    project: Option<String>,
    session: Option<String>,
    scope: Option<String>,
    family: Option<String>,
    instruction: Option<String>,
    resource: Option<String>,
    max_bytes: Option<usize>,
    json: bool,
}

fn parse(args: &[String]) -> Result<Opts, String> {
    let mut o = Opts::default();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut val = |f: &str| {
            it.next()
                .cloned()
                .ok_or_else(|| format!("{f} needs a value"))
        };
        match a.as_str() {
            "--json" => o.json = true,
            "--project" => o.project = Some(val("--project")?),
            "--session" => o.session = Some(val("--session")?),
            "--scope" => o.scope = Some(val("--scope")?),
            "--family" => o.family = Some(val("--family")?),
            "--instruction" => o.instruction = Some(val("--instruction")?),
            "--resource" => o.resource = Some(val("--resource")?),
            "--max-bytes" => {
                let v = val("--max-bytes")?;
                o.max_bytes = Some(
                    v.parse()
                        .map_err(|_| format!("invalid --max-bytes `{v}`"))?,
                );
            }
            f if f.starts_with("--") => return Err(format!("unexpected argument `{f}`")),
            w if o.action.is_none() => o.action = Some(w.to_string()),
            w => o.positional.push(w.to_string()),
        }
    }
    Ok(o)
}

fn render_status(r: &StatusReport) -> String {
    let mut s = String::new();
    if let Some(src) = r.switch_off {
        s.push_str(&format!("official skills disabled by {src}\n"));
    }
    for k in &r.skills {
        s.push_str(&format!(
            "{} {} mode={} ({}) available={} selected={} loaded={} applied-to-model={} omitted={} disabled={}\n",
            k.id, k.version, k.mode, k.source, k.default_available, k.selected, k.loaded,
            k.applied_to_model, k.omitted.as_deref().unwrap_or("-"), k.disabled.as_deref().unwrap_or("-")
        ));
    }
    for l in &r.status_lines {
        s.push_str(&format!("{l}\n"));
    }
    s
}

pub fn run(args: &[String], env: &Environment) -> Outcome {
    let o = match parse(args) {
        Ok(o) => o,
        Err(m) => return Outcome::usage(m),
    };
    let project = o.project.clone().unwrap_or_else(|| project_id(&env.cwd));
    let session = o.session.clone().unwrap_or_else(|| "default".into());
    let set = OfficialSet::embedded();
    let cfg_prefs = match crate::profile::config::HarnessConfig::load(&env.cwd) {
        Ok(c) => c.skills.prefs,
        Err(e) => return Outcome::refused(&e),
    };
    let built = DefaultSkills::new(set, env.harness_home.clone(), &project, &session)
        .and_then(|s| s.with_project_prefs(cfg_prefs));
    let mut svc = match built {
        Ok(s) => s,
        Err(e) => return Outcome::refused(&e),
    };
    let scope = match o.scope.as_deref().map(Scope::parse) {
        None => Scope::Session,
        Some(Some(s)) => s,
        Some(None) => return Outcome::usage("--scope must be session, project or user"),
    };
    let action = o.action.clone().unwrap_or_else(|| "list".into());
    let name = o.positional.first().cloned();
    let result: Result<Outcome, crate::diag::HarnessDiagnostic> = (|| match action.as_str() {
        "list" => {
            let skills = svc.list()?;
            let sel = o.family.as_deref().map(|f| {
                svc.select_for_task(
                    &TaskInput {
                        family: f,
                        instruction: o.instruction.as_deref(),
                    },
                    o.max_bytes.unwrap_or(DEFAULT_MAX_BYTES),
                )
            });
            let sel = sel.transpose()?;
            if o.json {
                let mut v = json!({"skills": skills});
                if let Some(s) = &sel {
                    v["selection"] = json!({"text": s.text, "model_visible_bytes": s.model_visible_bytes,
                        "selection_model_calls": s.selection_model_calls, "status_lines": s.status_lines,
                        "reports": s.reports.iter().map(|r| r.to_json()).collect::<Vec<_>>(),
                        "diagnostics": s.diagnostics.iter().map(|x| x.json()).collect::<Vec<_>>()});
                }
                return Ok(Outcome::ok(format!("{}\n", json::canonical(&v))));
            }
            let mut out = String::new();
            for k in &skills {
                out.push_str(&format!(
                    "{} {} {} [{}] {} digest={}\n",
                    k["id"].as_str().unwrap_or(""),
                    k["version"].as_str().unwrap_or(""),
                    k["license"].as_str().unwrap_or(""),
                    k["authorship"].as_str().unwrap_or(""),
                    k["compatibility"].as_str().unwrap_or(""),
                    k["bundle_digest"].as_str().unwrap_or("-")
                ));
            }
            if let Some(s) = sel {
                for r in &s.reports {
                    out.push_str(&format!(
                        "selection {} selected={} omitted={} disabled={}\n",
                        r.id,
                        r.selected,
                        r.omitted.as_deref().unwrap_or("-"),
                        r.disabled.as_deref().unwrap_or("-")
                    ));
                }
                for dg in &s.diagnostics {
                    out.push_str(&format!("{dg}\n"));
                }
                for l in &s.status_lines {
                    out.push_str(&format!("{l}\n"));
                }
            }
            Ok(Outcome::ok(out))
        }
        "load" => {
            let n =
                name.ok_or_else(|| d("SPX-HPM038", "load needs a skill id, alias or digest"))?;
            if let Some(path) = &o.resource {
                let r = svc.load_resource(&n, path)?;
                return Ok(Outcome::ok(if o.json {
                    format!(
                        "{}\n",
                        json::canonical(&json!({"skill_digest": r.skill_digest, "path": r.path,
                        "digest": r.digest, "text": r.text, "charged_bytes": r.charged_bytes}))
                    )
                } else {
                    r.text
                }));
            }
            let (rep, text) = svc.load(&n)?;
            Ok(Outcome::ok(if o.json {
                format!(
                    "{}\n",
                    json::canonical(&json!({"report": rep.to_json(), "text": text}))
                )
            } else {
                text
            }))
        }
        "use" => {
            let n = name.ok_or_else(|| d("SPX-HPM038", "use needs a skill id, alias or `all`"))?;
            let r = svc.use_skill(&n, o.positional.get(1).map(String::as_str), scope)?;
            Ok(Outcome::ok(if o.json {
                format!("{}\n", json::canonical(&r.to_json()))
            } else {
                render_status(&r)
            }))
        }
        "off" => {
            let n = name.ok_or_else(|| d("SPX-HPM038", "off needs a skill id, alias or `all`"))?;
            let r = svc.off(&n, scope)?;
            Ok(Outcome::ok(if o.json {
                format!("{}\n", json::canonical(&r.to_json()))
            } else {
                render_status(&r)
            }))
        }
        "status" => {
            let r = svc.status()?;
            Ok(Outcome::ok(if o.json {
                format!("{}\n", json::canonical(&r.to_json()))
            } else {
                render_status(&r)
            }))
        }
        other => Err(d(
            "SPX-HPM038",
            format!("unknown skills action `{other}` (list, load, use, status, off)"),
        )),
    })();
    match result {
        Ok(out) => out,
        Err(e) => Outcome::refused(&e),
    }
}
