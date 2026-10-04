//! TC-05: a small initial context target and provenance-safe span
//! deduplication. Pure host-side policy over [`ContextItem`]s; opt-in until the
//! TC-12 qualification (nothing in the default pipeline calls it).
//!
//! Three distinct limits are kept apart: the model's *hard capacity* (enforced
//! by `RequestBudget::fit`), the host *safety bound* (`context_max_bytes`) and
//! the configurable *initial target*. Compiler-verified items and items that
//! hold a required reference are protected: they are retained whole or the
//! selection is refused, never truncated. Optional items are ranked
//! deterministically. Chosen/omitted identities and reasons go to the report,
//! never into the model prompt.

use super::broker_stage::COMPILER_VERIFIED;
use super::budget::RequestBudget;
use super::stages::ContextItem;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// What one cost figure means. Named tokenizer counts and the byte policy are
/// never interchangeable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CostUnit {
    Tokens {
        tokenizer: String,
    },
    /// Explicit policy: UTF-8 bytes, an upper bound, not tokens.
    Bytes,
}

impl CostUnit {
    pub fn label(&self) -> String {
        match self {
            CostUnit::Tokens { tokenizer } => format!("tokens:{tokenizer}"),
            CostUnit::Bytes => "bytes:byte-policy-upper-bound".into(),
        }
    }
}

/// Measures item cost in one unit.
pub struct CostMeter<'a> {
    unit: CostUnit,
    source: Option<(&'a RequestBudget<'a>, String)>,
}

impl<'a> CostMeter<'a> {
    pub fn bytes() -> Self {
        Self {
            unit: CostUnit::Bytes,
            source: None,
        }
    }
    /// Named tokens when the model's tokenizer is mapped and provisioned,
    /// else the labelled byte policy.
    pub fn for_model(budget: &'a RequestBudget<'a>, model_id: &str) -> Self {
        match budget.count(model_id, "").tokenizer {
            Some((name, _)) => Self {
                unit: CostUnit::Tokens { tokenizer: name },
                source: Some((budget, model_id.to_string())),
            },
            None => Self::bytes(),
        }
    }
    pub fn unit(&self) -> &CostUnit {
        &self.unit
    }
    pub fn cost_text(&self, text: &str) -> u64 {
        match &self.source {
            Some((b, m)) => b.count(m, text).tokens.unwrap_or(text.len() as u64),
            None => text.len() as u64,
        }
    }
    pub fn cost(&self, it: &ContextItem) -> u64 {
        self.cost_text(&format!("{}\n{}\n{}", it.label, it.provenance, it.text))
    }
}

/// Why a target was raised. Both are bound to a concrete named fact.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Trigger {
    MissingDependency(String),
    ValidationFailure(String),
}

/// Configurable target with a bounded, justified escalation path. Units are
/// those of the [`CostMeter`] used for selection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContextTarget {
    /// Model hard capacity for context material (never exceeded).
    pub hard_capacity: Option<u64>,
    /// Host safety bound in bytes (`context_max_bytes`).
    pub safety_bound_bytes: usize,
    pub initial: u64,
    pub step: u64,
    pub max_escalations: u32,
    current: u64,
    escalations: Vec<(u64, String)>,
}

impl ContextTarget {
    pub fn new(
        hard_capacity: Option<u64>,
        safety_bound_bytes: usize,
        initial: u64,
        step: u64,
        max_escalations: u32,
    ) -> Self {
        Self {
            hard_capacity,
            safety_bound_bytes,
            initial,
            step,
            max_escalations,
            current: initial,
            escalations: Vec::new(),
        }
    }
    /// Current target, clamped to the hard capacity.
    pub fn current(&self) -> u64 {
        self.hard_capacity
            .map_or(self.current, |h| self.current.min(h))
    }
    /// Raise the target for a known missing dependency or validation failure.
    /// Refused (target unchanged) when the trigger names nothing, the bound of
    /// escalations is spent, the ceiling is reached, or the extra cost does
    /// not fit the remaining task budget.
    pub fn escalate(
        &mut self,
        trigger: &Trigger,
        remaining_task_budget: Option<u64>,
    ) -> Result<u64, String> {
        let (kind, named) = match trigger {
            Trigger::MissingDependency(n) => ("missing-dependency", n),
            Trigger::ValidationFailure(n) => ("validation-failure", n),
        };
        if named.trim().is_empty() {
            return Err("escalation needs a named missing dependency or failure".into());
        }
        if self.escalations.len() as u32 >= self.max_escalations {
            return Err(format!("escalation bound {} spent", self.max_escalations));
        }
        let ceiling = self.hard_capacity.unwrap_or(u64::MAX);
        let next = self.current.saturating_add(self.step).min(ceiling);
        if next <= self.current() {
            return Err("target already at the hard capacity".into());
        }
        let extra = next - self.current();
        if remaining_task_budget.is_some_and(|r| extra > r) {
            return Err("remaining task budget cannot pay for a larger target".into());
        }
        self.current = next;
        self.escalations.push((next, format!("{kind}:{named}")));
        Ok(next)
    }
    pub fn escalation_log(&self) -> &[(u64, String)] {
        &self.escalations
    }
}

/// Identifiers (>= 3 chars, alnum/underscore) from task or diagnostic text.
pub fn identifiers(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !(c.is_alphanumeric() || c == '_'))
        .filter(|w| w.chars().count() >= 3)
        .map(str::to_string)
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chosen {
    pub label: String,
    pub provenance: String,
    pub cost: u64,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub items: Vec<ContextItem>,
    pub chosen: Vec<Chosen>,
    pub omitted: Vec<Chosen>,
    pub unit: CostUnit,
    pub target: u64,
    pub used: u64,
}

impl Selection {
    /// Report-only audit; the model prompt never sees this.
    pub fn report_json(&self) -> Value {
        let row = |c: &Chosen| json!({"label": c.label, "provenance": c.provenance, "cost": c.cost, "reason": c.reason});
        json!({"unit": self.unit.label(), "target": self.target, "used": self.used,
               "chosen": self.chosen.iter().map(row).collect::<Vec<_>>(),
               "omitted": self.omitted.iter().map(row).collect::<Vec<_>>(),
               "exhaustive": self.omitted.is_empty()})
    }
}

fn refuse(msg: String) -> HarnessDiagnostic {
    HarnessDiagnostic::new("SPX-HPD020", msg)
}

/// Select context under the target. `task_idents` are identifiers from the
/// goal and diagnostics; `required_refs` must all stay represented.
pub fn select(
    items: &[ContextItem],
    task_idents: &BTreeSet<String>,
    required_refs: &BTreeSet<String>,
    target: &ContextTarget,
    meter: &CostMeter,
) -> HarnessResult<Selection> {
    let hits = |it: &ContextItem| {
        let hay = format!("{}\n{}", it.label, it.text);
        identifiers(&hay).intersection(task_idents).count()
    };
    let is_required = |it: &ContextItem| {
        let hay = format!("{}\n{}", it.label, it.text);
        required_refs.iter().any(|r| hay.contains(r.as_str()))
    };
    let mut chosen_idx: Vec<usize> = Vec::new();
    let mut reasons: Vec<Option<String>> = vec![None; items.len()];
    let (mut used, mut used_bytes) = (0u64, 0usize);
    for (i, it) in items.iter().enumerate() {
        let why = if it.provenance == COMPILER_VERIFIED {
            "protected: compiler-verified"
        } else if is_required(it) {
            "protected: required reference"
        } else {
            continue;
        };
        used += meter.cost(it);
        used_bytes += it.bytes();
        chosen_idx.push(i);
        reasons[i] = Some(why.into());
    }
    if used_bytes > target.safety_bound_bytes {
        return Err(refuse(format!(
            "protected context ({used_bytes} bytes) exceeds the host safety bound of {} bytes",
            target.safety_bound_bytes
        )));
    }
    if target.hard_capacity.is_some_and(|h| used > h) {
        return Err(refuse(format!(
            "protected context ({used} {}) exceeds the model hard capacity",
            meter.unit().label()
        )));
    }
    let limit = target.current();
    let mut seen: BTreeSet<String> = chosen_idx
        .iter()
        .flat_map(|&i| identifiers(&items[i].text))
        .collect();
    let mut remaining: Vec<usize> = (0..items.len())
        .filter(|i| !chosen_idx.contains(i))
        .collect();
    let mut omitted_why: Vec<(usize, String)> = Vec::new();
    while !remaining.is_empty() {
        // (hits desc, novelty desc, cost asc, input order asc)
        let best = remaining
            .iter()
            .copied()
            .map(|i| {
                let novelty = identifiers(&items[i].text).difference(&seen).count();
                (
                    std::cmp::Reverse(hits(&items[i])),
                    std::cmp::Reverse(novelty),
                    meter.cost(&items[i]),
                    i,
                )
            })
            .min()
            .expect("non-empty");
        let i = best.3;
        remaining.retain(|&x| x != i);
        let c = best.2;
        if used + c <= limit && used_bytes + items[i].bytes() <= target.safety_bound_bytes {
            used += c;
            used_bytes += items[i].bytes();
            seen.extend(identifiers(&items[i].text));
            chosen_idx.push(i);
            reasons[i] = Some(format!("ranked: {} task identifier hit(s)", (best.0).0));
        } else {
            omitted_why.push((i, format!("over target {limit} {}", meter.unit().label())));
        }
    }
    chosen_idx.sort_unstable();
    let mk = |i: usize, why: String| Chosen {
        label: items[i].label.clone(),
        provenance: items[i].provenance.clone(),
        cost: meter.cost(&items[i]),
        reason: why,
    };
    omitted_why.sort();
    Ok(Selection {
        items: chosen_idx.iter().map(|&i| items[i].clone()).collect(),
        chosen: chosen_idx
            .iter()
            .map(|&i| mk(i, reasons[i].clone().unwrap_or_default()))
            .collect(),
        omitted: omitted_why.into_iter().map(|(i, w)| mk(i, w)).collect(),
        unit: meter.unit().clone(),
        target: limit,
        used,
    })
}

// ---- span deduplication ------------------------------------------------

/// A source span with the revision it was read at.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanEntry {
    pub item: ContextItem,
    pub revision: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvenanceRecord {
    pub label: String,
    pub provenance: String,
    pub revision: String,
    /// Original labels folded into this item (own label included).
    pub sources: Vec<String>,
    /// Overlapping labels whose text disagreed and were kept distinct.
    pub conflicts: Vec<String>,
}

fn parse_label(label: &str) -> Option<(&str, usize, usize)> {
    let (path, span) = label.rsplit_once(':')?;
    let (a, b) = span.split_once('-')?;
    let (a, b) = (a.parse().ok()?, b.parse().ok()?);
    (a >= 1 && b >= a).then_some((path, a, b))
}

/// Line-aligned span text only: text whose line count equals the span. Items
/// with appended edges or structural placeholders are opaque (exact duplicates
/// only).
fn lines_of(it: &ContextItem) -> Option<(&str, usize, usize, Vec<&str>)> {
    let (p, a, b) = parse_label(&it.label)?;
    let lines: Vec<&str> = it.text.split('\n').collect();
    (lines.len() == b - a + 1 && !it.text.contains("\nedge ")).then_some((p, a, b, lines))
}

enum Rel {
    Disjoint,
    Merged(ContextItem),
    Conflict,
}

fn relate(x: &ContextItem, y: &ContextItem) -> Rel {
    let (Some((px, xa, xb, xl)), Some((py, ya, yb, yl))) = (lines_of(x), lines_of(y)) else {
        return if x.label == y.label && x.text == y.text {
            Rel::Merged(x.clone())
        } else {
            Rel::Disjoint
        };
    };
    if px != py || xb < ya || yb < xa {
        return Rel::Disjoint;
    }
    let (lo, hi) = (xa.max(ya), xb.min(yb));
    if (lo..=hi).any(|n| xl[n - xa] != yl[n - ya]) {
        return Rel::Conflict;
    }
    let (start, end) = (xa.min(ya), xb.max(yb));
    let text = (start..=end)
        .map(|n| {
            if n >= xa && n <= xb {
                xl[n - xa]
            } else {
                yl[n - ya]
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Rel::Merged(ContextItem {
        label: format!("{px}:{start}-{end}"),
        provenance: x.provenance.clone(),
        text,
    })
}

/// Merge overlapping and duplicate spans that share revision, path and
/// provenance (the authorization boundary). Different revisions, different
/// provenance (so external text is never folded into compiler evidence) and
/// disagreeing overlaps stay separate.
pub fn dedup_spans(entries: Vec<SpanEntry>) -> (Vec<ContextItem>, Vec<ProvenanceRecord>) {
    let mut out: Vec<(SpanEntry, ProvenanceRecord)> = Vec::new();
    for e in entries {
        let mut cur = (
            ProvenanceRecord {
                label: e.item.label.clone(),
                provenance: e.item.provenance.clone(),
                revision: e.revision.clone(),
                sources: vec![e.item.label.clone()],
                conflicts: vec![],
            },
            e,
        );
        loop {
            let mut merged_at = None;
            for (j, (o, rec)) in out.iter_mut().enumerate() {
                if o.revision != cur.1.revision || o.item.provenance != cur.1.item.provenance {
                    continue;
                }
                match relate(&o.item, &cur.1.item) {
                    Rel::Disjoint => {}
                    Rel::Conflict => {
                        if !rec.conflicts.contains(&cur.1.item.label) {
                            rec.conflicts.push(cur.1.item.label.clone());
                        }
                    }
                    Rel::Merged(m) => {
                        merged_at = Some((j, m));
                        break;
                    }
                }
            }
            let Some((j, m)) = merged_at else { break };
            let (mut o, mut rec) = out.remove(j);
            for s in &cur.0.sources {
                if !rec.sources.contains(s) {
                    rec.sources.push(s.clone());
                }
            }
            for c in &cur.0.conflicts {
                if !rec.conflicts.contains(c) {
                    rec.conflicts.push(c.clone());
                }
            }
            o.item = m;
            rec.label = o.item.label.clone();
            cur = (rec, o);
        }
        out.push((cur.1, cur.0));
    }
    (
        out.iter().map(|(e, _)| e.item.clone()).collect(),
        out.into_iter().map(|(_, r)| r).collect(),
    )
}

// ---- whole-task cost ---------------------------------------------------

/// Opt-in wiring config (`[budget] context_target_bytes`), byte policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetConfig {
    pub initial_bytes: u64,
    pub max_escalations: u32,
}

impl TargetConfig {
    /// Target state for this run, restored from the report's `target` block so
    /// no extra run state is carried.
    pub fn target(&self, safety_bound_bytes: usize, ctx: &Value) -> ContextTarget {
        let mut t = ContextTarget::new(
            None,
            safety_bound_bytes,
            self.initial_bytes,
            self.initial_bytes.max(1),
            self.max_escalations,
        );
        if let Some(cur) = ctx["target"]["current"].as_u64() {
            t.current = cur;
        }
        for e in ctx["target"]["escalations"]
            .as_array()
            .into_iter()
            .flatten()
        {
            t.escalations.push((
                e["to"].as_u64().unwrap_or(0),
                e["why"].as_str().unwrap_or("").to_string(),
            ));
        }
        t
    }
}

/// Initial selection for the pipeline: dedup, then select under the target.
/// Returns the kept items, omitted count and the report block.
pub fn targeted(
    items: Vec<ContextItem>,
    revision: &str,
    task_text: &str,
    target: &ContextTarget,
) -> HarnessResult<(Vec<ContextItem>, usize, Value)> {
    let n_in = items.len();
    let (deduped, map) = dedup_spans(
        items
            .into_iter()
            .map(|item| SpanEntry {
                item,
                revision: revision.into(),
            })
            .collect(),
    );
    let sel = select(
        &deduped,
        &identifiers(task_text),
        &BTreeSet::new(),
        target,
        &CostMeter::bytes(),
    )?;
    let merged: Vec<Value> = map
        .iter()
        .filter(|m| m.sources.len() > 1 || !m.conflicts.is_empty())
        .map(|m| {
            json!({"label": m.label, "provenance": m.provenance, "revision": m.revision,
                        "sources": m.sources, "conflicts": m.conflicts})
        })
        .collect();
    let mut rep = sel.report_json();
    rep["current"] = json!(target.current());
    rep["escalations"] = json!(target
        .escalation_log()
        .iter()
        .map(|(to, why)| json!({"to": to, "why": why}))
        .collect::<Vec<_>>());
    rep["input_items"] = json!(n_in);
    rep["provenance_map"] = json!(merged);
    Ok((sel.items.clone(), sel.omitted.len(), rep))
}

/// Follow-up/handle merge under the (possibly escalated) target. Escalates
/// once per call on a validation failure when the bound allows, records the
/// outcome in `ctx["target"]`, and returns the number of new items dropped.
pub fn merge_escalating(
    cfg: &TargetConfig,
    safety_bound_bytes: usize,
    ctx: &mut Value,
    revision: &str,
    failure: &str,
    kept: &mut Vec<ContextItem>,
    items: Vec<ContextItem>,
) -> usize {
    let mut t = cfg.target(safety_bound_bytes, ctx);
    let named: String = failure
        .lines()
        .next()
        .unwrap_or("")
        .chars()
        .take(80)
        .collect();
    if let Err(why) = t.escalate(&Trigger::ValidationFailure(named), None) {
        ctx["target"]["escalation_refused"] = json!(why);
    }
    let limit = (t.current() as usize).min(safety_bound_bytes);
    ctx["target"]["current"] = json!(t.current());
    ctx["target"]["escalations"] = json!(t
        .escalation_log()
        .iter()
        .map(|(to, why)| json!({"to": to, "why": why}))
        .collect::<Vec<_>>());
    let before = kept.len();
    let n_new = items.len();
    let (all, _) = dedup_spans(
        kept.drain(..)
            .chain(items)
            .map(|item| SpanEntry {
                item,
                revision: revision.into(),
            })
            .collect(),
    );
    let mut used = 0usize;
    let mut dropped = 0usize;
    for (i, it) in all.into_iter().enumerate() {
        // Earlier (already kept) material and compiler facts are never dropped.
        if i < before || it.provenance == COMPILER_VERIFIED || used + it.bytes() <= limit {
            used += it.bytes();
            kept.push(it);
        } else {
            dropped += 1;
        }
    }
    dropped.min(n_new)
}

/// Model-visible input and output cost of one attempt, in one unit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttemptCost {
    pub input: u64,
    pub output: u64,
}

/// Total over every attempt, extra retrieval/generation attempts included.
pub fn task_cost(attempts: &[AttemptCost]) -> u64 {
    attempts.iter().map(|a| a.input + a.output).sum()
}

/// A smaller target is kept only when its complete task cost (all attempts)
/// beats the baseline's. Costs in different units are refused, not compared.
pub fn smaller_target_pays(
    small: (&CostUnit, &[AttemptCost]),
    baseline: (&CostUnit, &[AttemptCost]),
) -> Result<bool, String> {
    if small.0 != baseline.0 {
        return Err(format!(
            "cost units differ: {} vs {}",
            small.0.label(),
            baseline.0.label()
        ));
    }
    Ok(task_cost(small.1) < task_cost(baseline.1))
}

#[cfg(test)]
mod tests;
