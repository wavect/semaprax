//! Where the workflow meets the task spend book (TC-03): the router and the
//! generator are admitted and journaled before dispatch and settled after.

use super::attempt::ROUTER_OUTPUT_RESERVE;
use super::budget::{Fit, RequestBudget};
use super::journal::Journal;
use super::pipeline::{Ctx, Stages};
use super::report::Report;
use super::spend::{call_bound, Billing, Settlement, SpendRecord, SpendState};
use crate::decision::ModelPlan;
use crate::diag::{HarnessDiagnostic, HarnessResult};
use crate::receipt::{CostEstimate, ProposalReceipt, Support};

/// One admitted generation attempt.
#[derive(Clone, Debug)]
pub(super) struct Attempt {
    pub id: String,
    /// Reserved input per dispatch: request plus framing.
    pub input_tokens: u64,
    pub output_cap: u64,
}

#[allow(clippy::too_many_arguments)]
fn record(
    id: String,
    kind: &str,
    label: String,
    model: &str,
    tokens: u64,
    bound: super::spend::CostBound,
    fallback_cost: u64,
    persist: bool,
) -> SpendRecord {
    SpendRecord {
        id,
        kind: kind.into(),
        label,
        model: model.into(),
        reserved_tokens: tokens,
        reserved_cost: bound.micros.unwrap_or(fallback_cost),
        billing: bound.billing,
        state: SpendState::Reserved,
        actual_cost: None,
        settled_tokens: None,
        breach: None,
        basis: None,
        persist,
        restored: false,
    }
}

/// Admit and journal the router's possible calls. `Ok(None)` when the router
/// may not be consulted (no provider, or unaffordable / unpriced in strict
/// mode / blocked by a breach): rules route without a model call.
#[allow(clippy::too_many_arguments)]
pub(super) fn reserve_router(
    cx: &mut Ctx,
    st: &Stages,
    journal: &mut Journal,
    r: &mut Report,
    budget: &RequestBudget,
    pool: &[ModelPlan],
    est: u64,
    allowance: u64,
    label: &str,
) -> HarnessResult<Option<String>> {
    let Some(dec) = st.decision.as_ref() else {
        return Ok(None);
    };
    let (_, _, text) = super::attempt::route_parts(&cx.cfg.task, pool, est, allowance, true)?;
    let count = budget.count(&dec.profile.provider_id, &text);
    let calls = u64::from(cx.cfg.routing.cfg.shadow_max_calls.max(1));
    let input = count.admission_tokens();
    let tokens = input
        .saturating_add(ROUTER_OUTPUT_RESERVE)
        .saturating_mul(calls);
    let bound = call_bound(
        &cx.cfg.budget.prices,
        &dec.profile.model_id,
        input,
        ROUTER_OUTPUT_RESERVE,
        calls,
    );
    let id = cx.ledger.spend.next_id(label, "router");
    let rec = record(
        id.clone(),
        "router",
        format!("{label}-router"),
        &dec.profile.provider_id,
        tokens,
        bound,
        0,
        true,
    );
    if let Err(e) = cx.ledger.spend.check(&rec) {
        r.notes.push(format!(
            "router not consulted: {} {}; rules decide without a model call",
            e.code, e.message
        ));
        return Ok(None);
    }
    cx.ledger.spend.reserve(journal, rec)?;
    Ok(Some(id))
}

/// Settle the router from the calls it actually made. None: known not
/// dispatched, released. A non-billed router settles at zero; a billable
/// router has no receipt, so its cost stays at the reservation.
pub(super) fn settle_router(
    cx: &mut Ctx,
    journal: &mut Journal,
    id: &str,
    calls: u32,
) -> HarnessResult<()> {
    let rec = cx
        .ledger
        .spend
        .record(id)
        .cloned()
        .expect("reserved router");
    let s = if calls == 0 {
        Settlement::released("router_not_consulted")
    } else if matches!(rec.billing, Billing::NonBilled(_)) {
        Settlement {
            state: SpendState::Settled,
            actual_cost: Some(0),
            settled_tokens: Some(rec.reserved_tokens),
            breach: None,
            basis: Some("non_billed_source".into()),
        }
    } else {
        Settlement::uncertain("no_router_receipt")
    };
    cx.ledger.spend.settle(journal, id, s)
}

/// Admit and journal one generation against the minimum of the task, session
/// and host limits, before any dispatch. Framing and disclosed gateway
/// retries are part of the reservation.
pub(super) fn reserve_generation(
    cx: &mut Ctx,
    st: &Stages,
    journal: &mut Journal,
    budget: &RequestBudget,
    plan: &ModelPlan,
    fit: &Fit,
    label: &str,
) -> HarnessResult<Attempt> {
    let spend = &cx.cfg.budget.spend;
    // Known framing is what the adapter declares it adds (TC-02); the
    // policy's generic protocol overhead stays a context-fit margin only.
    let input = fit
        .count
        .admission_tokens()
        .saturating_add(st.proposer.framing_overhead_tokens());
    let out = budget.policy.output_reserve_tokens;
    let dispatches = spend.gateway_retries.saturating_add(1);
    let bound = call_bound(&cx.cfg.budget.prices, &plan.id, input, out, dispatches);
    if spend.strict_monetary
        && matches!(bound.billing, Billing::Priced(_))
        && st.proposer.generation_support().output_cap != Support::Supported
    {
        return Err(HarnessDiagnostic::new(
            "SPX-HPD101",
            format!("strict monetary budget: `{}` is billable but its provider is not declared to enforce the output cap, so no cost bound holds; refused before dispatch", plan.id),
        ));
    }
    let id = cx.ledger.spend.next_id(label, "generation");
    let rec = record(
        id.clone(),
        "generation",
        label.into(),
        &plan.id,
        input.saturating_add(out).saturating_mul(dispatches),
        bound,
        plan.est_cost_micros,
        st.proposer.side_effecting(),
    );
    let cost = rec.reserved_cost;
    cx.ledger.spend.reserve(journal, rec)?;
    cx.ledger.entries.push(super::budget::LedgerEntry {
        id: id.clone(),
        label: label.into(),
        kind: "generation".into(),
        count: fit.count.clone(),
        output_reserve: out,
        cost_micros: cost,
    });
    Ok(Attempt {
        id,
        input_tokens: input,
        output_cap: out,
    })
}

/// Known never dispatched: the reservation is returned.
pub(super) fn release(
    cx: &mut Ctx,
    journal: &mut Journal,
    a: &Attempt,
    why: &str,
) -> HarnessResult<()> {
    cx.ledger
        .spend
        .settle(journal, &a.id, Settlement::released(why))
}

pub(super) fn settle_generation(
    cx: &mut Ctx,
    journal: &mut Journal,
    a: &Attempt,
    receipt: &ProposalReceipt,
    estimate: &CostEstimate,
    outcome_known: bool,
) -> HarnessResult<()> {
    let rec = cx.ledger.spend.record(&a.id).cloned().expect("reserved");
    let s = super::spend::settle_generation(
        &rec,
        receipt,
        estimate,
        outcome_known,
        a.input_tokens,
        a.output_cap,
    );
    cx.ledger.spend.settle(journal, &a.id, s)
}
