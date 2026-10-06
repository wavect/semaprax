//! TC-10 wiring: the opt-in cost-aware route over a host-approved ladder.
//!
//! Inactive unless `[routing] cost_aware = true` and the task family has a
//! `[routing.ladder.<family>]`. Rules routing, the governor, the evidence-key
//! gates and every spend bound stay in force: this only narrows the admissible
//! pool before `rules_choice`/`governed_decide` run, so a pin, privacy screen or
//! spend allowance can never be overridden. A paid router is bypassed.

use super::attempt::route_parts;
use super::pipeline::Ctx;
use crate::decision::cost_route::{
    choose_start, classify_failure, next_action, next_rung, ActionContext, CacheState,
    ChooseInputs, NextAction,
};
use crate::decision::qualify::gate_for;
use crate::decision::{
    EvidenceKey, EvidenceRecord, EvidenceRegistry, GateStatus, ModelPlan, ProviderProfile,
    RouteRequest, RoutingMode,
};
use serde_json::{json, Value};

/// Escalation state of one task run.
#[derive(Default)]
pub(super) struct LadderState {
    /// The model of the latest routed generation.
    pub current: Option<String>,
    pub escalations: u32,
    /// The decision after the latest known failure, consumed by the next route.
    pub pending: Option<NextAction>,
    pub seen_failures: usize,
    pub log: Vec<Value>,
}

pub(super) struct Applied {
    pub pool: Vec<ModelPlan>,
    pub json: Value,
}

fn pinned(cx: &Ctx) -> bool {
    let w = &cx.cfg.routing;
    w.cfg.project_pin.is_some() || matches!(w.cfg.mode, RoutingMode::Pin(_))
}

/// `true` when the cost-aware route governs this task's family.
pub(super) fn active(cx: &Ctx) -> bool {
    let w = &cx.cfg.routing;
    w.cost_aware && w.ladders.contains_key(&cx.cfg.task.family)
}

pub(super) fn note_model(cx: &Ctx, model: &str) {
    if active(cx) {
        cx.ladder.borrow_mut().current = Some(model.to_string());
    }
}

/// The registered record for `key`, only when the session lock admits it and the
/// predeclared gate passes it; otherwise the reason it grants no authority.
fn qualified_evidence<'a>(
    w: &super::routing::RoutingWiring,
    reg: &'a EvidenceRegistry,
    key: &EvidenceKey,
) -> Result<&'a EvidenceRecord, String> {
    let rec = reg.get(key).ok_or("no fresh evidence for the live key")?;
    match w.lock_for(key) {
        Some(l) if l.key_digest == key.digest() && l.record_digest == rec.digest() => {}
        _ => return Err("live key or record differs from the session's locked evidence".into()),
    }
    match gate_for(reg, key, &w.spec) {
        (g, _) if matches!(g.status, GateStatus::Passed { .. }) => Ok(rec),
        (_, Some(d)) => Err(format!("evidence not qualified: {}", d.reasons.join("; "))),
        (_, None) => Err("evidence not evaluated".into()),
    }
}

/// Narrow `pool` for this route. `None` leaves routing exactly as before.
pub(super) fn apply(
    cx: &Ctx,
    pool: &[ModelPlan],
    input_tokens: u64,
    output_cap: u64,
    profile: Option<&ProviderProfile>,
) -> Option<Applied> {
    if !active(cx) {
        return None;
    }
    let task = &cx.cfg.task;
    let lad = &cx.cfg.routing.ladders[&task.family];
    let (features, budget, _) = route_parts(
        task,
        pool,
        input_tokens,
        1,
        false,
        &crate::decision::RouteSignals::default(),
        false,
    )
    .ok()?;
    // Raw registry presence and `min_tasks` confer no authority: the record must
    // pass the predeclared qualification gate under the session-locked key.
    let (evidence, qualification) = match profile {
        None => (None, "no routing profile for the live key".to_string()),
        Some(p) => {
            let catalog = RouteRequest::new(features.clone(), pool.to_vec(), budget)
                .ok()
                .map(|r| r.catalog_digest());
            match (catalog, cx.cfg.routing.registry.as_ref()) {
                (Some(d), Some(reg)) => {
                    match qualified_evidence(&cx.cfg.routing, reg, &EvidenceKey::live(p, &d)) {
                        Ok(rec) => (Some(rec), "qualified".to_string()),
                        Err(why) => (None, why),
                    }
                }
                _ => (None, "no evidence registry".to_string()),
            }
        }
    };
    let c = ChooseInputs {
        ladder: &lad.models,
        pool,
        features: &features,
        input_tokens,
        output_cap,
        prices: &cx.cfg.budget.prices,
        // A confirmed cache read is only known after a dispatch; never assumed here.
        cache: CacheState::Conservative,
        allowance_micros: cx.ledger.spend.available_cost(),
        evidence,
        min_tasks: lad.min_tasks,
        pinned: pinned(cx),
    };
    let pick = |m: &str| {
        pool.iter()
            .filter(|p| p.id == m)
            .cloned()
            .collect::<Vec<_>>()
    };
    let mut st = cx.ladder.borrow_mut();
    let cur = st
        .current
        .clone()
        .filter(|c| pool.iter().any(|p| &p.id == c));
    let (pool_out, why, action): (Vec<ModelPlan>, String, &str) = match (st.pending.take(), cur) {
        (Some(NextAction::Escalate), Some(cur)) => match next_rung(&c, &cur) {
            Ok((m, e)) => {
                st.escalations += 1;
                st.current = Some(m.clone());
                (
                    pick(&m),
                    format!(
                        "escalation {} to `{m}` (attempt bound {:?} micros)",
                        st.escalations, e.billed_micros
                    ),
                    "escalate",
                )
            }
            Err(w) => (
                pick(&cur),
                format!("no affordable rung ({w}); same model, failure feedback changed"),
                "retry_changed_input",
            ),
        },
        (Some(a), Some(cur)) => (
            pick(&cur),
            format!("{}: model unchanged", a.as_str()),
            a.as_str(),
        ),
        // A sticky model: later routes of the same attempt do not move it.
        (None, Some(cur)) => (
            pick(&cur),
            "model unchanged since the last route".into(),
            "sticky",
        ),
        _ => {
            let ch = choose_start(&c);
            let mut json = ch.to_json();
            json["qualification"] = json!(qualification);
            let (p, w) = match &ch.model {
                Some(m) => (pick(m), ch.reason.clone()),
                None if !ch.eligible.is_empty() => (
                    pool.iter()
                        .filter(|p| ch.eligible.contains(&p.id))
                        .cloned()
                        .collect(),
                    format!(
                        "{}; pool limited to priced, capable ladder models",
                        ch.reason
                    ),
                ),
                None => (pool.to_vec(), ch.reason.clone()),
            };
            if let Some(m) = &ch.model {
                st.current = Some(m.clone());
            }
            let out = json!({"action": "start", "reason": w, "choice": json,
                    "router": "bypassed: its expected benefit is unknown", "escalations": st.escalations});
            st.log.push(out.clone());
            return Some(Applied { pool: p, json: out });
        }
    };
    let out = json!({"action": action, "reason": why, "escalations": st.escalations,
        "max_escalations": lad.max_escalations,
        "router": "bypassed: its expected benefit is unknown",
        "reservation": "the next dispatch is reserved against the remaining task cost before it is sent"});
    st.log.push(out.clone());
    Some(Applied {
        pool: pool_out,
        json: out,
    })
}

/// Classify the newest known failure and decide what the next route may do.
pub(super) fn observe(cx: &Ctx, feedback: &[Value], failures: usize) {
    if !active(cx) {
        return;
    }
    let mut st = cx.ladder.borrow_mut();
    if failures <= st.seen_failures {
        return;
    }
    st.seen_failures = failures;
    let Some(last) = feedback.last() else { return };
    let s = |k: &str| last[k].as_str().unwrap_or("");
    let class = classify_failure(s("stage"), s("code"), s("message"));
    let lad = &cx.cfg.routing.ladders[&cx.cfg.task.family];
    let on_ladder = st
        .current
        .as_ref()
        .and_then(|c| lad.models.iter().position(|m| m == c))
        .is_some_and(|i| i + 1 < lad.models.len());
    let action = next_action(
        &class,
        &ActionContext {
            escalations_used: st.escalations,
            max_escalations: lad.max_escalations,
            pinned: pinned(cx),
            can_expand_context: cx.cfg.context_target.is_some(),
            can_grow_cap: cx.cfg.budget.generation.length_retry_cap.is_some(),
            next_rung_ok: on_ladder,
            evidence_justifies_retry: false,
            // The failure feedback is itself a relevant new input of the next request.
            input_changed: true,
        },
    );
    st.log
        .push(json!({"failure": format!("{class:?}"), "next": action.as_str(),
                     "detail": match &action { NextAction::Stop(w) => json!(w), _ => Value::Null }}));
    st.pending = Some(action);
}
