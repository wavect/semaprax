//! One durable task spend budget (TC-03). Every router or generator dispatch
//! is an attempt with a stable id that is reserved (tokens and a cost upper
//! bound) and persisted in the lineage journal *before* control passes to the
//! provider, then settled from the TC-01 receipt. Unused headroom is released
//! only after a known terminal outcome; an attempt whose outcome or cost is
//! unknown stays counted at its reservation. A resume of the same lineage
//! restores every reservation and settlement; contradictory or malformed
//! accounting fails closed instead of resetting to zero.
//!
//! Units: tokens are admission tokens (a named tokenizer's count or the UTF-8
//! byte upper bound) plus framing and the output cap; cost is micro-units.
//! Monetary limits are opt-in; without them only the token, attempt and time
//! bounds refuse.

use super::journal::Journal;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::receipt::price::price_line;
use crate::receipt::{CostEstimate, PriceBook, Pricing, ProposalReceipt};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

pub const SPEND_SCHEMA: &str = "semaprax.harness-task-spend.v1";
const STEP_PREFIX: &str = "spend.";

fn d(code: &'static str, msg: impl Into<String>) -> HarnessDiagnostic {
    HarnessDiagnostic::new(code, msg)
}

fn corrupt(msg: impl std::fmt::Display) -> HarnessDiagnostic {
    d(
        "SPX-HPD070",
        format!("spend accounting in the journal is corrupted or contradictory ({msg}); refused rather than reset"),
    )
}

/// Host-owned monetary policy (`[budget]`; every member opt-in).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SpendPolicy {
    /// Refuse billable work that has no price record or no enforceable bound.
    pub strict_monetary: bool,
    /// Host ceiling on the whole task's cost, applied with any task limit.
    pub max_task_cost_micros: Option<u64>,
    /// Host ceiling on the whole task's tokens, applied with any task limit.
    pub max_task_tokens: Option<u64>,
    /// Disclosed gateway-owned retries per dispatch: each may bill again.
    pub gateway_retries: u64,
}

impl SpendPolicy {
    pub fn from_section(g: &crate::profile::config::GenerationSection) -> Self {
        Self {
            strict_monetary: g.strict_monetary,
            max_task_cost_micros: g.task_max_cost_micros,
            max_task_tokens: g.task_max_tokens,
            gateway_retries: g.gateway_max_retries.unwrap_or(0),
        }
    }
}

/// The applicable limits; admission uses the minimum of those that are set.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Limits {
    pub task_tokens: Option<u64>,
    pub task_cost: Option<u64>,
    pub host_tokens: Option<u64>,
    pub host_cost: Option<u64>,
    pub session_tokens: Option<u64>,
    pub strict_monetary: bool,
}

impl Limits {
    pub fn new(task: &super::budget::BudgetPolicy, host: &SpendPolicy) -> Self {
        Self {
            task_tokens: task.max_task_tokens,
            task_cost: task.max_task_cost_micros,
            host_tokens: host.max_task_tokens,
            host_cost: host.max_task_cost_micros,
            session_tokens: None,
            strict_monetary: host.strict_monetary,
        }
    }
    pub fn token_limit(&self) -> Option<u64> {
        [self.task_tokens, self.host_tokens, self.session_tokens]
            .into_iter()
            .flatten()
            .min()
    }
    pub fn cost_limit(&self) -> Option<u64> {
        [self.task_cost, self.host_cost].into_iter().flatten().min()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpendState {
    /// Reserved before dispatch; counted at the reservation.
    Reserved,
    /// Known terminal outcome with a known cost; counted at the actual.
    Settled,
    /// Outcome or cost unknown; stays counted at the reservation.
    Uncertain,
    /// Known never dispatched; counts nothing.
    Released,
}

impl SpendState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reserved => "reserved",
            Self::Settled => "settled",
            Self::Uncertain => "uncertain",
            Self::Released => "released",
        }
    }
    fn journal_state(self) -> &'static str {
        match self {
            Self::Reserved => "reserve",
            Self::Settled => "settle",
            Self::Uncertain => "uncertain",
            Self::Released => "release",
        }
    }
    fn from_journal(s: &str) -> Option<Self> {
        Some(match s {
            "reserve" => Self::Reserved,
            "settle" => Self::Settled,
            "uncertain" => Self::Uncertain,
            "release" => Self::Released,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Billing {
    /// A versioned price record bounds the cost.
    Priced(String),
    /// Explicitly non-billed (for example a local model); token and attempt bounds still apply.
    NonBilled(String),
    /// No price: the reserved figure is a catalog estimate, not a bound.
    Unpriced(String),
}

impl Billing {
    pub fn billable(&self) -> bool {
        !matches!(self, Self::NonBilled(_))
    }
    fn to_json(&self) -> Value {
        match self {
            Self::Priced(v) => json!({"kind": "priced", "price_version": v}),
            Self::NonBilled(v) => json!({"kind": "non_billed", "price_version": v}),
            Self::Unpriced(r) => json!({"kind": "unpriced", "reason": r}),
        }
    }
    fn from_json(v: &Value) -> Option<Self> {
        let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
        Some(match v.get("kind").and_then(Value::as_str)? {
            "priced" => Self::Priced(s("price_version")?),
            "non_billed" => Self::NonBilled(s("price_version")?),
            "unpriced" => Self::Unpriced(s("reason")?),
            _ => return None,
        })
    }
}

/// Conservative cost upper bound of one dispatch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CostBound {
    pub micros: Option<u64>,
    pub billing: Billing,
}

/// Upper bound for `input_tokens` (request plus framing) and an enforced
/// `output_cap`, `dispatches` times (one plus disclosed gateway retries).
/// Every input token is priced at the dearest input category (uncached,
/// cache read or cache write) because a cache hit is never confirmed before
/// dispatch. Missing input or output prices leave the call unpriced.
///
/// Rounding follows `PriceBook::estimate` (MN-02, `price_line`): input and
/// output are rounded up apart, and when the dearest input rate is not a
/// whole number of micro-units per token, one more micro-unit is reserved for
/// each further priced input category the provider may split the input
/// across (cache read, cache write, one-hour cache write), so in-bound usage
/// never estimates above its reservation. A bound that overflows `u64`
/// saturates at `u64::MAX` (still reported unpriced, so strict mode refuses
/// it) rather than falling back to a cheap catalog figure.
pub fn call_bound(
    prices: &PriceBook,
    model: &str,
    input_tokens: u64,
    output_cap: u64,
    dispatches: u64,
) -> CostBound {
    let unpriced = |r: &str| CostBound {
        micros: None,
        billing: Billing::Unpriced(r.into()),
    };
    let Some(rec) = prices.record_for(model) else {
        return unpriced("unpriced_model");
    };
    let (input, read, write, write_1h, output) = match &rec.pricing {
        Pricing::NonBilled => {
            return CostBound {
                micros: Some(0),
                billing: Billing::NonBilled(rec.version.clone()),
            }
        }
        Pricing::Rates {
            input,
            cache_read,
            cache_write,
            cache_write_1h,
            output,
        } => (*input, *cache_read, *cache_write, *cache_write_1h, *output),
    };
    let (Some(input), Some(output)) = (input, output) else {
        return unpriced("missing_price");
    };
    let in_rates = [Some(input), read, write, write_1h];
    let in_rate = in_rates.into_iter().flatten().max().unwrap_or(input);
    // Per-category rounding can exceed one combined ceiling by at most one
    // micro-unit per extra input partition, and only for a fractional rate.
    let partitions = in_rates.iter().flatten().count() as u128;
    let slack = if in_rate % 1_000_000 == 0 {
        0
    } else {
        partitions - 1
    };
    let one = price_line(input_tokens, in_rate)
        .checked_add(slack)
        .and_then(|n| n.checked_add(price_line(output_cap, output)));
    match one
        .and_then(|n| n.checked_mul(dispatches.max(1) as u128))
        .and_then(|n| u64::try_from(n).ok())
    {
        Some(n) => CostBound {
            micros: Some(n),
            billing: Billing::Priced(rec.version.clone()),
        },
        None => CostBound {
            micros: Some(u64::MAX),
            billing: Billing::Unpriced("overflow".into()),
        },
    }
}

/// One attempt's accounting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendRecord {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub model: String,
    /// Input (request plus framing) plus output cap, per dispatch, times dispatches.
    pub reserved_tokens: u64,
    /// The cost bound (priced), zero (non-billed), or a catalog estimate (unpriced).
    pub reserved_cost: u64,
    pub billing: Billing,
    pub state: SpendState,
    pub actual_cost: Option<u64>,
    pub settled_tokens: Option<u64>,
    pub breach: Option<String>,
    pub basis: Option<String>,
    /// Upstream attempts of a settled invocation whose usage or charge no
    /// receipt covers (DV-20): their reservation stays inside the settled
    /// figures until authoritative evidence covers or clears them.
    pub unresolved_attempts: u64,
    /// Journaled (and so restored on resume).
    pub persist: bool,
    /// Restored from an earlier invocation of this lineage.
    pub restored: bool,
}

impl SpendRecord {
    pub fn committed_tokens(&self) -> u64 {
        match self.state {
            SpendState::Reserved | SpendState::Uncertain => self.reserved_tokens,
            SpendState::Settled => self.settled_tokens.unwrap_or(self.reserved_tokens),
            SpendState::Released => 0,
        }
    }
    pub fn committed_cost(&self) -> u64 {
        match self.state {
            SpendState::Reserved | SpendState::Uncertain => self.reserved_cost,
            SpendState::Settled => self.actual_cost.unwrap_or(self.reserved_cost),
            SpendState::Released => 0,
        }
    }
    fn reserve_detail(&self) -> Value {
        json!({"id": self.id, "kind": self.kind, "label": self.label, "model": self.model,
               "reserved_tokens": self.reserved_tokens, "reserved_cost_micros": self.reserved_cost,
               "billing": self.billing.to_json()})
    }
    fn to_json(&self) -> Value {
        json!({"id": self.id, "kind": self.kind, "label": self.label, "model": self.model,
               "state": self.state.as_str(), "billing": self.billing.to_json(),
               "reserved_tokens": self.reserved_tokens, "reserved_cost_micros": self.reserved_cost,
               "actual_cost_micros": self.actual_cost, "settled_tokens": self.settled_tokens,
               "committed_tokens": self.committed_tokens(), "committed_cost_micros": self.committed_cost(),
               "basis": self.basis, "breach": self.breach,
               "unresolved_attempts": self.unresolved_attempts, "restored": self.restored})
    }
}

/// What a terminal (or uncertain) outcome settles an attempt to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settlement {
    pub state: SpendState,
    pub actual_cost: Option<u64>,
    pub settled_tokens: Option<u64>,
    pub breach: Option<String>,
    pub basis: Option<String>,
    /// See `SpendRecord::unresolved_attempts`.
    pub unresolved_attempts: u64,
}

impl Settlement {
    fn detail(&self, id: &str) -> Value {
        json!({"id": id, "actual_cost_micros": self.actual_cost, "settled_tokens": self.settled_tokens,
               "breach": self.breach, "basis": self.basis, "unresolved_attempts": self.unresolved_attempts})
    }
    pub fn released(why: &str) -> Self {
        Self {
            state: SpendState::Released,
            actual_cost: None,
            settled_tokens: None,
            breach: None,
            basis: Some(why.into()),
            unresolved_attempts: 0,
        }
    }
    pub fn uncertain(why: &str) -> Self {
        Self {
            state: SpendState::Uncertain,
            actual_cost: None,
            settled_tokens: None,
            breach: None,
            basis: Some(why.into()),
            unresolved_attempts: 0,
        }
    }
}

/// Settle one single-dispatch generation from its TC-01 receipt; see
/// `settle_generation_dispatches`.
pub fn settle_generation(
    rec: &SpendRecord,
    receipt: &ProposalReceipt,
    estimate: &CostEstimate,
    outcome_known: bool,
    input_tokens: u64,
    output_cap: u64,
) -> Settlement {
    settle_generation_dispatches(
        rec,
        receipt,
        estimate,
        outcome_known,
        input_tokens,
        output_cap,
        1,
    )
}

/// Settle one generation from its TC-01 receipt. `outcome_known` is false
/// for an uncertain outcome (cancellation, lost connection after send): that
/// attempt stays reserved. A known outcome with an unknown cost also stays
/// reserved. The provider-reported charge wins over the local estimate; a
/// charge above the priced bound, or output above the cap, is a breach.
///
/// `dispatches` is the admitted upstream attempt count (one plus disclosed
/// gateway retries). A receipt covers only the final upstream attempt unless
/// it declares aggregate coverage, and token and cost coverage are separate.
/// Attempts that no receipt covers and that are not proven unused keep their
/// per-dispatch reservation inside the settled figures (`unresolved_attempts`),
/// so a complete final answer never releases a possibly billed retry.
pub fn settle_generation_dispatches(
    rec: &SpendRecord,
    receipt: &ProposalReceipt,
    estimate: &CostEstimate,
    outcome_known: bool,
    input_tokens: u64,
    output_cap: u64,
    dispatches: u64,
) -> Settlement {
    if !outcome_known {
        return Settlement::uncertain("outcome_unknown");
    }
    let cov = receipt.coverage;
    let d = dispatches.max(1);
    let unused = cov.unused_attempts.min(d - 1);
    let remaining = d - 1 - unused;
    let (actual, basis, cost_aggregate) = match (receipt.provider_cost_micros, estimate.micros) {
        (Some(c), _) => (c, "provider_reported", cov.cost_aggregate),
        (None, Some(e)) => (e, estimate.basis, cov.usage_aggregate),
        (None, None) => return Settlement::uncertain("cost_unknown"),
    };
    let priced = matches!(rec.billing, Billing::Priced(_));
    // A priced bound is the exact multiple of one dispatch; any other
    // reservation is a per-call figure and is retained whole per attempt.
    let per_cost = if priced {
        rec.reserved_cost / d
    } else {
        rec.reserved_cost
    };
    let cost_unresolved = if cost_aggregate { 0 } else { remaining };
    let tok_unresolved = if cov.usage_aggregate { 0 } else { remaining };
    let out = receipt.usage.output;
    let mut breach = None;
    let cost_bound = if cost_aggregate {
        rec.reserved_cost
    } else {
        per_cost
    };
    if priced && actual > cost_bound {
        breach = Some(format!(
            "charged {actual} micros above the declared bound of {cost_bound}"
        ));
    }
    let usage_attempts = if cov.usage_aggregate { d - unused } else { 1 };
    let cap = output_cap.saturating_mul(usage_attempts);
    if let Some(o) = out.filter(|o| *o > cap) {
        breach.get_or_insert(format!(
            "returned {o} output tokens above the enforced cap of {cap}"
        ));
    }
    // Input is settled in the reserved unit; only the output (same model's
    // tokens as the cap) of covered attempts releases headroom.
    let per_tokens = input_tokens.saturating_add(output_cap);
    let settled_tokens = out
        .map(|o| {
            input_tokens
                .saturating_mul(usage_attempts)
                .saturating_add(o)
                .saturating_add(per_tokens.saturating_mul(tok_unresolved))
        })
        .filter(|t| *t <= rec.reserved_tokens || breach.is_some());
    Settlement {
        state: SpendState::Settled,
        actual_cost: Some(actual.saturating_add(per_cost.saturating_mul(cost_unresolved))),
        settled_tokens,
        breach,
        basis: Some(basis.into()),
        unresolved_attempts: cost_unresolved.max(tok_unresolved),
    }
}

/// Every attempt of the task across invocations of the lineage.
#[derive(Clone, Debug, Default)]
pub struct SpendBook {
    pub records: Vec<SpendRecord>,
    pub limits: Limits,
    pub breach: Option<String>,
    /// Set when a journal append failed (MN-03): what reached storage is
    /// unknown, so the live book keeps its last acknowledged state and
    /// refuses every further admission or settlement until the lineage is
    /// reopened and restored from the journal.
    pub unreconciled: Option<String>,
}

impl SpendBook {
    pub fn committed_tokens(&self) -> u64 {
        self.records
            .iter()
            .fold(0u64, |a, r| a.saturating_add(r.committed_tokens()))
    }
    pub fn committed_cost(&self) -> u64 {
        self.records
            .iter()
            .fold(0u64, |a, r| a.saturating_add(r.committed_cost()))
    }
    pub fn record(&self, id: &str) -> Option<&SpendRecord> {
        self.records.iter().find(|r| r.id == id)
    }
    /// A stable id for the next attempt of `label`.
    pub fn next_id(&self, label: &str, kind: &str) -> String {
        format!("{label}.{kind}.{}", self.records.len() + 1)
    }
    /// Remaining cost headroom under the cost limit, if one is set.
    pub fn available_cost(&self) -> Option<u64> {
        self.limits
            .cost_limit()
            .map(|m| m.saturating_sub(self.committed_cost()))
    }
    pub fn available_tokens(&self) -> Option<u64> {
        self.limits
            .token_limit()
            .map(|m| m.saturating_sub(self.committed_tokens()))
    }

    /// Would `rec` be admitted now? No side effect.
    pub fn check(&self, rec: &SpendRecord) -> HarnessResult<()> {
        self.reconciled()?;
        let l = &self.limits;
        if rec.billing.billable() {
            if let Some(b) = &self.breach {
                return Err(d(
                    "SPX-HPD101",
                    format!("spend breach: an earlier attempt exceeded its declared bound ({b}); further paid work is blocked for `{}`", rec.label),
                ));
            }
        }
        if l.strict_monetary {
            if let Billing::Unpriced(why) = &rec.billing {
                return Err(d(
                    "SPX-HPD101",
                    format!("strict monetary budget: `{}` for `{}` has no enforceable price bound ({why}); refused before dispatch", rec.model, rec.label),
                ));
            }
        }
        let tokens = self
            .committed_tokens()
            .checked_add(rec.reserved_tokens)
            .ok_or_else(|| d("SPX-HPD101", "task token accounting overflow"))?;
        for (limit, what, code) in [
            (l.task_tokens, "task token budget", "SPX-HPD101"),
            (l.host_tokens, "host task token budget", "SPX-HPD101"),
            (
                l.session_tokens,
                "session bound exhausted: max_tokens",
                "SPX-HPD111",
            ),
        ] {
            if let Some(max) = limit.filter(|m| tokens > *m) {
                return Err(d(
                    code,
                    format!(
                        "{what}: {} committed + {} for `{}` exceeds {max}; refused before dispatch",
                        self.committed_tokens(),
                        rec.reserved_tokens,
                        rec.label
                    ),
                ));
            }
        }
        let cost = self
            .committed_cost()
            .checked_add(rec.reserved_cost)
            .ok_or_else(|| d("SPX-HPD101", "task cost accounting overflow"))?;
        for (limit, what) in [
            (l.task_cost, "task cost budget"),
            (l.host_cost, "host task cost budget"),
        ] {
            if let Some(max) = limit.filter(|m| cost > *m) {
                return Err(d(
                    "SPX-HPD101",
                    format!(
                        "{what} exhausted: {} micros committed + {} for `{}` exceeds {max}; refused before dispatch",
                        self.committed_cost(),
                        rec.reserved_cost,
                        rec.label
                    ),
                ));
            }
        }
        Ok(())
    }

    /// Refuse while an earlier append's durability is unknown.
    fn reconciled(&self) -> HarnessResult<()> {
        match &self.unreconciled {
            Some(why) => Err(d(
                "SPX-HPD070",
                format!("spend accounting is pending reconciliation ({why}); reopen the lineage journal before further admission or settlement"),
            )),
            None => Ok(()),
        }
    }

    /// Append one spend record; on failure mark the book unreconciled.
    fn persist(
        &mut self,
        journal: &mut Journal,
        id: &str,
        state: &str,
        detail: Value,
    ) -> HarnessResult<()> {
        journal
            .append(&format!("{STEP_PREFIX}{id}"), state, detail)
            .inspect_err(|e| {
                self.unreconciled = Some(format!("`{id}` {state} append failed: {}", e.code));
            })
    }

    /// Admit and record `rec`, persisting it before the caller dispatches.
    pub fn reserve(&mut self, journal: &mut Journal, rec: SpendRecord) -> HarnessResult<()> {
        self.check(&rec)?;
        if self.record(&rec.id).is_some() {
            return Err(corrupt(format!("attempt id `{}` reused", rec.id)));
        }
        if rec.persist {
            self.persist(journal, &rec.id, "reserve", rec.reserve_detail())?;
        }
        self.records.push(rec);
        Ok(())
    }

    /// Validate the transition of `id` to `s` without publishing it: the
    /// index to update, or `None` for an identical repeat of a terminal state.
    fn transition(&self, id: &str, s: &Settlement) -> HarnessResult<Option<usize>> {
        let (i, rec) = self
            .records
            .iter()
            .enumerate()
            .find(|(_, r)| r.id == id)
            .ok_or_else(|| corrupt(format!("settlement of unknown attempt `{id}`")))?;
        let same = rec.state == s.state
            && rec.actual_cost == s.actual_cost
            && rec.settled_tokens == s.settled_tokens
            && rec.breach == s.breach
            && rec.unresolved_attempts == s.unresolved_attempts;
        match (rec.state, s.state) {
            _ if same && rec.state != SpendState::Reserved => Ok(None),
            // Authoritative evidence later covering retained attempts.
            (SpendState::Settled, SpendState::Settled)
                if s.unresolved_attempts < rec.unresolved_attempts =>
            {
                Ok(Some(i))
            }
            (
                SpendState::Reserved,
                SpendState::Settled | SpendState::Uncertain | SpendState::Released,
            )
            | (SpendState::Uncertain, SpendState::Settled) => Ok(Some(i)),
            (from, to) => Err(corrupt(format!(
                "attempt `{id}` cannot go from {} to {}",
                from.as_str(),
                to.as_str()
            ))),
        }
    }

    /// Publish a validated transition to live accounting.
    fn commit(&mut self, i: usize, s: Settlement) {
        let rec = &mut self.records[i];
        if let Some(b) = &s.breach {
            self.breach
                .get_or_insert_with(|| format!("{}: {b}", rec.id));
        }
        rec.state = s.state;
        rec.actual_cost = s.actual_cost;
        rec.settled_tokens = s.settled_tokens;
        rec.breach = s.breach;
        rec.basis = s.basis;
        rec.unresolved_attempts = s.unresolved_attempts;
    }

    /// Settle (idempotently) and persist before the caller records
    /// completion. The live book changes only after the append is
    /// acknowledged (MN-03); a failed append leaves the attempt at its
    /// conservative prior state and marks the book unreconciled, so an
    /// identical retry refuses instead of reporting false durable success.
    pub fn settle(&mut self, journal: &mut Journal, id: &str, s: Settlement) -> HarnessResult<()> {
        self.reconciled()?;
        let Some(i) = self.transition(id, &s)? else {
            return Ok(());
        };
        if self.records[i].persist {
            self.persist(journal, id, s.state.journal_state(), s.detail(id))?;
        }
        self.commit(i, s);
        Ok(())
    }

    /// Restore every journaled attempt of this lineage. Malformed,
    /// out-of-order or contradictory records refuse the run.
    pub fn restore(&mut self, journal: &Journal) -> HarnessResult<()> {
        for r in journal.records() {
            let Some(id) = r.step.strip_prefix(STEP_PREFIX) else {
                continue;
            };
            let det = &r.detail;
            if det.get("id").and_then(Value::as_str) != Some(id) {
                return Err(corrupt(format!("record {} names another attempt", r.seq)));
            }
            let state = SpendState::from_journal(&r.state)
                .ok_or_else(|| corrupt(format!("record {} has state `{}`", r.seq, r.state)))?;
            let num = |k: &str| det.get(k).and_then(Value::as_u64);
            let opt_num = |k: &str| match det.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(v) => v
                    .as_u64()
                    .map(Some)
                    .ok_or_else(|| corrupt(format!("record {} member `{k}`", r.seq))),
            };
            let opt_text = |k: &str| match det.get(k) {
                None | Some(Value::Null) => Ok(None),
                Some(Value::String(s)) => Ok(Some(s.clone())),
                _ => Err(corrupt(format!("record {} member `{k}`", r.seq))),
            };
            if state == SpendState::Reserved {
                let text = |k: &str| {
                    det.get(k)
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .ok_or_else(|| corrupt(format!("record {} member `{k}`", r.seq)))
                };
                let rec = SpendRecord {
                    id: id.into(),
                    kind: text("kind")?,
                    label: text("label")?,
                    model: text("model")?,
                    reserved_tokens: num("reserved_tokens")
                        .ok_or_else(|| corrupt(format!("record {} reserved_tokens", r.seq)))?,
                    reserved_cost: num("reserved_cost_micros")
                        .ok_or_else(|| corrupt(format!("record {} reserved_cost", r.seq)))?,
                    billing: det
                        .get("billing")
                        .and_then(Billing::from_json)
                        .ok_or_else(|| corrupt(format!("record {} billing", r.seq)))?,
                    state,
                    actual_cost: None,
                    settled_tokens: None,
                    breach: None,
                    basis: None,
                    unresolved_attempts: 0,
                    persist: true,
                    restored: true,
                };
                match self.record(id) {
                    Some(prev) if prev.reserve_detail() == rec.reserve_detail() => continue,
                    Some(_) => return Err(corrupt(format!("attempt `{id}` reserved twice"))),
                    None => self.records.push(rec),
                }
            } else {
                let s = Settlement {
                    state,
                    actual_cost: opt_num("actual_cost_micros")?,
                    settled_tokens: opt_num("settled_tokens")?,
                    breach: opt_text("breach")?,
                    basis: opt_text("basis")?,
                    unresolved_attempts: opt_num("unresolved_attempts")?.unwrap_or(0),
                };
                if state == SpendState::Settled && s.actual_cost.is_none() {
                    return Err(corrupt(format!("record {} settles without a cost", r.seq)));
                }
                if let Some(i) = self.transition(id, &s)? {
                    self.commit(i, s);
                }
            }
        }
        Ok(())
    }

    pub fn to_json(&self) -> Value {
        let mut actual = 0u64;
        let mut outstanding = 0u64;
        let mut unknown = 0u64;
        let mut non_billed = 0u64;
        let mut unpriced = 0u64;
        for r in &self.records {
            match r.state {
                SpendState::Settled => actual = actual.saturating_add(r.committed_cost()),
                SpendState::Reserved | SpendState::Uncertain => {
                    outstanding = outstanding.saturating_add(r.reserved_cost)
                }
                SpendState::Released => {}
            }
            if matches!(r.billing, Billing::NonBilled(_)) && r.state != SpendState::Released {
                non_billed += 1;
            }
            // Actual cost not (yet) known: counted at the reservation meanwhile.
            if matches!(r.state, SpendState::Reserved | SpendState::Uncertain)
                || r.unresolved_attempts > 0
            {
                unknown += 1;
            }
            if matches!(r.billing, Billing::Unpriced(_)) && r.state != SpendState::Released {
                unpriced += 1;
            }
        }
        let l = &self.limits;
        json!({
            "schema": SPEND_SCHEMA,
            "strict_monetary": l.strict_monetary,
            "limits": {"tokens": l.token_limit(), "cost_micros": l.cost_limit(),
                        "task_tokens": l.task_tokens, "task_cost_micros": l.task_cost,
                        "host_tokens": l.host_tokens, "host_cost_micros": l.host_cost,
                        "session_tokens": l.session_tokens},
            "known_actual_cost_micros": actual,
            "outstanding_upper_bound_micros": outstanding,
            "unknown_spend_attempts": unknown,
            "unresolved_retry_attempts": self.records.iter().fold(0u64, |a, r| a.saturating_add(r.unresolved_attempts)),
            "unpriced_attempts": unpriced,
            "non_billed_attempts": non_billed,
            "committed_tokens": self.committed_tokens(),
            "committed_cost_micros": self.committed_cost(),
            "available_tokens": self.available_tokens(),
            "available_cost_micros": self.available_cost(),
            "breach": self.breach,
            "attempts": self.records.iter().map(SpendRecord::to_json).collect::<Vec<_>>(),
        })
    }
}

/// The one local writer of a lineage's journal. A lock left by a process
/// that no longer runs is taken over; a live holder refuses the run.
pub struct WriterLock(PathBuf);

impl WriterLock {
    pub fn acquire(dir: &Path, lineage: &str) -> HarnessResult<Self> {
        std::fs::create_dir_all(dir).map_err(|e| d("SPX-HPD070", format!("journal lock: {e}")))?;
        let path = dir.join(format!("{lineage}.journal.lock"));
        for _ in 0..2 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut f) => {
                    use std::io::Write;
                    let _ = f.write_all(std::process::id().to_string().as_bytes());
                    return Ok(Self(path));
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let holder = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|s| s.trim().parse::<i32>().ok());
                    if holder.is_some_and(alive) {
                        return Err(d(
                            "SPX-HPD070",
                            format!("another local writer (pid {}) holds this lineage's journal; refused", holder.unwrap_or(0)),
                        ));
                    }
                    let _ = std::fs::remove_file(&path);
                }
                Err(e) => return Err(d("SPX-HPD070", format!("journal lock: {e}"))),
            }
        }
        Err(d("SPX-HPD070", "journal lock could not be taken"))
    }
}

impl Drop for WriterLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(unix)]
fn alive(pid: i32) -> bool {
    match rustix::process::Pid::from_raw(pid) {
        Some(p) => match rustix::process::test_kill_process(p) {
            Ok(()) => true,
            Err(e) => e == rustix::io::Errno::PERM,
        },
        None => false,
    }
}

#[cfg(not(unix))]
fn alive(_pid: i32) -> bool {
    // No portable liveness probe: a present lock is treated as held.
    true
}

#[cfg(test)]
mod tests;
