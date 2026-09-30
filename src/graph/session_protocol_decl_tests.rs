//! Graph v48 and `context` projection of declared session protocols (#297).

use crate::graph::{
    self, AgentContextDirection, AgentContextFilter, AgentContextOptions, AgentContextV2Options,
};

const DECLARED: &str = include_str!("../session_protocol/tests/fixtures/declared.spx");

fn without_declaration() -> String {
    let start = DECLARED
        .find("@id(\"fixture.session.transaction\")")
        .unwrap();
    DECLARED[..start].to_owned()
}

fn graph_of(source: &str) -> serde_json::Value {
    let program = crate::check(source, "session.spx").unwrap();
    serde_json::from_str(&graph::to_json(&program).unwrap()).unwrap()
}

#[test]
fn graph_v48_is_selected_only_for_a_declaring_program_and_extends_its_base_schema() {
    let with = graph_of(DECLARED);
    let without = graph_of(&without_declaration());
    assert_eq!(with["schema"], super::GRAPH_SCHEMA);
    assert_ne!(without["schema"], super::GRAPH_SCHEMA);
    assert!(without.get("session_protocols").is_none());
    assert_eq!(with["session_protocols"]["base_schema"], without["schema"]);
    assert_eq!(with["session_protocols"]["authority"], "none");
    // Apart from the header and the appended section, the declaring
    // program's graph carries exactly the facts of its base document.
    let mut stripped = with.clone();
    stripped["schema"] = without["schema"].clone();
    stripped
        .as_object_mut()
        .unwrap()
        .remove("session_protocols");
    for key in ["revision", "source_revision"] {
        if let Some(value) = without.get(key) {
            stripped[key] = value.clone();
        }
    }
    assert_eq!(stripped, without);
}

#[test]
fn the_declaration_fact_is_bound_to_its_identity_span_and_checked_via_targets() {
    let with = graph_of(DECLARED);
    let declarations = with["session_protocols"]["declarations"]
        .as_array()
        .unwrap();
    assert_eq!(declarations.len(), 1);
    let fact = &declarations[0];
    assert_eq!(fact["stable_id"], "fixture.session.transaction");
    assert_eq!(fact["name"], "fixture-transaction-v1");
    assert_eq!(fact["initial"], "Idle");
    assert_eq!(fact["static_validation"], "passed");
    assert_eq!(fact["bounded_reachability"], "passed");
    assert_eq!(fact["authority"], "none");
    assert!(fact["span"]["line"].as_u64().unwrap() > 0);
    assert_eq!(
        fact["states"],
        serde_json::json!(["Idle", "Open", "Committed", "Failed"])
    );
    assert_eq!(
        fact["terminals"],
        serde_json::json!([
            {"state": "Committed", "cleanup": ["release_snapshot"]},
            {"state": "Failed", "cleanup": ["discard_snapshot"]}
        ])
    );
    let transitions = fact["transitions"].as_array().unwrap();
    assert_eq!(transitions.len(), 4);
    assert_eq!(transitions[0]["via"], "fixture.session.begin");
    assert_eq!(
        transitions[0]["required_capability"],
        serde_json::Value::Null
    );
    assert_eq!(transitions[2]["required_capability"], "db.write");
    assert_eq!(transitions[2]["capability_binding"], "via_declared_effect");
    assert_eq!(transitions[2]["next"]["kind"], "choice");
    assert_eq!(transitions[1]["kind"], "fail");
}

#[test]
fn graph_is_deterministic_replays_and_refuses_a_forged_declaration() {
    let program = crate::check(DECLARED, "session.spx").unwrap();
    let first = graph::to_json(&program).unwrap();
    assert_eq!(first, graph::to_json(&program).unwrap());
    graph::verify_json(&program, &first).unwrap();
    let forged = first.replacen(
        "\"via\":\"fixture.session.begin\"",
        "\"via\":\"fixture.session.main\"",
        1,
    );
    assert_ne!(forged, first);
    assert!(graph::verify_json(&program, &forged).is_err());
    assert!(graph::to_legacy_json(&program).is_err());
}

#[test]
fn a_mutated_declaration_changes_the_projection() {
    let base = graph_of(DECLARED);
    let mutated = graph_of(&DECLARED.replace(
        "on Idle misuse: fail Unit -> Failed;",
        "on Idle misuse: cancel Unit -> Failed;",
    ));
    assert_ne!(
        base["session_protocols"]["declarations"][0]["transitions"],
        mutated["session_protocols"]["declarations"][0]["transitions"]
    );
}

#[test]
fn graph_binding_refuses_a_via_absent_from_checked_hir() {
    let program = crate::check(DECLARED, "session.spx").unwrap();
    let mut resolved = crate::hir::resolve(&program).unwrap();
    resolved
        .functions
        .retain(|function| function.id.as_str() != "fixture.session.commit");
    let error = super::attach(&program, &resolved, "{\"schema\":\"x\"}".to_owned()).unwrap_err();
    assert_eq!(error.code, "SPX-K104");
}

#[test]
fn context_carries_bound_declarations_only_when_selected_and_present() {
    let program = crate::check(DECLARED, "session.spx").unwrap();
    let selected = AgentContextOptions::new(
        1,
        64 * 1024,
        16,
        [
            AgentContextFilter::Effects,
            AgentContextFilter::SessionProtocol,
        ],
    )
    .unwrap();
    let unselected =
        AgentContextOptions::new(1, 64 * 1024, 16, [AgentContextFilter::Effects]).unwrap();
    let with = graph::agent_context_json(&program, "fixture.session.main", &selected)
        .unwrap()
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&with).unwrap();
    let declared = parsed["session_protocol_kernel"]["declared"]
        .as_array()
        .unwrap();
    assert_eq!(declared.len(), 1);
    assert_eq!(declared[0]["stable_id"], "fixture.session.transaction");
    assert_eq!(
        parsed["budget"]["used_bytes"].as_u64().unwrap() as usize,
        with.len()
    );
    let bare = graph::agent_context_json(&program, "fixture.session.main", &unselected)
        .unwrap()
        .unwrap();
    assert!(!bare.contains("\"declared\""));

    let v2 = AgentContextV2Options::new(
        1,
        64 * 1024,
        16,
        [
            AgentContextFilter::Effects,
            AgentContextFilter::SessionProtocol,
        ],
        AgentContextDirection::Forward,
    )
    .unwrap();
    let with_v2 = graph::agent_context_v2_json(&program, "fixture.session.main", &v2)
        .unwrap()
        .unwrap();
    let parsed_v2: serde_json::Value = serde_json::from_str(&with_v2).unwrap();
    assert_eq!(
        parsed_v2["session_protocol_kernel"]["declared"],
        parsed["session_protocol_kernel"]["declared"]
    );

    // A program without a declaration keeps the unchanged catalog bytes.
    let plain = crate::check(&without_declaration(), "session.spx").unwrap();
    let plain = graph::agent_context_json(&plain, "fixture.session.main", &selected)
        .unwrap()
        .unwrap();
    assert!(!plain.contains("\"declared\""));
    assert!(plain.contains("\"session_protocol_kernel\":{\"note\":"));
}
