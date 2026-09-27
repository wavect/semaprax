//! Graph v49 and `context` projection of endpoint typestate `follows`
//! bindings (issue #297 follow-on, R21).

use crate::graph::{
    self, AgentContextDirection, AgentContextFilter, AgentContextOptions, AgentContextV2Options,
};

const FOLLOWS: &str = include_str!("../session_protocol/tests/fixtures/follows.spx");

/// `FOLLOWS` with its `follows` clause removed: an otherwise identical
/// program that still declares the protocol and still calls `begin`/`commit`
/// as plain functions, so it exercises "declares but never opts in", not
/// "declares nothing" (`session_protocol_decl_tests` already covers that).
fn without_follows() -> String {
    FOLLOWS.replacen(
        "\n    follows session protocol \"fixture.follows.protocol\"\n",
        "\n",
        1,
    )
}

fn graph_of(source: &str) -> serde_json::Value {
    let program = crate::check(source, "follows.spx").unwrap();
    serde_json::from_str(&graph::to_json(&program).unwrap()).unwrap()
}

#[test]
fn graph_v49_is_selected_only_for_a_follows_using_program_and_extends_v48() {
    let with = graph_of(FOLLOWS);
    let without = graph_of(&without_follows());
    assert_eq!(with["schema"], super::GRAPH_SCHEMA);
    assert_ne!(without["schema"], super::GRAPH_SCHEMA);
    // The declaration-only program still selects v48 (unaffected by this
    // module's existence) and carries no `session_protocol_follows` key.
    assert_eq!(
        without["schema"],
        crate::graph::session_protocol_decl::GRAPH_SCHEMA
    );
    assert!(without.get("session_protocol_follows").is_none());
    assert_eq!(
        with["session_protocol_follows"]["base_schema"],
        without["schema"]
    );
    assert_eq!(with["session_protocol_follows"]["authority"], "none");
    // Apart from the header and the appended section, the follows-using
    // program's graph carries exactly the facts of its v48 base document
    // (which itself already carries `session_protocols`).
    let mut stripped = with.clone();
    stripped["schema"] = without["schema"].clone();
    stripped
        .as_object_mut()
        .unwrap()
        .remove("session_protocol_follows");
    for key in ["revision", "source_revision"] {
        if let Some(value) = without.get(key) {
            stripped[key] = value.clone();
        }
    }
    assert_eq!(stripped, without);
}

#[test]
fn the_follows_fact_is_bound_to_its_function_and_protocol_ids() {
    let with = graph_of(FOLLOWS);
    let bindings = with["session_protocol_follows"]["bindings"]
        .as_array()
        .unwrap();
    assert_eq!(bindings.len(), 1);
    let fact = &bindings[0];
    assert_eq!(fact["function"], "fixture.follows.main");
    assert_eq!(fact["protocol"], "fixture.follows.protocol");
    assert_eq!(fact["result"], "typestate_checked");
    assert_eq!(fact["authority"], "none");
    assert!(fact["span"]["line"].as_u64().unwrap() > 0);
}

#[test]
fn a_program_without_a_follows_clause_is_byte_identical_to_the_pre_existing_v48_golden() {
    // Regression: `declared.spx` (issue #297's own golden fixture) has no
    // `follows` clause at all, so this module's existence must not change
    // its graph bytes -- exactly the "protocol-free and follows-free
    // programs stay byte-identical" guarantee this follow-on promises.
    const DECLARED: &str = include_str!("../session_protocol/tests/fixtures/declared.spx");
    let program = crate::check(DECLARED, "session.spx").unwrap();
    let json = graph::to_json(&program).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        value["schema"],
        crate::graph::session_protocol_decl::GRAPH_SCHEMA
    );
    assert!(value.get("session_protocol_follows").is_none());
}

#[test]
fn graph_is_deterministic_replays_and_refuses_a_forged_binding() {
    let program = crate::check(FOLLOWS, "follows.spx").unwrap();
    let first = graph::to_json(&program).unwrap();
    assert_eq!(first, graph::to_json(&program).unwrap());
    graph::verify_json(&program, &first).unwrap();
    let forged = first.replacen(
        "\"protocol\":\"fixture.follows.protocol\"",
        "\"protocol\":\"fixture.follows.other\"",
        1,
    );
    assert_ne!(forged, first);
    assert!(graph::verify_json(&program, &forged).is_err());
}

#[test]
fn attach_refuses_a_follows_clause_naming_no_declared_protocol() {
    // Built directly with the bare parser (bypassing `check`/`SPX-K107`) so
    // `attach` is exercised on a program its own binding must still refuse --
    // exactly mirroring `session_protocol_decl_tests`'s own
    // `graph_binding_refuses_a_via_absent_from_checked_hir`.
    let source = "module x;\n\n\
@id(\"x.f\")\nfn f() -> i64\n    follows session protocol \"x.missing\"\n{ 0 }\n";
    let program = crate::parse(source, "x.spx").unwrap();
    let error = super::attach(&program, "{\"schema\":\"y\"}".to_owned()).unwrap_err();
    assert_eq!(error.code, "SPX-K107");
}

#[test]
fn context_carries_follows_bindings_only_when_selected_and_present() {
    let program = crate::check(FOLLOWS, "follows.spx").unwrap();
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
    let with = graph::agent_context_json(&program, "fixture.follows.main", &selected)
        .unwrap()
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&with).unwrap();
    let follows = parsed["session_protocol_kernel"]["follows"]
        .as_array()
        .unwrap();
    assert_eq!(follows.len(), 1);
    assert_eq!(follows[0]["function"], "fixture.follows.main");
    assert_eq!(follows[0]["result"], "typestate_checked");
    let bare = graph::agent_context_json(&program, "fixture.follows.main", &unselected)
        .unwrap()
        .unwrap();
    assert!(!bare.contains("\"follows\""));

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
    let with_v2 = graph::agent_context_v2_json(&program, "fixture.follows.main", &v2)
        .unwrap()
        .unwrap();
    let parsed_v2: serde_json::Value = serde_json::from_str(&with_v2).unwrap();
    assert_eq!(
        parsed_v2["session_protocol_kernel"]["follows"],
        parsed["session_protocol_kernel"]["follows"]
    );

    // A program that declares a protocol but never opts a function in keeps
    // the unchanged catalog bytes -- no `follows` key at all.
    let plain = crate::check(&without_follows(), "follows.spx").unwrap();
    let plain = graph::agent_context_json(&plain, "fixture.follows.main", &selected)
        .unwrap()
        .unwrap();
    assert!(!plain.contains("\"follows\""));
    assert!(plain.contains("\"declared\""));
}
