//! MR-14 (development domain): the committed phase routing example
//! (`examples/harness-phase-routing/semaprax.harness.toml`) parsed and run
//! through the existing workflow phase routing, and the route report's
//! `explain` object. Child of the MR-08 module; reuses its role-aware fake
//! generator, counting router and catalog.

use super::*;
use crate::support::repo_root;
use semaprax_harness::workflow::route_explain::EXPLAIN_SCHEMA;
use semaprax_harness::workflow::routing::RoutingWiring;

fn example() -> semaprax_harness::profile::HarnessConfig {
    let bytes =
        std::fs::read(repo_root().join("examples/harness-phase-routing/semaprax.harness.toml"))
            .unwrap();
    semaprax_harness::profile::config::parse(&bytes).unwrap()
}

#[test]
fn mr14_phase_routing_example_parses_and_routes_each_phase_with_an_explained_route() {
    let parsed = example();
    let text = std::fs::read_to_string(
        repo_root().join("examples/harness-phase-routing/semaprax.harness.toml"),
    )
    .unwrap();
    // No endpoint, credential or machine path in committed config.
    for banned in [
        "http",
        "endpoint =",
        "SECRET",
        "api_key",
        "/Users/",
        "token =",
    ] {
        assert!(!text.contains(banned), "example config contains `{banned}`");
    }
    let e = setup(FIXED);
    let fake = Fake::new(CHANGED);
    let mut t = task(&catalog(), 3);
    // `mechanical` is rules-only; this family may consult the router.
    t.family = "localized_debug".into();
    let goal = t.goal.clone();
    let mut cfg = three_roles(&e, t);
    cfg.routing = RoutingWiring::from_config(&parsed.routing, None).unwrap();
    assert_eq!(cfg.routing.phases["implement"].decision, "router");
    let roles = Roles::new(
        vec![plan_doc(json!({}))],
        vec![intent(json!({}))],
        vec![review_doc(json!({}))],
    );
    let mut router = Router(0);
    let r = exec(&cfg, &fake, &roles, Some(&mut router));
    assert!(
        ["candidate-ready", "approved-candidate-ready"].contains(&r.status),
        "{} {:?}",
        r.status,
        r.refusals
    );
    // Plan and review stay on their rules subsets; implement asks the router.
    assert_eq!(
        roles.calls(),
        pairs(&[
            ("plan", "m-plan"),
            ("implement", "m-strong"),
            ("review", "m-review")
        ])
    );
    assert_eq!(router.0, 1, "one router call, for the implement phase only");

    let x = &r.route["explain"];
    assert_eq!(x["schema"], EXPLAIN_SCHEMA);
    assert_eq!(x["execution_domain"], "development");
    assert_eq!(x["phase"], "implement");
    assert_eq!(x["routing_owner"], "semaprax");
    assert!(x["authoritative_pin"].is_null());
    assert_eq!(x["mode"], "experimental");
    assert_eq!(x["candidates"]["admitted"], json!(["m-cheap", "m-strong"]));
    assert_eq!(x["decision"]["source"], "provider");
    assert_eq!(x["decision"]["provider"], "org.example/route");
    assert_eq!(x["decision"]["choice"], "m-strong");
    assert!(x["score_semantics"]["authority"]
        .as_str()
        .unwrap()
        .contains("not a probability"));
    assert_eq!(x["router_overhead"]["calls"], 1);
    assert!(
        x["router_overhead"]["reserved_request_tokens"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(x["evidence_key"]["live"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(
        x["deployment"],
        json!({"generation_provider": "org.example/role-model", "model": "m-strong"})
    );
    // The generator reported no model label: shown as unreported, never as
    // the requested model.
    assert_eq!(x["generation_model"]["requested"], "m-strong");
    assert_eq!(x["generation_model"]["reported"], false);
    assert!(x["generation_model"]["answering"].is_null());
    // Phase log entries carry their own explain.
    let es = entries(&r);
    assert_eq!(entry(&es, "plan")["explain"]["phase"], "plan");
    assert_eq!(entry(&es, "review")["explain"]["phase"], "review");
    // Never the task text or the rendered router request.
    let whole = r.to_json().to_string();
    let route = r.route.to_string();
    assert!(!route.contains(&goal), "route report leaks the task text");
    assert!(!whole.contains("Which one of the listed"));
}
