//! MR-13: matched routing evidence across generation profiles, learned
//! decision adapters and both execution domains.
//!
//! The matrix is discovered, never listed: arms come from the approved
//! adapter/profile registry (`registry`) and their declared capabilities. Every
//! arm routes the same sealed items through the one routing engine (`decide`);
//! the chosen model's outcome comes from an executor (`exec`) and is shared by
//! every arm that chose it, so arms are matched by construction. Labels come
//! only from independent verifiers named by the task set. Qualification is the
//! HN-16 registry and gate (`decision::qualify::evaluate_domain`); no second
//! registry exists here. See `docs/HARNESS-DECISION-V1.md` (MR-13).

pub mod cli;
pub mod exec;
pub mod registry;
pub mod report;
pub mod run;

use crate::decision::evidence::MatchedBudget;
use crate::decision::route_v2::ExecutionDomain;
use crate::diag::HarnessDiagnostic;
use crate::json;
use crate::receipt::PriceBook;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const TASKS_SCHEMA: &str = "semaprax.harness-routing-tasks.v1";
pub const MANIFEST_SCHEMA: &str = "semaprax.harness-routing-matrix.v1";

pub(crate) fn q(code: &'static str, m: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, m)
}

/// Closed member check shared by every MR-13 input file (`SPX-HPQ003`).
pub(crate) fn closed<'a>(
    v: &'a Value,
    what: &str,
    required: &[&str],
    optional: &[&str],
) -> Result<&'a Map<String, Value>, HarnessDiagnostic> {
    let m = v
        .as_object()
        .ok_or_else(|| q("SPX-HPQ002", format!("{what} must be an object")))?;
    if let Some(k) = m
        .keys()
        .find(|k| !required.contains(&k.as_str()) && !optional.contains(&k.as_str()))
    {
        return Err(q("SPX-HPQ003", format!("{what}: unknown member `{k}`")));
    }
    if let Some(k) = required.iter().find(|k| !m.contains_key(**k)) {
        return Err(q("SPX-HPQ002", format!("{what}: missing `{k}`")));
    }
    Ok(m)
}

pub(crate) fn text(
    m: &Map<String, Value>,
    k: &str,
    what: &str,
) -> Result<String, HarnessDiagnostic> {
    m.get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 256)
        .map(str::to_string)
        .ok_or_else(|| {
            q(
                "SPX-HPQ002",
                format!("{what}: `{k}` must be a short non-empty string"),
            )
        })
}

pub(crate) fn domain_of(s: &str) -> Result<ExecutionDomain, HarnessDiagnostic> {
    ExecutionDomain::parse(s)
        .ok_or_else(|| q("SPX-HPQ002", format!("unknown execution domain `{s}`")))
}

pub(crate) fn domains(
    v: Option<&Value>,
    what: &str,
) -> Result<BTreeSet<ExecutionDomain>, HarnessDiagnostic> {
    let a = v
        .and_then(Value::as_array)
        .filter(|a| !a.is_empty())
        .ok_or_else(|| {
            q(
                "SPX-HPQ002",
                format!("{what}: `domains` must be a non-empty array"),
            )
        })?;
    a.iter()
        .map(|d| domain_of(d.as_str().unwrap_or("")))
        .collect()
}

macro_rules! closed_enum {
    ($name:ident { $($v:ident = $s:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub enum $name { $($v),+ }
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$v),+];
            pub fn as_str(self) -> &'static str { match self { $($name::$v => $s),+ } }
            pub fn parse(s: &str) -> Option<Self> { Self::ALL.iter().copied().find(|x| x.as_str() == s) }
        }
    };
}

closed_enum!(Stratum {
    Mechanical = "mechanical",
    Tests = "tests",
    LocalizedDebug = "localized_debug",
    HardSemantic = "hard_semantic",
    RuntimeClassification = "runtime_classification",
    MultiTurnRecovery = "multi_turn_recovery",
    AgentToolSelection = "agent_tool_selection",
});

// `calibration`: permitted training/calibration data; `eval`: sealed held-out items.
closed_enum!(Split {
    Calibration = "calibration",
    Eval = "eval",
});

/// An independent verifier (compiler, tests, acceptance, typed outcome
/// assertion or policy/side-effect invariant) at an exact revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verifier {
    pub id: String,
    pub kind: String,
    pub revision: String,
}

impl Verifier {
    /// The `verified_by` an outcome carries: `<kind>:<id>@<revision>`.
    pub fn label(&self) -> String {
        format!("{}:{}@{}", self.kind, self.id, self.revision)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub domain: ExecutionDomain,
    /// Repository/project (development) or application/customer/task
    /// distribution (application).
    pub partition: String,
    pub stratum: Stratum,
    pub split: Split,
    /// `model-route/v1` features.
    pub features: Value,
    pub verifier: String,
    /// Digest of the item's content; equal content in two splits is leakage.
    pub content_digest: String,
}

#[derive(Clone, Debug)]
pub struct TaskSet {
    pub digest: String,
    pub task: String,
    pub catalog: Value,
    pub budget: Value,
    pub prices: PriceBook,
    pub matched: MatchedBudget,
    pub verifiers: BTreeMap<String, Verifier>,
    pub candidate_revision: String,
    pub renderer_revision: String,
    pub items: Vec<Item>,
}

impl TaskSet {
    pub fn from_json(v: &Value) -> Result<Self, HarnessDiagnostic> {
        let w = "routing tasks";
        let m = closed(
            v,
            w,
            &[
                "schema",
                "task",
                "catalog",
                "budget",
                "price_book",
                "matched_budget",
                "verifiers",
                "candidate_revision",
                "renderer_revision",
                "items",
            ],
            &["description"],
        )?;
        if m["schema"] != TASKS_SCHEMA {
            return Err(q(
                "SPX-HPQ002",
                format!("{w}: schema must be `{TASKS_SCHEMA}`"),
            ));
        }
        let prices = PriceBook::from_json(&m["price_book"])
            .map_err(|e| q("SPX-HPQ002", format!("{w}: price_book: {e}")))?;
        let mb = closed(
            &m["matched_budget"],
            "matched_budget",
            &["max_cost_micros", "max_attempts"],
            &[],
        )?;
        let n = |k: &str| {
            mb[k].as_u64().filter(|x| *x > 0).ok_or_else(|| {
                q(
                    "SPX-HPQ002",
                    format!("matched_budget `{k}` must be a positive integer"),
                )
            })
        };
        let matched = MatchedBudget {
            max_cost_micros: n("max_cost_micros")?,
            max_attempts: n("max_attempts")? as u32,
        };
        let mut verifiers = BTreeMap::new();
        for x in m["verifiers"].as_array().into_iter().flatten() {
            let vm = closed(x, "verifier", &["id", "kind", "revision"], &[])?;
            let ver = Verifier {
                id: text(vm, "id", "verifier")?,
                kind: text(vm, "kind", "verifier")?,
                revision: text(vm, "revision", "verifier")?,
            };
            if verifiers.insert(ver.id.clone(), ver).is_some() {
                return Err(q("SPX-HPQ005", "duplicate verifier id"));
            }
        }
        let mut items = Vec::new();
        let mut ids = BTreeSet::new();
        for x in m["items"].as_array().into_iter().flatten() {
            let im = closed(
                x,
                "item",
                &[
                    "id",
                    "domain",
                    "partition",
                    "stratum",
                    "split",
                    "features",
                    "verifier",
                    "content_digest",
                ],
                &[],
            )?;
            let id = text(im, "id", "item")?;
            if !ids.insert(id.clone()) {
                return Err(q("SPX-HPQ005", format!("duplicate item id `{id}`")));
            }
            let verifier = text(im, "verifier", "item")?;
            if !verifiers.contains_key(&verifier) {
                return Err(q(
                    "SPX-HPQ006",
                    format!("item `{id}`: unknown verifier `{verifier}`"),
                ));
            }
            let stratum = Stratum::parse(im["stratum"].as_str().unwrap_or(""))
                .ok_or_else(|| q("SPX-HPQ002", format!("item `{id}`: unknown stratum")))?;
            let split = Split::parse(im["split"].as_str().unwrap_or("")).ok_or_else(|| {
                q(
                    "SPX-HPQ002",
                    format!("item `{id}`: split must be calibration|eval"),
                )
            })?;
            items.push(Item {
                domain: domain_of(im["domain"].as_str().unwrap_or(""))?,
                partition: text(im, "partition", "item")?,
                stratum,
                split,
                features: im["features"].clone(),
                verifier,
                content_digest: text(im, "content_digest", "item")?,
                id,
            });
        }
        if items.is_empty() {
            return Err(q("SPX-HPQ002", format!("{w}: no items")));
        }
        Ok(Self {
            digest: json::digest("semaprax.harness-routing-tasks.v1", v),
            task: text(m, "task", w)?,
            catalog: m["catalog"].clone(),
            budget: m["budget"].clone(),
            prices,
            matched,
            verifiers,
            candidate_revision: text(m, "candidate_revision", w)?,
            renderer_revision: text(m, "renderer_revision", w)?,
            items,
        })
    }

    /// The `model-route/v1` request document of one item.
    pub fn route_doc(&self, item: &Item) -> Value {
        json!({"task": self.task, "features": item.features, "budget": self.budget,
               "catalog": self.catalog})
    }

    /// Digest of the sealed evaluation items (ids and content), computed
    /// before any calibration or execution.
    pub fn seal_digest(&self) -> String {
        let eval: Vec<Value> = self
            .items
            .iter()
            .filter(|i| i.split == Split::Eval)
            .map(|i| json!([i.id, i.domain.as_str(), i.content_digest]))
            .collect();
        json::digest("semaprax.harness-routing-seal.v1", &json!(eval))
    }

    pub fn verifier_label(&self, item: &Item) -> String {
        self.verifiers[&item.verifier].label()
    }
}
