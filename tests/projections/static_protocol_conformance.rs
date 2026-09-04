//! Static conformance evidence authored here; intentionally unrun locally.
use semaprax::{format, hir, parse, static_protocol, verify};
use serde_json::json;

const SOURCE: &str = r#"module conformance;
@id("point") record Point { @id("point.x") x: i64, }
@id("has-x") protocol HasX {
    @id("has-x.get") fn get(self: Self) -> i64;
}
@id("point.has-x") impl "has-x" for "point" {
    "has-x.get" = "point.get";
}
@id("point.get") fn get(point: Point) -> i64 { point.x }
@id("main") fn main() -> i64 { get(Point { x: 7 }) }
"#;

fn program(source: &str) -> semaprax::ast::Program {
    parse(source, "protocol.spx").unwrap()
}
fn reject(source: &str, code: &str) {
    let program = program(source);
    let diagnostics = verify::verify(&program);
    assert!(
        diagnostics.iter().any(|item| item.code == code),
        "{diagnostics:?}"
    );
    assert!(
        hir::resolve(&program).is_err(),
        "backend HIR must reject the same invalid table"
    );
    assert!(static_protocol::facts(&program).is_err());
}

#[test]
fn source_roundtrip_preserves_protocols_and_impls_and_exposes_admitted_facts() {
    let original = program(SOURCE);
    hir::resolve(&original).unwrap();
    let canonical = format::canonical(&original);
    assert!(canonical.contains("protocol HasX {"));
    assert!(canonical.contains("impl \"has-x\" for \"point\" {"));
    let reparsed = program(&canonical);
    assert_eq!(format::canonical(&reparsed), canonical);
    assert_eq!(reparsed.protocols.len(), 1);
    assert_eq!(reparsed.implementations.len(), 1);
    let facts = static_protocol::facts(&reparsed).unwrap();
    assert_eq!(facts["full_source_admitted"], true);
    assert_eq!(
        facts["protocols"][0]["methods"][0]["params"][0]["type"],
        "Self"
    );
    assert_eq!(
        facts["implementations"][0]["members"],
        json!([{"method_id":"has-x.get","function_id":"point.get"}])
    );
    assert_eq!(facts["implementations"][0]["receiver_id"], "point");
    assert_eq!(facts["source_authority"], false);
}

#[test]
fn function_display_rename_preserves_static_bindings() {
    let renamed = SOURCE
        .replace("fn get(point:", "fn fetch(point:")
        .replace("{ get(Point", "{ fetch(Point");
    let before = static_protocol::facts(&program(SOURCE)).unwrap();
    let after = static_protocol::facts(&program(&renamed)).unwrap();
    assert_eq!(before["implementations"], after["implementations"]);
}

#[test]
fn complete_exact_local_inventory_is_required() {
    reject(
        &SOURCE.replace("\"has-x.get\" = \"point.get\";", ""),
        "SPX-Q107",
    );
    reject(
        &SOURCE.replace("\"has-x.get\" =", "\"other.method\" ="),
        "SPX-Q107",
    );
    // A member function that is not local is a cross-module dependency, so the
    // sidecar lane requires its exact typed import before any local binding
    // check can apply.
    reject(
        &SOURCE.replace("= \"point.get\"", "= \"missing.function\""),
        "SPX-Q106",
    );
    reject(
        &SOURCE.replace("impl \"has-x\"", "impl \"missing.protocol\""),
        "SPX-Q106",
    );
    reject(
        &SOURCE.replace("for \"point\"", "for \"missing.record\""),
        "SPX-Q106",
    );
    reject(&SOURCE.replace("@id(\"point.get\") fn", "fn"), "SPX-Q106");
    reject(
        &SOURCE.replace("@id(\"point\") record", "record"),
        "SPX-Q106",
    );
    reject(
        &SOURCE.replace("@id(\"point.has-x\")", "@id(\"point.x\")"),
        "SPX-Q108",
    );
}

#[test]
fn signature_modes_effects_and_preconditions_cannot_be_strengthened() {
    reject(
        &SOURCE.replace(
            "fn get(point: Point) -> i64",
            "fn get(point: Point) -> bool",
        ),
        "SPX-Q107",
    );
    reject(
        &SOURCE.replace("fn get(point: Point)", "fn get(point: own Point)"),
        "SPX-Q107",
    );
    reject(
        &SOURCE.replace("fn get(point: Point)", "fn get(point: Point, extra: i64)"),
        "SPX-Q107",
    );
    reject(
        &SOURCE.replace(
            "-> i64 { point.x }",
            "-> i64 requires point.x >= 0 { point.x }",
        ),
        "SPX-Q107",
    );
    reject(
        &SOURCE.replace("-> i64 { point.x }", "-> i64 uses { io } { point.x }"),
        "SPX-Q107",
    );
    reject(
        &SOURCE.replace("fn get(point: Point)", "fn get<T>(point: Point)"),
        "SPX-Q107",
    );
    // Exact signature alone is never enough to admit a malformed body.
    let bad_body = program(&SOURCE.replace("{ point.x }", "{ true }"));
    static_protocol::validate(&bad_body).unwrap();
    assert!(hir::resolve(&bad_body).is_err());
    assert!(static_protocol::facts(&bad_body).is_err());
}

#[test]
fn duplicate_pair_and_duplicate_member_functions_reject() {
    let doubled = format!("{SOURCE}\n@id(\"duplicate.impl\") impl \"has-x\" for \"point\" {{ \"has-x.get\" = \"point.get\"; }}\n");
    reject(&doubled, "SPX-Q108");
    let duplicate = SOURCE
        .replace(
            "fn get(self: Self) -> i64;",
            "fn get(self: Self) -> i64; @id(\"has-x.other\") fn other(self: Self) -> i64;",
        )
        .replace(
            "\"has-x.get\" = \"point.get\";",
            "\"has-x.get\" = \"point.get\"; \"has-x.other\" = \"point.get\";",
        );
    reject(&duplicate, "SPX-Q107");
}

#[test]
fn invalid_protocol_signature_rejects_even_without_an_impl() {
    let source = "module t; @id(\"p\") protocol P { @id(\"p.m\") fn m(self: i64) -> i64; } @id(\"main\") fn main() -> i64 { 0 }";
    reject(source, "SPX-Q104");
}

#[test]
fn explicit_impl_identity_and_capacity_are_fail_closed() {
    assert_eq!(
        parse(&SOURCE.replace("@id(\"point.has-x\") ", ""), "protocol.spx")
            .unwrap_err()
            .code,
        "SPX-Q106"
    );
    reject(
        &SOURCE.replace(
            "point.has-x",
            &"x".repeat(static_protocol::MAX_STABLE_ID_BYTES + 1),
        ),
        "SPX-Q106",
    );
    let mut ast = program(SOURCE);
    ast.implementations =
        vec![ast.implementations[0].clone(); static_protocol::MAX_IMPLEMENTATIONS + 1];
    assert_eq!(
        static_protocol::validate(&ast).unwrap_err().code,
        "SPX-Q109"
    );
}

#[test]
fn legacy_protocol_projection_rejects_real_impls_without_reporting_empty_conformance() {
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "spx-static-protocol-{}-{}.spx",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, SOURCE).unwrap();
    let result = semaprax::protocol_check::generate(&path, &Default::default());
    let _ = std::fs::remove_file(path);
    let diagnostics = result.unwrap_err();
    assert!(diagnostics.iter().any(|item| item.code == "SPX-Q110"));
}
