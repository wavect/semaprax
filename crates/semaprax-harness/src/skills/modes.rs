//! Preferences, scoped mode state and deterministic precedence (HN-06).
//!
//! Precedence per skill: explicit current instruction > session override >
//! project preset > user preset > shipped recommendation. There is no global
//! mutable mode file: session state lives in
//! `<home>/skills/state/<project>/sessions/<session>.json`, the project layer in
//! `<home>/skills/state/<project>/project.json`, the user layer in
//! `<home>/skills/user.json`.

use super::d;
use super::official::{OfficialSet, OfficialSkill};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

pub const SESSION_SCHEMA: &str = "semaprax.skill-session.v1";
pub const PREFS_SCHEMA: &str = "semaprax.skill-prefs.v1";
static SERIAL: AtomicUsize = AtomicUsize::new(0);

/// One preference layer. `modes` maps a skill id to a declared mode
/// (`off`, `lite|full|ultra`, `on`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    /// The one switch: `Some(false)` disables every optional skill.
    pub official: Option<bool>,
    pub preset: Option<String>,
    pub modes: BTreeMap<String, String>,
}

impl Prefs {
    pub fn is_empty(&self) -> bool {
        *self == Prefs::default()
    }

    pub fn to_json(&self) -> Value {
        let mut o = Map::new();
        if let Some(b) = self.official {
            o.insert("official".into(), json!(b));
        }
        if let Some(p) = &self.preset {
            o.insert("preset".into(), json!(p));
        }
        if !self.modes.is_empty() {
            o.insert("modes".into(), json!(self.modes));
        }
        Value::Object(o)
    }

    pub fn from_json(v: &Value) -> Prefs {
        Prefs {
            official: v["official"].as_bool(),
            preset: v["preset"].as_str().map(String::from),
            modes: v["modes"]
                .as_object()
                .map(|m| {
                    m.iter()
                        .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }

    /// Overlay `other` (higher priority) on `self`.
    pub fn overlaid_by(&self, other: &Prefs) -> Prefs {
        let mut p = self.clone();
        p.official = other.official.or(p.official);
        p.preset = other.preset.clone().or(p.preset);
        p.modes.extend(other.modes.clone());
        p
    }
}

/// Refuse a mode the skill does not declare, naming the unavailable variant.
pub fn validate_mode(set: &OfficialSet, id: &str, mode: &str) -> HarnessResult<()> {
    let skill = set.find(id).ok_or_else(|| super::official::unknown(id))?;
    let Some(a) = &skill.applicability else {
        return Err(d("SPX-HPM039", format!("`{id}` declares no modes")));
    };
    if mode == "off" || a.modes.iter().any(|m| m == mode) {
        return Ok(());
    }
    Err(d(
        "SPX-HPM039",
        format!(
            "`{mode}` is not a supported mode of `{}` (supported: off, {}); no variant, language or style was substituted",
            skill.id,
            a.modes.iter().filter(|m| *m != "off").cloned().collect::<Vec<_>>().join(", ")
        ),
    ))
}

pub fn validate_prefs(set: &OfficialSet, p: &Prefs) -> HarnessResult<()> {
    if let Some(name) = &p.preset {
        if !set.presets.contains_key(name) {
            return Err(d(
                "SPX-HPM039",
                format!(
                    "unknown preset `{name}` (shipped: {})",
                    set.presets.keys().cloned().collect::<Vec<_>>().join(", ")
                ),
            ));
        }
    }
    for (id, m) in &p.modes {
        validate_mode(set, id, m)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Session,
    Project,
    User,
}

impl Scope {
    pub fn parse(s: &str) -> Option<Scope> {
        match s {
            "session" => Some(Scope::Session),
            "project" => Some(Scope::Project),
            "user" => Some(Scope::User),
            _ => None,
        }
    }
}

/// Source label of a resolved value, strongest first.
pub const SOURCES: [&str; 4] = [
    "explicit-instruction",
    "session-override",
    "project-preset",
    "user-preset",
];
pub const SHIPPED: &str = "shipped-recommendation";

/// The four preference layers, strongest first.
#[derive(Clone, Debug, Default)]
pub struct Layers {
    pub explicit: Prefs,
    pub session: Prefs,
    pub project: Prefs,
    pub user: Prefs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub mode: String,
    pub source: &'static str,
}

impl Layers {
    fn all(&self) -> [(&'static str, &Prefs); 4] {
        [
            (SOURCES[0], &self.explicit),
            (SOURCES[1], &self.session),
            (SOURCES[2], &self.project),
            (SOURCES[3], &self.user),
        ]
    }

    /// Mode of `skill`: the first layer that names it, directly or by preset.
    pub fn resolve(&self, set: &OfficialSet, skill: &OfficialSkill) -> Resolved {
        for (src, p) in self.all() {
            if let Some(m) = p.modes.get(&skill.id) {
                return Resolved {
                    mode: m.clone(),
                    source: src,
                };
            }
            if let Some(m) = p
                .preset
                .as_ref()
                .and_then(|n| set.presets.get(n))
                .and_then(|pr| pr.get(&skill.id))
            {
                return Resolved {
                    mode: m.clone(),
                    source: src,
                };
            }
        }
        Resolved {
            mode: skill
                .applicability
                .as_ref()
                .map(|a| a.shipped_mode.clone())
                .unwrap_or_else(|| "off".into()),
            source: SHIPPED,
        }
    }

    /// The one switch: the first layer (project before user) that disables
    /// every optional skill. The session layer cannot flip it.
    pub fn switch_off(&self) -> Option<&'static str> {
        [(SOURCES[2], &self.project), (SOURCES[3], &self.user)]
            .into_iter()
            .find(|(_, p)| p.official == Some(false))
            .map(|(s, _)| s)
    }
}

/// Session facts that are not preferences: locked revisions and what the
/// host did. A persisted preference is not proof a mode was active.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SessionState {
    pub revision: u64,
    pub prefs: Prefs,
    /// skill id -> (digest, version) locked at first use in this session.
    pub locks: BTreeMap<String, (String, String)>,
    /// skill id -> mode the host framed for it in this session.
    pub active: BTreeMap<String, String>,
    /// skill ids confirmed consumed by a model request.
    pub applied: Vec<String>,
}

fn ident_ok(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
        && !s.starts_with('.')
}

pub fn check_ident(what: &str, s: &str) -> HarnessResult<()> {
    if ident_ok(s) {
        Ok(())
    } else {
        Err(d(
            "SPX-HPM041",
            format!("{what} id `{s}` must be 1-64 of [A-Za-z0-9._-] and not start with `.`"),
        ))
    }
}

/// File-backed state under one harness home.
#[derive(Clone, Debug)]
pub struct StateStore {
    home: PathBuf,
}

fn io(what: &str, e: impl std::fmt::Display) -> HarnessDiagnostic {
    d("SPX-HPM041", format!("{what}: {e}"))
}

fn read_json(path: &Path) -> HarnessResult<Option<Value>> {
    match std::fs::read(path) {
        Ok(b) => serde_json::from_slice(&b).map(Some).map_err(|e| {
            io(
                &format!("{} is corrupt (refusing to reset it)", path.display()),
                e,
            )
        }),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io(&format!("cannot read {}", path.display()), e)),
    }
}

fn write_atomic(path: &Path, v: &Value) -> HarnessResult<()> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| io("cannot create state directory", e))?;
    }
    let tmp = path.with_extension(format!(
        "tmp-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&tmp, format!("{}\n", crate::json::canonical(v)))
        .map_err(|e| io("cannot write state", e))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        io("cannot publish state", e)
    })
}

impl StateStore {
    pub fn new(home: PathBuf) -> Self {
        Self { home }
    }

    fn user_path(&self) -> PathBuf {
        self.home.join("skills/user.json")
    }

    fn project_path(&self, project: &str) -> PathBuf {
        self.home
            .join("skills/state")
            .join(project)
            .join("project.json")
    }

    pub fn session_path(&self, project: &str, session: &str) -> PathBuf {
        self.home
            .join("skills/state")
            .join(project)
            .join("sessions")
            .join(format!("{session}.json"))
    }

    /// Directory for materialization scratch files.
    pub fn scratch(&self) -> PathBuf {
        self.home.join("skills/staging")
    }

    /// Content-addressed immutable snapshot store.
    pub fn snapshots(&self) -> PathBuf {
        self.home.join("artifacts")
    }

    pub fn load_prefs(&self, scope: Scope, project: &str) -> HarnessResult<Prefs> {
        let path = match scope {
            Scope::User => self.user_path(),
            Scope::Project => self.project_path(project),
            Scope::Session => unreachable!("session prefs live in the session file"),
        };
        Ok(read_json(&path)?
            .map(|v| Prefs::from_json(&v["prefs"]))
            .unwrap_or_default())
    }

    pub fn save_prefs(&self, scope: Scope, project: &str, prefs: &Prefs) -> HarnessResult<()> {
        let path = match scope {
            Scope::User => self.user_path(),
            Scope::Project => self.project_path(project),
            Scope::Session => unreachable!("session prefs live in the session file"),
        };
        write_atomic(
            &path,
            &json!({"schema": PREFS_SCHEMA, "prefs": prefs.to_json()}),
        )
    }

    pub fn load_session(&self, project: &str, session: &str) -> HarnessResult<SessionState> {
        let Some(v) = read_json(&self.session_path(project, session))? else {
            return Ok(SessionState::default());
        };
        let locks = v["locks"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, l)| {
                        Some((
                            k.clone(),
                            (
                                l["digest"].as_str()?.to_string(),
                                l["version"].as_str()?.to_string(),
                            ),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
        let active = v["active"]
            .as_object()
            .map(|o| {
                o.iter()
                    .filter_map(|(k, m)| Some((k.clone(), m.as_str()?.to_string())))
                    .collect()
            })
            .unwrap_or_default();
        Ok(SessionState {
            revision: v["revision"].as_u64().unwrap_or(0),
            prefs: Prefs::from_json(&v["prefs"]),
            locks,
            active,
            applied: v["applied"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        })
    }

    /// Persist the session, bumping its revision.
    pub fn save_session(
        &self,
        project: &str,
        session: &str,
        st: &mut SessionState,
    ) -> HarnessResult<()> {
        st.revision += 1;
        let locks: BTreeMap<_, _> = st
            .locks
            .iter()
            .map(|(k, (dg, ver))| (k.clone(), json!({"digest": dg, "version": ver})))
            .collect();
        write_atomic(
            &self.session_path(project, session),
            &json!({"schema": SESSION_SCHEMA, "project": project, "session": session,
                    "revision": st.revision, "prefs": st.prefs.to_json(), "locks": locks,
                    "active": st.active, "applied": st.applied}),
        )
    }
}

// ---- explicit instruction parsing ----

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Instruction {
    /// skill id -> mode requested now (`off` for a stop).
    pub modes: BTreeMap<String, String>,
    /// skill ids whose status was requested.
    pub status: Vec<String>,
    /// (skill id, variant, message) for unavailable variants.
    pub refused: Vec<(String, String, String)>,
}

fn contains_phrase(text: &str, phrase: &str) -> bool {
    let mut from = 0;
    while let Some(i) = text[from..].find(phrase) {
        let s = from + i;
        let e = s + phrase.len();
        let before = text[..s].chars().next_back();
        let after = text[e..].chars().next();
        let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
        let starts_ok = phrase.starts_with('/') || !word(before);
        // A command or phrase may not be a prefix of a longer hyphenated command.
        let ends_ok = !word(after) && after != Some('-');
        if starts_ok && ends_ok {
            return true;
        }
        from = s + 1;
    }
    false
}

/// Deterministic parse of the current user instruction; zero model calls.
pub fn parse_instruction(set: &OfficialSet, text: &str) -> Instruction {
    let t = text.to_lowercase();
    let mut out = Instruction::default();
    for skill in set.embedded_skills() {
        let Some(a) = &skill.applicability else {
            continue;
        };
        let hit = |ps: &[String]| ps.iter().any(|p| contains_phrase(&t, p));
        let mut ids = vec![skill.id.clone()];
        ids.extend(skill.aliases.iter().cloned());
        if ids
            .iter()
            .any(|i| contains_phrase(&t, &format!("/{i} status")))
        {
            out.status.push(skill.id.clone());
            continue;
        }
        if hit(&a.stop_triggers) {
            out.modes.insert(skill.id.clone(), "off".into());
            continue;
        }
        let mut refused = false;
        for (variant, ps) in &a.unsupported_triggers {
            if hit(ps) {
                let note = skill
                    .feature(variant)
                    .map(|f| f.note.as_str())
                    .unwrap_or("");
                out.refused.push((
                    skill.id.clone(),
                    variant.clone(),
                    format!(
                        "`{variant}` is not available in this harness ({note}); nothing was switched, and the language and style are unchanged"
                    ),
                ));
                refused = true;
            }
        }
        if refused {
            continue;
        }
        // Longest trigger first so `/ponytail ultra` beats `/ponytail`.
        let mut modes: Vec<(&String, &String)> = a
            .mode_triggers
            .iter()
            .flat_map(|(m, ps)| ps.iter().map(move |p| (m, p)))
            .collect();
        modes.sort_by(|x, y| y.1.len().cmp(&x.1.len()).then(x.1.cmp(y.1)));
        if let Some((m, _)) = modes.into_iter().find(|(_, p)| contains_phrase(&t, p)) {
            out.modes.insert(skill.id.clone(), m.clone());
        } else if hit(&a.explicit_triggers) {
            out.modes.insert(skill.id.clone(), a.shipped_or_default());
        }
    }
    out
}

impl super::official::Applicability {
    /// Mode an unqualified invocation switches on: the shipped mode, else the
    /// first declared non-off mode.
    pub fn shipped_or_default(&self) -> String {
        if self.shipped_mode != "off" {
            return self.shipped_mode.clone();
        }
        self.modes
            .iter()
            .find(|m| *m == "on")
            .or(self.modes.first())
            .cloned()
            .unwrap_or_else(|| "on".into())
    }

    fn norm(s: &str) -> String {
        s.to_lowercase().replace('-', "_")
    }

    /// Whether `family` is a coding family for this skill.
    pub fn applies_to(&self, family: &str) -> bool {
        let f = Self::norm(family);
        !self.excluded_families.iter().any(|e| Self::norm(e) == f)
            && self.task_families.iter().any(|e| Self::norm(e) == f)
    }
}
