//! Task evidence needs and bounded retrieval plans (HN-13). Needs come from the
//! task (selected symbols, goal, diagnostics, manifest), never from whether the
//! native graph traversal was complete. `.spx` meaning stays with the compiler;
//! only a concrete foreign-language, configuration or unresolved-reference need
//! reaches the single project-selected repository provider. A plan is at most
//! [`MAX_PROVIDER_CALLS`] calls: the initial query, one unmet-need retry and one failed-candidate follow-up.
//! Diagnostics `SPX-HPE070..073`.

use super::identity::{scan_spx, span_digest, Snapshot};
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::json::sha256_plain;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::{Component, Path};

pub const PLAN_SCHEMA: &str = "semaprax.harness-context-plan.v1";
/// Initial query, at most one goal-worded retry for an unmet need and at most
/// one focused follow-up after a failed candidate; nothing recursive.
pub const MAX_PROVIDER_CALLS: u32 = 3;
const INITIAL_MAX_ITEMS: u64 = 24;
const FOLLOW_UP_MAX_ITEMS: u64 = 8;
const MAX_TOKENS: usize = 8;

const LANGUAGE_TERMS: &[(&str, &str)] = &[
    ("typescript", "typescript"),
    ("ts", "typescript"),
    ("tsx", "typescript"),
    ("frontend", "typescript"),
    ("browser", "typescript"),
    ("web", "typescript"),
    ("javascript", "javascript"),
    ("js", "javascript"),
    ("node", "javascript"),
    ("rust", "rust"),
    ("cargo", "rust"),
    ("crate", "rust"),
    ("python", "python"),
    ("py", "python"),
    ("golang", "go"),
];
const CONFIG_TERMS: &[&str] = &["config", "configuration", "toml", "yaml", "env", "settings"];
/// Words in a goal that say the public interface (not just a body) changes.
const INTERFACE_TERMS: &[&str] = &[
    "signature",
    "rename",
    "export",
    "abi",
    "interface",
    "api",
    "boundary",
    "type",
    "contract",
    "binding",
];
const EXHAUSTIVE_PHRASES: &[&str] = &[
    "all callers",
    "every caller",
    "all usages",
    "every usage",
    "all references",
    "every reference",
    "find usages",
    "who calls",
];
const UNRESOLVED_MARKS: &[&str] = &[
    "unresolved",
    "unknown",
    "undefined",
    "not found",
    "cannot find",
    "undeclared",
    "no such",
];
const STOP: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "not", "are", "was", "keep",
    "make", "use", "when", "then", "than", "its", "but", "all", "any", "each", "error", "failed",
    "failure", "test", "line", "column", "expected", "found",
];

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForeignNeed {
    pub language: String,
    /// `goal`, `path-named` or `manifest-export`.
    pub reason: &'static str,
}

/// What the task needs to know, independent of graph traversal completeness.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvidenceNeeds {
    pub symbols: Vec<String>,
    pub foreign: Vec<ForeignNeed>,
    pub explicit_paths: Vec<String>,
    pub config: Vec<String>,
    pub unresolved: Vec<String>,
    pub exhaustive_references: bool,
}

impl EvidenceNeeds {
    /// A concrete need only the repository provider can answer.
    pub fn needs_provider(&self) -> bool {
        !(self.foreign.is_empty() && self.config.is_empty() && self.unresolved.is_empty())
            || !self.explicit_paths.is_empty()
            || self.exhaustive_references
    }

    /// Compiler-answerable: named `.spx` symbols and nothing foreign.
    pub fn spx_local(&self) -> bool {
        !self.symbols.is_empty() && !self.needs_provider()
    }

    pub fn languages(&self) -> Vec<String> {
        let s: BTreeSet<String> = self.foreign.iter().map(|f| f.language.clone()).collect();
        s.into_iter().collect()
    }

    pub fn to_json(&self) -> Value {
        json!({"symbols": self.symbols,
               "foreign": self.foreign.iter().map(|f| json!({"language": f.language, "reason": f.reason})).collect::<Vec<_>>(),
               "explicit_paths": self.explicit_paths, "config": self.config, "unresolved": self.unresolved,
               "exhaustive_references": self.exhaustive_references, "spx_local": self.spx_local()})
    }
}

/// One bounded provider query.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlanStep {
    /// `initial` or `follow-up`.
    pub label: &'static str,
    pub query: String,
    pub max_items: u64,
    /// Languages the step must surface (empty: any non-`.spx` item).
    pub want_languages: Vec<String>,
    /// Goal-derived terms for the single unmet-need follow-up.
    pub fallback_query: Option<String>,
    pub references: bool,
    pub exhaustive: bool,
    pub symbol: Option<String>,
}

impl PlanStep {
    pub fn query_digest(&self) -> String {
        sha256_plain(self.query.as_bytes())
    }
    pub fn to_json(&self) -> Value {
        // Goal-derived text never enters reports: digest and sizes only.
        json!({"label": self.label, "query_digest": self.query_digest(), "query_terms": self.query.split(' ').count(),
               "max_items": self.max_items, "references": self.references, "exhaustive": self.exhaustive,
               "languages": self.want_languages})
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RetrievalPlan {
    pub needs: EvidenceNeeds,
    /// `None`: native-only (no provider call is planned).
    pub initial: Option<PlanStep>,
}

fn terms(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || matches!(c, '_' | '.' | '-' | '/')))
        .map(|t| t.trim_matches(|c: char| matches!(c, '.' | '-' | '/')))
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

fn idents(text: &str) -> Vec<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|t| {
            t.len() >= 3
                && !t.chars().all(|c| c.is_ascii_digit())
                && !t.starts_with("SPX")
                && !STOP.contains(&t.to_lowercase().as_str())
        })
        .map(str::to_string)
        .collect()
}

fn dedup(v: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    v.into_iter().filter(|x| seen.insert(x.clone())).collect()
}

/// `web_exports` of the project manifest: the `.spx` ids that cross into TypeScript.
fn web_exports(root: &Path) -> Result<Vec<String>, String> {
    let path = root.join("semaprax.toml");
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = std::fs::read_to_string(path).map_err(|_| "manifest unreadable".to_string())?;
    semaprax::project::ProjectManifest::parse(&text)
        .map(|manifest| manifest.web_exports().to_vec())
        .map_err(|_| "manifest exports unknown".to_string())
}

/// Derive the evidence needs of one task and the smallest provider query that
/// answers them. `diagnostics` are `(code, message)` pairs from the compiler.
pub fn plan(
    root: &Path,
    goal: &str,
    seed: Option<&str>,
    diagnostics: &[(String, String)],
) -> RetrievalPlan {
    let decls = Snapshot::capture(root)
        .map(|s| scan_spx(&s))
        .unwrap_or_default();
    let goal_terms = terms(goal);
    let lower: Vec<String> = goal_terms.iter().map(|t| t.to_lowercase()).collect();
    let mut needs = EvidenceNeeds::default();

    let mut symbols: Vec<String> = seed.map(str::to_string).into_iter().collect();
    for t in &goal_terms {
        if let Some(x) = decls.iter().find(|x| x.id == *t || x.name == *t) {
            symbols.push(x.id.clone());
        }
    }
    needs.symbols = dedup(symbols);

    for (term, lang) in LANGUAGE_TERMS {
        if lower.iter().any(|t| t == term) {
            needs.foreign.push(ForeignNeed {
                language: (*lang).into(),
                reason: "goal",
            });
        }
    }
    for t in &goal_terms {
        if let Some((stem, ext)) = t.rsplit_once('.') {
            let lang = match ext {
                "ts" | "tsx" => "typescript",
                "js" | "mjs" => "javascript",
                "rs" => "rust",
                "py" => "python",
                "go" => "go",
                _ => "",
            };
            if !stem.is_empty() && !lang.is_empty() {
                needs.explicit_paths.push(t.clone());
                needs.foreign.push(ForeignNeed {
                    language: lang.into(),
                    reason: "path-named",
                });
            }
        }
    }
    needs.config = dedup(
        lower
            .iter()
            .filter(|t| CONFIG_TERMS.contains(&t.as_str()))
            .cloned()
            .collect(),
    );
    let interface_change = lower.iter().any(|t| INTERFACE_TERMS.contains(&t.as_str()));
    if interface_change || !needs.foreign.is_empty() {
        let exported = match web_exports(root) {
            Ok(exports) => exports,
            Err(unknown) => {
                needs.unresolved.push(unknown);
                vec![]
            }
        };
        if needs.symbols.iter().any(|s| exported.contains(s)) {
            needs.foreign.push(ForeignNeed {
                language: "typescript".into(),
                reason: "manifest-export",
            });
        }
    }
    let phrase = goal.to_lowercase();
    needs.exhaustive_references = EXHAUSTIVE_PHRASES.iter().any(|p| phrase.contains(p));

    // Unresolved references: quoted names in "unresolved"-style diagnostics that
    // no `.spx` declaration owns.
    for (_, msg) in diagnostics {
        let m = msg.to_lowercase();
        if !UNRESOLVED_MARKS.iter().any(|k| m.contains(k)) {
            continue;
        }
        for part in msg.split(['`', '\'', '"']).skip(1).step_by(2) {
            let p = part.trim();
            if !p.is_empty()
                && p.len() <= 128
                && !decls.iter().any(|x| x.id == p || x.name == p)
                && !needs.unresolved.contains(&p.to_string())
            {
                needs.unresolved.push(p.to_string());
            }
        }
    }
    needs.unresolved.truncate(4);
    needs.foreign.dedup();
    needs.explicit_paths = dedup(needs.explicit_paths);

    let initial = needs
        .needs_provider()
        .then(|| initial_step(&needs, &decls, &goal_terms, goal));
    RetrievalPlan { needs, initial }
}

fn initial_step(
    needs: &EvidenceNeeds,
    decls: &[super::identity::SpxDecl],
    goal_terms: &[String],
    goal: &str,
) -> PlanStep {
    let mut q: Vec<String> = Vec::new();
    for s in &needs.symbols {
        q.push(s.clone());
        if let Some(x) = decls.iter().find(|x| x.id == *s) {
            if !x.name.is_empty() {
                q.push(x.name.clone());
            }
        }
    }
    q.extend(needs.unresolved.iter().cloned());
    for p in &needs.explicit_paths {
        let base = p.rsplit('/').next().unwrap_or(p);
        q.push(
            base.rsplit_once('.')
                .map_or(base.to_string(), |(s, _)| s.to_string()),
        );
    }
    q.extend(needs.config.iter().cloned());
    let mut q = dedup(q);
    q.truncate(MAX_TOKENS);
    let fallback = dedup(idents(goal))
        .into_iter()
        .take(MAX_TOKENS)
        .collect::<Vec<_>>();
    let query = if q.is_empty() {
        fallback.join(" ")
    } else {
        q.join(" ")
    };
    let _ = goal_terms;
    let symbol = needs.symbols.first().cloned();
    PlanStep {
        label: "initial",
        query,
        max_items: INITIAL_MAX_ITEMS,
        want_languages: needs.languages(),
        fallback_query: (!fallback.is_empty()).then(|| fallback.join(" ")),
        references: needs.exhaustive_references && symbol.is_some(),
        exhaustive: needs.exhaustive_references,
        symbol,
    }
}

/// The single focused follow-up after a failed candidate: only identifiers the
/// failure names that the prior query did not already ask about. `None` when
/// the failure adds nothing new (then no call is made).
pub fn follow_up_step(prior: &PlanStep, failure: &str) -> Option<PlanStep> {
    let asked: BTreeSet<String> = terms(&prior.query).into_iter().collect();
    let mut fresh: Vec<String> = Vec::new();
    // Quoted names first: they are the diagnostic's own subject.
    for part in failure.split(['`', '\'', '"']).skip(1).step_by(2) {
        let p = part.trim();
        if !p.is_empty() && p.len() <= 128 && !p.contains(' ') && !asked.contains(p) {
            fresh.push(p.to_string());
        }
    }
    for t in idents(failure) {
        if !asked.contains(&t) {
            fresh.push(t);
        }
    }
    let fresh: Vec<String> = dedup(fresh).into_iter().take(MAX_TOKENS).collect();
    if fresh.is_empty() {
        return None;
    }
    Some(PlanStep {
        label: "follow-up",
        query: fresh.join(" "),
        max_items: FOLLOW_UP_MAX_ITEMS,
        want_languages: vec![],
        fallback_query: None,
        references: false,
        exhaustive: false,
        symbol: None,
    })
}

/// A parsed `ctx:<provider>:<path>#<a>-<b>@<digest>` continuation handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handle {
    pub provider: String,
    pub path: String,
    pub start: u64,
    pub end: u64,
    pub digest: String,
}

pub fn parse_handle(h: &str) -> Option<Handle> {
    let rest = h.strip_prefix("ctx:")?;
    let (provider, rest) = rest.split_once(':')?;
    let (rest, digest) = rest.rsplit_once('@')?;
    let (path, span) = rest.rsplit_once('#')?;
    let (a, b) = span.split_once('-')?;
    Some(Handle {
        provider: provider.to_string(),
        path: path.to_string(),
        start: a.parse().ok()?,
        end: b.parse().ok()?,
        digest: digest.to_string(),
    })
}

/// Resolve a continuation handle against the current working tree with no
/// provider call: the slice is read only if it still hashes to the handle's
/// digest (`SPX-HPE071` otherwise) and fits `max_bytes` (`SPX-HPE072`).
pub fn expand_handle(project: &Path, h: &Handle, max_bytes: usize) -> HarnessResult<String> {
    let p = Path::new(&h.path);
    if p.is_absolute() || p.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(d(
            "SPX-HPE073",
            format!("handle path `{}` leaves the project", h.path),
        ));
    }
    let text = std::fs::read_to_string(project.join(p)).map_err(|_| {
        d(
            "SPX-HPE071",
            format!("`{}` is gone or unreadable: the handle is stale", h.path),
        )
    })?;
    match span_digest(&text, h.start, h.end) {
        Some(x) if x == h.digest => {}
        _ => {
            return Err(d(
                "SPX-HPE071",
                format!(
                    "source drift: `{}` lines {}-{} no longer match the handle digest",
                    h.path, h.start, h.end
                ),
            ))
        }
    }
    let body =
        text.split('\n').collect::<Vec<_>>()[(h.start - 1) as usize..h.end as usize].join("\n");
    if body.len() > max_bytes {
        return Err(d(
            "SPX-HPE072",
            format!(
                "slice of {} bytes exceeds the continuation budget of {max_bytes} bytes",
                body.len()
            ),
        ));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_round_trip_and_reject_garbage() {
        let h = parse_handle("ctx:org.example/x:web/app.ts#3-9@sha256:ab").unwrap();
        assert_eq!((h.path.as_str(), h.start, h.end), ("web/app.ts", 3, 9));
        assert!(parse_handle("ctx:nope").is_none());
    }

    #[test]
    fn follow_up_is_none_without_new_identifiers() {
        let p = PlanStep {
            label: "initial",
            query: "foo bar".into(),
            max_items: 1,
            want_languages: vec![],
            fallback_query: None,
            references: false,
            exhaustive: false,
            symbol: None,
        };
        assert!(follow_up_step(&p, "foo bar failed").is_none());
        assert_eq!(
            follow_up_step(&p, "missing `Widget`").unwrap().query,
            "Widget missing"
        );
    }
}
