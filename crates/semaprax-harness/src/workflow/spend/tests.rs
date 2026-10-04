use super::*;
use crate::receipt::{GenerationControls, PriceRecord};

fn rates(input: u64, read: u64, write: Option<u64>, output: u64) -> PriceRecord {
    PriceRecord {
        version: "synthetic-v1".into(),
        pricing: Pricing::Rates {
            input: Some(input),
            cache_read: Some(read),
            cache_write: write,
            cache_write_1h: None,
            output: Some(output),
        },
    }
}

fn book() -> PriceBook {
    PriceBook::default()
        .with(
            "paid-",
            rates(1_000_000, 100_000, Some(2_000_000), 4_000_000),
        )
        .with(
            "local-",
            PriceRecord {
                version: "local-v1".into(),
                pricing: Pricing::NonBilled,
            },
        )
}

fn rec(id: &str, tokens: u64, cost: u64, billing: Billing) -> SpendRecord {
    SpendRecord {
        id: id.into(),
        kind: "generation".into(),
        label: "generate".into(),
        model: "paid-a".into(),
        reserved_tokens: tokens,
        reserved_cost: cost,
        billing,
        state: SpendState::Reserved,
        actual_cost: None,
        settled_tokens: None,
        breach: None,
        basis: None,
        persist: true,
        restored: false,
    }
}

fn priced() -> Billing {
    Billing::Priced("synthetic-v1".into())
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("hp-tc03-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn receipt(payload: Value) -> ProposalReceipt {
    ProposalReceipt::from_result(
        &GenerationControls {
            max_output_tokens: Some(100),
            ..Default::default()
        },
        &json!({ "receipt": payload }),
    )
}

#[test]
fn tc03_bound_covers_an_unconfirmed_cache_hit_at_the_write_rate_and_gateway_retries() {
    // 1000 input tokens at the dearest input rate (cache write, 2/token) + 100 output at 4/token.
    let b = call_bound(&book(), "paid-a", 1000, 100, 1);
    assert_eq!(b.micros, Some(2400));
    assert_eq!(b.billing, priced());
    // One disclosed gateway retry may bill the whole call again.
    assert_eq!(
        call_bound(&book(), "paid-a", 1000, 100, 2).micros,
        Some(4800)
    );
    // Non-billed is explicit and distinct from unpriced.
    let nb = call_bound(&book(), "local-x", 1000, 100, 1);
    assert_eq!(
        (nb.micros, nb.billing),
        (Some(0), Billing::NonBilled("local-v1".into()))
    );
    let un = call_bound(&book(), "other", 1000, 100, 1);
    assert_eq!(un.micros, None);
    assert!(matches!(un.billing, Billing::Unpriced(_)));
    // A missing output price is not a bound.
    let no_out = PriceBook::default().with(
        "p",
        PriceRecord {
            version: "v".into(),
            pricing: Pricing::Rates {
                input: Some(1),
                cache_read: None,
                cache_write: None,
                cache_write_1h: None,
                output: None,
            },
        },
    );
    assert!(matches!(
        call_bound(&no_out, "p", 1, 1, 1).billing,
        Billing::Unpriced(_)
    ));
    // Overflow is unpriced, never wrapped.
    assert!(matches!(
        call_bound(&book(), "paid-a", u64::MAX, u64::MAX, u64::MAX).billing,
        Billing::Unpriced(_)
    ));
}

#[test]
fn tc03_cache_hit_miss_write_and_output_limit_receipts_settle_within_the_bound() {
    let prices = book();
    let bound = call_bound(&prices, "paid-a", 1000, 100, 1).micros.unwrap();
    let r = rec("a", 1100, bound, priced());
    let settle = |p: Value| {
        let rc = receipt(p);
        let est = prices.estimate("paid-a", &rc.usage);
        settle_generation(&r, &rc, &est, true, 1000, 100)
    };
    // Cache miss with a full cache write: 1000 written at 2 + 50 out at 4 = 2200.
    let miss = settle(
        json!({"protocol": "anthropic_messages", "finish_reason": "end_turn",
        "usage": {"input_tokens": 0, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 1000, "output_tokens": 50}}),
    );
    assert_eq!(
        (miss.state, miss.actual_cost, miss.breach.clone()),
        (SpendState::Settled, Some(2200), None)
    );
    assert_eq!(
        miss.settled_tokens,
        Some(1050),
        "unused output headroom returns"
    );
    // Cache hit: 1000 read at 0.1 + 50 out = 300.
    let hit = settle(
        json!({"protocol": "anthropic_messages", "finish_reason": "end_turn",
        "usage": {"input_tokens": 0, "cache_read_input_tokens": 1000, "cache_creation_input_tokens": 0, "output_tokens": 50}}),
    );
    assert_eq!(hit.actual_cost, Some(300));
    // Output-limit exhaustion is a known terminal outcome: settled at the cap.
    let cut = settle(
        json!({"protocol": "chat_completions", "finish_reason": "length",
        "usage": {"prompt_tokens": 1000, "prompt_tokens_details": {"cached_tokens": 0}, "completion_tokens": 100}}),
    );
    assert_eq!(
        (cut.state, cut.actual_cost, cut.settled_tokens),
        (SpendState::Settled, Some(1400), Some(1100))
    );
    assert!(cut.breach.is_none());
    // Gateway-owned retry disclosed only by the charge: above the bound is a breach.
    let retried = settle(
        json!({"protocol": "responses", "finish_reason": "completed",
        "provider_cost_micros": bound * 2, "usage": {"input_tokens": 1000, "output_tokens": 50}}),
    );
    assert!(retried.breach.is_some(), "{retried:?}");
    assert_eq!(retried.actual_cost, Some(bound * 2));
    // With the retry disclosed the bound covers it.
    let r2 = rec(
        "b",
        2200,
        call_bound(&prices, "paid-a", 1000, 100, 2).micros.unwrap(),
        priced(),
    );
    let rc = receipt(
        json!({"protocol": "responses", "finish_reason": "completed",
        "provider_cost_micros": bound * 2, "usage": {"input_tokens": 1000, "output_tokens": 50}}),
    );
    let s = settle_generation(
        &r2,
        &rc,
        &prices.estimate("paid-a", &rc.usage),
        true,
        1000,
        100,
    );
    assert!(s.breach.is_none(), "{s:?}");
    // Unknown outcome or unknown cost stays reserved.
    let rc = receipt(json!({"protocol": "responses"}));
    let est = prices.estimate("paid-a", &rc.usage);
    assert_eq!(
        settle_generation(&r, &rc, &est, false, 1000, 100).state,
        SpendState::Uncertain
    );
    assert_eq!(
        settle_generation(&r, &rc, &est, true, 1000, 100).state,
        SpendState::Uncertain
    );
}

#[test]
fn tc03_admission_uses_the_minimum_limit_and_strict_mode_refuses_unpriced_work() {
    let mut b = SpendBook {
        limits: Limits {
            task_tokens: Some(1000),
            host_tokens: Some(500),
            session_tokens: Some(800),
            task_cost: Some(100),
            host_cost: Some(50),
            ..Default::default()
        },
        ..Default::default()
    };
    assert_eq!(b.limits.token_limit(), Some(500));
    assert_eq!(b.limits.cost_limit(), Some(50));
    assert!(b.check(&rec("a", 500, 50, priced())).is_ok());
    assert_eq!(
        b.check(&rec("a", 501, 0, priced())).unwrap_err().code,
        "SPX-HPD101"
    );
    assert_eq!(
        b.check(&rec("a", 1, 51, priced())).unwrap_err().code,
        "SPX-HPD101"
    );
    b.limits.host_tokens = None;
    // The session bound is the smallest now and refuses as a session bound.
    assert_eq!(
        b.check(&rec("a", 801, 0, priced())).unwrap_err().code,
        "SPX-HPD111"
    );
    // Strict: unpriced billable work is refused; explicitly non-billed is not.
    b.limits.strict_monetary = true;
    assert!(b
        .check(&rec("a", 1, 0, Billing::Unpriced("unpriced_model".into())))
        .unwrap_err()
        .message
        .contains("strict monetary"));
    assert!(b
        .check(&rec("a", 1, 0, Billing::NonBilled("v".into())))
        .is_ok());
    // ...but non-billed still obeys the token bounds.
    assert!(b
        .check(&rec("a", 900, 0, Billing::NonBilled("v".into())))
        .is_err());
    // A breach blocks paid work only.
    b.breach = Some("x".into());
    assert!(b
        .check(&rec("a", 1, 0, priced()))
        .unwrap_err()
        .message
        .contains("breach"));
    assert!(b
        .check(&rec("a", 1, 0, Billing::NonBilled("v".into())))
        .is_ok());
}

#[test]
fn tc03_settled_headroom_returns_while_uncertain_and_reserved_stay_counted() {
    let d = dir("headroom");
    let mut j = Journal::open(&d, "l").unwrap();
    let mut b = SpendBook {
        limits: Limits {
            task_tokens: Some(2000),
            task_cost: Some(5000),
            ..Default::default()
        },
        ..Default::default()
    };
    b.reserve(&mut j, rec("g.1", 1100, 2400, priced())).unwrap();
    // A second reservation of the same size does not fit beside the first.
    assert!(b.check(&rec("g.2", 1100, 2400, priced())).is_err());
    let small = Settlement {
        state: SpendState::Settled,
        actual_cost: Some(300),
        settled_tokens: Some(850),
        breach: None,
        basis: Some("estimated_from_price_record".into()),
    };
    b.settle(&mut j, "g.1", small.clone()).unwrap();
    // The smaller completed response returned its headroom.
    assert_eq!((b.committed_tokens(), b.committed_cost()), (850, 300));
    b.reserve(&mut j, rec("g.2", 1100, 2400, priced())).unwrap();
    // An uncertain one keeps its reservation.
    b.settle(&mut j, "g.2", Settlement::uncertain("outcome_unknown"))
        .unwrap();
    assert_eq!((b.committed_tokens(), b.committed_cost()), (1950, 2700));
    assert!(b.check(&rec("g.3", 100, 0, priced())).is_err());
    // Settlement is idempotent; a contradictory second settlement fails closed.
    b.settle(&mut j, "g.1", small).unwrap();
    assert_eq!(j.records().len(), 4, "the repeat wrote nothing");
    let other = Settlement {
        state: SpendState::Settled,
        actual_cost: Some(1),
        settled_tokens: Some(1),
        breach: None,
        basis: None,
    };
    assert_eq!(
        b.settle(&mut j, "g.1", other).unwrap_err().code,
        "SPX-HPD070"
    );
    // Restoring the same journal reproduces the same account.
    let mut again = SpendBook::default();
    again.restore(&Journal::open(&d, "l").unwrap()).unwrap();
    assert_eq!(
        (again.committed_tokens(), again.committed_cost()),
        (1950, 2700)
    );
    assert!(again.records.iter().all(|r| r.restored));
    let _ = std::fs::remove_dir_all(&d);
}

fn restore_lines(tag: &str, lines: &[Value]) -> HarnessResult<SpendBook> {
    let d = dir(tag);
    std::fs::create_dir_all(&d).unwrap();
    let text: String = lines
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let mut v = v.clone();
            v["seq"] = json!(i + 1);
            format!("{}\n", crate::json::canonical(&v))
        })
        .collect();
    std::fs::write(d.join("l.journal.jsonl"), text).unwrap();
    let mut b = SpendBook::default();
    let out = Journal::open(&d, "l").and_then(|j| b.restore(&j));
    let _ = std::fs::remove_dir_all(&d);
    out.map(|_| b)
}

fn reserve_line(id: &str) -> Value {
    json!({"step": format!("spend.{id}"), "state": "reserve", "detail": rec(id, 1100, 2400, priced()).reserve_detail()})
}

fn settle_line(id: &str, cost: u64) -> Value {
    json!({"step": format!("spend.{id}"), "state": "settle",
           "detail": {"id": id, "actual_cost_micros": cost, "settled_tokens": 900, "breach": null, "basis": "provider_reported"}})
}

#[test]
fn tc03_crash_points_never_reset_release_or_double_count() {
    let begin = json!({"step": "generate", "state": "begin", "detail": {}});
    // Crash before dispatch (reservation only) and after dispatch (begin, no receipt):
    // the reservation stays outstanding, never reset to zero.
    for (tag, lines) in [
        ("pre", vec![reserve_line("g.1")]),
        ("mid", vec![reserve_line("g.1"), begin.clone()]),
    ] {
        let b = restore_lines(tag, &lines).unwrap();
        assert_eq!(b.records[0].state, SpendState::Reserved);
        assert_eq!((b.committed_tokens(), b.committed_cost()), (1100, 2400));
    }
    // After the receipt was settled but before the step completed: the actual counts once.
    let b = restore_lines(
        "post",
        &[reserve_line("g.1"), begin.clone(), settle_line("g.1", 700)],
    )
    .unwrap();
    assert_eq!(b.committed_cost(), 700);
    // A settlement replayed (written again during a crash) is not double-counted.
    let b = restore_lines(
        "dup",
        &[
            reserve_line("g.1"),
            settle_line("g.1", 700),
            settle_line("g.1", 700),
        ],
    )
    .unwrap();
    assert_eq!((b.records.len(), b.committed_cost()), (1, 700));
    // An uncertain record is never released by a resume.
    let unc = json!({"step": "spend.g.1", "state": "uncertain",
                     "detail": {"id": "g.1", "basis": "outcome_unknown"}});
    let b = restore_lines("unc", &[reserve_line("g.1"), unc.clone()]).unwrap();
    assert_eq!(b.committed_cost(), 2400);
    // ...but can be reconciled to a settlement later.
    let b = restore_lines("recon", &[reserve_line("g.1"), unc, settle_line("g.1", 10)]).unwrap();
    assert_eq!(b.committed_cost(), 10);
    // A crash in the middle of writing the settlement leaves a torn line: refused.
    let d = dir("torn");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("l.journal.jsonl"),
        format!(
            "{}\n{{\"detail\":{{\"id\":\"g.1\",\"actual",
            crate::json::canonical(&reserve_line("g.1"))
        ),
    )
    .unwrap();
    assert_eq!(Journal::open(&d, "l").err().unwrap().code, "SPX-HPD070");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn tc03_malformed_or_contradictory_accounting_fails_closed() {
    let mut no_tokens = reserve_line("g.1");
    no_tokens["detail"]
        .as_object_mut()
        .unwrap()
        .remove("reserved_tokens");
    let mut wrong_id = reserve_line("g.1");
    wrong_id["detail"]["id"] = json!("g.2");
    let mut bad_billing = reserve_line("g.1");
    bad_billing["detail"]["billing"] = json!({"kind": "free"});
    let mut other = reserve_line("g.1");
    other["detail"]["reserved_cost_micros"] = json!(1);
    let unknown_state = json!({"step": "spend.g.1", "state": "forgotten", "detail": {"id": "g.1"}});
    let no_cost = json!({"step": "spend.g.1", "state": "settle", "detail": {"id": "g.1"}});
    let cases: Vec<(&str, Vec<Value>)> = vec![
        ("missing reserved tokens", vec![no_tokens]),
        ("id mismatch", vec![wrong_id]),
        ("bad billing", vec![bad_billing]),
        ("unknown state", vec![reserve_line("g.1"), unknown_state]),
        ("settle without reservation", vec![settle_line("g.1", 1)]),
        ("settle without cost", vec![reserve_line("g.1"), no_cost]),
        (
            "reserved twice differently",
            vec![reserve_line("g.1"), other],
        ),
        (
            "contradictory settlements",
            vec![
                reserve_line("g.1"),
                settle_line("g.1", 1),
                settle_line("g.1", 2),
            ],
        ),
        (
            "release after settlement",
            vec![
                reserve_line("g.1"),
                settle_line("g.1", 1),
                json!({"step": "spend.g.1", "state": "release", "detail": {"id": "g.1"}}),
            ],
        ),
    ];
    for (i, (what, lines)) in cases.into_iter().enumerate() {
        let e = restore_lines(&format!("bad{i}"), &lines)
            .err()
            .unwrap_or_else(|| panic!("{what}: accepted"));
        assert_eq!(e.code, "SPX-HPD070", "{what}");
    }
}

#[test]
fn tc03_one_local_writer_per_lineage_and_a_dead_holder_is_taken_over() {
    let d = dir("lock");
    let held = WriterLock::acquire(&d, "l").unwrap();
    assert_eq!(
        WriterLock::acquire(&d, "l").err().unwrap().code,
        "SPX-HPD070"
    );
    drop(held);
    let again = WriterLock::acquire(&d, "l").unwrap();
    drop(again);
    // A lock left by a process that no longer exists is stale.
    std::fs::write(d.join("l.journal.lock"), "2147483646").unwrap();
    assert!(WriterLock::acquire(&d, "l").is_ok());
    let _ = std::fs::remove_dir_all(&d);
}
