//! MR-14: the route report's `explain` object. One place states what
//! controlled a development-domain route: domain and phase, the
//! policy-filtered candidates and why the others were excluded, the
//! authoritative pin and routing owner, the decision provider apart from the
//! generation model, score semantics, the cache/bypass/fallback reason, the
//! evidence key, the selected generation deployment and the router overhead.
//!
//! It carries identifiers, digests, counts and closed reason words only:
//! never the task text, the rendered router request, a prompt, a secret or a
//! token value.

use crate::decision::{
    route::screen, DecisionSource, GovernedRoute, RouteInputs, RoutingConfig, RoutingMode,
};
use serde_json::{json, Value};

pub const EXPLAIN_SCHEMA: &str = "semaprax.harness-route-explain.v1";

fn source_word(s: &DecisionSource) -> (&'static str, Option<String>) {
    match s {
        DecisionSource::Rules => ("rules", None),
        DecisionSource::Trivial => ("trivial", None),
        DecisionSource::Cache => ("cache", None),
        DecisionSource::Provider => ("provider", None),
        DecisionSource::Fallback(r) => ("fallback", Some(format!("{r:?}"))),
    }
}

/// The `explain` object of one route, built before generation. The phase,
/// reroute exclusions and the generation model are filled in by the caller
/// as they become known ([`set_phase`], [`set_generation`]).
pub(super) fn explain(
    inputs: &RouteInputs,
    cfg: &RoutingConfig,
    gr: &GovernedRoute,
    reuse: &Value,
    live_key_digest: Option<String>,
    router_request_tokens: u64,
) -> Value {
    let dec = &gr.decision;
    let scr = screen(&inputs.request, &inputs.policy);
    let pin = match (&cfg.project_pin, &cfg.mode) {
        (Some(p), _) => json!({"model": p, "owner": "project"}),
        (None, RoutingMode::Pin(p)) => json!({"model": p, "owner": "user"}),
        _ => Value::Null,
    };
    let (source, fallback) = source_word(&dec.source);
    let call = dec.wire.call.as_ref();
    let wire = dec.wire.to_json();
    let evidence = &gr.explanation["evidence"];
    json!({
        "schema": EXPLAIN_SCHEMA,
        "execution_domain": "development",
        "phase": Value::Null,
        "routing_owner": "semaprax",
        "authoritative_pin": pin,
        "mode": gr.mode,
        "candidates": {
            "admitted": scr.admissible.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            "excluded": scr.excluded.iter().map(|(id, why)| json!({"model": id, "reason": why})).collect::<Vec<_>>(),
            "rerouted": [],
        },
        "decision": {
            "source": source,
            "choice": dec.choice,
            "provider": dec.provider_id,
            "checkpoint": dec.checkpoint,
            "status": dec.provider_status,
            "wire_version": dec.wire.version,
            "answering_model": call.and_then(|c| c.answering_model.clone()),
            "identity_kind": call.map(|c| c.identity_kind.as_str()),
        },
        "score_semantics": {"score_kind": wire["score_kind"], "scores_are": wire["scores_are"],
                            "authority": "advisory; a score grants nothing and is not a probability of task success"},
        "reason": {
            "cache": reuse["cache"],
            "cache_reason": reuse["reason"],
            "bypass": gr.rules_reason,
            "fallback": fallback,
            "abstention": wire["abstention_reason"],
        },
        "evidence_key": {
            "applied": evidence.get("key_digest").cloned().unwrap_or(Value::Null),
            "live": live_key_digest,
        },
        "deployment": Value::Null,
        "generation_model": Value::Null,
        "router_overhead": {
            "calls": gr.router_calls_total,
            "ms": dec.router_ms,
            "reserved_request_tokens": router_request_tokens,
            "reported_input_tokens": call.and_then(|c| c.usage.input_tokens),
            "reported_output_tokens": call.and_then(|c| c.usage.output_tokens),
            "billing": call.map(|c| c.billing.as_str()),
        },
    })
}

/// Records the phase role and the pre-dispatch reroutes of the route.
pub(super) fn set_phase(route: &mut Value, phase: &str, rerouted: &[Value]) {
    if route["explain"].is_object() {
        route["explain"]["phase"] = json!(phase);
        route["explain"]["candidates"]["rerouted"] = json!(rerouted);
    }
}

/// Records the selected generation deployment (provider and logical model).
pub(super) fn set_deployment(route: &mut Value, generation_provider: &str, model: &str) {
    if route["explain"].is_object() {
        route["explain"]["deployment"] =
            json!({"generation_provider": generation_provider, "model": model});
    }
}

/// Records which model the generation provider reported answering with.
/// `None` stays visible as unreported; the requested model is never
/// presented as the answering one.
pub(super) fn set_generation(
    route: &mut Value,
    provider: &str,
    requested: &str,
    answering: Option<&str>,
) {
    // Only the route that selected this dispatch, and only once: a later
    // phase generation (plan/review) never overwrites the implement record.
    let e = &route["explain"];
    if e.is_object() && e["generation_model"].is_null() && e["deployment"]["model"] == requested {
        route["explain"]["generation_model"] = json!({
            "provider": provider, "requested": requested, "answering": answering,
            "reported": answering.is_some(),
        });
    }
}
