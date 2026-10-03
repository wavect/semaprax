use std::path::Path;

use semaprax::format;
use semaprax::hir::{
    self, DeclarationId, ResolvedExprKind, ResolvedImportFailure, ResolvedImportResultKind,
    ResolvedType,
};
use semaprax::native_rust_binding::{prepare_scalar_binding, SelectedRustItem};
use semaprax::{graph, parse, wasm};

const SOURCE: &str = r#"module test.native_rust;

@id("rust.host")
interface RustHost
    permits {  }
{
    @id("rust.host.combine")
    import rust fn combine(left: i64, selected: bool) -> i64
        effects {  }
        failure status "rust.test";
    @id("rust.host.invert")
    import rust fn invert(value: bool) -> bool
        effects {  }
        failure infallible;
    @id("rust.host.ping")
    import rust fn ping(value: i64) -> unit
        effects {  }
        failure status "1x";
}

@id("test.call_combine")
fn call_combine(value: i64, selected: bool) -> i64
{
    combine(value, selected)
}

@id("test.call_invert")
fn call_invert(value: bool) -> bool
{
    invert(value)
}

@id("test.ping_once")
fn ping_once(value: i64) -> i64
{
    let acknowledged = ping(value);
    1
}

@id("test.main")
fn main() -> i64
{
    call_combine(41, true)
}
"#;

const OPAQUE_OWNER_SOURCE: &str = r#"module test.native_rust_owner;

@id("owner.regex") resource Regex {
    @id("owner.regex.drop") drop trivial;
}

@id("owner.host") interface RegexHost permits {  } {
    @id("owner.new")
    import rust fn regex_new(pattern: i64) -> Regex from "fixture_regex::Regex::new"
        effects {  }
        failure infallible;
    @id("owner.matches")
    import rust fn regex_matches(regex: own Regex, value: i64) -> bool from "fixture_regex::Regex::consume_matches"
        effects {  }
        failure infallible;
}

@id("owner.run") fn run(pattern: i64, value: i64) -> bool {
    let regex = regex_new(pattern);
    regex_matches(regex, value)
}

@id("owner.main") fn main() -> i64 {
    if run(7, 7) { 1 } else { 0 }
}
"#;

#[test]
fn native_rust_opaque_owner_result_is_affine_in_source_and_hir() {
    let path = Path::new("native-rust-owner.spx");
    let program = parse(OPAQUE_OWNER_SOURCE, path).unwrap();
    let diagnostics = semaprax::verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let resolved = hir::resolve(&program).unwrap();
    let imports = &resolved.interfaces[0].imports;
    assert!(matches!(
        &imports[0].result.kind,
        ResolvedImportResultKind::OwnedResource { ref resource } if resource.as_str() == "owner.regex"
    ));
    assert_eq!(imports[0].result.ownership, hir::OwnershipMode::Own);
    assert_eq!(imports[1].parameters[0].ownership, hir::OwnershipMode::Own);
    let run = resolved
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &run.body.kind else {
        panic!("owner function body")
    };
    let hir::ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("owner constructor binding")
    };
    assert_eq!(value.ownership, hir::OwnershipMode::Own);
    hir::validate(&resolved).unwrap();
    let graph = graph::to_json(&program).unwrap();
    assert!(graph.contains("\"type\":\"Regex\""));

    let moved = OPAQUE_OWNER_SOURCE.replace(
        "regex_matches(regex, value)",
        "let first = regex_matches(regex, value);\n    regex_matches(regex, value)",
    );
    let diagnostics = semaprax::check(&moved, path).unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-O101"));
}

fn selected_item<'a>(alias: &'a str, path: &'a str, signature: &'a str) -> SelectedRustItem<'a> {
    SelectedRustItem {
        cargo_alias: alias,
        package_name: "fixture-api",
        package_version: "1.0.0",
        package_source_sha256:
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        index_digest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        target: "x86_64-unknown-linux-gnu",
        feature_digest: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        path,
        kind: "function",
        receiver: "none",
        signature,
        supported: true,
    }
}

#[test]
fn selected_scalar_binding_uses_exact_package_and_import_identity() {
    let program = hir::resolve(&parse(SOURCE, Path::new("binding.spx")).unwrap()).unwrap();
    let import = &program.interfaces[0].imports[0];
    let signature = "fn combine(left: i64, selected: bool) -> i64";
    let first = prepare_scalar_binding(import, selected_item("first", "first::combine", signature))
        .unwrap();
    let second = prepare_scalar_binding(
        import,
        selected_item("second", "second::combine", signature),
    )
    .unwrap();
    assert_eq!(first.import_id, "rust.host.combine");
    assert_ne!(first.physical_symbol, second.physical_symbol);
    assert!(first.physical_symbol.starts_with("spx_ri04_"));
    assert_eq!(
        first,
        prepare_scalar_binding(import, selected_item("first", "first::combine", signature))
            .unwrap()
    );
    let mut other_version = selected_item("first", "first::combine", signature);
    other_version.package_version = "2.0.0";
    assert_ne!(
        first.physical_symbol,
        prepare_scalar_binding(import, other_version)
            .unwrap()
            .physical_symbol
    );
    let aliased_source = SOURCE.replacen(
        "    @id(\"rust.host.invert\")",
        "    @id(\"rust.host.combine_alias\")\n    import rust fn combine_alias(left: i64, selected: bool) -> i64 from \"first::combine\"\n        effects {  }\n        failure status \"rust.test\";\n    @id(\"rust.host.invert\")",
        1,
    );
    let aliased = hir::resolve(&parse(&aliased_source, Path::new("aliased.spx")).unwrap()).unwrap();
    let aliased_import = aliased.interfaces[0]
        .imports
        .iter()
        .find(|candidate| candidate.id.as_str() == "rust.host.combine_alias")
        .unwrap();
    let same_item_different_import = prepare_scalar_binding(
        aliased_import,
        selected_item("first", "first::combine", signature),
    )
    .unwrap();
    assert_ne!(
        first.physical_symbol, same_item_different_import.physical_symbol,
        "distinct persistent Semaprax import IDs cannot collide on one Rust item"
    );
    let mut other_index = selected_item("first", "first::combine", signature);
    other_index.index_digest =
        "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    assert_ne!(
        first.physical_symbol,
        prepare_scalar_binding(import, other_index)
            .unwrap()
            .physical_symbol
    );
}

#[test]
fn selected_borrowed_rust_shapes_refuse_with_source_located_reasons() {
    let path = Path::new("borrowed-rust.spx");
    let program = hir::resolve(&parse(SOURCE, path).unwrap()).unwrap();
    let import = &program.interfaces[0].imports[0];
    for (signature, reason) in [
        (
            "fn combine(value: &str) -> bool",
            "borrowed Rust text requires an authenticated invocation loan",
        ),
        (
            "fn combine<'h>(value: &'h str) -> bool",
            "borrowed Rust text requires an authenticated invocation loan",
        ),
        (
            "fn combine(value: &[u8]) -> bool",
            "borrowed Rust bytes require an authenticated invocation loan",
        ),
        (
            "fn combine(value: &mut [u8]) -> bool",
            "mutable Rust reference requires a verified exclusive loan",
        ),
        (
            "fn combine(value: &str) -> &str",
            "returned Rust reference has no representable owner relation",
        ),
        (
            "fn combine(&self) -> &str",
            "returned Rust reference requires an owner-bound live view",
        ),
        (
            "fn combine(value: Pin<&mut [u8]>) -> bool",
            "pinned Rust reference requires an unsupported stable owner relation",
        ),
        (
            "fn combine(value: &UnsafeCell<u8>) -> bool",
            "interior-mutable Rust reference requires an exclusive loan model",
        ),
        (
            "fn combine(value: *const u8) -> bool",
            "raw Rust pointer has no verified provenance or initialized extent",
        ),
        (
            "for<'a> fn combine(value: &'a str) -> bool",
            "higher-ranked Rust reference requires an unsupported lifetime relation",
        ),
    ] {
        let error = prepare_scalar_binding(
            import,
            selected_item("fixture", "fixture::combine", signature),
        )
        .unwrap_err();
        assert_eq!(error.code, "SPX-B145", "{signature}");
        assert_eq!(error.message, reason, "{signature}");
        assert_eq!(error.span, Some(import.span), "{signature}");
    }
}

#[test]
fn selected_rust_result_is_a_domain_value_with_distinct_graph_identity() {
    let source = r#"module result.fixture;
@id("result.host") interface Host permits {  } {
    @id("result.divide") import rust selected fn divide from "fixture::divide"
        effects {  } failure status "result.bridge";
}
@id("result.project") fn project(left: i64, right: i64) -> i64 {
    match divide(left, right) {
        Result::Ok { value: value } => value,
        Result::Err { error: error } => error + 100,
    }
}
@id("result.main") fn main() -> i64 { project(8, 2) }
"#;
    let path = Path::new("selected-result.spx");
    let mut parsed = parse(source, path).unwrap();
    let import = &mut parsed.interfaces[0].imports[0];
    let signature = "fn divide(left: i64, right: i64) -> core::result::Result<i64, i64>";
    semaprax::native_rust_binding::bind_selected_scalar_signature(
        import,
        signature,
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "none",
    )
    .unwrap();
    assert_eq!(import.result, semaprax::ast::ImportResult::ResultI64I64);
    let errors = semaprax::verify::verify(&parsed);
    assert!(errors.is_empty(), "{errors:?}");
    let resolved = hir::resolve(&parsed).unwrap();
    assert_eq!(
        resolved.interfaces[0].imports[0].result.kind,
        ResolvedImportResultKind::ResultI64I64
    );
    let binding = prepare_scalar_binding(
        &resolved.interfaces[0].imports[0],
        selected_item("fixture", "fixture::divide", signature),
    )
    .unwrap();
    assert_eq!(binding.receiver, "none");
    let mut drifted = selected_item("fixture", "fixture::divide", signature);
    drifted.index_digest =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    assert_eq!(
        prepare_scalar_binding(&resolved.interfaces[0].imports[0], drifted)
            .unwrap_err()
            .code,
        "SPX-B146"
    );
    let json = graph::to_json(&parsed).unwrap();
    assert!(json.contains("\"schema\":\"semaprax.graph.v55\""));
    assert!(json.contains("\"result\":\"Result<i64, i64>\""));
    assert!(json.contains("\"selected_index_digest\":\"sha256:bbbb"));
    let canonical = format::canonical(&parsed);
    let mut reparsed = parse(&canonical, path).unwrap();
    assert!(reparsed.interfaces[0].imports[0].index_selected);
    semaprax::native_rust_binding::bind_selected_scalar_signature(
        &mut reparsed.interfaces[0].imports[0],
        signature,
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "none",
    )
    .unwrap();
    assert_eq!(graph::to_json(&reparsed).unwrap(), json);
    assert_eq!(
        semaprax::check(source, path).unwrap_err()[0].code,
        "SPX-B147"
    );
}

#[test]
fn selected_scalar_binding_rejects_unchecked_or_drifting_metadata() {
    let program = hir::resolve(&parse(SOURCE, Path::new("binding.spx")).unwrap()).unwrap();
    let import = &program.interfaces[0].imports[0];
    let cases = [
        (
            selected_item(
                "api",
                "other::combine",
                "fn combine(left: i64, selected: bool) -> i64",
            ),
            "SPX-B143",
        ),
        (
            selected_item(
                "api",
                "api::combine",
                "fn combine(left: bool, selected: bool) -> i64",
            ),
            "SPX-B146",
        ),
        (
            selected_item(
                "api",
                "api::combine",
                "fn combine<T>(left: i64, selected: bool) -> i64",
            ),
            "SPX-B145",
        ),
        (
            selected_item(
                "api",
                "api::combine",
                "fn combine(left: i64, selected: bool) -> Result<i64, E>",
            ),
            "SPX-B145",
        ),
    ];
    for (item, code) in cases {
        let diagnostic = prepare_scalar_binding(import, item).unwrap_err();
        assert_eq!(diagnostic.code, code);
        assert_eq!(diagnostic.span, Some(import.span));
    }
    let mut inaccessible = selected_item(
        "api",
        "api::combine",
        "fn combine(left: i64, selected: bool) -> i64",
    );
    inaccessible.supported = false;
    let inaccessible_error = prepare_scalar_binding(import, inaccessible).unwrap_err();
    assert_eq!(inaccessible_error.code, "SPX-B141");
    assert_eq!(inaccessible_error.span, Some(import.span));
    let mut method = selected_item(
        "api",
        "api::combine",
        "fn combine(left: i64, selected: bool) -> i64",
    );
    method.kind = "inherent_method";
    method.receiver = "shared";
    let method_error = prepare_scalar_binding(import, method).unwrap_err();
    assert_eq!(method_error.code, "SPX-B144");
    assert_eq!(method_error.span, Some(import.span));
}

#[test]
fn indexed_rust_source_path_round_trips_and_selects_graph_v52() {
    let source = SOURCE.replacen(
        "import rust fn combine(left: i64, selected: bool) -> i64",
        "import rust fn combine(left: i64, selected: bool) -> i64 from \"api::combine\"",
        1,
    );
    let program = parse(&source, Path::new("indexed.spx")).unwrap();
    let canonical = format::canonical(&program);
    assert_eq!(canonical, source);
    let reparsed = parse(&canonical, Path::new("canonical-indexed.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    let resolved = hir::resolve(&program).unwrap();
    let canonical_resolved = hir::resolve(&reparsed).unwrap();
    assert_eq!(
        resolved.interfaces[0].imports[0].rust_path.as_deref(),
        Some("api::combine")
    );
    assert_eq!(
        resolved.interfaces[0].imports[0].id, canonical_resolved.interfaces[0].imports[0].id,
        "the persistent Semaprax import identity is independent of source-file path"
    );
    assert_eq!(
        resolved.interfaces[0].imports[0].id.as_str(),
        "rust.host.combine"
    );
    hir::validate(&resolved).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert_eq!(json, graph::to_json(&reparsed).unwrap());
    assert!(json.contains("\"schema\":\"semaprax.graph.v52\""));
    assert!(json.contains("\"rust_path\":\"api::combine\""));
    assert!(json.contains("\"effects\":[]"));
    assert!(json.contains("\"id\":\"rust.host.combine\""));
    assert_eq!(wasm::emit_module(&program).unwrap_err().code, "SPX-W114");
    assert_eq!(
        wasm::emit_resolved_module(&resolved).unwrap_err().code,
        "SPX-W114"
    );
    assert_eq!(
        semaprax::codegen::emit_hir_c(&resolved).unwrap_err().code,
        "SPX-B103"
    );
    let context_error = graph::context_json(&program, "test.call_combine", 1).unwrap_err();
    assert_eq!(context_error[0].code, "SPX-G218");
    let mut forged = resolved.clone();
    forged.interfaces[0].imports[0].rust_path = Some("combine".to_owned());
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
    let import = &resolved.interfaces[0].imports[0];
    assert!(prepare_scalar_binding(
        import,
        selected_item(
            "api",
            "api::combine",
            "fn combine(left: i64, selected: bool) -> i64"
        )
    )
    .is_ok());
    let unindexed_source = parse(SOURCE, Path::new("unindexed.spx")).unwrap();
    let unindexed = hir::resolve(&unindexed_source).unwrap();
    assert_eq!(
        unindexed.interfaces[0].imports[0].id, import.id,
        "adding a selected Rust path must not replace the persistent source identity"
    );
    let invalid_source = SOURCE.replacen(
        "import rust fn combine(left: i64, selected: bool) -> i64",
        "import rust fn combine(left: i64, selected: bool) -> i64 from \"api::bad-name\"",
        1,
    );
    let invalid_program = parse(&invalid_source, Path::new("invalid-indexed.spx")).unwrap();
    let source_error = hir::resolve(&invalid_program).unwrap_err();
    assert_eq!(source_error.len(), 1);
    assert_eq!(source_error[0].code, "SPX-B143");
    assert_eq!(
        source_error[0].message,
        "Rust API path must be a bounded package-qualified path"
    );
    assert_eq!(source_error[0].path.as_deref(), Some("invalid-indexed.spx"));
    assert_eq!(
        source_error[0].span,
        Some(invalid_program.interfaces[0].imports[0].span),
        "invalid selected paths diagnose the exact import declaration"
    );
    assert_eq!(
        prepare_scalar_binding(
            import,
            selected_item(
                "other",
                "other::combine",
                "fn combine(left: i64, selected: bool) -> i64"
            )
        )
        .unwrap_err()
        .code,
        "SPX-B143"
    );
}

#[test]
fn indexed_rust_source_path_rejects_unqualified_item() {
    let source = SOURCE.replacen(
        "import rust fn combine(left: i64, selected: bool) -> i64",
        "import rust fn combine(left: i64, selected: bool) -> i64 from \"combine\"",
        1,
    );
    let program = parse(&source, Path::new("invalid-indexed.spx")).unwrap();
    let diagnostic = hir::resolve(&program).unwrap_err();
    assert_eq!(diagnostic[0].code, "SPX-B143");
    assert!(diagnostic[0].span.is_some());
}

#[test]
fn native_rust_import_syntax_format_and_hir_are_exact_and_deterministic() {
    let parsed = parse(SOURCE, Path::new("native-rust.spx")).unwrap();
    assert_eq!(format::canonical(&parsed), SOURCE);
    let first = hir::resolve(&parsed).unwrap();
    let second = hir::resolve(&parse(SOURCE, Path::new("other.spx")).unwrap()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.interfaces.len(), 1);
    let imports = &first.interfaces[0].imports;
    assert_eq!(imports.len(), 3);
    assert!(imports.iter().all(|import| import.native_rust));
    assert_eq!(imports[0].id.as_str(), "rust.host.combine");
    assert_eq!(imports[0].parameters.len(), 2);
    assert_eq!(imports[0].parameters[0].ty, ResolvedType::I64);
    assert_eq!(imports[0].parameters[1].ty, ResolvedType::Bool);
    assert_eq!(imports[0].result.kind, ResolvedImportResultKind::I64);
    assert_eq!(
        imports[0].failure,
        ResolvedImportFailure::Status {
            domain_id: "rust.test".to_owned(),
            normalization: "semaprax.status.v1",
        }
    );
    assert_eq!(imports[1].result.kind, ResolvedImportResultKind::Bool);
    assert_eq!(imports[1].failure, ResolvedImportFailure::Infallible);
    assert_eq!(imports[2].result.kind, ResolvedImportResultKind::Unit);

    let wrapper = first
        .functions
        .iter()
        .find(|function| function.name == "call_combine")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &wrapper.body.kind else {
        panic!("wrapper body")
    };
    let ResolvedExprKind::NativeRustImportCall(call) = &tail.kind else {
        panic!("native Rust call")
    };
    assert_eq!(call.import.as_str(), "rust.host.combine");
    assert_eq!(call.args.len(), 2);
    assert_eq!(call.result, ResolvedImportResultKind::I64);
    assert_eq!(tail.ty, ResolvedType::I64);
    assert_eq!(call.expression, tail.id);
    hir::validate(&first).unwrap();

    let endpoint = SOURCE.replace("failure status \"1x\"", "failure status \"x1\"");
    hir::resolve(&parse(&endpoint, Path::new("endpoint.spx")).unwrap()).unwrap();
}

#[test]
fn native_rust_import_declaration_set_is_closed_with_exact_diagnostics() {
    let cases = [
        (SOURCE.replace("    @id(\"rust.host.combine\")\n", ""), "SPX-B107", "explicit persistent ID required"),
        (SOURCE.replace("left: i64", "left: borrow i64"), "SPX-B107", "scalar value signature required"),
        (SOURCE.replace("left: i64, selected: bool", "a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64,i:i64"), "SPX-B107", "scalar value signature required"),
        (SOURCE.replace("failure status \"rust.test\"", "failure status \"Rust/Test\""), "SPX-B107", "status domain is invalid"),
        (SOURCE.replace("import rust fn invert", "import rust fn combine"), "SPX-B107", "symbol collision"),
        (SOURCE.replace("permits {  }", "permits { allowed.effect }").replace("effects {  }\n        failure status \"rust.test\"", "effects { outside.effect }\n        failure status \"rust.test\""), "SPX-B107", "effect or capability mismatch"),
        (SOURCE.replace("effects {  }\n        failure status \"rust.test\"", "effects { repeated.effect, repeated.effect }\n        failure status \"rust.test\""), "SPX-B107", "effect or capability mismatch"),
        (SOURCE.replace("fn call_combine", "fn combine"), "SPX-B107", "symbol collision"),
    ];
    for (index, (source, code, message)) in cases.into_iter().enumerate() {
        let program = parse(&source, Path::new(&format!("hostile-{index}.spx"))).unwrap();
        let diagnostics = hir::resolve(&program).unwrap_err();
        assert_eq!(diagnostics.len(), 1, "case {index}: {diagnostics:?}");
        assert_eq!(diagnostics[0].code, code, "case {index}");
        assert!(
            diagnostics[0].message.contains(message),
            "case {index}: {}",
            diagnostics[0].message
        );
    }

    let omitted_failure = SOURCE.replace("\n        failure infallible", "");
    let diagnostic = parse(&omitted_failure, Path::new("omitted-failure.spx")).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-P106");
    assert_eq!(diagnostic.message, "expected keyword `failure`");

    let ordinary_scalar = SOURCE.replace("import rust fn combine", "import fn combine");
    let diagnostic = parse(&ordinary_scalar, Path::new("ordinary-scalar.spx")).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-P106");
    assert_eq!(diagnostic.message, "expected admitted import result type");

    let forged_unit_value = SOURCE.replace(
        "    let acknowledged = ping(value);\n    1",
        "    ping(value) + 1",
    );
    let diagnostics =
        hir::resolve(&parse(&forged_unit_value, Path::new("forged-unit-value.spx")).unwrap())
            .unwrap_err();
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert_eq!(diagnostics[0].code, "SPX-B107");
    assert_eq!(
        diagnostics[0].message,
        "Native Rust Interop declaration set is unsupported: scalar value signature required"
    );
}

#[test]
fn native_rust_imports_project_into_graph_v25_and_stay_out_of_wasm() {
    let program = parse(SOURCE, Path::new("native-rust.spx")).unwrap();
    let json = graph::to_json(&program).expect("the module Graph projects native Rust imports");
    assert!(json.contains("\"schema\":\"semaprax.graph.v25\""));

    // The declaration keeps its real result type and is marked as native.
    assert!(json.contains(
        "{\"id\":\"rust.host.combine\",\"kind\":\"import\",\"name\":\"combine\",\"owner\":\"rust.host\""
    ));
    assert!(json.contains("\"result\":{\"type\":\"i64\",\"ownership_mode\":\"value\""));
    assert!(json.contains("\"failure\":{\"kind\":\"status\",\"domain_id\":\"rust.test\""));
    assert!(json.contains("\"native_rust\":true}"));

    // The call site is projected rather than dropped.
    assert!(json.contains("\"kind\":\"native_rust_import_call\",\"import\":\"rust.host.combine\""));

    // A program without a native Rust import keeps the schema it already had.
    let ordinary = parse(
        "module plain;\n\n@id(\"plain.main\")\nfn main() -> i64\n{\n    42\n}\n",
        Path::new("plain.spx"),
    )
    .unwrap();
    let ordinary_json = graph::to_json(&ordinary).unwrap();
    assert!(ordinary_json.contains("\"schema\":\"semaprax.graph.v10\""));
    assert!(!ordinary_json.contains("native_rust"));

    let wasm_error = wasm::emit_module(&program).unwrap_err();
    assert_eq!(wasm_error.code, "SPX-W114");
    assert_eq!(
        wasm_error.message,
        "Native Rust imports are unavailable for WebAssembly targets"
    );

    // The projections that omit import nodes by construction stay closed, so a
    // native Rust import can never be silently dropped from an agent view.
    let context_error = graph::context_json(&program, "test.call_combine", 1).unwrap_err();
    assert_eq!(context_error.len(), 1);
    assert_eq!(context_error[0].code, "SPX-G218");
    assert_eq!(
        context_error[0].message,
        "Native Rust import declarations are outside the agent, review, impact, and evidence Graph projections"
    );
    let resolved = hir::resolve(&program).unwrap();
    let resolved_wasm_error = wasm::emit_resolved_module(&resolved).unwrap_err();
    assert_eq!(resolved_wasm_error.code, "SPX-W114");
    assert_eq!(
        resolved_wasm_error.message,
        "Native Rust imports are unavailable for WebAssembly targets"
    );
}

#[test]
fn forged_native_rust_call_hir_is_rejected_by_target_result_and_effect() {
    fn native_call_mut(
        resolved: &mut hir::ResolvedProgram,
    ) -> &mut hir::ResolvedNativeRustImportCall {
        let wrapper = resolved
            .functions
            .iter_mut()
            .find(|function| function.name == "call_combine")
            .unwrap();
        let ResolvedExprKind::Block { tail, .. } = &mut wrapper.body.kind else {
            panic!("wrapper body")
        };
        let ResolvedExprKind::NativeRustImportCall(call) = &mut tail.kind else {
            panic!("native Rust call")
        };
        call
    }

    let program = parse(SOURCE, Path::new("native-rust.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();

    let mut unknown_target = resolved.clone();
    native_call_mut(&mut unknown_target).import = DeclarationId::new("forged.target".to_owned());
    let diagnostic = hir::validate(&unknown_target).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        "native Rust import call has an unknown target"
    );

    let mut wrong_result = resolved.clone();
    native_call_mut(&mut wrong_result).result = ResolvedImportResultKind::Bool;
    let diagnostic = hir::validate(&wrong_result).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        "native Rust import call disagrees with its declaration"
    );

    let mut undeclared_effect = resolved;
    undeclared_effect.permits.push("forged.effect".to_owned());
    undeclared_effect.interfaces[0]
        .permits
        .push("forged.effect".to_owned());
    undeclared_effect.interfaces[0].imports[0]
        .effects
        .push("forged.effect".to_owned());
    undeclared_effect.interfaces[0].imports[0]
        .required_authority
        .push("forged.effect".to_owned());
    let diagnostic = hir::validate(&undeclared_effect).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        "native Rust import call requires an undeclared effect"
    );

    let mut forged_failure = hir::resolve(&program).unwrap();
    forged_failure.interfaces[0].imports[0].failure = ResolvedImportFailure::Status {
        domain_id: "rust.test".to_owned(),
        normalization: "forged.status",
    };
    let diagnostic = hir::validate(&forged_failure).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert_eq!(
        diagnostic.message,
        "import `rust.host.combine` has an invalid status contract"
    );
}
