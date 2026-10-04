//! Aggregation into the machine report. Counts from different tokenizer kinds
//! are never summed: each lives in its own group.

use super::event::{
    Availability, CacheState, Observation, Outcome, Role, Stage, TokenizerId, Warmth,
};
use super::lineage::{self, Pairing};
use super::sink::HostTraffic;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
struct Group {
    id: Option<TokenizerId>,
    payloads: u64,
    baseline: u64,
    final_: u64,
    reduction: i64,
    stages: BTreeMap<&'static str, (u64, u64, u64)>, // before, after, count (local pairs only)
    stage_reduction: BTreeMap<&'static str, i64>,
    incurred: u64,
    incurred_events: u64,
    failed_incurred: u64,
    incurred_by_stage: BTreeMap<&'static str, u64>,
    exposures: u64,
    exposure_tokens: u64,
    digests: BTreeMap<String, u64>,
}

pub fn aggregate(events: &[Observation], dropped: u64, traffic: Option<HostTraffic>) -> Value {
    let lineages = lineage::build(events);
    let mut groups: BTreeMap<String, Group> = BTreeMap::new();
    let mut unpaired = Vec::new();
    let (mut paired_n, mut unpaired_n) = (0u64, 0u64);

    for l in &lineages {
        match &l.pairing {
            Pairing::Paired {
                baseline,
                final_,
                reduction,
            } => {
                paired_n += 1;
                let g = groups.entry(baseline.tokenizer.key()).or_default();
                g.id = Some(baseline.tokenizer.clone());
                g.payloads += 1;
                g.baseline += baseline.value;
                g.final_ += final_.value;
                g.reduction += reduction;
                g.exposures += 1;
                g.exposure_tokens += final_.value;
                if let Some(d) = &l.final_digest {
                    *g.digests.entry(d.clone()).or_default() += 1;
                }
                for s in &l.steps {
                    if let (Some(b), Some(a), Some(r)) = (&s.before, &s.after, s.local_reduction())
                    {
                        let e = g.stages.entry(s.stage.as_str()).or_default();
                        e.0 += b.value;
                        e.1 += a.value;
                        e.2 += 1;
                        *g.stage_reduction.entry(s.stage.as_str()).or_default() += r;
                    }
                }
            }
            Pairing::Unpaired(reason) => {
                unpaired_n += 1;
                unpaired.push(json!({"payload_id": l.payload_id, "reason": reason}));
            }
        }
    }

    // Measurement status per event, incurred requests, cost provenance.
    let (mut named, mut byte_only, mut missing) = (0u64, 0u64, 0u64);
    let mut hidden_unknown = 0u64;
    let mut hidden_known = 0u64;
    let (mut billed_known, mut billed_unknown) = (0u64, 0u64);
    let (mut local_compute, mut cold_ms, mut warm_ms, mut decision_ms) = (0u64, 0u64, 0u64, 0u64);
    let (mut failed, mut fallbacks, mut unavailable, mut cache_hits, mut extra_retrievals) =
        (0u64, 0, 0u64, 0u64, 0u64);
    let mut invocations = BTreeSet::new();
    for e in events {
        invocations.insert(e.invocation_id.as_str());
        let expected: Vec<&Option<_>> = match e.role {
            Role::Transform => vec![&e.before, &e.after],
            Role::Incurred => vec![&e.incurred],
            Role::Local => vec![],
        };
        if !expected.is_empty() {
            if expected.iter().any(|c| c.is_none()) {
                missing += 1;
            } else if expected
                .iter()
                .copied()
                .flatten()
                .any(|c| c.tokenizer == TokenizerId::ByteOnly)
            {
                byte_only += 1;
            } else {
                named += 1;
            }
        }
        match e.cost.provider_billed {
            Some(v) => billed_known += v,
            None => billed_unknown += 1,
        }
        if e.role == Role::Incurred {
            match e.cost.hidden_attempts {
                Some(v) => hidden_known += v,
                None => hidden_unknown += 1,
            }
            if matches!(e.stage, Stage::ContextSelect | Stage::RetrievalWrapper) {
                extra_retrievals += 1;
            }
            if let Some(c) = &e.incurred {
                let g = groups.entry(c.tokenizer.key()).or_default();
                g.id = Some(c.tokenizer.clone());
                g.incurred += c.value;
                g.incurred_events += 1;
                if e.outcome == Outcome::Failed {
                    g.failed_incurred += 1;
                }
                *g.incurred_by_stage.entry(e.stage.as_str()).or_default() += c.value;
            }
        }
        local_compute += e.cost.local_compute_ms;
        match (e.stage, e.warmth) {
            (Stage::IndexBuild | Stage::ModelLoad, Warmth::Cold) => cold_ms += e.latency_ms,
            (_, Warmth::Warm) => warm_ms += e.latency_ms,
            _ => {}
        }
        if e.stage == Stage::Decision {
            decision_ms += e.latency_ms;
        }
        failed += (e.outcome == Outcome::Failed) as u64;
        fallbacks += (e.availability == Availability::Fallback) as u64;
        unavailable += (e.availability == Availability::Unavailable) as u64;
        cache_hits += (e.cache == CacheState::Hit) as u64;
    }

    let mut group_json = Vec::new();
    for (key, g) in &groups {
        let id = g.id.clone().unwrap_or(TokenizerId::ByteOnly);
        let unit = if id == TokenizerId::ByteOnly {
            "bytes"
        } else {
            "model_tokens"
        };
        let mut stages = Map::new();
        for (s, (b, a, n)) in &g.stages {
            stages.insert(
                s.to_string(),
                json!({"before": b, "after": a, "reduction": g.stage_reduction[s], "count": n}),
            );
        }
        let repeated: u64 = g.digests.values().map(|n| n.saturating_sub(1)).sum();
        group_json.push(json!({
            "key": key, "tokenizer": id.to_json(), "unit": unit,
            "paired_payloads": g.payloads, "baseline": g.baseline, "final": g.final_,
            "end_to_end_reduction": g.reduction,
            "stage_local": stages,
            "incurred": {"total": g.incurred, "events": g.incurred_events,
                         "failed_attempts": g.failed_incurred, "by_stage": g.incurred_by_stage},
            "net_savings": g.reduction - g.incurred as i64,
            "exposure": {"model_visible_events": g.exposures, "model_visible_total": g.exposure_tokens,
                         "repeated_identical_exposures": repeated},
        }));
    }

    let mut reasons: Vec<&str> = Vec::new();
    if dropped > 0 {
        reasons.push("dropped_observations");
    }
    match traffic {
        None => reasons.push("host_traffic_undeclared"),
        Some(t) if t.unobserved > 0 => reasons.push("unobserved_host_traffic"),
        _ => {}
    }
    if missing > 0 {
        reasons.push("missing_tokenizer_events");
    }
    if unpaired_n > 0 {
        reasons.push("unpaired_comparisons");
    }
    if paired_n == 0 {
        reasons.push("no_paired_payload");
    }
    if hidden_unknown > 0 {
        reasons.push("hidden_attempts_unknown");
    }
    if groups.len() > 1 {
        reasons.push("mixed_measurement_kinds");
    }
    let complete = reasons.is_empty();

    json!({
        "schema": "semaprax.harness-observation-report.v1",
        "groups": group_json,
        "unpaired": unpaired,
        "costs": {
            "provider_billed": {"total": if billed_unknown > 0 { json!("unknown") } else { json!(billed_known) },
                                "known_partial": billed_known, "unknown_events": billed_unknown},
            "hidden_attempts": {"known": hidden_known, "unknown_events": hidden_unknown},
            "local": {"compute_ms": local_compute, "cold_load_ms": cold_ms, "warm_query_ms": warm_ms,
                      "decision_ms": decision_ms},
            "extra_retrievals": extra_retrievals, "failed_attempts": failed,
            "fallbacks": fallbacks, "unavailable": unavailable, "cache_hits": cache_hits,
            "note": "cache hits are not model-token savings; local compute is not provider billing",
        },
        "coverage": {
            "events": events.len(), "invocations": invocations.len(), "dropped": dropped,
            "host_traffic": traffic.map_or(Value::Null, |t| json!({"observed": t.observed, "unobserved": t.unobserved})),
            "named_measured": named, "byte_only": byte_only, "missing_tokenizer": missing,
            "paired": paired_n, "unpaired": unpaired_n,
            "complete": complete, "reasons": reasons,
        },
        "partial": !complete,
        "whole_task_claim_allowed": complete,
    })
}
