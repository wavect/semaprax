//! TC-01: provider usage normalization, streaming merge and local price
//! estimates. Fixtures: tests/fixtures/receipt/usage. Pipeline and HostModel
//! evidence is in `workflow::tc`.

use semaprax_harness::endpoint::probe::SseEvent;
use semaprax_harness::endpoint::{assess_stream, Protocol};
use semaprax_harness::receipt::{
    event_usage, merge_stream, normalize, PriceBook, PriceRecord, Pricing, ProposalReceipt, Usage,
};
use serde_json::{json, Value};
use std::path::PathBuf;

fn fixtures() -> Vec<(String, Value)> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/receipt/usage");
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .map(|p| {
            let n = p.file_stem().unwrap().to_string_lossy().into_owned();
            (
                n,
                serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap(),
            )
        })
        .collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

fn usage_of(f: &Value) -> Usage {
    let p = Protocol::parse(f["protocol"].as_str().unwrap()).unwrap();
    match f.get("events") {
        Some(ev) => {
            let ups: Vec<&Value> = ev
                .as_array()
                .unwrap()
                .iter()
                .filter_map(event_usage)
                .collect();
            normalize(p, &merge_stream(ups))
        }
        None => normalize(p, &f["usage"]),
    }
}

fn field(u: &Usage, k: &str) -> Option<u64> {
    match k {
        "input_total" => u.input_total,
        "uncached_input" => u.uncached_input,
        "cache_read" => u.cache_read,
        "cache_write" => u.cache_write,
        "output" => u.output,
        "reasoning" => u.reasoning,
        o => panic!("unknown category {o}"),
    }
}

#[test]
fn tc01_buffered_and_streaming_fixtures_of_all_three_protocols_normalize_to_the_same_result() {
    let all = fixtures();
    assert_eq!(all.len(), 6, "buffered + streaming for three protocols");
    for (name, f) in &all {
        let u = usage_of(f);
        for (k, want) in f["expect"].as_object().unwrap() {
            assert_eq!(field(&u, k), want.as_u64(), "{name}: {k} in {u:?}");
        }
    }
    // Equivalent information, equal common categories, whatever protocol or transport.
    let common: Vec<_> = all
        .iter()
        .map(|(_, f)| {
            let u = usage_of(f);
            (
                u.input_total,
                u.uncached_input,
                u.cache_read,
                u.cache_write,
                u.output,
            )
        })
        .collect();
    assert!(common.windows(2).all(|w| w[0] == w[1]), "{common:?}");
}

#[test]
fn tc01_cache_only_input_and_inclusive_vs_exclusive_totals_are_not_double_counted() {
    // OpenAI inclusive: everything cached means zero uncached, total unchanged.
    let u = normalize(
        Protocol::Responses,
        &json!({"input_tokens": 100, "input_tokens_details": {"cached_tokens": 100}, "output_tokens": 5}),
    );
    assert_eq!(
        (u.input_total, u.uncached_input, u.cache_read),
        (Some(100), Some(0), Some(100))
    );
    // Anthropic exclusive: the total is the sum of the reported parts.
    let u = normalize(
        Protocol::AnthropicMessages,
        &json!({"input_tokens": 0, "cache_read_input_tokens": 100, "cache_creation_input_tokens": 0, "output_tokens": 5}),
    );
    assert_eq!(
        (u.input_total, u.uncached_input, u.cache_read),
        (Some(100), Some(0), Some(100))
    );
    // An inconsistent report is not repaired.
    let u = normalize(
        Protocol::ChatCompletions,
        &json!({"prompt_tokens": 10, "prompt_tokens_details": {"cached_tokens": 30}}),
    );
    assert_eq!(
        (u.input_total, u.uncached_input, u.cache_read),
        (Some(10), None, Some(30))
    );
}

#[test]
fn tc01_cache_writes_by_tier_and_reasoning_as_a_subset() {
    let u = normalize(
        Protocol::AnthropicMessages,
        &json!({"input_tokens": 40, "cache_read_input_tokens": 60, "cache_creation_input_tokens": 30,
                "cache_creation": {"ephemeral_5m_input_tokens": 10, "ephemeral_1h_input_tokens": 20}, "output_tokens": 7}),
    );
    assert_eq!((u.cache_write, u.cache_write_1h), (Some(30), Some(20)));
    assert_eq!(u.input_total, Some(130));
    // Only a split: the write total is the tier sum.
    let u = normalize(
        Protocol::AnthropicMessages,
        &json!({"input_tokens": 1, "cache_creation": {"ephemeral_5m_input_tokens": 4, "ephemeral_1h_input_tokens": 6}}),
    );
    assert_eq!(u.cache_write, Some(10));
    // Reasoning is a detail of output, never another copy.
    let u = normalize(
        Protocol::Responses,
        &json!({"input_tokens": 3, "output_tokens": 50, "output_tokens_details": {"reasoning_tokens": 20}}),
    );
    assert_eq!((u.output, u.reasoning), (Some(50), Some(20)));
    // Nonsense (reasoning above output) is dropped, not trusted.
    let u = normalize(
        Protocol::Responses,
        &json!({"output_tokens": 5, "output_tokens_details": {"reasoning_tokens": 20}}),
    );
    assert_eq!((u.output, u.reasoning), (Some(5), None));
}

#[test]
fn tc01_cumulative_updates_duplicates_and_partial_terminal_updates_merge_without_double_counting() {
    let ev = [
        json!({"input_tokens": 25, "output_tokens": 1}),
        json!({"output_tokens": 8}),
        json!({"output_tokens": 15}),
        json!({"output_tokens": 15}),
        json!({"output_tokens": 15}),
    ];
    let m = merge_stream(ev.iter());
    let u = normalize(Protocol::AnthropicMessages, &m);
    assert_eq!(
        u.uncached_input,
        Some(25),
        "an output-only update keeps the input"
    );
    assert_eq!(
        u.output,
        Some(15),
        "cumulative snapshots replace, never sum"
    );
    // Merging the same snapshot twice is idempotent.
    let once = merge_stream([&ev[2]]);
    let twice = merge_stream([&ev[2], &ev[2]]);
    assert_eq!(once, twice);
    // Nested details merge key by key (a later snapshot with only some details).
    let m = merge_stream(
        [
            json!({"input_tokens": 9, "input_tokens_details": {"cached_tokens": 4}}),
            json!({"output_tokens": 2, "input_tokens_details": {}}),
        ]
        .iter(),
    );
    let u = normalize(Protocol::Responses, &m);
    assert_eq!(
        (u.input_total, u.cache_read, u.output),
        (Some(9), Some(4), Some(2))
    );
}

fn sse(data: Value) -> SseEvent {
    SseEvent {
        event: None,
        data: data.to_string(),
    }
}

#[test]
fn tc01_assess_stream_merges_cumulative_usage_and_reads_anthropic_message_start() {
    let events = vec![
        sse(
            json!({"type": "message_start", "message": {"model": "m", "usage": {"input_tokens": 25, "output_tokens": 1}}}),
        ),
        sse(json!({"type": "message_delta", "usage": {"output_tokens": 15}})),
        sse(json!({"type": "message_delta", "usage": {"output_tokens": 15}})),
    ];
    let a = assess_stream(Protocol::AnthropicMessages, &events, Some("m"));
    let j = a.usage.to_json();
    assert_eq!(j["input_tokens"], 25, "{j}");
    assert_eq!(j["output_tokens"], 15, "{j}");
    // No usage in any event stays unknown, with the MissingUsage outcome.
    let a = assess_stream(
        Protocol::AnthropicMessages,
        &[sse(
            json!({"type": "message_start", "message": {"model": "m"}}),
        )],
        Some("m"),
    );
    assert_eq!(a.usage.to_json()["input_tokens"], "unknown");
    assert!(a.outcomes.iter().any(|o| o.code() == "SPX-HPL023"));
}

#[test]
fn tc01_missing_usage_is_unknown_never_zero() {
    for p in [
        Protocol::Responses,
        Protocol::ChatCompletions,
        Protocol::AnthropicMessages,
    ] {
        let u = normalize(p, &json!({}));
        assert!(u.is_empty(), "{p:?}: {u:?}");
        assert!(normalize(p, &Value::Null).is_empty());
    }
    let j = Usage::default().to_json();
    assert_eq!(j["output"], "unknown");
    assert_eq!(j["input_total"], "unknown");
}

fn rates() -> PriceRecord {
    // Synthetic fixed prices: micro-units per million tokens.
    PriceRecord {
        version: "synthetic-v1".into(),
        pricing: Pricing::Rates {
            input: Some(3_000_000),
            cache_read: Some(300_000),
            cache_write: Some(3_750_000),
            cache_write_1h: Some(6_000_000),
            output: Some(15_000_000),
        },
    }
}

fn full(w: Option<u64>, w1h: Option<u64>) -> Usage {
    Usage {
        input_total: None,
        uncached_input: Some(40),
        cache_read: Some(60),
        cache_write: w,
        cache_write_1h: w1h,
        output: Some(50),
        reasoning: Some(20),
    }
}

#[test]
fn tc01_price_estimates_use_checked_arithmetic_and_synthetic_prices() {
    let book = PriceBook::default().with("syn-", rates());
    // 40*3 + 60*0.3 + 0 + 50*15 = 120 + 18 + 750; reasoning is not priced again.
    let e = book.estimate("syn-1", &full(Some(0), Some(0)));
    assert_eq!(e.micros, Some(888));
    assert_eq!(e.basis, "estimated_from_price_record");
    assert_eq!(e.price_version.as_deref(), Some("synthetic-v1"));
    // Cache writes by tier: 10 at the standard rate, 20 at the 1h rate (rounded up).
    let e = book.estimate("syn-1", &full(Some(30), Some(20)));
    assert_eq!(e.micros, Some(888 + 38 + 120)); // 10*3.75=37.5 -> 38, 20*6=120
                                                // Rounding never under-reports a priced category.
    let tiny = Usage {
        uncached_input: Some(1),
        cache_read: Some(0),
        cache_write: Some(0),
        cache_write_1h: Some(0),
        output: Some(0),
        ..Default::default()
    };
    assert_eq!(book.estimate("syn-1", &tiny).micros, Some(3));
}

#[test]
fn tc01_unknown_stays_unknown_and_zero_billing_needs_an_explicit_non_billed_source() {
    let book = PriceBook::default().with("syn-", rates()).with(
        "local-",
        PriceRecord {
            version: "local-v1".into(),
            pricing: Pricing::NonBilled,
        },
    );
    // Unpriced model.
    let e = book.estimate("other", &full(Some(0), Some(0)));
    assert_eq!((e.micros, e.basis), (None, "unpriced_model"));
    // Incomplete usage (cache write unknown).
    let e = book.estimate("syn-1", &full(None, None));
    assert_eq!((e.micros, e.basis), (None, "incomplete_usage"));
    // A 1h rate exists but the write total is unsplit: cannot price.
    let e = book.estimate("syn-1", &full(Some(30), None));
    assert_eq!(e.micros, None);
    // A non-zero category without a price.
    let partial = PriceBook::default().with(
        "p-",
        PriceRecord {
            version: "p".into(),
            pricing: Pricing::Rates {
                input: Some(1),
                cache_read: None,
                cache_write: Some(0),
                cache_write_1h: None,
                output: Some(1),
            },
        },
    );
    let e = partial.estimate("p-1", &full(Some(0), Some(0)));
    assert_eq!((e.micros, e.basis), (None, "missing_price"));
    // Overflow is unknown, not a wrapped number.
    let big = Usage {
        uncached_input: Some(u64::MAX),
        cache_read: Some(0),
        cache_write: Some(0),
        cache_write_1h: Some(0),
        output: Some(0),
        ..Default::default()
    };
    let huge = PriceBook::default().with(
        "h-",
        PriceRecord {
            version: "h".into(),
            pricing: Pricing::Rates {
                input: Some(u64::MAX),
                cache_read: None,
                cache_write: None,
                cache_write_1h: None,
                output: None,
            },
        },
    );
    let e = huge.estimate("h-1", &big);
    assert_eq!((e.micros, e.basis), (None, "overflow"));
    // Explicit non-billed source: a known zero billing, still labelled as an estimate basis.
    let e = book.estimate("local-m", &Usage::default());
    assert_eq!((e.micros, e.basis), (Some(0), "non_billed_source"));
    let j = e.to_json();
    assert_eq!(j["kind"], "estimate");
}

#[test]
fn tc01_provider_reported_cost_is_distinct_from_the_local_estimate() {
    let ctl = Default::default();
    let payload = json!({"receipt": {"protocol": "chat_completions", "finish_reason": "stop", "provider_cost_micros": 777,
        "usage": {"prompt_tokens": 10, "completion_tokens": 2}}});
    let r = ProposalReceipt::from_result(&ctl, &payload);
    let est = PriceBook::default().estimate("syn-1", &r.usage);
    let j = r.to_json(Some(&est));
    assert_eq!(j["cost"]["provider_reported_micros"], 777);
    assert_eq!(j["cost"]["estimated"]["micros"], "unknown");
    assert_eq!(j["cost"]["estimated"]["kind"], "estimate");
    // No reported charge and no price: both unknown, neither zero.
    let r =
        ProposalReceipt::from_result(&ctl, &json!({"receipt": {"protocol": "chat_completions"}}));
    assert_eq!(
        r.to_json(None)["cost"]["provider_reported_micros"],
        "unknown"
    );
}
