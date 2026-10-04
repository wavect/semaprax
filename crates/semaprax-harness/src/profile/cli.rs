//! Verb implementations for `status explain resolve adopt trust revoke inspect`.

use super::adopt::{adopt, AdoptOptions};
use super::config::HarnessConfig;
use super::installations::LocalState;
use super::resolve::resolve;
use super::trust::{requested_as_granted, TrustRecord};
use super::{lock, status};
use crate::cli::{Environment, Outcome};
use crate::contract::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::PathBuf;

struct Args {
    pos: Vec<String>,
    val: BTreeMap<String, String>,
    flag: Vec<String>,
}

fn usage(msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPB050", msg)
}

fn parse_args(args: &[String], valued: &[&str], booleans: &[&str]) -> HarnessResult<Args> {
    let mut a = Args {
        pos: Vec::new(),
        val: BTreeMap::new(),
        flag: Vec::new(),
    };
    let mut it = args.iter();
    while let Some(x) = it.next() {
        if valued.contains(&x.as_str()) {
            let v = it
                .next()
                .ok_or_else(|| usage(format!("`{x}` needs a value")))?;
            a.val.insert(x.clone(), v.clone());
        } else if booleans.contains(&x.as_str()) {
            a.flag.push(x.clone());
        } else if x.starts_with("--") {
            return Err(usage(format!("unknown option `{x}`")));
        } else {
            a.pos.push(x.clone());
        }
    }
    Ok(a)
}

impl Args {
    fn has(&self, f: &str) -> bool {
        self.flag.iter().any(|x| x == f)
    }
    fn project(&self, env: &Environment) -> PathBuf {
        self.val
            .get("--project")
            .map_or_else(|| env.cwd.clone(), |p| env.cwd.join(p))
    }
    fn one(&self, what: &str) -> HarnessResult<&str> {
        match self.pos.as_slice() {
            [x] => Ok(x),
            _ => Err(usage(format!("expected exactly one {what}"))),
        }
    }
}

fn finish(r: HarnessResult<Outcome>) -> Outcome {
    match r {
        Ok(o) => o,
        Err(e) if e.code == "SPX-HPB050" => Outcome::usage(e.message),
        Err(e) => Outcome::refused(&e),
    }
}

fn emit(as_json: bool, human: String, doc: Value) -> Outcome {
    Outcome::ok(if as_json {
        format!("{}\n", json::canonical(&doc))
    } else {
        human
    })
}

pub fn status_verb(args: &[String], env: &Environment) -> Outcome {
    finish((|| {
        let a = parse_args(args, &["--project"], &["--json"])?;
        let project = a.project(env);
        let config = HarnessConfig::load(&project)?;
        let state = LocalState::load(env)?;
        let (text, ok) = status::status(&project, &config, &state, a.has("--json"));
        Ok(Outcome {
            code: if ok { 0 } else { 1 },
            stdout: text,
            stderr: String::new(),
        })
    })())
}

pub fn explain_verb(args: &[String], env: &Environment) -> Outcome {
    finish((|| {
        let a = parse_args(args, &["--project"], &["--json"])?;
        let name = a.one("capability kind")?;
        let kind = CapabilityKind::parse(name)
            .ok_or_else(|| usage(format!("unknown capability kind `{name}`")))?;
        let config = HarnessConfig::load(&a.project(env))?;
        let text = status::explain(&config, &LocalState::load(env)?, kind);
        Ok(emit(
            a.has("--json"),
            text.clone(),
            json!({"kind": name, "explanation": text.lines().collect::<Vec<_>>()}),
        ))
    })())
}

pub fn resolve_verb(args: &[String], env: &Environment) -> Outcome {
    finish((|| {
        let a = parse_args(args, &["--project"], &["--frozen", "--json"])?;
        let project = a.project(env);
        let config = HarnessConfig::load(&project)?;
        let state = LocalState::load(env)?;
        let res = resolve(&config, &state)?;
        let frozen = a.has("--frozen");
        if frozen {
            let l = lock::load(&project)?.ok_or_else(|| {
                HarnessDiagnostic::new(
                    "SPX-HPB012",
                    format!(
                        "--frozen needs {}; run `semaprax harness resolve` and commit it",
                        lock::LOCK_FILE
                    ),
                )
            })?;
            lock::verify_frozen(&l, &res.profile)?;
        } else {
            lock::write(&project, &res.profile)?;
        }
        let lines: Vec<String> = res
            .profile
            .bindings
            .iter()
            .map(|b| {
                format!(
                    "{} {} {}",
                    b.kind.as_str(),
                    b.state.as_str(),
                    if b.provider_id.is_empty() {
                        "-"
                    } else {
                        &b.provider_id
                    }
                )
            })
            .collect();
        let verb = if frozen { "verified" } else { "wrote" };
        let human = format!("{verb} {}\n{}\n", lock::LOCK_FILE, lines.join("\n"));
        Ok(emit(
            a.has("--json"),
            human,
            json!({"ok": true, "frozen": frozen, "lock_digest": res.profile.lock_digest(), "bindings": lines}),
        ))
    })())
}

pub fn adopt_verb(args: &[String], env: &Environment) -> Outcome {
    finish((|| {
        let a = parse_args(
            args,
            &[
                "--project",
                "--upstream",
                "--runtime",
                "--skills",
                "--origin",
            ],
            &["--allow-project-local", "--json"],
        )?;
        if let Some(dir) = a.val.get("--skills") {
            if !a.pos.is_empty() || a.val.contains_key("--runtime") {
                return Err(usage(
                    "`adopt --skills <dir>` takes no descriptor or --runtime",
                ));
            }
            let r = super::adopt::add_skill_root(
                env,
                std::path::Path::new(dir),
                a.val.get("--origin").map(String::as_str),
                &a.project(env),
            )?;
            return Ok(emit(
                a.has("--json"),
                format!(
                    "approved skill root {} (origin {})\n",
                    r.path.display(),
                    r.origin
                ),
                json!({"ok": true, "skill_root": r.path.to_string_lossy(), "origin": r.origin}),
            ));
        }
        let desc = env.cwd.join(a.one("descriptor path")?);
        let opts = AdoptOptions {
            upstream: a.val.get("--upstream").map(PathBuf::from),
            project: a.project(env),
            allow_project_local: a.has("--allow-project-local"),
        };
        // The identity probe finds a shebang runtime in the directory of the
        // runtime the user named (the probe PATH holds only those directories).
        let mut probe_env = env.clone();
        if let Some(rt) = a.val.get("--runtime") {
            for k in ["HARNESS_NODE", "HARNESS_PYTHON"] {
                probe_env.vars.insert(k.into(), rt.clone());
            }
        }
        let mut r = adopt(&probe_env, &desc, &opts)?;
        if let Some(rt) = a.val.get("--runtime") {
            let exe = super::adopt::set_runtime(
                env,
                &r.installation.provider_id,
                std::path::Path::new(rt),
                &opts.project,
            )?;
            r.installation.runtime = Some(exe);
        }
        let i = &r.installation;
        let mut human = format!(
            "adopted {} (descriptor {})\n",
            i.provider_id, i.descriptor_digest
        );
        if let Some(u) = &i.upstream {
            human.push_str(&format!(
                "upstream version {} compatible={}\n",
                u.version.as_deref().unwrap_or("unidentified"),
                u.compatible
            ));
        }
        if let Some(rt) = &i.runtime {
            human.push_str(&format!("runtime {}\n", rt.display()));
        }
        for n in &r.notes {
            human.push_str(&format!("note: {n}\n"));
        }
        human.push_str(&format!(
            "not trusted yet: run `semaprax harness trust {}` to approve its permissions\n",
            i.provider_id
        ));
        Ok(emit(
            a.has("--json"),
            human,
            json!({"ok": true, "provider_id": i.provider_id, "descriptor_digest": i.descriptor_digest, "notes": r.notes}),
        ))
    })())
}

pub fn trust_verb(args: &[String], env: &Environment) -> Outcome {
    finish((|| {
        let a = parse_args(args, &["--prefer"], &["--json"])?;
        let id = a.one("provider id")?.to_string();
        if super::builtin::is_builtin(&id) {
            return Ok(Outcome::ok(format!(
                "{id} is builtin; builtin providers need no trust grant\n"
            )));
        }
        let mut state = LocalState::load(env)?;
        let inst = state
            .installations
            .get(&id)
            .ok_or_else(|| HarnessDiagnostic::new("SPX-HPB023", format!("provider `{id}` is not adopted; run `semaprax harness adopt <descriptor>` first")))?;
        let insp = inst.inspect()?;
        let changed = insp.current.descriptor_digest != inst.descriptor_digest
            || insp.current.entry_digest != inst.entry_digest;
        let upstream_changed = inst
            .upstream
            .as_ref()
            .is_some_and(|u| insp.current.upstream_digest.as_deref() != Some(u.digest.as_str()));
        if changed || upstream_changed {
            return Err(HarnessDiagnostic::new(
                "SPX-HPB031",
                format!("`{id}` changed on disk since adoption; run adopt again before trusting"),
            ));
        }
        if insp.current.requires_upstream {
            let ok = inst.upstream.as_ref().is_some_and(|u| u.compatible)
                && insp.current.upstream_digest.is_some();
            if !ok {
                return Err(HarnessDiagnostic::new("SPX-HPB033", format!("`{id}` has no adopted, compatible upstream executable; adopt it with --upstream first")));
            }
        }
        let rec = TrustRecord {
            descriptor_digest: insp.current.descriptor_digest.clone(),
            entry_digest: insp.current.entry_digest.clone(),
            upstream_digest: insp.current.upstream_digest.clone(),
            granted: requested_as_granted(&insp.current.requested),
        };
        let p = &rec.granted;
        let summary = format!(
            "read=[{}] write=[{}] network=[{}] process=[{}] secrets=[{}]",
            p.read.join(","),
            p.write.join(","),
            p.network.join(","),
            p.process.join(","),
            p.secrets.join(",")
        );
        let mut preferred = None;
        if let Some(k) = a.val.get("--prefer") {
            let kind = CapabilityKind::parse(k)
                .ok_or_else(|| usage(format!("unknown capability kind `{k}`")))?;
            if !insp
                .descriptor
                .capabilities
                .iter()
                .any(|c| c.kind == Some(kind))
            {
                return Err(usage(format!("`{id}` does not declare `{k}`")));
            }
            state.preferences.insert(kind, id.clone());
            preferred = Some(kind.as_str());
        }
        state.trust.insert(id.clone(), rec);
        state.save_trust()?;
        if preferred.is_some() {
            state.save_preferences()?;
        }
        let human = format!("trusted {id}: {summary}\nbound to the current descriptor, entry and upstream digests; any change requires trusting again\n");
        Ok(emit(
            a.has("--json"),
            human,
            json!({"ok": true, "provider_id": id, "granted": summary, "preferred": preferred}),
        ))
    })())
}

pub fn revoke_verb(args: &[String], env: &Environment) -> Outcome {
    finish((|| {
        let a = parse_args(args, &[], &["--json"])?;
        let id = a.one("provider id")?.to_string();
        let mut state = LocalState::load(env)?;
        let had = state.trust.remove(&id).is_some();
        state.save_trust()?;
        let human = if had {
            format!("revoked {id}; its next dispatch is refused\n")
        } else {
            format!("{id} was not trusted\n")
        };
        Ok(emit(
            a.has("--json"),
            human,
            json!({"ok": true, "provider_id": id, "was_trusted": had}),
        ))
    })())
}

pub fn inspect_verb(args: &[String], env: &Environment) -> Outcome {
    finish((|| {
        let a = parse_args(args, &[], &["--json"])?;
        let id = a.one("provider id")?;
        let text = status::inspect(&LocalState::load(env)?, id)?;
        Ok(emit(
            a.has("--json"),
            text.clone(),
            json!({"provider_id": id, "report": text.lines().collect::<Vec<_>>()}),
        ))
    })())
}
