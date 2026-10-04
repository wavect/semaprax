//! `semaprax.harness.toml`: strict TOML-subset parser and typed configuration.
//!
//! Supported subset: `[table]` / `[a."quoted.key"]` headers, bare or quoted
//! keys, basic and literal strings, integers, booleans, single-line string
//! arrays and `#` comments. Everything else is a syntax error. Diagnostics are
//! `SPX-HPB001..008`, each naming the source line.

use crate::contract::descriptor::valid_provider_id;
use crate::contract::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json;
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

pub const CONFIG_SCHEMA: &str = "semaprax.harness-config.v1";
pub const CONFIG_FILE: &str = "semaprax.harness.toml";
const MAX_CONFIG_BYTES: usize = 64 * 1024;
const MAX_BUDGET: i64 = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Disabled,
    Auto,
    Required,
}

impl Mode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Auto => "auto",
            Self::Required => "required",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CapabilityConfig {
    pub mode: Mode,
    /// Explicit project pin (a provider id).
    pub provider: Option<String>,
    /// Project-relative path prefixes the capability is limited to.
    pub scope: Vec<String>,
    /// `[capability."<kind>".config]`: adapter fields (HN-10), validated against
    /// the selected descriptor's `config.fields` when the adapter is launched.
    pub config: BTreeMap<String, Value>,
}

impl Default for CapabilityConfig {
    fn default() -> Self {
        Self {
            mode: Mode::Auto,
            provider: None,
            scope: Vec::new(),
            config: BTreeMap::new(),
        }
    }
}

/// `[routing]` (HN-16): `mode`, an optional project `pin` and `allow_remote`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoutingSection {
    /// `rules` (default), `pin`, `experimental` or `auto`.
    pub mode: String,
    pub pin: Option<String>,
    /// `Some(false)` prohibits remote routing in every mode; `Some(true)` approves
    /// the remote origins of the task's own catalog; `None` leaves the default
    /// (remote destinations stay unapproved).
    pub allow_remote: Option<bool>,
    /// `mode` was written in the file (the provider's own mode decides otherwise).
    pub explicit: bool,
}

impl Default for RoutingSection {
    fn default() -> Self {
        Self {
            mode: "rules".into(),
            pin: None,
            allow_remote: None,
            explicit: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BudgetConfig {
    pub context_max_bytes: u64,
    pub command_view_max_bytes: u64,
    /// Opt-in small initial context target in bytes (TC-05); `None` keeps the
    /// context at `context_max_bytes`.
    pub context_target_bytes: Option<u64>,
    /// Bounded target escalations through the follow-up path.
    pub context_target_escalations: u64,
    /// Opt-in repair-feedback allowance in named tokens (TC-06).
    pub feedback_max_tokens: Option<u64>,
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self {
            context_max_bytes: 16384,
            command_view_max_bytes: 8192,
            context_target_bytes: None,
            context_target_escalations: 2,
            feedback_max_tokens: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SkillsConfig {
    pub enabled: bool,
    pub select: Vec<String>,
    pub max_bytes: u64,
    /// Project preset for the curated official skills (HN-06): the one switch
    /// `official`, `preset`, and per-skill modes `ponytail` / `caveman`.
    pub prefs: crate::skills::modes::Prefs,
    /// Opt-in `standard` (default) or `compact` cost profile (TC-08).
    pub cost_profile: Option<String>,
    /// Skill ids the outer host already delivers; never injected again.
    pub host_delivered: Vec<String>,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            select: Vec::new(),
            max_bytes: 65536,
            prefs: Default::default(),
            cost_profile: None,
            host_delivered: Vec::new(),
        }
    }
}

/// `[workflow.check.<name>] argv = [...]`: authorized checks, argv only (never a shell line).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkflowConfig {
    pub checks: BTreeMap<String, Vec<String>>,
}

/// `[model]`: project guarantees the model route must be able to enforce.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModelConfig {
    pub local_only: bool,
    pub strict_one_attempt: bool,
    /// Logical model id (bound machine-locally by `endpoints`).
    pub logical: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HarnessConfig {
    pub profile_enabled: bool,
    pub capabilities: BTreeMap<CapabilityKind, CapabilityConfig>,
    pub budget: BudgetConfig,
    pub skills: SkillsConfig,
    pub workflow: WorkflowConfig,
    pub model: ModelConfig,
    pub routing: RoutingSection,
    /// Namespaced `x.` capability tables: visible, always inactive.
    pub inactive: Vec<String>,
}

impl Default for HarnessConfig {
    fn default() -> Self {
        Self {
            profile_enabled: true,
            capabilities: BTreeMap::new(),
            budget: BudgetConfig::default(),
            skills: SkillsConfig::default(),
            workflow: WorkflowConfig::default(),
            model: ModelConfig::default(),
            routing: RoutingSection::default(),
            inactive: Vec::new(),
        }
    }
}

impl HarnessConfig {
    /// Configuration for `kind`; a kind without a table is `auto`.
    pub fn capability(&self, kind: CapabilityKind) -> CapabilityConfig {
        self.capabilities.get(&kind).cloned().unwrap_or_default()
    }

    /// Canonical form (independent of comments, whitespace and key order).
    pub fn to_json(&self) -> Value {
        let mut caps = Map::new();
        for (k, c) in &self.capabilities {
            let mut o = Map::new();
            o.insert("mode".into(), json!(c.mode.as_str()));
            if let Some(p) = &c.provider {
                o.insert("provider".into(), json!(p));
            }
            o.insert("scope".into(), json!(c.scope));
            if !c.config.is_empty() {
                o.insert("config".into(), json!(c.config));
            }
            caps.insert(k.as_str().into(), Value::Object(o));
        }
        let mut doc = json!({
            "schema": CONFIG_SCHEMA,
            "profile": {"enabled": self.profile_enabled},
            "capabilities": caps,
            "budget": {
                "context_max_bytes": self.budget.context_max_bytes,
                "command_view_max_bytes": self.budget.command_view_max_bytes,
            },
            "skills": {
                "enabled": self.skills.enabled,
                "select": self.skills.select,
                "max_bytes": self.skills.max_bytes,
            },
            "inactive": self.inactive,
        });
        // Only present when set, so existing configurations keep their digest.
        if let Some(n) = self.budget.feedback_max_tokens {
            doc["budget"]["feedback_max_tokens"] = json!(n);
        }
        if let Some(n) = self.budget.context_target_bytes {
            doc["budget"]["context_target_bytes"] = json!(n);
            doc["budget"]["context_target_escalations"] =
                json!(self.budget.context_target_escalations);
        }
        if !self.skills.prefs.is_empty() {
            doc["skills"]["prefs"] = self.skills.prefs.to_json();
        }
        if let Some(c) = &self.skills.cost_profile {
            doc["skills"]["cost_profile"] = json!(c);
        }
        if !self.skills.host_delivered.is_empty() {
            doc["skills"]["host_delivered"] = json!(self.skills.host_delivered);
        }
        if !self.workflow.checks.is_empty() {
            doc["workflow"] = json!({"checks": self.workflow.checks});
        }
        if self.model != ModelConfig::default() {
            doc["model"] = json!({"local_only": self.model.local_only,
                "strict_one_attempt": self.model.strict_one_attempt, "logical": self.model.logical});
        }
        if self.routing != RoutingSection::default() {
            doc["routing"] = json!({"mode": self.routing.mode, "pin": self.routing.pin,
                "allow_remote": self.routing.allow_remote});
        }
        doc
    }

    pub fn digest(&self) -> String {
        json::digest(CONFIG_SCHEMA, &self.to_json())
    }

    /// Load `<project>/semaprax.harness.toml`; an absent file is the default
    /// configuration (every capability `auto`).
    pub fn load(project: &Path) -> HarnessResult<HarnessConfig> {
        let path = project.join(CONFIG_FILE);
        match std::fs::read(&path) {
            Ok(bytes) => parse(&bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HarnessConfig::default()),
            Err(e) => Err(HarnessDiagnostic::new(
                "SPX-HPB008",
                format!("cannot read {CONFIG_FILE}: {e}"),
            )),
        }
    }
}

fn bad(code: &'static str, line: usize, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, format!("{CONFIG_FILE}:{line}: {}", msg.into()))
}

#[derive(Clone, Debug, PartialEq)]
enum Val {
    Str(String),
    Int(i64),
    Bool(bool),
    List(Vec<String>),
}

impl Val {
    fn type_name(&self) -> &'static str {
        match self {
            Val::Str(_) => "string",
            Val::Int(_) => "integer",
            Val::Bool(_) => "boolean",
            Val::List(_) => "string array",
        }
    }
}

type Entries = BTreeMap<String, (Val, usize)>;

#[derive(Default)]
struct RawDoc {
    root: Entries,
    tables: Vec<(Vec<String>, usize, Entries)>,
}

fn strip_comment(s: &str) -> &str {
    let mut quote: Option<char> = None;
    let mut esc = false;
    for (i, c) in s.char_indices() {
        match quote {
            Some(q) => {
                if esc {
                    esc = false;
                } else if c == '\\' && q == '"' {
                    esc = true;
                } else if c == q {
                    quote = None;
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '#' => return &s[..i],
            None => {}
        }
    }
    s
}

struct Cursor<'a> {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    _src: &'a str,
}

impl<'a> Cursor<'a> {
    fn new(src: &'a str, line: usize) -> Self {
        Self {
            chars: src.chars().collect(),
            pos: 0,
            line,
            _src: src,
        }
    }
    fn ws(&mut self) {
        while self
            .chars
            .get(self.pos)
            .is_some_and(|c| *c == ' ' || *c == '\t')
        {
            self.pos += 1;
        }
    }
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn end(&mut self) -> HarnessResult<()> {
        self.ws();
        match self.peek() {
            None => Ok(()),
            Some(c) => Err(bad("SPX-HPB001", self.line, format!("unexpected `{c}`"))),
        }
    }
    fn string(&mut self) -> HarnessResult<String> {
        let q = self.peek().unwrap_or('"');
        self.pos += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(bad("SPX-HPB001", self.line, "unterminated string"));
            };
            self.pos += 1;
            if c == q {
                return Ok(out);
            }
            if c == '\\' && q == '"' {
                let Some(e) = self.peek() else {
                    return Err(bad("SPX-HPB001", self.line, "unterminated escape"));
                };
                self.pos += 1;
                out.push(match e {
                    '"' => '"',
                    '\\' => '\\',
                    'n' => '\n',
                    't' => '\t',
                    other => {
                        return Err(bad(
                            "SPX-HPB001",
                            self.line,
                            format!("unsupported escape `\\{other}`"),
                        ))
                    }
                });
            } else if c.is_control() {
                return Err(bad("SPX-HPB001", self.line, "control character in string"));
            } else {
                out.push(c);
            }
        }
    }
    fn key(&mut self) -> HarnessResult<String> {
        self.ws();
        match self.peek() {
            Some('"') | Some('\'') => self.string(),
            _ => {
                let start = self.pos;
                while self
                    .peek()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
                {
                    self.pos += 1;
                }
                if start == self.pos {
                    return Err(bad("SPX-HPB001", self.line, "expected a key"));
                }
                Ok(self.chars[start..self.pos].iter().collect())
            }
        }
    }
    fn value(&mut self) -> HarnessResult<Val> {
        self.ws();
        match self.peek() {
            Some('"') | Some('\'') => {
                let s = self.string()?;
                self.end()?;
                Ok(Val::Str(s))
            }
            Some('[') => {
                self.pos += 1;
                let mut items = Vec::new();
                loop {
                    self.ws();
                    match self.peek() {
                        Some(']') => {
                            self.pos += 1;
                            break;
                        }
                        Some('"') | Some('\'') => {
                            items.push(self.string()?);
                            self.ws();
                            match self.peek() {
                                Some(',') => self.pos += 1,
                                Some(']') => {}
                                _ => {
                                    return Err(bad(
                                        "SPX-HPB001",
                                        self.line,
                                        "expected `,` or `]` in array",
                                    ))
                                }
                            }
                        }
                        Some(_) => {
                            return Err(bad(
                                "SPX-HPB004",
                                self.line,
                                "arrays may contain only strings",
                            ))
                        }
                        None => {
                            return Err(bad(
                                "SPX-HPB001",
                                self.line,
                                "unterminated array (arrays must fit on one line)",
                            ))
                        }
                    }
                }
                self.end()?;
                Ok(Val::List(items))
            }
            Some(_) => {
                let rest: String = self.chars[self.pos..].iter().collect();
                let rest = rest.trim();
                match rest {
                    "true" => Ok(Val::Bool(true)),
                    "false" => Ok(Val::Bool(false)),
                    r if !r.is_empty()
                        && r.trim_start_matches('-')
                            .bytes()
                            .all(|b| b.is_ascii_digit())
                        && !r.trim_start_matches('-').is_empty() =>
                    {
                        r.parse::<i64>()
                            .map(Val::Int)
                            .map_err(|_| bad("SPX-HPB004", self.line, "integer out of range"))
                    }
                    r => Err(bad(
                        "SPX-HPB001",
                        self.line,
                        format!("cannot parse value `{r}`"),
                    )),
                }
            }
            None => Err(bad("SPX-HPB001", self.line, "missing value")),
        }
    }
}

fn parse_raw(text: &str) -> HarnessResult<RawDoc> {
    let mut doc = RawDoc::default();
    let mut current: Option<usize> = None;
    for (i, raw_line) in text.lines().enumerate() {
        let line = i + 1;
        let stripped = strip_comment(raw_line);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with("[[") {
            return Err(bad(
                "SPX-HPB001",
                line,
                "arrays of tables are not supported",
            ));
        }
        if trimmed.starts_with('[') {
            let mut c = Cursor::new(trimmed, line);
            c.pos = 1;
            let mut path = Vec::new();
            loop {
                path.push(c.key()?);
                c.ws();
                match c.peek() {
                    Some('.') => c.pos += 1,
                    Some(']') => {
                        c.pos += 1;
                        break;
                    }
                    _ => return Err(bad("SPX-HPB001", line, "malformed table header")),
                }
            }
            c.end()?;
            if let Some((_, first, _)) = doc.tables.iter().find(|(p, _, _)| *p == path) {
                return Err(bad(
                    "SPX-HPB002",
                    line,
                    format!(
                        "duplicate table [{}] (first at line {first})",
                        path.join(".")
                    ),
                ));
            }
            doc.tables.push((path, line, Entries::new()));
            current = Some(doc.tables.len() - 1);
            continue;
        }
        let mut c = Cursor::new(trimmed, line);
        let key = c.key()?;
        c.ws();
        match c.peek() {
            Some('=') => c.pos += 1,
            Some('.') => {
                return Err(bad(
                    "SPX-HPB001",
                    line,
                    "dotted keys are not supported; use a table header",
                ))
            }
            _ => return Err(bad("SPX-HPB001", line, "expected `=` after key")),
        }
        let val = c.value()?;
        let entries = match current {
            Some(t) => &mut doc.tables[t].2,
            None => &mut doc.root,
        };
        if let Some((_, first)) = entries.get(&key) {
            return Err(bad(
                "SPX-HPB002",
                line,
                format!("duplicate key `{key}` (first at line {first})"),
            ));
        }
        entries.insert(key, (val, line));
    }
    Ok(doc)
}

fn edit_distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur.push((prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost));
        }
        prev = cur;
    }
    prev[b.len()]
}

fn hint(name: &str, known: &[&str]) -> String {
    known
        .iter()
        .filter(|k| edit_distance(name, k) <= 2)
        .min_by_key(|k| edit_distance(name, k))
        .map(|k| format!("; did you mean `{k}`?"))
        .unwrap_or_default()
}

/// A table being consumed; leftovers are unknown keys.
struct Tab {
    what: String,
    entries: Entries,
}

impl Tab {
    fn new(what: &str, entries: Entries) -> Self {
        Self {
            what: what.into(),
            entries,
        }
    }
    fn take(&mut self, key: &str) -> Option<(Val, usize)> {
        self.entries.remove(key)
    }
    fn wrong(&self, key: &str, want: &str, v: &Val, line: usize) -> HarnessDiagnostic {
        bad(
            "SPX-HPB004",
            line,
            format!(
                "`{key}` in {} must be a {want}, found {}",
                self.what,
                v.type_name()
            ),
        )
    }
    fn string(&mut self, key: &str) -> HarnessResult<Option<(String, usize)>> {
        match self.take(key) {
            None => Ok(None),
            Some((Val::Str(s), l)) => Ok(Some((s, l))),
            Some((v, l)) => Err(self.wrong(key, "string", &v, l)),
        }
    }
    fn boolean(&mut self, key: &str) -> HarnessResult<Option<bool>> {
        match self.take(key) {
            None => Ok(None),
            Some((Val::Bool(b), _)) => Ok(Some(b)),
            Some((v, l)) => Err(self.wrong(key, "boolean", &v, l)),
        }
    }
    fn size(&mut self, key: &str) -> HarnessResult<Option<u64>> {
        match self.take(key) {
            None => Ok(None),
            Some((Val::Int(n), l)) => {
                if (1..=MAX_BUDGET).contains(&n) {
                    Ok(Some(n as u64))
                } else {
                    Err(bad(
                        "SPX-HPB004",
                        l,
                        format!("`{key}` must be between 1 and {MAX_BUDGET}"),
                    ))
                }
            }
            Some((v, l)) => Err(self.wrong(key, "integer", &v, l)),
        }
    }
    fn list(&mut self, key: &str) -> HarnessResult<Option<(Vec<String>, usize)>> {
        match self.take(key) {
            None => Ok(None),
            Some((Val::List(v), l)) => Ok(Some((v, l))),
            Some((v, l)) => Err(self.wrong(key, "string array", &v, l)),
        }
    }
    fn finish(self, known: &[&str]) -> HarnessResult<()> {
        match self.entries.iter().min_by_key(|(_, (_, l))| *l) {
            None => Ok(()),
            Some((k, (_, l))) => Err(bad(
                "SPX-HPB003",
                *l,
                format!("unknown key `{k}` in {}{}", self.what, hint(k, known)),
            )),
        }
    }
}

/// Parse and validate a configuration document.
pub fn parse(bytes: &[u8]) -> HarnessResult<HarnessConfig> {
    if bytes.len() > MAX_CONFIG_BYTES {
        return Err(bad(
            "SPX-HPB001",
            1,
            format!("configuration exceeds {MAX_CONFIG_BYTES} bytes"),
        ));
    }
    let text = std::str::from_utf8(bytes)
        .map_err(|_| bad("SPX-HPB001", 1, "configuration is not valid UTF-8"))?;
    let doc = parse_raw(text)?;

    let mut root = Tab::new("the document root", doc.root.clone());
    match root.string("schema")? {
        Some((s, _)) if s == CONFIG_SCHEMA => {}
        Some((s, l)) => {
            return Err(bad(
                "SPX-HPB005",
                l,
                format!("unsupported schema `{s}`; expected `{CONFIG_SCHEMA}`"),
            ))
        }
        None => {
            return Err(bad(
                "SPX-HPB005",
                1,
                format!("missing `schema = \"{CONFIG_SCHEMA}\"`"),
            ))
        }
    }
    root.finish(&["schema"])?;

    let mut cfg = HarnessConfig::default();
    let mut tables: Vec<&(Vec<String>, usize, Entries)> = doc.tables.iter().collect();
    tables.sort_by_key(|t| t.1);
    for (path, line, entries) in tables {
        let line = *line;
        let segs: Vec<&str> = path.iter().map(String::as_str).collect();
        match segs.as_slice() {
            ["profile"] => {
                let mut t = Tab::new("[profile]", entries.clone());
                if let Some(b) = t.boolean("enabled")? {
                    cfg.profile_enabled = b;
                }
                t.finish(&["enabled"])?;
            }
            ["budget"] => {
                let mut t = Tab::new("[budget]", entries.clone());
                if let Some(n) = t.size("context_max_bytes")? {
                    cfg.budget.context_max_bytes = n;
                }
                if let Some(n) = t.size("command_view_max_bytes")? {
                    cfg.budget.command_view_max_bytes = n;
                }
                if let Some(n) = t.size("context_target_bytes")? {
                    cfg.budget.context_target_bytes = Some(n);
                }
                if let Some(n) = t.size("context_target_escalations")? {
                    cfg.budget.context_target_escalations = n;
                }
                if let Some(n) = t.size("feedback_max_tokens")? {
                    cfg.budget.feedback_max_tokens = Some(n);
                }
                t.finish(&[
                    "feedback_max_tokens",
                    "context_max_bytes",
                    "command_view_max_bytes",
                    "context_target_bytes",
                    "context_target_escalations",
                ])?;
            }
            ["skills"] => {
                let mut t = Tab::new("[skills]", entries.clone());
                if let Some(b) = t.boolean("enabled")? {
                    cfg.skills.enabled = b;
                }
                if let Some((v, _)) = t.list("select")? {
                    cfg.skills.select = v;
                }
                if let Some(n) = t.size("max_bytes")? {
                    cfg.skills.max_bytes = n;
                }
                let set = crate::skills::official::embedded_cached();
                if let Some(b) = t.boolean("official")? {
                    cfg.skills.prefs.official = Some(b);
                }
                if let Some((p, l)) = t.string("preset")? {
                    cfg.skills.prefs.preset = Some(p);
                    crate::skills::modes::validate_prefs(set, &cfg.skills.prefs)
                        .map_err(|e| bad("SPX-HPB004", l, e.message))?;
                }
                for id in ["ponytail", "caveman"] {
                    if let Some((m, l)) = t.string(id)? {
                        crate::skills::modes::validate_mode(set, id, &m)
                            .map_err(|e| bad("SPX-HPB004", l, e.message))?;
                        cfg.skills.prefs.modes.insert(id.to_string(), m);
                    }
                }
                if let Some((c, l)) = t.string("cost_profile")? {
                    if crate::skills::cost_profile::CostProfile::parse(&c).is_none() {
                        return Err(bad(
                            "SPX-HPB004",
                            l,
                            format!("`cost_profile` must be `standard` or `compact`, not `{c}`"),
                        ));
                    }
                    cfg.skills.cost_profile = Some(c);
                }
                if let Some((v, _)) = t.list("host_delivered")? {
                    cfg.skills.host_delivered = v;
                }
                t.finish(&[
                    "enabled",
                    "select",
                    "max_bytes",
                    "cost_profile",
                    "host_delivered",
                    "official",
                    "preset",
                    "ponytail",
                    "caveman",
                ])?;
            }
            ["workflow", "check", name] => {
                let what = format!("[workflow.check.{name}]");
                let mut t = Tab::new(&what, entries.clone());
                let (argv, l) = t
                    .list("argv")?
                    .ok_or_else(|| bad("SPX-HPB004", line, format!("{what} needs `argv`")))?;
                if argv.is_empty() || argv.iter().any(String::is_empty) || name.is_empty() {
                    return Err(bad("SPX-HPB004", l, "`argv` must hold non-empty strings"));
                }
                t.finish(&["argv"])?;
                cfg.workflow.checks.insert((*name).to_string(), argv);
            }
            ["model"] => {
                let mut t = Tab::new("[model]", entries.clone());
                if let Some(b) = t.boolean("local_only")? {
                    cfg.model.local_only = b;
                }
                if let Some(b) = t.boolean("strict_one_attempt")? {
                    cfg.model.strict_one_attempt = b;
                }
                if let Some((id, l)) = t.string("logical")? {
                    if id.is_empty() || id.len() > 128 {
                        return Err(bad("SPX-HPB004", l, "`logical` must be a short model id"));
                    }
                    cfg.model.logical = Some(id);
                }
                t.finish(&["local_only", "strict_one_attempt", "logical"])?;
            }
            ["routing"] => {
                let mut t = Tab::new("[routing]", entries.clone());
                if let Some((m, l)) = t.string("mode")? {
                    if !["rules", "pin", "experimental", "auto"].contains(&m.as_str()) {
                        return Err(bad(
                            "SPX-HPB004",
                            l,
                            format!("`mode` must be rules, pin, experimental or auto, found `{m}`"),
                        ));
                    }
                    cfg.routing.mode = m;
                    cfg.routing.explicit = true;
                }
                if let Some((p, l)) = t.string("pin")? {
                    if p.is_empty() || p.len() > 128 {
                        return Err(bad(
                            "SPX-HPB004",
                            l,
                            "`pin` must be a short logical model id",
                        ));
                    }
                    cfg.routing.pin = Some(p);
                }
                if let Some(b) = t.boolean("allow_remote")? {
                    cfg.routing.allow_remote = Some(b);
                }
                t.finish(&["mode", "pin", "allow_remote"])?;
                if cfg.routing.mode == "pin" && cfg.routing.pin.is_none() {
                    return Err(bad("SPX-HPB004", line, "`mode = \"pin\"` needs `pin`"));
                }
            }
            ["capability", name, "config"] => {
                let Some(kind) = CapabilityKind::parse(name) else {
                    if name.starts_with("x.") {
                        continue;
                    }
                    return Err(bad(
                        "SPX-HPB006",
                        line,
                        format!("unknown capability kind `{name}`"),
                    ));
                };
                let mut fields = BTreeMap::new();
                for (k, (v, l)) in entries.iter() {
                    let j = match v {
                        Val::Str(s) => json!(s),
                        Val::Int(n) => json!(n),
                        Val::Bool(b) => json!(b),
                        Val::List(items) => json!(items),
                    };
                    if k.is_empty() || k.len() > 64 {
                        return Err(bad("SPX-HPB004", *l, "config field names are 1..64 bytes"));
                    }
                    fields.insert(k.clone(), j);
                }
                cfg.capabilities.entry(kind).or_default().config = fields;
            }
            ["capability", name] => {
                let Some(kind) = CapabilityKind::parse(name) else {
                    if name.starts_with("x.") {
                        cfg.inactive.push((*name).to_string());
                        continue;
                    }
                    let known: Vec<&str> = CapabilityKind::ALL
                        .iter()
                        .map(CapabilityKind::as_str)
                        .collect();
                    return Err(bad(
                        "SPX-HPB006",
                        line,
                        format!("unknown capability kind `{name}` (extensions must be namespaced `x.`){}", hint(name, &known)),
                    ));
                };
                let what = format!("[capability.\"{name}\"]");
                let mut t = Tab::new(&what, entries.clone());
                let mut c = CapabilityConfig::default();
                if let Some((m, l)) = t.string("mode")? {
                    c.mode = match m.as_str() {
                        "disabled" => Mode::Disabled,
                        "auto" => Mode::Auto,
                        "required" => Mode::Required,
                        other => {
                            return Err(bad(
                                "SPX-HPB004",
                                l,
                                format!(
                                    "`mode` must be disabled, auto or required, found `{other}`"
                                ),
                            ))
                        }
                    };
                }
                if let Some((p, l)) = t.string("provider")? {
                    if !valid_provider_id(&p) {
                        return Err(bad(
                            "SPX-HPB004",
                            l,
                            format!("`provider` `{p}` is not a provider id (`<org>/<name>`)"),
                        ));
                    }
                    c.provider = Some(p);
                }
                if let Some((s, l)) = t.list("scope")? {
                    for p in &s {
                        if p.is_empty() || p.contains('\\') || p.split('/').any(|seg| seg == "..") {
                            return Err(bad(
                                "SPX-HPB004",
                                l,
                                format!("scope `{p}` must be a project-relative path"),
                            ));
                        }
                    }
                    c.scope = s;
                }
                t.finish(&["mode", "provider", "scope"])?;
                c.config = cfg
                    .capabilities
                    .get(&kind)
                    .map(|p| p.config.clone())
                    .unwrap_or_default();
                cfg.capabilities.insert(kind, c);
            }
            other => {
                let name = other.join(".");
                return Err(bad(
                    "SPX-HPB003",
                    line,
                    format!(
                        "unknown table [{name}]{}",
                        hint(
                            &name,
                            &[
                                "profile",
                                "budget",
                                "skills",
                                "capability",
                                "workflow",
                                "model",
                                "routing"
                            ]
                        )
                    ),
                ));
            }
        }
    }
    cfg.inactive.sort();

    // Committed configuration may request but never embed machine or secret data.
    let mut all: Vec<(&Entries, ())> = vec![(&doc.root, ())];
    all.extend(doc.tables.iter().map(|t| (&t.2, ())));
    for (entries, _) in all {
        for (k, (v, l)) in entries {
            let strings: Vec<&String> = match v {
                Val::Str(s) => vec![s],
                Val::List(items) => items.iter().collect(),
                _ => continue,
            };
            for s in strings {
                if looks_like_absolute_path(s) {
                    return Err(bad("SPX-HPB007", *l, format!("`{k}` holds an absolute or home-relative path; committed configuration must be portable")));
                }
                if looks_like_secret(s) {
                    return Err(bad(
                        "SPX-HPB007",
                        *l,
                        format!("`{k}` holds a secret-looking value; never commit secrets"),
                    ));
                }
            }
        }
    }
    Ok(cfg)
}

/// Absolute POSIX, UNC, drive-letter or `~` paths.
pub fn looks_like_absolute_path(s: &str) -> bool {
    let b = s.as_bytes();
    s.starts_with('/')
        || s.starts_with('~')
        || s.starts_with("\\\\")
        || (b.len() >= 3
            && b[0].is_ascii_alphabetic()
            && b[1] == b':'
            && (b[2] == b'\\' || b[2] == b'/'))
        || s.contains("/Users/")
        || s.contains("/home/")
}

/// Conservative secret heuristics: well-known token prefixes, credentials in a
/// URL, `key=value` credential assignments and long opaque tokens.
pub fn looks_like_secret(s: &str) -> bool {
    const PREFIXES: [&str; 11] = [
        "sk-",
        "ghp_",
        "gho_",
        "ghs_",
        "github_pat_",
        "xox",
        "AKIA",
        "AIza",
        "-----BEGIN",
        "Bearer ",
        "eyJ",
    ];
    if PREFIXES.iter().any(|p| s.starts_with(p)) {
        return true;
    }
    let lower = s.to_ascii_lowercase();
    if ["api_key=", "apikey=", "token=", "password=", "secret="]
        .iter()
        .any(|p| lower.contains(p))
    {
        return true;
    }
    if let Some((_, rest)) = s.split_once("://") {
        let authority = rest.split('/').next().unwrap_or("");
        if authority.contains('@') && authority.split('@').next().is_some_and(|u| u.contains(':')) {
            return true;
        }
    }
    s.len() >= 32
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'=' | b'_' | b'-'))
        && s.bytes().any(|b| b.is_ascii_digit())
        && s.bytes().any(|b| b.is_ascii_alphabetic())
}
