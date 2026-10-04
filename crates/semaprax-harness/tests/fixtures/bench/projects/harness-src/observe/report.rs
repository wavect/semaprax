//! Machine JSON and human text, both derived from one aggregate value.

use super::aggregate::aggregate;
use super::event::Observation;
use super::sink::HostTraffic;
use serde_json::Value;

pub struct Report {
    pub json: Value,
}

pub fn build_report(events: &[Observation], dropped: u64, traffic: Option<HostTraffic>) -> Report {
    Report {
        json: aggregate(events, dropped, traffic),
    }
}

impl Report {
    pub fn text(&self) -> String {
        render_text(&self.json)
    }
}

fn n(v: &Value) -> String {
    match v {
        Value::Null => "undeclared".into(),
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Human view of a report value (never recomputes anything).
pub fn render_text(r: &Value) -> String {
    let mut o = String::new();
    let cov = &r["coverage"];
    let partial = r["partial"].as_bool().unwrap_or(true);
    o.push_str(&format!(
        "harness observation report{}\n",
        if partial {
            " (PARTIAL: whole-task savings claim NOT allowed)"
        } else {
            ""
        }
    ));
    for g in r["groups"].as_array().into_iter().flatten() {
        let unit = g["unit"].as_str().unwrap_or("?");
        let red = g["end_to_end_reduction"].as_i64().unwrap_or(0);
        let net = g["net_savings"].as_i64().unwrap_or(0);
        o.push_str(&format!(
            "group {} [{unit}]: {} paired payloads, {} -> {} end-to-end ({} {} {unit})\n",
            g["key"].as_str().unwrap_or(""),
            n(&g["paired_payloads"]),
            n(&g["baseline"]),
            n(&g["final"]),
            red.abs(),
            if red >= 0 { "fewer" } else { "MORE" },
        ));
        for (stage, s) in g["stage_local"].as_object().into_iter().flatten() {
            o.push_str(&format!(
                "  stage {stage}: {} -> {} (local {:+})\n",
                n(&s["before"]),
                n(&s["after"]),
                -s["reduction"].as_i64().unwrap_or(0)
            ));
        }
        o.push_str(&format!(
            "  incurred separately: {} {unit} in {} requests ({} failed)\n  net savings: {net:+} {unit}{}\n",
            n(&g["incurred"]["total"]), n(&g["incurred"]["events"]), n(&g["incurred"]["failed_attempts"]),
            if net < 0 { " (NEGATIVE)" } else { "" },
        ));
        o.push_str(&format!(
            "  model exposure: {} {unit} over {} sends ({} repeated identical)\n",
            n(&g["exposure"]["model_visible_total"]),
            n(&g["exposure"]["model_visible_events"]),
            n(&g["exposure"]["repeated_identical_exposures"]),
        ));
    }
    let c = &r["costs"];
    o.push_str(&format!(
        "costs: provider billed {} (known part {}, {} unknown events); hidden attempts known {} / {} unknown events\n",
        n(&c["provider_billed"]["total"]), n(&c["provider_billed"]["known_partial"]), n(&c["provider_billed"]["unknown_events"]),
        n(&c["hidden_attempts"]["known"]), n(&c["hidden_attempts"]["unknown_events"]),
    ));
    o.push_str(&format!(
        "local (not billed): compute {} ms, cold load {} ms, warm queries {} ms, decisions {} ms; extra retrievals {}, failed attempts {}, cache hits {}\n",
        n(&c["local"]["compute_ms"]), n(&c["local"]["cold_load_ms"]), n(&c["local"]["warm_query_ms"]),
        n(&c["local"]["decision_ms"]), n(&c["extra_retrievals"]), n(&c["failed_attempts"]), n(&c["cache_hits"]),
    ));
    for u in r["unpaired"].as_array().into_iter().flatten() {
        o.push_str(&format!(
            "unpaired payload {}: {}\n",
            n(&u["payload_id"]),
            n(&u["reason"])
        ));
    }
    o.push_str(&format!(
        "coverage: {} events; named-measured {}, byte-only {}, missing tokenizer {}; paired {}, unpaired {}; dropped {}; host traffic {}\n",
        n(&cov["events"]), n(&cov["named_measured"]), n(&cov["byte_only"]), n(&cov["missing_tokenizer"]),
        n(&cov["paired"]), n(&cov["unpaired"]), n(&cov["dropped"]), n(&cov["host_traffic"]),
    ));
    let reasons: Vec<String> = cov["reasons"]
        .as_array()
        .into_iter()
        .flatten()
        .map(n)
        .collect();
    if !reasons.is_empty() {
        o.push_str(&format!("incomplete because: {}\n", reasons.join(", ")));
    }
    o.push_str(&format!(
        "whole_task_claim_allowed: {}\n",
        n(&r["whole_task_claim_allowed"])
    ));
    o
}
