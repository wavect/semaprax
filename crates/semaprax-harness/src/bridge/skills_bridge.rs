//! `bridge/skills/*` (protocol v2): one adapter over the same `DefaultSkills`
//! catalog that native runs and `skills ...` use. No prompt text lives here:
//! every delivered byte comes from `DefaultSkills::load`/`load_resource`.
//!
//! Identity: project id (`cli_defaults::project_id` of the project directory),
//! session id (handshake `session`, default `default` as in the CLI). Locks,
//! modes and `off` live in the same session state the CLI reads, so CLI, bridge
//! and editor agree. The bridge observes delivery to the host, never model use.

use super::hostskills::{HostSkill, Revision};
use crate::cli::Environment;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::canonical;
use crate::profile::HarnessConfig;
use crate::skills::cli_defaults::project_id;
use crate::skills::defaults::DefaultSkills;
use crate::skills::modes::Scope;
use serde_json::{json, Map, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

pub const METHODS: [&str; 6] = [
    "bridge/skills/list",
    "bridge/skills/load",
    "bridge/skills/resource",
    "bridge/skills/use",
    "bridge/skills/status",
    "bridge/skills/off",
];
pub const STATUS_SCHEMA: &str = "semaprax.bridge-skills-status.v1";
pub const DEFAULT_SESSION: &str = "default";

fn diag(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

fn closed<'a>(v: &'a Value, allowed: &[&str]) -> HarnessResult<&'a Map<String, Value>> {
    let m = v
        .as_object()
        .ok_or_else(|| diag("SPX-HPN005", "params must be an object"))?;
    if let Some(k) = m.keys().find(|k| !allowed.contains(&k.as_str())) {
        return Err(diag("SPX-HPN005", format!("unknown param `{k}`")));
    }
    Ok(m)
}

fn name_of(m: &Map<String, Value>) -> HarnessResult<&str> {
    m.get("name")
        .and_then(Value::as_str)
        .filter(|n| !n.is_empty() && n.len() <= 128)
        .ok_or_else(|| diag("SPX-HPN005", "`name` must be a skill id, alias or digest"))
}

fn scope_of(m: &Map<String, Value>) -> HarnessResult<Scope> {
    match m.get("scope").and_then(Value::as_str) {
        None => Ok(Scope::Session),
        Some(s) => Scope::parse(s)
            .ok_or_else(|| diag("SPX-HPN005", "`scope` must be session, project or user")),
    }
}

pub struct SkillsBridge<'a> {
    env: &'a Environment,
    project: PathBuf,
    pub project_id: String,
    pub session: String,
    pub host: String,
    pub host_skills: Vec<HostSkill>,
    pub model_routing_delegated: bool,
    pub log: Option<PathBuf>,
    delivered: Vec<Value>,
}

impl<'a> SkillsBridge<'a> {
    pub fn new(env: &'a Environment, project: &Path, session: Option<&str>, host: &str) -> Self {
        let project = env.cwd.join(project);
        let project = project.canonicalize().unwrap_or(project);
        Self {
            env,
            project_id: project_id(&project),
            project,
            session: session.unwrap_or(DEFAULT_SESSION).to_string(),
            host: host.to_string(),
            host_skills: Vec::new(),
            model_routing_delegated: false,
            log: None,
            delivered: Vec::new(),
        }
    }

    fn service(&self) -> HarnessResult<DefaultSkills> {
        let prefs = HarnessConfig::load(&self.project)?.skills.prefs;
        DefaultSkills::embedded(
            self.env.harness_home.clone(),
            &self.project_id,
            &self.session,
        )?
        .with_project_prefs(prefs)
    }

    fn owner_entry(&self, id: &str) -> Option<&HostSkill> {
        self.host_skills.iter().find(|h| h.id == id)
    }

    /// Append one observation (also to `--log` when set).
    pub fn observe(&mut self, event: Value) {
        self.record(event);
    }

    fn record(&mut self, mut event: Value) {
        let seq = self.delivered.len() as u64 + 1;
        event["seq"] = json!(seq);
        event["host"] = json!(self.host);
        event["project"] = json!(self.project_id);
        event["session"] = json!(self.session);
        if let Some(path) = &self.log {
            let line = format!("{}\n", canonical(&event));
            let _ = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| f.write_all(line.as_bytes()));
        }
        self.delivered.push(event);
    }

    /// Exact delivery observations so far (host-side delivery, not model use).
    pub fn deliveries(&self) -> &[Value] {
        &self.delivered
    }

    /// Ownership document for the handshake response.
    pub fn ownership(&self) -> Value {
        let owned: Vec<Value> = self.host_skills.iter().map(HostSkill::to_json).collect();
        json!({
            "owner": if owned.is_empty() { "semaprax" } else { "per-skill" },
            "host_owned": owned,
            "reason": "an official skill the host already has installed is never injected again; `force` on load/use overrides explicitly",
        })
    }

    pub fn handle(&mut self, method: &str, params: &Value) -> HarnessResult<Value> {
        match method {
            "bridge/skills/list" => {
                closed(params, &[])?;
                let skills = self.service()?.list()?;
                self.record(json!({"event": "skills.list", "count": skills.len()}));
                Ok(
                    json!({"schema": "semaprax.bridge-skills-list.v1", "project": self.project_id,
                    "session": self.session, "skills": skills, "host_owned": self.ownership()["host_owned"]}),
                )
            }
            "bridge/skills/load" => {
                let m = closed(params, &["name", "force"])?;
                let force = m.get("force").and_then(Value::as_bool) == Some(true);
                self.deliver(name_of(m)?, force, "load", None)
            }
            "bridge/skills/resource" => {
                let m = closed(params, &["name", "path"])?;
                let path = m
                    .get("path")
                    .and_then(Value::as_str)
                    .ok_or_else(|| diag("SPX-HPN005", "`path` must be a string"))?;
                let r = self.service()?.load_resource(name_of(m)?, path)?;
                self.record(
                    json!({"event": "skills.resource", "skill": name_of(m)?, "path": r.path,
                    "revision": r.skill_digest, "digest": r.digest, "bytes": r.text.len()}),
                );
                Ok(r.payload())
            }
            "bridge/skills/use" => {
                let m = closed(params, &["name", "mode", "scope", "force"])?;
                let name = name_of(m)?;
                let scope = scope_of(m)?;
                let mode = m.get("mode").and_then(Value::as_str);
                let status = self.service()?.use_skill(name, mode, scope)?;
                self.record(json!({"event": "skills.mode", "skill": name, "mode": mode,
                    "scope": format!("{scope:?}").to_lowercase()}));
                if name == "all" || mode == Some("off") {
                    return Ok(
                        json!({"status": status.to_json(), "delivery": {"state": "none", "reason": "nothing to deliver"}}),
                    );
                }
                let force = m.get("force").and_then(Value::as_bool) == Some(true);
                self.deliver(name, force, "use", Some(status.to_json()))
            }
            "bridge/skills/off" => {
                let m = closed(params, &["name", "scope"])?;
                let name = name_of(m)?;
                let status = self.service()?.off(name, scope_of(m)?)?;
                self.record(json!({"event": "skills.off", "skill": name}));
                Ok(json!({"status": status.to_json()}))
            }
            "bridge/skills/status" => {
                let m = closed(params, &["updates"])?;
                let mut doc = self.service()?.status()?.to_json();
                doc["schema"] = json!(STATUS_SCHEMA);
                doc["project"] = json!(self.project_id);
                doc["session"] = json!(self.session);
                doc["host"] = json!(self.host);
                doc["host_owned"] = self.ownership()["host_owned"].clone();
                doc["model_routing"] = json!(if self.model_routing_delegated {
                    "delegated"
                } else {
                    "not-delegated"
                });
                doc["delivery"] = json!(self.delivered);
                doc["delivery_scope"] = json!("delivery to the host only; model consumption is not observed (applied_to_model stays false unless the host confirms)");
                if m.get("updates").and_then(Value::as_bool) == Some(true) {
                    doc["updates"] = updates_view(self.env);
                }
                Ok(doc)
            }
            other => Err(diag("SPX-HPN004", format!("unknown method `{other}`"))),
        }
    }

    fn deliver(
        &mut self,
        name: &str,
        force: bool,
        via: &str,
        status: Option<Value>,
    ) -> HarnessResult<Value> {
        let mut svc = self.service()?;
        let owned = svc
            .set()
            .embedded_skills()
            .find(|k| k.matches(name))
            .and_then(|k| self.owner_entry(&k.id).cloned());
        if let (Some(h), false) = (&owned, force) {
            self.record(
                json!({"event": "skills.host-owned", "skill": h.id, "via": via,
                "host_digest": h.digest, "revision_match": h.revision.as_str()}),
            );
            let note = match h.revision {
                Revision::Same => "the host already has this exact official revision installed",
                Revision::Different => "the host already has this skill installed at a different revision; not replaced (pass force to deliver Semaprax's pinned revision)",
                Revision::Unverified => "the host declared this skill installed; its revision was not verified",
            };
            return Ok(
                json!({"status": status, "delivery": {"state": "host-owned", "skill": h.id,
                "owner": h.to_json(), "reason": format!("{note}; no duplicate insertion")}}),
            );
        }
        let (rep, text) = svc.load(name)?;
        let revision = rep.locked_revision.clone().unwrap_or_default();
        self.record(json!({"event": "skills.delivered", "skill": rep.id, "via": via, "version": rep.version,
            "revision": revision, "mode": rep.mode, "mode_source": rep.source, "bytes": text.len(), "forced": force && owned.is_some()}));
        Ok(
            json!({"status": status, "report": rep.to_json(), "delivery": {"state": "delivered", "skill": rep.id,
            "revision": revision, "version": rep.version, "mode": rep.mode, "bytes": text.len(), "text": text}}),
        )
    }
}

/// Pending updates from `updates status` (offline, read-only); unavailable is stated, not hidden.
fn updates_view(env: &Environment) -> Value {
    let out =
        crate::updates::cli_updates(&["status".into(), "--json".into(), "--offline".into()], env);
    if out.code != 0 {
        return json!({"available": false, "reason": out.stderr.trim()});
    }
    match serde_json::from_str::<Value>(&out.stdout) {
        Ok(v) => {
            let pending: Vec<Value> = v["sources"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter(|s| !s["candidate"].is_null())
                        .map(|s| json!({"id": s["id"], "active": s["active"], "candidate": s["candidate"], "state": s["state"]}))
                        .collect()
                })
                .unwrap_or_default();
            json!({"available": true, "pending": pending})
        }
        Err(e) => json!({"available": false, "reason": format!("updates status document: {e}")}),
    }
}
