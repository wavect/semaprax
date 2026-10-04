//! Typed, deterministic stage composition derived from the resolved profile.
//! A stage has exactly one owner; dependencies form a DAG; interception edges
//! between tools must not recurse. Violations are refused before any step runs.

use crate::contract::CapabilityKind;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::profile::{BindingState, Resolution};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// Pipeline stages in their natural order (also the deterministic tiebreak).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StageId {
    Snapshot,
    Diagnose,
    Context,
    Budget,
    Route,
    Generate,
    Validate,
    Check,
    Present,
    Publish,
}

impl StageId {
    pub const ALL: [StageId; 10] = [
        Self::Snapshot,
        Self::Diagnose,
        Self::Context,
        Self::Budget,
        Self::Route,
        Self::Generate,
        Self::Validate,
        Self::Check,
        Self::Present,
        Self::Publish,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::Diagnose => "diagnose",
            Self::Context => "context",
            Self::Budget => "budget",
            Self::Route => "route",
            Self::Generate => "generate",
            Self::Validate => "validate",
            Self::Check => "check",
            Self::Present => "present",
            Self::Publish => "publish",
        }
    }

    /// Fixed dependencies of the default pipeline.
    fn default_after(self) -> Vec<StageId> {
        let i = Self::ALL.iter().position(|s| *s == self).unwrap();
        if i == 0 {
            vec![]
        } else {
            vec![Self::ALL[i - 1]]
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slot {
    pub stage: StageId,
    /// Provider id, `host`, or a builtin id.
    pub owner: String,
    pub after: Vec<StageId>,
}

/// A tool interception: `from` (a tool name) is wrapped by an interceptor that
/// itself runs `to`. A cycle (including `from == to`) is recursion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interception {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Composition {
    pub order: Vec<StageId>,
    pub slots: BTreeMap<StageId, Slot>,
}

fn refuse(code: &'static str, msg: String) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

impl Composition {
    /// Validate `slots` and order them. Every default stage must be present.
    pub fn build(slots: Vec<Slot>, interceptions: &[Interception]) -> HarnessResult<Composition> {
        let mut by: BTreeMap<StageId, Slot> = BTreeMap::new();
        for s in slots {
            if let Some(prev) = by.get(&s.stage) {
                return Err(refuse(
                    "SPX-HPD011",
                    format!(
                        "stage `{}` has two owners (`{}` and `{}`)",
                        s.stage.as_str(),
                        prev.owner,
                        s.owner
                    ),
                ));
            }
            by.insert(s.stage, s);
        }
        for stage in StageId::ALL {
            if !by.contains_key(&stage) {
                return Err(refuse(
                    "SPX-HPD013",
                    format!("stage `{}` has no owner", stage.as_str()),
                ));
            }
        }
        check_interception(interceptions)?;
        // Kahn with the stage enum order as the deterministic tiebreak.
        let mut indeg: BTreeMap<StageId, usize> = by.keys().map(|k| (*k, 0)).collect();
        for s in by.values() {
            for dep in &s.after {
                if !by.contains_key(dep) {
                    return Err(refuse(
                        "SPX-HPD013",
                        format!(
                            "stage `{}` depends on unknown `{}`",
                            s.stage.as_str(),
                            dep.as_str()
                        ),
                    ));
                }
                *indeg.get_mut(&s.stage).unwrap() += 1;
            }
        }
        let mut ready: BTreeSet<StageId> = indeg
            .iter()
            .filter(|(_, d)| **d == 0)
            .map(|(k, _)| *k)
            .collect();
        let mut order = Vec::new();
        while let Some(next) = ready.iter().next().copied() {
            ready.remove(&next);
            order.push(next);
            for s in by.values().filter(|s| s.after.contains(&next)) {
                let d = indeg.get_mut(&s.stage).unwrap();
                *d -= 1;
                if *d == 0 {
                    ready.insert(s.stage);
                }
            }
        }
        if order.len() != by.len() {
            let stuck: Vec<&str> = by
                .keys()
                .filter(|k| !order.contains(k))
                .map(|k| k.as_str())
                .collect();
            return Err(refuse(
                "SPX-HPD010",
                format!("stage dependency cycle among: {}", stuck.join(", ")),
            ));
        }
        Ok(Composition { order, slots: by })
    }

    /// Default composition for a resolved profile. `disabled` forces builtin
    /// owners (the single disable switch). `extra` slots are appended and are
    /// subject to the same uniqueness and cycle checks.
    pub fn from_profile(
        resolution: Option<&Resolution>,
        disabled: bool,
        extra: Vec<Slot>,
        interceptions: &[Interception],
    ) -> HarnessResult<Composition> {
        let owner_of = |kind: CapabilityKind, fallback: &str| -> String {
            match resolution
                .filter(|_| !disabled)
                .and_then(|r| r.profile.binding(kind))
            {
                Some(b)
                    if b.state == BindingState::Selected || b.state == BindingState::Fallback =>
                {
                    if b.provider_id.is_empty() {
                        fallback.into()
                    } else {
                        b.provider_id.clone()
                    }
                }
                _ => fallback.into(),
            }
        };
        let mut slots: Vec<Slot> = StageId::ALL
            .iter()
            .map(|s| Slot {
                stage: *s,
                owner: match s {
                    StageId::Context => {
                        owner_of(CapabilityKind::ContextRepository, "semaprax/native-context")
                    }
                    StageId::Route => {
                        owner_of(CapabilityKind::DecisionEvaluate, "semaprax/rules-decision")
                    }
                    StageId::Generate => {
                        owner_of(CapabilityKind::ModelGenerate, "semaprax/scripted-proposer")
                    }
                    _ => "host".into(),
                },
                after: s.default_after(),
            })
            .collect();
        slots.extend(extra);
        Composition::build(slots, interceptions)
    }

    pub fn to_json(&self) -> Value {
        json!(self.order.iter().map(|s| {
            let slot = &self.slots[s];
            json!({"stage": s.as_str(), "owner": slot.owner, "after": slot.after.iter().map(|a| a.as_str()).collect::<Vec<_>>()})
        }).collect::<Vec<_>>())
    }
}

fn check_interception(edges: &[Interception]) -> HarnessResult<()> {
    let mut graph: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in edges {
        graph
            .entry(e.from.as_str())
            .or_default()
            .push(e.to.as_str());
    }
    fn visit<'a>(
        n: &'a str,
        g: &BTreeMap<&'a str, Vec<&'a str>>,
        path: &mut Vec<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Option<Vec<String>> {
        if let Some(i) = path.iter().position(|p| *p == n) {
            return Some(
                path[i..]
                    .iter()
                    .chain(std::iter::once(&n))
                    .map(|s| s.to_string())
                    .collect(),
            );
        }
        if done.contains(n) {
            return None;
        }
        path.push(n);
        for next in g.get(n).into_iter().flatten() {
            if let Some(c) = visit(next, g, path, done) {
                return Some(c);
            }
        }
        path.pop();
        done.insert(n);
        None
    }
    let mut done = BTreeSet::new();
    for start in graph.keys().copied().collect::<Vec<_>>() {
        if let Some(cycle) = visit(start, &graph, &mut Vec::new(), &mut done) {
            return Err(refuse(
                "SPX-HPD012",
                format!("recursive tool interception: {}", cycle.join(" -> ")),
            ));
        }
    }
    Ok(())
}
