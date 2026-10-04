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
    /// A caller-supplied named-token counter (a measurement the caller owns).
    counter: Option<Box<dyn Fn(&str) -> u64 + 'a>>,
}

impl<'a> CostMeter<'a> {
    pub fn bytes() -> Self {
        Self {
            unit: CostUnit::Bytes,
            source: None,
            counter: None,
        }
    }
    /// Named tokens counted by `f` (for a host that holds its own tokenizer).
    pub fn with_counter(tokenizer: &str, f: Box<dyn Fn(&str) -> u64 + 'a>) -> Self {
        Self {
            unit: CostUnit::Tokens {
                tokenizer: tokenizer.into(),
            },
            source: None,
            counter: Some(f),
        }
    }
    /// Named tokens when the model's tokenizer is mapped and provisioned,
    /// else the labelled byte policy.
    pub fn for_model(budget: &'a RequestBudget<'a>, model_id: &str) -> Self {
        match budget.count(model_id, "").tokenizer {
            Some((name, _)) => Self {
                unit: CostUnit::Tokens { tokenizer: name },
                source: Some((budget, model_id.to_string())),
                counter: None,
            },
            None => Self::bytes(),
        }
    }
    pub fn unit(&self) -> &CostUnit {
        &self.unit
    }
    pub fn cost_text(&self, text: &str) -> u64 {
        if let Some(f) = &self.counter {
            return f(text);
        }
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

/// A packet marks a protected fact or contract name as required by prefixing
/// the item label with this.
pub const REQUIRED_PREFIX: &str = "required:";

/// Required references actually known: the seed, acceptance stable ids,
/// backtick-quoted identifiers in the goal and diagnostics, and diagnostic
/// paths. Nothing is guessed from free prose.
pub fn required_refs(
    goal: &str,
    seed: Option<&str>,
    acceptance_ids: &[String],
    diagnostics: &[(String, String, Option<String>)],
) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut add = |s: &str| {
        if s.chars().count() >= 3 {
            out.insert(s.to_string());
        }
    };
    seed.iter().for_each(|x| add(x));
    acceptance_ids.iter().for_each(|x| add(x));
    let quoted = |t: &str| -> Vec<String> {
        t.split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_string)
            .collect()
    };
    quoted(goal).iter().for_each(|x| add(x));
    for (_, msg, path) in diagnostics {
        quoted(msg).iter().for_each(|x| add(x));
        if let Some(p) = path {
            add(p);
        }
    }
    out
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
        } else if it.label.starts_with(REQUIRED_PREFIX) {
            "protected: packet-marked required"
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

/// Opt-in wiring config (`[budget] context_target_bytes`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetConfig {
    /// In the selection unit: named-tokenizer tokens when the selected model
    /// has one, else the labelled byte policy (the report states which).
    pub initial: u64,
    pub max_escalations: u32,
}

impl TargetConfig {
    /// Target state for this run, restored from the report's `target` block so
    /// no extra run state is carried.
    pub fn target(
        &self,
        safety_bound_bytes: usize,
        ctx: &Value,
        hard_capacity: Option<u64>,
    ) -> ContextTarget {
        let mut t = ContextTarget::new(
            hard_capacity,
            safety_bound_bytes,
            self.initial,
            self.initial.max(1),
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
    required: &BTreeSet<String>,
    target: &ContextTarget,
    meter: &CostMeter,
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
    let sel = select(&deduped, &identifiers(task_text), required, target, meter)?;
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
    rep["required_refs"] = json!(required);
    rep["provenance_map"] = json!(merged);
    Ok((sel.items.clone(), sel.omitted.len(), rep))
}

/// Meter for the run: the first planned model's named tokenizer when mapped and
/// provisioned (with that model's capacity as the hard bound), else the labelled
/// byte policy.
pub fn meter_for<'a>(
    rb: &'a RequestBudget<'a>,
    plans: Option<&[crate::decision::ModelPlan]>,
) -> (CostMeter<'a>, Option<u64>) {
    match plans.and_then(|p| p.first()) {
        Some(m) => {
            let meter = CostMeter::for_model(rb, &m.id);
            let hard = matches!(meter.unit(), CostUnit::Tokens { .. }).then_some(m.max_context);
            (meter, hard)
        }
        None => (CostMeter::bytes(), None),
    }
}

/// The task's model catalog as the proposal stage sees it (explicit models,
/// else the machine-local binding); `None` when neither is known.
fn plans_of(cfg: &super::pipeline::RunConfig) -> Option<Vec<crate::decision::ModelPlan>> {
    match &cfg.task.models {
        Some(m) => crate::decision::RouteRequest::catalog_from_json(m).ok(),
        None => cfg.model_plans.clone(),
    }
}

/// Pipeline entry: initial selection under the run's target and meter.
pub(super) fn select_for_run(
    cfg: &super::pipeline::RunConfig,
    tc: &TargetConfig,
    ctx: &Value,
    items: Vec<ContextItem>,
    diagnostics: &[super::compiler::CompilerDiagnostic],
) -> HarnessResult<(Vec<ContextItem>, usize, Value)> {
    let rb = cfg.budget.for_task(&cfg.task);
    let (meter, hard) = meter_for(&rb, plans_of(cfg).as_deref());
    let t = tc.target(cfg.context_max_bytes, ctx, hard);
    let diags: Vec<(String, String, Option<String>)> = diagnostics
        .iter()
        .map(|x| (x.code.clone(), x.message.clone(), x.path.clone()))
        .collect();
    let ids: Vec<String> = cfg
        .task
        .acceptance
        .iter()
        .filter_map(|a| a["stable_id"].as_str().map(str::to_string))
        .collect();
    let required = required_refs(&cfg.task.goal, cfg.task.seed.as_deref(), &ids, &diags);
    let text = format!(
        "{} {}",
        cfg.task.goal,
        diags
            .iter()
            .map(|(c, m, _)| format!("{c} {m}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    targeted(items, &cfg.snapshot.revision, &text, &required, &t, &meter)
}

/// Pipeline entry: follow-up or handle merge under the run's target and meter.
pub(super) fn merge_for_run(
    cfg: &super::pipeline::RunConfig,
    tc: &TargetConfig,
    ctx: &mut Value,
    failure: &str,
    kept: &mut Vec<ContextItem>,
    items: Vec<ContextItem>,
) -> usize {
    let rb = cfg.budget.for_task(&cfg.task);
    let (meter, hard) = meter_for(&rb, plans_of(cfg).as_deref());
    merge_escalating(
        tc,
        cfg.context_max_bytes,
        ctx,
        &cfg.snapshot.revision,
        failure,
        kept,
        items,
        &meter,
        hard,
    )
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
    meter: &CostMeter,
    hard_capacity: Option<u64>,
) -> usize {
    let mut t = cfg.target(safety_bound_bytes, ctx, hard_capacity);
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
    let limit = t.current();
    ctx["target"]["unit"] = json!(meter.unit().label());
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
    let (mut used, mut used_bytes) = (0u64, 0usize);
    let mut dropped = 0usize;
    for (i, it) in all.into_iter().enumerate() {
        let c = meter.cost(&it);
        // Earlier (already kept) material and compiler facts are never dropped.
        if i < before
            || it.provenance == COMPILER_VERIFIED
            || it.label.starts_with(REQUIRED_PREFIX)
            || (used + c <= limit && used_bytes + it.bytes() <= safety_bound_bytes)
        {
            used += c;
            used_bytes += it.bytes();
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
