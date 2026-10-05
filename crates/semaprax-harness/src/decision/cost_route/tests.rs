use super::*;
use crate::decision::evidence::{EvidenceKey, MatchedBudget, RetryOwner};
use crate::decision::route::{Confidentiality, LatencyClass, TaskFamily};
use crate::receipt::{PriceRecord, Pricing};
use std::collections::BTreeSet;

fn plan(id: &str, ctx: u64) -> ModelPlan {
    ModelPlan {
        id: id.into(),
        destination: Destination::Remote {
            origin: "https://x.example".into(),
        },
        structured_output: true,
        tools: false,
        max_context: ctx,
        est_cost_micros: 1,
        est_latency_ms: 100,
        strength_rank: 1,
        descriptor: Default::default(),
    }
}

fn rates(input: u64, output: u64, read: Option<u64>, write: Option<u64>) -> PriceRecord {
    PriceRecord {
        version: "v1".into(),
        pricing: Pricing::Rates {
            input: Some(input),
            cache_read: read,
            cache_write: write,
            cache_write_1h: None,
            output: Some(output),
        },
    }
}

fn features() -> TaskFeatures {
    TaskFeatures {
        task_family: TaskFamily::LocalizedDebug,
        estimated_context_tokens: 1000,
        requires_structured_output: true,
        requires_tools: false,
        confidentiality: Confidentiality::Project,
        latency_class: LatencyClass::Batch,
    }
}

fn outcome(model: &str, item: usize, done: bool, attempts: u32, cost: Option<u64>) -> Outcome {
    Outcome {
        item: format!("t{item}"),
        arm: "a".into(),
        model: model.into(),
        origin: Origin::Real,
        verified_by: if done { "oracle".into() } else { String::new() },
        completed: done,
        regressions: 0,
        attempts,
        cost_micros: cost,
        latency_ms: None,
        router_cost_micros: 0,
        context_cost_micros: 0,
        retry_owner: RetryOwner::Host,
    }
}

fn record(outcomes: Vec<Outcome>) -> EvidenceRecord {
    EvidenceRecord {
        key: EvidenceKey {
            task: "t".into(),
            provider_id: "p".into(),
            weights_digest: "w".into(),
            catalog_digest: "c".into(),
            normalization: "n".into(),
            distribution: "d".into(),
        },
        budget: MatchedBudget {
            max_cost_micros: 1_000_000,
            max_attempts: 3,
        },
        eval_items: BTreeSet::new(),
        trained_on: BTreeSet::new(),
        outcomes,
        calibration: None,
    }
}

fn run<'a>(
    ladder: &'a [String],
    pool: &'a [ModelPlan],
    f: &'a TaskFeatures,
    prices: &'a PriceBook,
    ev: Option<&'a EvidenceRecord>,
) -> ChooseInputs<'a> {
    ChooseInputs {
        ladder,
        pool,
        features: f,
        input_tokens: 2000,
        output_cap: 500,
        prices,
        cache: CacheState::Conservative,
        allowance_micros: None,
        evidence: ev,
        min_tasks: 5,
        pinned: false,
    }
}

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

fn prices2() -> PriceBook {
    PriceBook::default()
        .with("cheap", rates(1, 1, None, None))
        .with("mid", rates(2, 2, None, None))
}

#[test]
fn cheap_repeatedly_failing_strategy_loses_to_dearer_lower_total_cost() {
    // cheap: 10 tasks, 2 accepted, 10 micros each trial => 100 / 2 = 50 per accepted.
    let mut o: Vec<Outcome> = (0..10)
        .map(|i| outcome("cheap", i, i < 2, 3, Some(10)))
        .collect();
    // mid: 10 tasks, 9 accepted, 14 micros each => 140 / 9 ~ 15.6 per accepted.
    o.extend((0..10).map(|i| outcome("mid", i, i < 9, 1, Some(14))));
    let rec = record(o);
    let (l, p, f, pr) = (
        names(&["cheap", "mid"]),
        vec![plan("cheap", 9000), plan("mid", 9000)],
        features(),
        prices2(),
    );
    let c = choose_start(&run(&l, &p, &f, &pr, Some(&rec)));
    assert_eq!(c.model.as_deref(), Some("mid"), "{c:?}");
    let s = c.stats.iter().find(|s| s.model == "mid").unwrap();
    assert_eq!((s.first_try, s.recovered), (9, 0));
}

#[test]
fn zero_accepted_or_incomplete_pricing_is_never_a_cheap_winner() {
    let mut o: Vec<Outcome> = (0..6)
        .map(|i| outcome("cheap", i, false, 3, Some(0)))
        .collect();
    o.extend((0..6).map(|i| outcome("mid", i, i < 3, 1, if i == 5 { None } else { Some(9) })));
    let rec = record(o);
    let (l, p, f, pr) = (
        names(&["cheap", "mid"]),
        vec![plan("cheap", 9000), plan("mid", 9000)],
        features(),
        prices2(),
    );
    let c = choose_start(&run(&l, &p, &f, &pr, Some(&rec)));
    assert_eq!(c.model, None);
    assert!(c.reason.contains("existing rules"));
    assert_eq!(c.eligible, names(&["cheap", "mid"]));
}

#[test]
fn unknown_price_stale_evidence_and_unsupported_features_block_cheapest() {
    let rec = record(
        (0..6)
            .map(|i| outcome("free", i, true, 1, Some(1)))
            .collect(),
    );
    let pr = PriceBook::default().with("mid", rates(2, 2, None, None));
    let mut unstructured = plan("mid", 9000);
    unstructured.structured_output = false;
    let (l, f) = (names(&["free", "mid"]), features());
    // "free" has great evidence but no price: excluded, never the winner.
    let p = vec![plan("free", 9000), plan("mid", 9000)];
    let c = choose_start(&run(&l, &p, &f, &pr, Some(&rec)));
    assert_eq!(c.model, None);
    assert!(c
        .excluded
        .iter()
        .any(|e| e["model"] == "free" && e["reason"] == "unknown price"));
    assert_eq!(c.eligible, names(&["mid"]));
    // Stale evidence (no record for the live key) leaves the rules policy.
    let p = vec![plan("mid", 9000)];
    let c = choose_start(&run(&l, &p, &f, &pr, None));
    assert_eq!(c.model, None);
    assert!(c
        .excluded
        .iter()
        .any(|e| e["reason"].as_str().unwrap().contains("no fresh evidence")));
    // Unsupported feature / small context are excluded before any cost comparison.
    let p = vec![unstructured, plan("free", 10)];
    let c = choose_start(&run(&l, &p, &f, &pr, Some(&rec)));
    assert!(c.eligible.is_empty());
    let why: Vec<_> = c
        .excluded
        .iter()
        .map(|e| e["reason"].as_str().unwrap().to_string())
        .collect();
    assert!(why.contains(&"lacks structured output".to_string()));
}

#[test]
fn pin_and_allowance_are_not_overridden() {
    let rec = record(
        (0..6)
            .map(|i| outcome("mid", i, true, 1, Some(5)))
            .collect(),
    );
    let (l, p, f, pr) = (
        names(&["mid"]),
        vec![plan("mid", 9000)],
        features(),
        prices2(),
    );
    let mut c = run(&l, &p, &f, &pr, Some(&rec));
    c.pinned = true;
    let got = choose_start(&c);
    assert!(got.model.is_none() && got.reason.contains("pin"));
    c.pinned = false;
    c.allowance_micros = Some(0);
    let got = choose_start(&c);
    assert!(got.model.is_none() && got.eligible.is_empty());
    assert!(got.excluded[0]["reason"]
        .as_str()
        .unwrap()
        .contains("remaining task cost"));
}

#[test]
fn cache_state_changes_the_estimate_without_discounting_context() {
    let pr = PriceBook::default().with(
        "m",
        rates(1_000_000, 1_000_000, Some(100_000), Some(1_250_000)),
    );
    let p = plan("m", 100_000);
    let est = |c| attempt_estimate(&pr, &p, 10_000, 1_000, c);
    let cons = est(CacheState::Conservative);
    let read = est(CacheState::ConfirmedRead { read_tokens: 8_000 });
    let miss = est(CacheState::Miss {
        write_tokens: 8_000,
    });
    // Conservative prices every input token at the dearest category (write 1250).
    assert_eq!(cons.billed_micros, Some(10_000 * 125 / 100 + 1_000));
    assert_eq!(read.billed_micros, Some(2_000 + 800 + 1_000));
    assert_eq!(miss.billed_micros, Some(2_000 + 10_000 + 1_000));
    assert!(read.billed_micros < miss.billed_micros && miss.billed_micros <= cons.billed_micros);
    assert_eq!(read.basis, "confirmed_cache_read");
    // Capacity is untouched by a confirmed read.
    assert_eq!(read.context_tokens, cons.context_tokens);
    // An unpriced cache read is never assumed cheaper.
    let pr2 = PriceBook::default().with("m", rates(1_000_000, 1_000_000, None, None));
    let e = attempt_estimate(
        &pr2,
        &p,
        10_000,
        1_000,
        CacheState::ConfirmedRead { read_tokens: 8_000 },
    );
    assert_eq!(e.basis, "cache_category_unpriced");
}

#[test]
fn local_non_billed_is_known_zero_and_unpriced_is_unknown() {
    let pr = PriceBook::default().with(
        "loc",
        PriceRecord {
            version: "v".into(),
            pricing: Pricing::NonBilled,
        },
    );
    let mut local = plan("loc", 9000);
    local.destination = Destination::Local;
    local.est_latency_ms = 5000;
    let e = attempt_estimate(&pr, &local, 100, 10, CacheState::Conservative);
    assert_eq!(
        (e.billed_micros, e.local, e.latency_ms),
        (Some(0), true, 5000)
    );
    let e = attempt_estimate(&pr, &plan("other", 9000), 100, 10, CacheState::Conservative);
    assert_eq!(e.billed_micros, None);
}

fn ctx() -> ActionContext {
    ActionContext {
        escalations_used: 0,
        max_escalations: 1,
        pinned: false,
        can_expand_context: true,
        can_grow_cap: true,
        next_rung_ok: true,
        evidence_justifies_retry: false,
        input_changed: true,
    }
}

#[test]
fn known_failure_escalates_at_most_the_configured_bound() {
    let rej = FailureClass::Rejected;
    assert_eq!(next_action(&rej, &ctx()), NextAction::Escalate);
    let spent = ActionContext {
        escalations_used: 1,
        ..ctx()
    };
    assert_eq!(next_action(&rej, &spent), NextAction::RetryChangedInput);
    let none = ActionContext {
        escalations_used: 1,
        input_changed: false,
        ..ctx()
    };
    assert!(matches!(next_action(&rej, &none), NextAction::Stop(_)));
    // A pin or an unaffordable rung can never be escalated past.
    assert_ne!(
        next_action(
            &rej,
            &ActionContext {
                pinned: true,
                ..ctx()
            }
        ),
        NextAction::Escalate
    );
    assert_ne!(
        next_action(
            &rej,
            &ActionContext {
                next_rung_ok: false,
                ..ctx()
            }
        ),
        NextAction::Escalate
    );
}

#[test]
fn uncertain_auth_and_missing_dependency_never_move_models() {
    let a = ctx();
    for (stage, code, msg) in [
        (
            "proposal",
            "SPX-HPD072",
            "uncertain: a model generation began",
        ),
        ("proposal", "SPX-HPD001", "401 not authorized"),
        ("checks", "SPX-HPD050", "toolchain missing: cargo"),
    ] {
        let c = classify_failure(stage, code, msg);
        assert!(matches!(next_action(&c, &a), NextAction::Stop(_)), "{c:?}");
    }
}

#[test]
fn changed_context_retry_is_distinct_from_escalation() {
    let c = classify_failure("preview", "SPX-HPD050", "unresolved name `foo`");
    assert!(matches!(c, FailureClass::MissingContext(_)));
    assert_eq!(next_action(&c, &ctx()), NextAction::ExpandContext);
    assert_ne!(
        NextAction::ExpandContext.as_str(),
        NextAction::Escalate.as_str()
    );
    let t = classify_failure("proposal", "SPX-HPD030", "reply length-limited");
    assert_eq!(next_action(&t, &ctx()), NextAction::LargerOutputCap);
}

#[test]
fn router_only_pays_when_known_benefit_exceeds_known_cost() {
    assert!(router_pays(Some(10), Some(3)));
    assert!(!router_pays(Some(3), Some(3)));
    assert!(!router_pays(None, Some(1)));
    assert!(!router_pays(Some(9), None));
}

#[test]
fn next_rung_skips_filtered_rungs() {
    let pr = prices2();
    let (l, f) = (names(&["cheap", "mid", "big"]), features());
    let p = vec![plan("cheap", 9000), plan("mid", 9000), plan("big", 9000)];
    let c = run(&l, &p, &f, &pr, None);
    // `big` is unpriced, so from `cheap` the next rung is `mid`; past `mid` none.
    assert_eq!(next_rung(&c, "cheap").unwrap().0, "mid");
    assert!(next_rung(&c, "mid").unwrap_err().contains("unknown price"));
}
