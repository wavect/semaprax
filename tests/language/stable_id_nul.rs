use std::path::Path;

use semaprax::{graph, parse, verify};

const IDENTITIES: &str = r#"module test.stable_id_nul;

@id("token.type")
resource Token {
    @id("token.drop")
    drop import "host.dispose";
}

@id("pair.type")
record Pair {
    @id("pair.value")
    value: i64,
}

@id("host.interface")
interface Host permits {} {
    @id("host.dispose")
    import fn dispose(token: own Token) -> unit
        effects {}
        failure infallible
        consumes token always;
}

@id("helper.function")
fn helper() -> i64 { 1 }

@id("app.main")
fn main() -> i64 { helper() }
"#;

fn replace_id(id: &str) -> String {
    IDENTITIES.replacen(
        &format!("@id(\"{id}\")"),
        &format!("@id(\"{id}\0forged\")"),
        1,
    )
}

#[test]
fn literal_nul_is_rejected_for_every_source_stable_identity_kind() {
    for (id, expected_code, subject) in [
        ("token.type", "SPX-S102", "resource `Token`"),
        ("pair.type", "SPX-S102", "record `Pair`"),
        ("token.drop", "SPX-O113", "resource lifecycle `Token.drop`"),
        ("pair.value", "SPX-S102", "field `Pair.value`"),
        ("host.interface", "SPX-I403", "interface `Host`"),
        ("host.dispose", "SPX-I403", "import `Host.dispose`"),
        ("helper.function", "SPX-S102", "function `helper`"),
    ] {
        let source = replace_id(id);
        let program = parse(&source, Path::new("stable-id-nul.spx"))
            .expect("a literal NUL is currently representable in a source string literal");
        let diagnostics = verify::verify(&program);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| diagnostic.message.contains("forbid NUL"))
            .unwrap_or_else(|| panic!("missing NUL diagnostic for {subject}: {diagnostics:?}"));
        assert_eq!(diagnostic.code, expected_code, "wrong code for {subject}");
        assert!(
            diagnostic.message.starts_with(subject),
            "wrong subject for {subject}: {}",
            diagnostic.message
        );
    }
}

#[test]
fn source_graph_rejects_nul_before_serialization() {
    let source = replace_id("helper.function");
    let program = parse(&source, Path::new("stable-id-nul-graph.spx")).unwrap();
    let diagnostics = graph::to_json(&program).unwrap_err();
    assert_eq!(diagnostics[0].code, "SPX-S102");
    assert!(diagnostics[0].message.contains("forbid NUL"));
}

#[test]
fn lifecycle_logical_import_key_nul_is_normalized_before_lookup_or_graph_serialization() {
    let source = IDENTITIES.replacen(
        "drop import \"host.dispose\";",
        "drop import \"host.dispose\0forged\";",
        1,
    );
    let program = parse(&source, Path::new("lifecycle-key-nul.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, "SPX-O113");
    assert_eq!(
        diagnostics[0].message,
        "resource lifecycle `Token.drop` has an invalid logical import key; persistent identities forbid NUL"
    );

    let graph_diagnostics = graph::to_json(&program).unwrap_err();
    assert_eq!(graph_diagnostics.len(), 1);
    assert_eq!(graph_diagnostics[0].code, "SPX-O113");
    assert_eq!(graph_diagnostics[0].message, diagnostics[0].message);
}

#[test]
fn duplicate_invalid_import_ids_never_enter_identity_or_logical_key_maps() {
    let source = "module test.invalid_import_ids;\n\
@id(\"token.type\") resource Token { @id(\"token.drop\") drop trivial; }\n\
@id(\"host.interface\") interface Host permits {} {\n\
    @id(\"host.bad\0key\") import fn first(token: own Token) -> unit effects {} failure infallible consumes token always;\n\
    @id(\"host.bad\0key\") import fn second(token: own Token) -> unit effects {} failure infallible consumes token always;\n\
}\n\
@id(\"app.main\") fn main() -> i64 { 0 }\n";
    let program = parse(source, Path::new("duplicate-invalid-import-ids.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.code == "SPX-I403"
            && diagnostic
                .message
                .contains("persistent identities forbid NUL")
            && !diagnostic.message.contains('\0')
    }));
}

#[test]
fn backslash_zero_remains_an_unsupported_string_escape() {
    let source = IDENTITIES.replacen("@id(\"helper.function\")", "@id(\"helper\\0function\")", 1);
    assert_eq!(
        parse(&source, Path::new("stable-id-backslash-zero.spx"))
            .unwrap_err()
            .code,
        "SPX-P005"
    );
}

#[test]
fn empty_explicit_function_identity_is_a_located_source_diagnostic() {
    let source = "module test.empty_id;\n@id(\"\")\nfn main() -> i64 { 0 }\n";
    let program = parse(source, Path::new("empty-id.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    let diagnostic = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == "SPX-S102")
        .expect("empty identity must fail at source verification");
    assert_eq!(
        diagnostic.span.map(|span| (span.line, span.column)),
        Some((3, 4))
    );
    assert!(diagnostic
        .help
        .as_deref()
        .is_some_and(|help| help.contains("dotted stable identity")));
}
