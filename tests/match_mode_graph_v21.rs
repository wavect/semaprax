use std::path::Path;

use semaprax::graph;
use semaprax::parse;
use sha2::{Digest, Sha256};

const LEGACY: &str = r#"
module test.legacy_match_graph;

@id("legacy.pair")
record Pair {
    @id("legacy.pair.left") left: i64,
    @id("legacy.pair.right") right: bool,
}

@id("legacy.read")
fn read(value: Pair) -> i64 {
    match value { Pair { left, right: _ } => left, }
}

@id("app.main")
fn main() -> i64 { read(Pair { left: 7, right: true }) }
"#;

const OWNED: &str = r#"
module test.owned_match_graph;

@id("owned.packet")
record Packet {
    @id("owned.packet.left") left: Bytes,
    @id("owned.packet.right") right: Bytes,
    @id("owned.packet.marker") marker: i64,
}

@id("owned.take")
fn take(value: own Packet) -> i64 {
    match own value { Packet { left, right, marker } => marker, }
}

@id("owned.inspect")
fn inspect(value: borrow Packet) -> i64 {
    match borrow value { Packet { left, right: _, marker } => marker, }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program(source: &str, name: &str) -> semaprax::ast::Program {
    parse(source, Path::new(name)).unwrap()
}

fn digest(text: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}

#[test]
fn value_match_keeps_legacy_schema_bytes_and_implicit_mode() {
    let graph = graph::to_json(&program(LEGACY, "legacy-match-graph.spx")).unwrap();
    assert!(graph.starts_with("{\"schema\":\"semaprax.graph.v13\","));
    assert!(!graph.contains("\"kind\":\"match\",\"ownership_mode\""));
    assert_eq!(
        digest(&graph),
        "939d6609bc504c14f45e837c50a99bbd9ed33abf2557d609b547e6cd32f82678"
    );
}

#[test]
fn v21_stays_fail_closed_for_patch_evidence_and_review_consumers() {
    let diagnostic = graph::reject_evidence_schema("semaprax.graph.v21").unwrap_err();
    assert_eq!(diagnostic.code, "SPX-G410");
    assert!(diagnostic.message.contains("ownership-aware match"));
}

#[test]
fn valid_owned_byte_record_matches_select_and_pin_graph_v21() {
    let graph = graph::to_json(&program(OWNED, "owned-match-graph.spx")).unwrap();
    assert!(graph.starts_with("{\"schema\":\"semaprax.graph.v21\","));
    assert!(graph.contains("\"ownership_mode\":\"own\""));
    assert!(graph.contains("\"ownership_mode\":\"borrow\""));
    assert!(graph.contains(
        "\"cleanup\":{\"kind\":\"cleanup_plan\",\"schema\":\"semaprax.cleanup-plan.v5\""
    ));
    assert_eq!(
        digest(&graph),
        "77590aadd5154795b4f7cb187f7a9bb84e97b60943de09bffed4448d9059e66a"
    );
}
