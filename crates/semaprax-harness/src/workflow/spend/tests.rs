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
        unresolved_attempts: 0,
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
        unresolved_attempts: 0,
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
        unresolved_attempts: 0,
    };
    assert_eq!(
        b.settle(&mut j, "g.1", other).unwrap_err().code,
        "SPX-HPD070"
    );
    // Restoring the same journal reproduces the same account.
    // (The single-writer lock is released first: reopening needs the sole writer.)
    drop(j);
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

// ---- MN-02: one rounding contract for reservations and estimates ----

fn full_rates(
    input: u64,
    read: Option<u64>,
    write: Option<u64>,
    write_1h: Option<u64>,
    output: u64,
) -> PriceBook {
    PriceBook::default().with(
        "syn-",
        PriceRecord {
            version: "syn-v1".into(),
            pricing: Pricing::Rates {
                input: Some(input),
                cache_read: read,
                cache_write: write,
                cache_write_1h: write_1h,
                output: Some(output),
            },
        },
    )
}

fn usage(
    uncached: u64,
    read: u64,
    write: u64,
    write_1h: u64,
    output: u64,
) -> crate::receipt::Usage {
    crate::receipt::Usage {
        input_total: Some(uncached + read + write),
        uncached_input: Some(uncached),
        cache_read: Some(read),
        cache_write: Some(write),
        cache_write_1h: Some(write_1h),
        output: Some(output),
        reasoning: None,
    }
}

fn anthropic(uncached: u64, read: u64, write: u64, output: u64) -> ProposalReceipt {
    receipt(
        json!({"protocol": "anthropic_messages", "finish_reason": "end_turn",
        "usage": {"input_tokens": uncached, "cache_read_input_tokens": read,
                  "cache_creation_input_tokens": write, "output_tokens": output}}),
    )
}

#[test]
fn mn02_witness_in_bound_usage_settles_without_breach_and_paid_work_continues() {
    // The issue's witness: 0.3 micro-units per token for input and output,
    // 107 input tokens, a 4096 output cap fully used, no cache, one dispatch.
    let prices = full_rates(300_000, None, None, None, 300_000);
    let bound = call_bound(&prices, "syn-a", 107, 4096, 1);
    assert_eq!(bound.billing, Billing::Priced("syn-v1".into()));
    let est = prices.estimate("syn-a", &usage(107, 0, 0, 0, 4096));
    // ceil(32.1) + ceil(1228.8): the per-category estimate.
    assert_eq!(est.micros, Some(1262));
    assert_eq!(est.basis, "estimated_from_price_record");
    assert!(bound.micros.unwrap() >= 1262, "{bound:?}");
    let d = dir("mn02-witness");
    let mut j = Journal::open(&d, "l").unwrap();
    let mut b = SpendBook {
        limits: Limits {
            task_cost: Some(100_000),
            ..Default::default()
        },
        ..Default::default()
    };
    let r = rec(
        "g.1",
        107 + 4096,
        bound.micros.unwrap(),
        bound.billing.clone(),
    );
    b.reserve(&mut j, r.clone()).unwrap();
    let rc = receipt(
        json!({"protocol": "anthropic_messages", "finish_reason": "max_tokens",
        "usage": {"input_tokens": 107, "cache_read_input_tokens": 0,
                  "cache_creation_input_tokens": 0, "output_tokens": 4096}}),
    );
    let s = settle_generation(
        &r,
        &rc,
        &prices.estimate("syn-a", &rc.usage),
        true,
        107,
        4096,
    );
    assert_eq!(s.breach, None, "{s:?}");
    assert_eq!(s.actual_cost, Some(1262));
    // The local figure keeps its estimate basis; it is not a vendor invoice.
    assert_eq!(s.basis.as_deref(), Some("estimated_from_price_record"));
    b.settle(&mut j, "g.1", s).unwrap();
    assert_eq!(b.breach, None);
    // A subsequent legitimate paid admission remains possible.
    b.reserve(&mut j, rec("g.2", 10, 10, bound.billing))
        .unwrap();
    drop(j);
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn mn02_cache_partition_and_one_hour_write_witnesses_stay_within_the_bound() {
    // Every input category priced at 0.3/token: three one-token partitions
    // round to 1 each (3), while the combined input rounds to ceil(0.9) = 1.
    let prices = full_rates(
        300_000,
        Some(300_000),
        Some(300_000),
        Some(300_000),
        300_000,
    );
    let est = prices.estimate("syn-a", &usage(1, 1, 2, 1, 0));
    assert_eq!(est.micros, Some(4));
    let bound = call_bound(&prices, "syn-a", 4, 0, 1).micros.unwrap();
    assert!(est.micros.unwrap() <= bound, "{est:?} > {bound}");
    // Mixed cache read/write rates below the dearest input rate.
    let prices = full_rates(
        300_000,
        Some(30_000),
        Some(375_000),
        Some(600_000),
        1_500_000,
    );
    // 7 cache writes, 4 of them at the one-hour tier: 1 + 1 + 2 + 3 + 17.
    let est = prices.estimate("syn-a", &usage(3, 5, 7, 4, 11));
    assert_eq!(est.micros, Some(24));
    let bound = call_bound(&prices, "syn-a", 15, 11, 1).micros.unwrap();
    assert!(est.micros.unwrap() <= bound, "{est:?} > {bound}");
    // A receipt's own unsplit cache write (no one-hour tier priced): the
    // three partitions round to 1 + 1 + 1 against a combined ceil(0.9).
    let prices = full_rates(300_000, Some(300_000), Some(300_000), None, 300_000);
    let rc = anthropic(1, 1, 1, 0);
    let est = prices.estimate("syn-a", &rc.usage);
    assert_eq!(est.micros, Some(3));
    let bound = call_bound(&prices, "syn-a", 3, 0, 1).micros.unwrap();
    assert!(est.micros.unwrap() <= bound, "{est:?} > {bound}");
    // Integral per-token prices keep their exact (unchanged) bound.
    assert_eq!(
        call_bound(&book(), "paid-a", 1000, 100, 1).micros,
        Some(2400)
    );
}

/// Deterministic xorshift for the property sweep (no extra dependency).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
    fn rate(&mut self) -> u64 {
        match self.below(5) {
            0 => 0,
            1 => self.below(1_000_000), // fractional below one micro-unit
            2 => (1 + self.below(20)) * 1_000_000, // integral
            3 => self.below(30_000_000), // arbitrary
            _ => 1 + self.below(999) * 1_001, // fractional, odd steps
        }
    }
    /// Half large counts, half small ones where per-category rounding dominates.
    fn tokens(&mut self) -> u64 {
        if self.below(2) == 0 {
            self.below(5_000)
        } else {
            self.below(20)
        }
    }
    fn opt_rate(&mut self) -> Option<u64> {
        (self.below(4) != 0).then(|| self.rate())
    }
}

#[test]
fn mn02_property_estimate_of_in_bound_usage_never_exceeds_the_reservation() {
    let mut rng = Rng(0x9E37_79B9_7F4A_7C15);
    let (mut known, mut unknown) = (0u32, 0u32);
    for _ in 0..20_000 {
        let (input_rate, output_rate) = (rng.rate(), rng.rate());
        let (read, write, write_1h) = (rng.opt_rate(), rng.opt_rate(), rng.opt_rate());
        let prices = full_rates(input_rate, read, write, write_1h, output_rate);
        let input = rng.tokens();
        let cap = rng.tokens();
        let dispatches = 1 + rng.below(3);
        // Observed usage within the bound: total input and output at most
        // `dispatches` times the per-dispatch reservation, partitioned freely.
        // Half the cases use the whole bound: rounding gaps live at the edge.
        let tight = rng.below(2) == 0;
        let total_in = if tight {
            input * dispatches
        } else {
            rng.below(input * dispatches + 1)
        };
        let read_n = rng.below(total_in + 1);
        let write_n = rng.below(total_in - read_n + 1);
        let uncached = total_in - read_n - write_n;
        let write_1h_n = rng.below(write_n + 1);
        let out = if tight {
            cap * dispatches
        } else {
            rng.below(cap * dispatches + 1)
        };
        let u = usage(uncached, read_n, write_n, write_1h_n, out);
        let bound = call_bound(&prices, "syn-a", input, cap, dispatches);
        assert_eq!(bound.billing, Billing::Priced("syn-v1".into()));
        let est = prices.estimate("syn-a", &u);
        match est.micros {
            Some(e) => {
                known += 1;
                let b = bound.micros.unwrap();
                assert!(
                    e <= b,
                    "estimate {e} > bound {b}: rates in={input_rate} r={read:?} w={write:?} w1h={write_1h:?} out={output_rate}; \
                     input={input} cap={cap} d={dispatches} usage={u:?}"
                );
                assert_eq!(est.basis, "estimated_from_price_record");
            }
            None => {
                unknown += 1;
                // An unpriced non-zero category leaves the cost unknown, never zero.
                assert_ne!(est.basis, "estimated_from_price_record");
            }
        }
    }
    assert!(
        known > 5_000 && unknown > 500,
        "sweep coverage: {known} known, {unknown} unknown"
    );
}

#[test]
fn mn02_genuine_overcharge_and_output_overrun_still_breach_and_unknown_stays_conservative() {
    let prices = full_rates(300_000, None, None, None, 300_000);
    let bound = call_bound(&prices, "syn-a", 107, 4096, 1).micros.unwrap();
    let r = rec("g.1", 107 + 4096, bound, Billing::Priced("syn-v1".into()));
    // A provider-reported charge above the bound is a breach that blocks
    // further billable work.
    let rc = receipt(
        json!({"protocol": "responses", "finish_reason": "completed",
        "provider_cost_micros": bound + 1, "usage": {"input_tokens": 107, "output_tokens": 10}}),
    );
    let s = settle_generation(
        &r,
        &rc,
        &prices.estimate("syn-a", &rc.usage),
        true,
        107,
        4096,
    );
    assert!(
        s.breach
            .as_deref()
            .unwrap()
            .contains("above the declared bound"),
        "{s:?}"
    );
    assert_eq!(s.basis.as_deref(), Some("provider_reported"));
    let d = dir("mn02-breach");
    let mut j = Journal::open(&d, "l").unwrap();
    let mut b = SpendBook::default();
    b.reserve(&mut j, r.clone()).unwrap();
    b.settle(&mut j, "g.1", s).unwrap();
    assert_eq!(
        b.check(&rec("g.2", 1, 1, priced())).unwrap_err().code,
        "SPX-HPD101"
    );
    drop(j);
    let _ = std::fs::remove_dir_all(&d);
    // Output above the enforced cap breaches independently of the cost.
    let rc = receipt(
        json!({"protocol": "responses", "finish_reason": "completed",
        "provider_cost_micros": 100, "usage": {"input_tokens": 107, "output_tokens": 4097}}),
    );
    let s = settle_generation(
        &r,
        &rc,
        &prices.estimate("syn-a", &rc.usage),
        true,
        107,
        4096,
    );
    assert!(
        s.breach
            .as_deref()
            .unwrap()
            .contains("above the enforced cap"),
        "{s:?}"
    );
    // Unknown usage or price stays at the reservation.
    let rc = receipt(json!({"protocol": "responses"}));
    let s = settle_generation(
        &r,
        &rc,
        &prices.estimate("syn-a", &rc.usage),
        true,
        107,
        4096,
    );
    assert_eq!((s.state, s.actual_cost), (SpendState::Uncertain, None));
    // Overflow never wraps into a cheap admission: the reservation saturates
    // and strict mode still refuses it as unpriced.
    let huge = full_rates(
        u64::MAX,
        Some(u64::MAX),
        Some(u64::MAX),
        Some(u64::MAX),
        u64::MAX,
    );
    for (i, c, n) in [
        (u64::MAX, u64::MAX, u64::MAX),
        (u64::MAX, 0, 1),
        (1 << 40, 1 << 40, 3),
    ] {
        let o = call_bound(&huge, "syn-a", i, c, n);
        assert_eq!(o.micros, Some(u64::MAX), "{i} {c} {n}");
        assert!(matches!(o.billing, Billing::Unpriced(_)));
    }
    let capped = SpendBook {
        limits: Limits {
            task_cost: Some(1_000_000),
            ..Default::default()
        },
        ..Default::default()
    };
    let o = call_bound(&huge, "syn-a", 1 << 40, 1 << 40, 3);
    let over = rec("g.9", 1, o.micros.unwrap(), o.billing);
    assert_eq!(capped.check(&over).unwrap_err().code, "SPX-HPD101");
}

// ---- MN-03: settlement is committed in memory only after the durable append ----

fn fault_on_settle(j: &mut Journal, f: super::super::journal::Fault) {
    j.set_fault(Some(std::sync::Arc::new(move |st: &str| {
        (st == "settle").then_some(f)
    })));
}

fn ten() -> Settlement {
    Settlement {
        state: SpendState::Settled,
        actual_cost: Some(10),
        settled_tokens: Some(10),
        breach: None,
        basis: Some("provider_reported".into()),
        unresolved_attempts: 0,
    }
}

/// Reserve 100/100, then attempt a 10/10 settlement under `fault`.
fn faulted(tag: &str, fault: super::super::journal::Fault) -> (PathBuf, Journal, SpendBook) {
    let d = dir(tag);
    let mut j = Journal::open(&d, "l").unwrap();
    let mut b = SpendBook {
        limits: Limits {
            task_cost: Some(150),
            task_tokens: Some(150),
            ..Default::default()
        },
        ..Default::default()
    };
    b.reserve(&mut j, rec("g.1", 100, 100, priced())).unwrap();
    fault_on_settle(&mut j, fault);
    assert!(b.settle(&mut j, "g.1", ten()).is_err());
    // Nothing was acknowledged: the live book still counts the reservation
    // and releases no headroom.
    assert_eq!(b.record("g.1").unwrap().state, SpendState::Reserved);
    assert_eq!((b.committed_tokens(), b.committed_cost()), (100, 100));
    assert_eq!(b.available_cost(), Some(50));
    // The uncertain outcome refuses further admission until reconciled.
    assert_eq!(
        b.check(&rec("g.2", 40, 40, priced())).unwrap_err().code,
        "SPX-HPD070"
    );
    j.set_fault(None);
    // The identical retry refuses pending reconciliation; it never reports
    // false durable success.
    assert_eq!(
        b.settle(&mut j, "g.1", ten()).unwrap_err().code,
        "SPX-HPD070"
    );
    assert_eq!(b.record("g.1").unwrap().state, SpendState::Reserved);
    (d, j, b)
}

#[test]
fn mn03_write_fault_releases_no_headroom_and_reopen_agrees_with_the_live_book() {
    let (d, j, live) = faulted("mn03-write", super::super::journal::Fault::Write);
    assert_eq!(j.records().len(), 1, "only the reservation reached storage");
    drop(j);
    let mut j = Journal::open(&d, "l").unwrap();
    let mut fresh = SpendBook {
        limits: live.limits.clone(),
        ..Default::default()
    };
    fresh.restore(&j).unwrap();
    assert_eq!(
        (fresh.committed_tokens(), fresh.committed_cost()),
        (live.committed_tokens(), live.committed_cost())
    );
    assert_eq!(fresh.committed_cost(), 100);
    // After the reopen the same settlement persists and is then acknowledged.
    fresh.settle(&mut j, "g.1", ten()).unwrap();
    assert_eq!(fresh.committed_cost(), 10);
    // A duplicate durable settlement stays idempotent: nothing written twice.
    fresh.settle(&mut j, "g.1", ten()).unwrap();
    assert_eq!(j.records().len(), 2);
    drop(j);
    let mut again = SpendBook::default();
    again.restore(&Journal::open(&d, "l").unwrap()).unwrap();
    assert_eq!((again.committed_tokens(), again.committed_cost()), (10, 10));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn mn03_sync_and_torn_faults_stay_conservative_without_blind_rollback() {
    use super::super::journal::Fault;
    // Sync: the bytes may be visible. The live owner stays at the
    // reservation; a reopen reconciles from what actually reached storage
    // and the identical settlement is then an idempotent no-op.
    let (d, j, live) = faulted("mn03-sync", Fault::Sync);
    assert_eq!(live.committed_cost(), 100);
    drop(j);
    let mut j = Journal::open(&d, "l").unwrap();
    let mut fresh = SpendBook::default();
    fresh.restore(&j).unwrap();
    assert_eq!(fresh.committed_cost(), 10);
    let n = j.records().len();
    fresh.settle(&mut j, "g.1", ten()).unwrap();
    assert_eq!(j.records().len(), n, "no double charge");
    drop(j);
    let _ = std::fs::remove_dir_all(&d);
    // Torn: the half-written settlement fails the reopen closed rather than
    // resetting or releasing anything.
    let (d, j, live) = faulted("mn03-torn", Fault::Torn);
    assert_eq!(live.committed_cost(), 100);
    drop(j);
    assert_eq!(Journal::open(&d, "l").err().unwrap().code, "SPX-HPD070");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn mn03_a_failed_breach_or_release_append_does_not_publish_the_transition() {
    use super::super::journal::Fault;
    let d = dir("mn03-breach");
    let mut j = Journal::open(&d, "l").unwrap();
    let mut b = SpendBook::default();
    b.reserve(&mut j, rec("g.1", 100, 100, priced())).unwrap();
    j.set_fault(Some(std::sync::Arc::new(|_: &str| Some(Fault::Write))));
    let breach = Settlement {
        breach: Some("charged 200 micros above the declared bound of 100".into()),
        actual_cost: Some(200),
        ..ten()
    };
    assert!(b.settle(&mut j, "g.1", breach).is_err());
    // Neither the terminal state nor its cost was published, and the book
    // still refuses further work (pending reconciliation) rather than admitting.
    assert_eq!(b.record("g.1").unwrap().actual_cost, None);
    assert_eq!(b.committed_cost(), 100);
    assert!(b.check(&rec("g.2", 1, 1, priced())).is_err());
    assert!(b
        .settle(
            &mut j,
            "g.1",
            Settlement::released("refused_not_dispatched")
        )
        .is_err());
    assert_eq!(b.committed_cost(), 100);
    // Invalid transitions keep failing closed on a healthy book.
    drop(j);
    let mut j = Journal::open(&d, "l").unwrap();
    let mut fresh = SpendBook::default();
    fresh.restore(&j).unwrap();
    fresh.settle(&mut j, "g.1", ten()).unwrap();
    assert_eq!(
        fresh
            .settle(&mut j, "g.1", Settlement::released("late"))
            .unwrap_err()
            .code,
        "SPX-HPD070"
    );
    assert_eq!(
        fresh.settle(&mut j, "nope", ten()).unwrap_err().code,
        "SPX-HPD070"
    );
    drop(j);
    let _ = std::fs::remove_dir_all(&d);
}

// ---- DV-20: gateway retry reservations survive a final-attempt receipt ----

fn unit_prices() -> PriceBook {
    full_rates(1_000_000, None, None, None, 1_000_000)
}

/// A final-attempt receipt of 100 input / 50 output, plus `extra` members.
fn final_receipt(extra: Value) -> ProposalReceipt {
    let mut c = json!({"protocol": "anthropic_messages", "finish_reason": "end_turn",
        "usage": {"input_tokens": 100, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0, "output_tokens": 50}});
    for (k, v) in extra.as_object().unwrap() {
        c[k] = v.clone();
    }
    receipt(c)
}

fn settle_n(r: &SpendRecord, rc: &ProposalReceipt, dispatches: u64) -> Settlement {
    settle_generation_dispatches(
        r,
        rc,
        &unit_prices().estimate("syn-a", &rc.usage),
        true,
        100,
        50,
        dispatches,
    )
}

fn settle2(r: &SpendRecord, rc: &ProposalReceipt) -> Settlement {
    settle_n(r, rc, 2)
}

fn record_n(id: &str, dispatches: u64) -> SpendRecord {
    let bound = call_bound(&unit_prices(), "syn-a", 100, 50, dispatches);
    rec(id, 150 * dispatches, bound.micros.unwrap(), bound.billing)
}

fn figures(s: &Settlement) -> (Option<u64>, Option<u64>, u64) {
    (s.actual_cost, s.settled_tokens, s.unresolved_attempts)
}

#[test]
fn dv20_final_receipt_keeps_the_unresolved_dispatch_reserved_and_refuses_the_next_reservation() {
    let d = dir("dv20-450");
    let mut j = Journal::open(&d, "l").unwrap();
    let mut b = SpendBook {
        limits: Limits {
            task_tokens: Some(450),
            task_cost: Some(450),
            ..Default::default()
        },
        ..Default::default()
    };
    let r = record_n("g.1", 2);
    assert_eq!(r.reserved_cost, 300);
    b.reserve(&mut j, r.clone()).unwrap();
    let s = settle2(&r, &final_receipt(json!({})));
    assert_eq!(figures(&s), (Some(300), Some(300), 1));
    assert_eq!(s.breach, None);
    b.settle(&mut j, "g.1", s).unwrap();
    assert_eq!((b.committed_tokens(), b.committed_cost()), (300, 300));
    assert!(b.check(&record_n("g.2", 2)).is_err());
    assert_eq!(b.to_json()["unresolved_retry_attempts"], 1);
    assert_eq!(b.to_json()["unknown_spend_attempts"], 1);
    // Reopen: the unresolved portion is restored once, not doubled or dropped.
    drop(j);
    let mut again = SpendBook {
        limits: b.limits.clone(),
        ..Default::default()
    };
    again.restore(&Journal::open(&d, "l").unwrap()).unwrap();
    assert_eq!(
        (again.committed_tokens(), again.committed_cost()),
        (300, 300)
    );
    assert_eq!(again.records[0].unresolved_attempts, 1);
    assert!(again.check(&record_n("g.2", 2)).is_err());
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn dv20_zero_retry_and_proven_unused_retries_release_only_what_is_proven() {
    // One dispatch: unchanged behavior.
    let one = record_n("g.1", 1);
    let rc = final_receipt(json!({}));
    let s = settle_generation(
        &one,
        &rc,
        &unit_prices().estimate("syn-a", &rc.usage),
        true,
        100,
        50,
    );
    assert_eq!(figures(&s), (Some(150), Some(150), 0));
    // Two dispatches, the gateway proves the other was never dispatched.
    let rc = final_receipt(json!({"unused_attempts": 1}));
    let s = settle2(&record_n("g.1", 2), &rc);
    assert_eq!(figures(&s), (Some(150), Some(150), 0));
    // Three dispatches, one proven unused: the other stays retained.
    let s = settle_n(&record_n("g.1", 3), &rc, 3);
    assert_eq!(figures(&s), (Some(300), Some(300), 1));
    // Proof of more unused attempts than exist cannot release the final one.
    let rc = final_receipt(json!({"unused_attempts": 9}));
    let s = settle2(&record_n("g.1", 2), &rc);
    assert_eq!(figures(&s), (Some(150), Some(150), 0));
}

#[test]
fn dv20_aggregate_scopes_are_checked_separately_for_tokens_and_cost() {
    let r = record_n("g.1", 2);
    let agg = |extra: Value| {
        let mut c = json!({"protocol": "anthropic_messages", "finish_reason": "end_turn",
            "usage_scope": "aggregate", "usage": {"input_tokens": 200, "cache_read_input_tokens": 0, "cache_creation_input_tokens": 0, "output_tokens": 80}});
        for (k, v) in extra.as_object().unwrap() {
            c[k] = v.clone();
        }
        receipt(c)
    };
    // Aggregate usage covers both attempts: nothing remains unresolved.
    let s = settle2(&r, &agg(json!({})));
    assert_eq!(figures(&s), (Some(280), Some(280), 0));
    // An aggregate charge alone does not erase unknown retry tokens.
    let rc = final_receipt(json!({"provider_cost_micros": 280, "cost_scope": "aggregate"}));
    assert_eq!(figures(&settle2(&r, &rc)), (Some(280), Some(300), 1));
    // A final-attempt charge keeps the other dispatch's cost bound.
    let rc = final_receipt(json!({"provider_cost_micros": 140}));
    assert_eq!(settle2(&r, &rc).actual_cost, Some(290));
    // Aggregate usage with a final-attempt charge keeps the cost bound only.
    let s = settle2(&r, &agg(json!({"provider_cost_micros": 140})));
    assert_eq!(figures(&s), (Some(290), Some(280), 1));
}

#[test]
fn dv20_known_breach_handling_is_preserved_per_dispatch_and_in_aggregate() {
    let r = record_n("g.1", 2);
    let s = settle2(&r, &final_receipt(json!({"provider_cost_micros": 151})));
    assert!(s
        .breach
        .as_deref()
        .unwrap()
        .contains("above the declared bound of 150"));
    let agg = |c: u64| final_receipt(json!({"provider_cost_micros": c, "cost_scope": "aggregate"}));
    let s = settle2(&r, &agg(301));
    assert!(s
        .breach
        .as_deref()
        .unwrap()
        .contains("above the declared bound of 300"));
    assert_eq!(settle2(&r, &agg(300)).breach, None);
}

#[test]
fn dv20_receipt_coverage_parses_and_defaults_to_final_attempt() {
    let rc = final_receipt(json!({}));
    assert_eq!(rc.coverage, crate::receipt::ReceiptCoverage::default());
    assert!(rc.to_json(None).get("coverage").is_none());
    let rc = final_receipt(json!({"usage_scope": "aggregate", "unused_attempts": 2}));
    assert!(rc.coverage.usage_aggregate && !rc.coverage.cost_aggregate);
    assert_eq!(rc.coverage.unused_attempts, 2);
    assert_eq!(rc.to_json(None)["coverage"]["usage"], "aggregate");
}
