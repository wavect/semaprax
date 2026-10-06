//! Where the workflow meets the task spend book (TC-03): the router and the
//! generator are admitted and journaled before dispatch and settled after.

use super::attempt::router_reserve;
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
    /// Admitted upstream dispatches: one plus disclosed gateway retries.
    pub dispatches: u64,
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
        unresolved_attempts: 0,
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
    signals: &crate::decision::RouteSignals,
) -> HarnessResult<Option<String>> {
    let Some(dec) = st.decision.as_ref() else {
        return Ok(None);
    };
    // MR-03: reserve against the host-rendered prepared request (v2) or the
    // v1 projection, with a protocol-derived closed-choice output reserve.
    let wire_v2 = crate::decision::wire_version(&dec.profile, &*dec.invoker) == 2;
    let (_, _, text) =
        super::attempt::route_parts(&cx.cfg.task, pool, est, allowance, true, signals, wire_v2)?;
    let reserve = router_reserve(pool);
    let count = budget.count(&dec.profile.provider_id, &text);
    let calls = u64::from(cx.cfg.routing.cfg.shadow_max_calls.max(1));
    let input = count.admission_tokens();
    let tokens = input.saturating_add(reserve).saturating_mul(calls);
    let bound = call_bound(
        &cx.cfg.budget.prices,
        &dec.profile.model_id,
        input,
        reserve,
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
/// dispatched, released. A non-billed router settles at zero. A single priced
/// call with provider-reported input usage (MR-03 `call.usage`) settles from
/// that receipt; anything else (no usage, timeout after dispatch, several
/// calls, unpriced) stays uncertain at the reservation, never zero.
pub(crate) fn router_settlement(
    rec: &SpendRecord,
    calls: u32,
    call: Option<&crate::decision::CallMetadata>,
    prices: &crate::receipt::PriceBook,
    model: &str,
    output_reserve: u64,
) -> Settlement {
    if calls == 0 {
        return Settlement::released("router_not_consulted");
    }
    if matches!(rec.billing, Billing::NonBilled(_)) {
        return Settlement {
            state: SpendState::Settled,
            actual_cost: Some(0),
            settled_tokens: Some(rec.reserved_tokens),
            breach: None,
            basis: Some("non_billed_source".into()),
            unresolved_attempts: 0,
        };
    }
    let reported = call.and_then(|c| c.usage.authoritative_input().map(|i| (i, c)));
    match reported {
        Some((input, c)) if calls == 1 => {
            let output = c.usage.output_tokens.unwrap_or(output_reserve);
            match call_bound(prices, model, input, output, 1).micros {
                Some(cost) => Settlement {
                    state: SpendState::Settled,
                    actual_cost: Some(cost),
                    settled_tokens: Some(input.saturating_add(output)),
                    breach: None,
                    basis: Some("router_provider_usage".into()),
                    unresolved_attempts: 0,
                },
                None => Settlement::uncertain("router_usage_unpriced"),
            }
        }
        Some(_) => Settlement::uncertain("router_usage_covers_one_of_several_calls"),
        None => Settlement::uncertain("no_router_receipt"),
    }
}

pub(super) fn settle_router(
    cx: &mut Ctx,
    journal: &mut Journal,
    id: &str,
    calls: u32,
    call: Option<&crate::decision::CallMetadata>,
    model: &str,
    output_reserve: u64,
) -> HarnessResult<()> {
    let rec = cx
        .ledger
        .spend
        .record(id)
        .cloned()
        .expect("reserved router");
    let s = router_settlement(
        &rec,
        calls,
        call,
        &cx.cfg.budget.prices,
        model,
        output_reserve,
    );
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
        dispatches,
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
    let s = super::spend::settle_generation_dispatches(
        &rec,
        receipt,
        estimate,
        outcome_known,
        a.input_tokens,
        a.output_cap,
        a.dispatches,
    );
    cx.ledger.spend.settle(journal, &a.id, s)
}

#[cfg(test)]
mod router_settlement_tests {
    use super::*;
    use crate::decision::{Billing as CallBilling, CallMetadata, IdentityKind, Usage, UsageBasis};
    use crate::receipt::PriceBook;
    use serde_json::json;

    fn book() -> PriceBook {
        PriceBook::from_json(&json!({"schema": "semaprax.harness-price-book.v1", "records": [
            {"model_prefix": "router-m", "version": "2026-10",
             "pricing": {"input": 1_000_000, "cache_read": 1_000_000, "cache_write": 1_000_000, "output": 2_000_000}}]}))
        .unwrap()
    }

    fn rec(billing: Billing) -> SpendRecord {
        let bound = super::super::spend::CostBound {
            micros: Some(10_000),
            billing,
        };
        record(
            "r1".into(),
            "router",
            "x-router".into(),
            "router-m",
            10_000,
            bound,
            0,
            true,
        )
    }

    fn call(basis: UsageBasis, input: Option<u64>) -> CallMetadata {
        CallMetadata {
            adapter: "a@1".into(),
            requested_model: None,
            answering_model: None,
            checkpoint: None,
            identity_kind: IdentityKind::Unknown,
            rendered_digest: format!("sha256:{}", "0".repeat(64)),
            wire_bytes: 10,
            usage: Usage {
                input_tokens: input,
                output_tokens: None,
                basis,
            },
            billing: CallBilling::Api,
        }
    }

    #[test]
    fn hp_mr03_router_spend_settles_from_reported_usage_and_stays_conservative_otherwise() {
        let priced = rec(Billing::Priced("router-m@2026-10".into()));
        let p = book();
        let s = |calls, c: Option<&CallMetadata>| {
            router_settlement(&priced, calls, c, &p, "router-m", 32)
        };
        // Provider-reported usage settles the single call from that receipt.
        let got = s(1, Some(&call(UsageBasis::ProviderReported, Some(500))));
        assert_eq!(got.state, SpendState::Settled);
        assert_eq!(got.settled_tokens, Some(532));
        assert_eq!(got.actual_cost, Some(500 + 64));
        // Absent, unknown or locally estimated usage, and a timeout after
        // dispatch (no call metadata), never refund to zero.
        for c in [
            None,
            Some(call(UsageBasis::Unknown, Some(1))),
            Some(call(UsageBasis::LocalMeasured, Some(1))),
            Some(call(UsageBasis::ProviderReported, None)),
        ] {
            let got = s(1, c.as_ref());
            assert_eq!(got.state, SpendState::Uncertain);
            assert_eq!(got.actual_cost, None);
        }
        // One receipt cannot settle a decision plus shadow call.
        assert_eq!(
            s(2, Some(&call(UsageBasis::ProviderReported, Some(5)))).state,
            SpendState::Uncertain
        );
        assert_eq!(s(0, None).state, SpendState::Released);
        // An adapter's `billing: local` claim does not zero a priced router;
        // only host-declared non-billed sources settle at zero.
        let nb = rec(Billing::NonBilled("local".into()));
        let got = router_settlement(&nb, 1, None, &p, "router-m", 32);
        assert_eq!((got.state, got.actual_cost), (SpendState::Settled, Some(0)));
    }
}
