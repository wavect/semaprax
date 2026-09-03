use std::path::Path;

use semaprax::format;
use semaprax::hir::{
    self, DeclarationId, ResolvedExprKind, ResolvedImportFailure, ResolvedImportResultKind,
    ResolvedType,
};
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
