//! Source/HIR lifetime controls for the selected receiver-tied Url view.
use super::*;

const PREFIX: &str = r#"module url.loans;
@id("url.resource") resource Url { @id("url.resource.drop") drop import "url.drop"; }
@id("url.host") interface Host permits { } {
 @id("url.drop") import fn drop_url(url: own Url) -> unit effects { } failure infallible consumes url always;
 @id("url.new") import rust selected fn url_new from "url_alias::Url::parse" effects { } failure infallible;
 @id("url.view") import rust selected fn url_view from "url_alias::Url::as_str" effects { } failure infallible;
}
@id("url.consume") fn consume(value: own Url) -> i64 { 0 }
@id("url.main") fn main() -> i64 { 0 }
"#;

fn bound(function: &str) -> semaprax::ast::Program {
    let source = format!("{PREFIX}\n@id(\"url.inspect\") {function}\n");
    let mut program = semaprax::parse(&source, Path::new("url-loans.spx")).unwrap();
    let admitted = RustApiIndex::admit_extractor_output(include_bytes!(
        "../../../semaprax-rust-api-index/fixtures/url-2.5.8-index-envelope.json"
    ))
    .unwrap();
    let index = RustApiIndex::replay(admitted.canonical_json().as_bytes()).unwrap();
    let types = program.types.clone();
    for import in program.interfaces.iter_mut().flat_map(|i| &mut i.imports) {
        if !import.index_selected {
            continue;
        }
        let (path, receiver) = if import.stable_id == "url.new" {
            ("url::Url::parse", "none")
        } else {
            ("url::Url::as_str", "shared")
        };
        let item = index.select_closed_url_method(path).unwrap();
        assert!(semaprax::native_rust_binding::bind_selected_url_signature(
            import,
            &types,
            &item.signature,
            index.digest(),
            receiver,
        )
        .unwrap());
    }
    program
}

#[test]
fn indexed_url_loan_source_negative_matrix() {
    let valid = bound("fn inspect(owner: own Url) -> i64 { let view = url_view(owner); let bytes = str_as_bytes(view); if byte_len(bytes) == 0usize { 0 } else { 1 } }");
    let diagnostics = semaprax::verify::verify(&valid);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let hir = semaprax::hir::resolve(&valid).unwrap();
    semaprax::hir::validate(&hir).unwrap();
    let function = hir
        .functions
        .iter()
        .find(|f| f.id.as_str() == "url.inspect")
        .unwrap();
    assert!(
        function.loan_plan.loans.len() >= 2,
        "returned view and byte view retain loans"
    );
    for (name, function, expected) in [
        ("move after view", "fn inspect(owner: own Url) -> i64 { let view = url_view(owner); let moved = owner; let bytes = str_as_bytes(view); if byte_len(bytes) == 0usize { 0 } else { 1 } }", "SPX-T265"),
        ("drop during view", "fn inspect(owner: own Url) -> i64 { let view = url_view(owner); let dropped = consume(owner); let bytes = str_as_bytes(view); if byte_len(bytes) == 0usize { 0 } else { 1 } }", "SPX-T265"),
        ("read after move", "fn inspect(owner: own Url) -> i64 { let dropped = consume(owner); let view = url_view(owner); let bytes = str_as_bytes(view); if byte_len(bytes) == 0usize { 0 } else { 1 } }", "SPX-O101"),
        ("replacement while borrowed", "fn inspect(owner: own Url, replacement: own Url) -> i64 { let mut local = owner; let view = url_view(local); local = replacement; let bytes = str_as_bytes(view); if byte_len(bytes) == 0usize { 0 } else { 1 } }", "SPX-T265"),
        ("returned view escape", "fn inspect(owner: own Url) -> str { url_view(owner) }", "SPX-O116"),
        ("temporary receiver", "fn inspect(owner: own Url) -> i64 { let view = url_view({ owner }); let bytes = str_as_bytes(view); if byte_len(bytes) == 0usize { 0 } else { 1 } }", "SPX-B107"),
    ] {
        let diagnostics = semaprax::verify::verify(&bound(function));
        assert!(diagnostics.iter().any(|d| d.code == expected), "{name}: expected {expected}, got {diagnostics:?}");
        assert!(diagnostics.iter().filter(|d| d.code == expected).all(|d| d.span.is_some()), "{name}: source-located refusal");
    }
    // Suspension is conservatively refused even if this particular run would
    // resume immediately: persisted snapshots cannot carry a live Url loan.
    let checkpoint = bound("fn inspect(owner: borrow Url) -> i64 yields i64 -> i64 { let view = url_view(owner); let response = yield 0; let bytes = str_as_bytes(view); if byte_len(bytes) == 0usize { response } else { 1 } }");
    let diagnostics = semaprax::hir::resolve(&checkpoint).unwrap_err();
    assert!(
        diagnostics.iter().any(|d| d.code == "SPX-T305"),
        "checkpoint escape: {diagnostics:?}"
    );
}
