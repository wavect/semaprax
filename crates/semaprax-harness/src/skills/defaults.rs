//! Default-available official skills with task-aware, scoped selection
//! (HN-04, HN-06). Deterministic and local: selection makes zero model calls.
//! Bodies load only when selected or requested; upstream bytes are never edited.

use super::catalog::{ApprovedRoot, Catalog};
use super::frame::policy_frame;
use super::modes::{
    check_ident, parse_instruction, validate_mode, validate_prefs, Layers, Prefs, Resolved, Scope,
    SessionState, StateStore, SHIPPED,
};
use super::official::{unknown, OfficialSet, OfficialSkill};
use super::resources::ResourceLoad;
use super::snapshot;
use super::{d, SkillCatalogConfig, SkillService};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};

/// Per-skill state for reports. Default-available, selected, loaded,
/// applied-to-model, omitted and disabled are distinct facts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillReport {
    pub id: String,
    pub version: String,
    pub digest: String,
    pub default_available: bool,
    pub selected: bool,
    /// Body read and framed by the host in this call.
    pub loaded: bool,
    /// Confirmed consumed by a model request (`mark_applied`); never inferred.
    pub applied_to_model: bool,
    pub omitted: Option<String>,
    pub disabled: Option<String>,
    pub mode: String,
    pub source: &'static str,
    pub locked_revision: Option<String>,
    pub model_visible_bytes: usize,
}

impl SkillReport {
    pub fn to_json(&self) -> Value {
        json!({"id": self.id, "version": self.version, "digest": self.digest,
               "default_available": self.default_available, "selected": self.selected,
               "loaded": self.loaded, "applied_to_model": self.applied_to_model,
               "omitted": self.omitted, "disabled": self.disabled, "mode": self.mode,
               "mode_source": self.source, "locked_revision": self.locked_revision,
               "model_visible_bytes": self.model_visible_bytes})
    }
}

#[derive(Clone, Debug, Default)]
pub struct TaskInput<'a> {
    /// Task family (`localized_debug`, `refactor`, `translation`, ...).
    pub family: &'a str,
    /// The current user instruction, when there is one.
    pub instruction: Option<&'a str>,
}

#[derive(Clone, Debug, Default)]
pub struct DefaultSelection {
    /// Exactly the text a model would see (empty when nothing is selected).
    pub text: String,
    pub model_visible_bytes: usize,
    pub reports: Vec<SkillReport>,
    pub diagnostics: Vec<HarnessDiagnostic>,
    /// Host answers to `/<skill> status` requests.
    pub status_lines: Vec<String>,
    /// Always 0: selection is deterministic metadata matching.
    pub selection_model_calls: u32,
}

#[derive(Clone, Debug, Default)]
pub struct StatusReport {
    /// Layer that disabled all optional skills, when one did.
    pub switch_off: Option<&'static str>,
    pub skills: Vec<SkillReport>,
    pub status_lines: Vec<String>,
}

impl StatusReport {
    pub fn to_json(&self) -> Value {
        json!({"official_switch_off_by": self.switch_off,
               "skills": self.skills.iter().map(SkillReport::to_json).collect::<Vec<_>>(),
               "status_lines": self.status_lines})
    }
}

pub struct DefaultSkills {
    set: OfficialSet,
    store: Option<StateStore>,
    project: String,
    session: String,
    /// `[skills]` keys of the project's configuration (project layer).
    config_prefs: Prefs,
    mem: SessionState,
    cost: super::cost_profile::CostPolicy,
}

fn cap(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

impl DefaultSkills {
    /// `home` is the harness home; `None` keeps state in memory only.
    pub fn new(
        set: OfficialSet,
        home: Option<std::path::PathBuf>,
        project: &str,
        session: &str,
    ) -> HarnessResult<Self> {
        check_ident("project", project)?;
        check_ident("session", session)?;
        Ok(Self {
            set,
            store: home.map(StateStore::new),
            project: project.into(),
            session: session.into(),
            config_prefs: Prefs::default(),
            mem: SessionState::default(),
            cost: Default::default(),
        })
    }

    pub fn embedded(
        home: Option<std::path::PathBuf>,
        project: &str,
        session: &str,
    ) -> HarnessResult<Self> {
        Self::new(OfficialSet::embedded(), home, project, session)
    }

    /// The project's configured preset (`[skills] official/preset/ponytail/caveman`).
    pub fn with_project_prefs(mut self, prefs: Prefs) -> HarnessResult<Self> {
        validate_prefs(&self.set, &prefs)?;
        self.config_prefs = prefs;
        Ok(self)
    }

    /// Opt-in cost-aware activation (TC-08); the default policy changes nothing.
    pub fn with_cost_policy(mut self, cost: super::cost_profile::CostPolicy) -> Self {
        self.cost = cost;
        self
    }

    pub fn cost_policy(&self) -> &super::cost_profile::CostPolicy {
        &self.cost
    }

    pub fn set(&self) -> &OfficialSet {
        &self.set
    }

    fn skill(&self, name: &str) -> HarnessResult<&OfficialSkill> {
        if let Some(k) = self.set.embedded_skills().find(|k| k.matches(name)) {
            return Ok(k);
        }
        // A declared-but-unsupported variant gets its own refusal.
        for k in self.set.embedded_skills() {
            let named = k
                .applicability
                .as_ref()
                .is_some_and(|a| a.unsupported_triggers.contains_key(name));
            if let (true, Some(f)) = (named, k.feature(name)) {
                return Err(d(
                    "SPX-HPM039",
                    format!("`{name}` is not available in this harness: {}", f.note),
                ));
            }
        }
        Err(unknown(name))
    }

    fn load_state(&self) -> HarnessResult<SessionState> {
        match &self.store {
            Some(s) => s.load_session(&self.project, &self.session),
            None => Ok(self.mem.clone()),
        }
    }

    fn save_state(&mut self, st: &mut SessionState) -> HarnessResult<()> {
        match &self.store {
            Some(s) => s.save_session(&self.project, &self.session, st),
            None => {
                st.revision += 1;
                self.mem = st.clone();
                Ok(())
            }
        }
    }

    fn layers(&self, st: &SessionState, explicit: Prefs) -> HarnessResult<Layers> {
        let (project, user) = match &self.store {
            Some(s) => (
                self.config_prefs
                    .overlaid_by(&s.load_prefs(Scope::Project, &self.project)?),
                s.load_prefs(Scope::User, &self.project)?,
            ),
            None => (self.config_prefs.clone(), Prefs::default()),
        };
        Ok(Layers {
            explicit,
            session: st.prefs.clone(),
            project,
            user,
        })
    }

    fn report(&self, k: &OfficialSkill, layers: &Layers, st: &SessionState) -> SkillReport {
        let Resolved { mode, source } = layers.resolve(&self.set, k);
        let disabled = layers
            .switch_off()
            .map(|s| format!("official-skills-switch-off:{s}"))
            .or_else(|| (mode == "off" && source != SHIPPED).then(|| format!("mode-off:{source}")));
        SkillReport {
            id: k.id.clone(),
            version: k.version.clone(),
            digest: k.bundle_digest.clone().unwrap_or_default(),
            default_available: layers.switch_off().is_none(),
            selected: false,
            loaded: false,
            applied_to_model: st.applied.contains(&k.id),
            omitted: None,
            disabled,
            mode,
            source,
            locked_revision: st.locks.get(&k.id).map(|(dg, _)| dg.clone()),
            model_visible_bytes: 0,
        }
    }

    /// `Caveman mode: <on|off|unknown>`: never inferred from a configured default.
    fn status_line(&self, k: &OfficialSkill, st: &SessionState) -> String {
        let v = st
            .active
            .get(&k.id)
            .cloned()
            .or_else(|| {
                (st.prefs.modes.get(&k.id).map(String::as_str) == Some("off")).then(|| "off".into())
            })
            .unwrap_or_else(|| "unknown".into());
        format!("{} mode: {v}", cap(&k.id))
    }

    /// Metadata listing of every curated skill; reads no body and no disk.
    pub fn list(&self) -> HarnessResult<Vec<Value>> {
        let st = self.load_state()?;
        let layers = self.layers(&st, Prefs::default())?;
        Ok(self
            .set
            .skills
            .iter()
            .map(|k| {
                let mut v = json!({
                    "id": k.id, "aliases": k.aliases, "authorship": k.authorship,
                    "version": k.version, "license": k.license, "license_file": k.license_file,
                    "repo": k.repo, "subpath": k.subpath, "channel": k.channel, "tag": k.tag,
                    "commit": k.commit, "re_resolved": k.re_resolved,
                    "bundle_digest": k.bundle_digest, "compatibility": k.compatibility,
                    "embedded": k.embedded,
                    "files": k.files.iter().map(|f| json!({"path": f.path, "upstream_path": f.upstream_path,
                        "git_blob_sha": f.git_blob_sha, "sha256": f.sha256, "bytes": f.bytes})).collect::<Vec<_>>(),
                    "features": k.features.iter().map(|f| json!({"name": f.name, "status": f.status, "note": f.note})).collect::<Vec<_>>(),
                });
                if k.embedded {
                    v["state"] = self.report(k, &layers, &st).to_json();
                }
                v
            })
            .collect())
    }

    fn snapshot_for(&self, k: &OfficialSkill, digest: &str) -> HarnessResult<snapshot::Snapshot> {
        let store = self.store.as_ref().ok_or_else(|| {
            d(
                "SPX-HPM041",
                "no harness home: official skills cannot be materialized",
            )
        })?;
        if Some(digest) == k.bundle_digest.as_deref() {
            self.set
                .materialize(&k.id, &store.snapshots(), &store.scratch())
        } else {
            snapshot::open(&store.snapshots(), digest).map_err(|_| {
                d("SPX-HPM037", format!("locked revision {digest} of `{}` is no longer available; end the session or reset its lock", k.id))
            })
        }
    }

    fn service(
        &self,
        k: &OfficialSkill,
        digest: &str,
    ) -> HarnessResult<(SkillService, snapshot::Snapshot, String)> {
        let snap = self.snapshot_for(k, digest)?;
        let version = if Some(digest) == k.bundle_digest.as_deref() {
            k.version.clone()
        } else {
            "locked".into()
        };
        let root = ApprovedRoot {
            path: snap.files_dir.clone(),
            origin: format!("official:{}@{version} ({})", k.id, k.authorship),
            approved_digest: Some(digest.to_string()),
        };
        let cfg = SkillCatalogConfig {
            enabled: true,
            ..Default::default()
        };
        Ok((SkillService::new(vec![root], cfg), snap, version))
    }

    /// Revision to use in this session: the existing lock, else the current one.
    fn lock(&self, k: &OfficialSkill, st: &mut SessionState) -> HarnessResult<String> {
        if let Some((dg, _)) = st.locks.get(&k.id) {
            return Ok(dg.clone());
        }
        let dg = k.bundle_digest.clone().ok_or_else(|| unknown(&k.id))?;
        st.locks
            .insert(k.id.clone(), (dg.clone(), k.version.clone()));
        Ok(dg)
    }

    fn refuse_if_off(&self, layers: &Layers) -> HarnessResult<()> {
        match layers.switch_off() {
            Some(src) => Err(d("SPX-HPM042", format!("official skills are disabled by the {src} switch; re-enable with `skills use all --scope project|user` or `[skills] official = true`"))),
            None => Ok(()),
        }
    }

    fn rendered(
        &self,
        k: &OfficialSkill,
        st: &mut SessionState,
        mode: &str,
        source: &str,
    ) -> HarnessResult<(String, String)> {
        let digest = self.lock(k, st)?;
        let (mut svc, _, _) = self.service(k, &digest)?;
        let body = svc.load(&digest)?;
        let mut text = body.text;
        text.push_str(&policy_frame(k, mode, source, &digest));
        Ok((text, digest))
    }

    /// Explicit lazy load by id, alias or digest: framed body for the mode
    /// currently resolved (host policy rendered separately from upstream text).
    pub fn load(&mut self, name: &str) -> HarnessResult<(SkillReport, String)> {
        let k = match self
            .set
            .embedded_skills()
            .find(|k| k.bundle_digest.as_deref() == Some(name))
        {
            Some(k) => k.clone(),
            None => self.skill(name)?.clone(),
        };
        let mut st = self.load_state()?;
        let layers = self.layers(&st, Prefs::default())?;
        self.refuse_if_off(&layers)?;
        let r = layers.resolve(&self.set, &k);
        let (text, _) = self.rendered(&k, &mut st, &r.mode, r.source)?;
        let mut rep = self.report(&k, &layers, &st);
        rep.loaded = true;
        rep.locked_revision = st.locks.get(&k.id).map(|(d, _)| d.clone());
        rep.model_visible_bytes = text.len();
        self.save_state(&mut st)?;
        Ok((rep, text))
    }

    /// Progressive resource (for example `LICENSE`) of a skill's locked revision.
    pub fn load_resource(&mut self, name: &str, path: &str) -> HarnessResult<ResourceLoad> {
        let k = self.skill(name)?.clone();
        let st = self.load_state()?;
        let layers = self.layers(&st, Prefs::default())?;
        self.refuse_if_off(&layers)?;
        let digest = st
            .locks
            .get(&k.id)
            .map(|(dg, _)| dg.clone())
            .or_else(|| k.bundle_digest.clone())
            .ok_or_else(|| unknown(name))?;
        let (mut svc, snap, _) = self.service(&k, &digest)?;
        let want = snap
            .inventory
            .get(path)
            .map(|e| e.sha256.clone())
            .ok_or_else(|| {
                d(
                    "SPX-HPM033",
                    format!("resource `{path}`: not in the skill's inventory"),
                )
            })?;
        svc.load_resource(&digest, path, &want)
    }

    /// `skills use <name> [mode]` at `scope`; `all` re-enables the switch.
    pub fn use_skill(
        &mut self,
        name: &str,
        mode: Option<&str>,
        scope: Scope,
    ) -> HarnessResult<StatusReport> {
        if name == "all" {
            return self.set_switch(true, scope);
        }
        let k = self.skill(name)?.clone();
        let a = k.applicability.clone().unwrap_or_default();
        let mode = mode
            .map(String::from)
            .unwrap_or_else(|| a.shipped_or_default());
        validate_mode(&self.set, &k.id, &mode)?;
        let mut st = self.load_state()?;
        let layers = self.layers(&st, Prefs::default())?;
        self.refuse_if_off(&layers)?;
        self.write_mode(&k, &mode, scope, &mut st)?;
        if scope == Scope::Session && mode != "off" {
            self.lock(&k, &mut st)?;
            st.active.insert(k.id.clone(), mode);
        }
        self.save_session_if(scope, &mut st)?;
        self.status()
    }

    /// `skills off <name|all> [--scope S]`. `all` at project/user scope is the
    /// one switch; at session scope it turns every mode off for the session.
    pub fn off(&mut self, name: &str, scope: Scope) -> HarnessResult<StatusReport> {
        if name == "all" && scope != Scope::Session {
            return self.set_switch(false, scope);
        }
        let ids: Vec<String> = if name == "all" {
            self.set.embedded_skills().map(|k| k.id.clone()).collect()
        } else {
            vec![self.skill(name)?.id.clone()]
        };
        let mut st = self.load_state()?;
        for id in ids {
            let k = self.skill(&id)?.clone();
            self.write_mode(&k, "off", scope, &mut st)?;
            if scope == Scope::Session {
                st.active.remove(&k.id);
                st.applied.retain(|a| a != &k.id);
            }
        }
        self.save_session_if(scope, &mut st)?;
        self.status()
    }

    fn write_mode(
        &self,
        k: &OfficialSkill,
        mode: &str,
        scope: Scope,
        st: &mut SessionState,
    ) -> HarnessResult<()> {
        if scope == Scope::Session {
            st.prefs.modes.insert(k.id.clone(), mode.into());
            return Ok(());
        }
        let store = self.store.as_ref().ok_or_else(|| {
            d(
                "SPX-HPM041",
                "no harness home to store a project or user preset",
            )
        })?;
        let mut p = store.load_prefs(scope, &self.project)?;
        p.modes.insert(k.id.clone(), mode.into());
        store.save_prefs(scope, &self.project, &p)
    }

    fn save_session_if(&mut self, scope: Scope, st: &mut SessionState) -> HarnessResult<()> {
        if scope == Scope::Session {
            self.save_state(st)?;
        }
        Ok(())
    }

    fn set_switch(&mut self, on: bool, scope: Scope) -> HarnessResult<StatusReport> {
        let store = match (&self.store, scope) {
            (Some(s), Scope::Project | Scope::User) => s.clone(),
            _ => return Err(d("SPX-HPM041", "the official-skills switch lives at --scope project or user and needs a harness home")),
        };
        let mut p = store.load_prefs(scope, &self.project)?;
        p.official = Some(on);
        store.save_prefs(scope, &self.project, &p)?;
        self.status()
    }

    /// Resolved state of every curated skill; `Caveman mode:` lines come from
    /// session facts, never from configured defaults.
    pub fn status(&self) -> HarnessResult<StatusReport> {
        let st = self.load_state()?;
        let layers = self.layers(&st, Prefs::default())?;
        let mut rep = StatusReport {
            switch_off: layers.switch_off(),
            ..Default::default()
        };
        for k in self.set.embedded_skills() {
            let mut r = self.report(k, &layers, &st);
            r.selected = st.active.contains_key(&k.id) && r.disabled.is_none();
            rep.skills.push(r);
            rep.status_lines.push(self.status_line(k, &st));
        }
        Ok(rep)
    }

    /// Record that the listed skills were part of a model request.
    pub fn mark_applied(&mut self, ids: &[String]) -> HarnessResult<()> {
        let mut st = self.load_state()?;
        for id in ids {
            if st.active.contains_key(id) && !st.applied.contains(id) {
                st.applied.push(id.clone());
            }
        }
        st.applied.sort();
        self.save_state(&mut st)
    }

    /// Select and frame default skills for a task within `max_bytes`. Zero
    /// model calls: curated applicability metadata, task family, the explicit
    /// instruction, scoped presets and the byte budget decide.
    pub fn select_for_task(
        &mut self,
        task: &TaskInput,
        max_bytes: usize,
    ) -> HarnessResult<DefaultSelection> {
        let mut out = DefaultSelection::default();
        let mut st = self.load_state()?;
        let instr = task
            .instruction
            .map(|t| parse_instruction(&self.set, t))
            .unwrap_or_default();
        let mut explicit = Prefs::default();
        let mut layers = self.layers(&st, Prefs::default())?;
        let before = st.clone();
        for (_, _, msg) in &instr.refused {
            out.diagnostics.push(d("SPX-HPM039", msg.clone()));
        }
        if !instr.modes.is_empty() {
            if let Err(e) = self.refuse_if_off(&layers) {
                out.diagnostics.push(e);
            } else {
                for (id, m) in &instr.modes {
                    explicit.modes.insert(id.clone(), m.clone());
                    st.prefs.modes.insert(id.clone(), m.clone());
                    if m == "off" {
                        st.active.remove(id);
                        st.applied.retain(|a| a != id);
                    }
                }
                layers = self.layers(&st, explicit)?;
            }
        }
        let mut chosen: Vec<(OfficialSkill, Resolved)> = Vec::new();
        let mut reports: Vec<SkillReport> = Vec::new();
        for k in self.set.embedded_skills() {
            let mut r = self.report(k, &layers, &st);
            let res = layers.resolve(&self.set, k);
            let a = k.applicability.clone().unwrap_or_default();
            let pick = if r.disabled.is_some() {
                false
            } else if res.mode == "off" {
                r.omitted = Some("not-requested".into());
                false
            } else if self.cost.delivered_by_host(&k.id) && res.source != "explicit-instruction" {
                r.omitted = Some("already-delivered-by-host".into());
                false
            } else if res.source == "explicit-instruction" {
                true
            } else if !a.applies_to(task.family) {
                r.omitted = Some("not-applicable-to-task-family".into());
                false
            } else if res.source == SHIPPED && !a.automatic {
                r.omitted = Some("not-requested".into());
                false
            } else if res.source == SHIPPED && self.cost.suppresses_automatic(task.family) {
                r.omitted = Some("cost-profile:tiny-structured-task".into());
                false
            } else {
                true
            };
            if pick {
                chosen.push((k.clone(), res));
            }
            reports.push(r);
        }
        chosen.sort_by_key(|(_, r)| r.source != "explicit-instruction");
        let mut text = String::new();
        for (k, res) in chosen {
            let rep = reports.iter_mut().find(|r| r.id == k.id).expect("report");
            rep.selected = true;
            match self.rendered(&k, &mut st, &res.mode, res.source) {
                Ok((t, dg)) if text.len() + t.len() <= max_bytes => {
                    rep.loaded = true;
                    rep.model_visible_bytes = t.len();
                    rep.locked_revision = Some(dg);
                    st.active.insert(k.id.clone(), res.mode.clone());
                    text.push_str(&t);
                }
                Ok((t, _)) => {
                    rep.selected = false;
                    rep.omitted = Some("content-budget".into());
                    st.locks.remove(&k.id);
                    out.diagnostics.push(d(
                        "SPX-HPM009",
                        format!(
                            "official skill `{}` ({} bytes) does not fit the remaining budget",
                            k.id,
                            t.len()
                        ),
                    ));
                }
                Err(e) => {
                    rep.selected = false;
                    rep.omitted = Some("unavailable".into());
                    out.diagnostics.push(e);
                }
            }
        }
        // Status reflects the latest turn: a skill this turn did not select is not active.
        for r in reports.iter().filter(|r| !r.selected) {
            st.active.remove(&r.id);
            st.applied.retain(|a| a != &r.id);
        }
        for id in &instr.status {
            if let Ok(k) = self.skill(id) {
                out.status_lines.push(self.status_line(k, &st));
            }
        }
        if !text.is_empty() {
            text = format!(
                "default skills selected by the host (deterministic, no model call):\n{text}"
            );
        }
        if st != before {
            self.save_state(&mut st)?;
        }
        for r in &mut reports {
            r.applied_to_model = st.applied.contains(&r.id);
        }
        out.model_visible_bytes = text.len();
        out.text = text;
        out.reports = reports;
        Ok(out)
    }

    /// Drop approved roots whose bundles shadow an official canonical id or
    /// alias with different bytes: refused loudly (`SPX-HPM040`), never silently.
    pub fn refuse_shadowing(
        &self,
        roots: &[ApprovedRoot],
    ) -> (Vec<ApprovedRoot>, Vec<HarnessDiagnostic>) {
        let reserved = self.set.reserved_names();
        let official: Vec<&str> = self
            .set
            .embedded_skills()
            .filter_map(|k| k.bundle_digest.as_deref())
            .collect();
        let cfg = SkillCatalogConfig {
            enabled: true,
            ..Default::default()
        };
        let (mut keep, mut diags) = (Vec::new(), Vec::new());
        for r in roots {
            let cat = Catalog::scan(std::slice::from_ref(r), &cfg);
            let shadow = cat
                .entries
                .iter()
                .find(|e| reserved.contains(&e.name) && !official.contains(&e.digest.as_str()));
            match shadow {
                Some(e) => diags.push(d("SPX-HPM040", format!("{} would shadow the official skill `{}` (digest {}); refused. Rename it or use the official skill", r.path.display(), e.name, e.digest))),
                None => keep.push(r.clone()),
            }
        }
        (keep, diags)
    }
}
