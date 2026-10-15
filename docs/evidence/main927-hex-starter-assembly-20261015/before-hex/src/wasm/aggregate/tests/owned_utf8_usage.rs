//! Runtime storage selection is a census of emitted functions, not ABI admission.
use crate::hir::{self, DeclarationId, ResolvedProgram};
use crate::wasm::owned_data_exports::{OwnedDataExportPlan, ResultLayout};
fn resolve(source: &str) -> ResolvedProgram {
    let parsed = crate::check(source, "owned-utf8-census.spx").unwrap();
    hir::resolve(&parsed).unwrap()
}
fn plan() -> OwnedDataExportPlan {
    OwnedDataExportPlan {
        stable_id: "selected".to_owned(),
        wasm_export: "selected".to_owned(),
        function_id: DeclarationId::new("selected"),
        parameters: Vec::new(),
        result: ResultLayout::I64,
    }
}
#[test]
fn scalar_signature_lexical_body_string_selects_existing_owned_literal_storage() {
    let program = resolve(
        r#"module test.utf8_body;
@id("selected") fn selected()->i64 {
    let text="local\u{0}é";
    let mut next=0;
    while next<1 {
        next=next+1;
        if next==1 {return 7;}else{0}
        0
    }
    0
}
@id("app.main") fn main()->i64 {0}
"#,
    );
    assert!(program.functions.iter().all(|function| function.return_type
        != hir::ResolvedType::String
        && function
            .params
            .iter()
            .all(|param| param.ty != hir::ResolvedType::String)));
    let bytes = super::super::emit_owned_data_exports(&program, &[plan()]).unwrap();
    assert_eq!(
        bytes,
        super::super::emit_owned_data_exports(&program, &[plan()]).unwrap()
    );
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    assert!(bytes
        .windows("local\0é".len())
        .any(|window| window == "local\0é".as_bytes()));
    // The fixture remains a real lexical return, including its String cleanup.
    let selected = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "selected")
        .unwrap();
    assert!(selected.cleanup_plan.lexical_control.is_some());
}
#[test]
fn exact_function_iterator_counts_contract_strings_without_unselected_body_leakage() {
    let program = resolve(
        r#"module test.utf8_contract;
@id("selected") fn selected()->i64
    requires string_len("pre")==3
    ensures string_len("post")==4
{7}
@id("plain") fn plain()->i64 {8}
@id("app.main") fn main()->i64 {0}
"#,
    );
    let selected = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "selected")
        .unwrap();
    let plain = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "plain")
        .unwrap();
    assert!(crate::wasm::functions_use_strings([selected]));
    assert!(!crate::wasm::functions_use_strings([plain]));
    let bytes = super::super::emit_owned_data_exports(&program, &[plan()]).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    for literal in [b"pre".as_slice(), b"post".as_slice()] {
        assert!(bytes.windows(literal.len()).any(|window| window == literal));
    }
}
#[test]
fn current_generic_and_closure_body_inventory_includes_local_strings() {
    let generic = resolve(
        r#"module test.utf8_generic;
@id("identity") fn identity<T>(value:T)->T {let text="generic local";value}
@id("selected") fn selected()->i64 {identity<i64>(7)}
@id("app.main") fn main()->i64 {0}
"#,
    );
    assert!(!generic.function_instances.is_empty());
    assert!(!crate::wasm::functions_use_strings(
        generic.functions.iter()
    ));
    let executable = super::super::function_value::executable_functions(&generic);
    assert!(crate::wasm::functions_use_strings(
        executable.iter().map(|(function, _)| *function)
    ));
    let bytes = super::super::emit_owned_data_exports(&generic, &[plan()]).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let owning_closure = r#"module test.utf8_closure;
@id("selected") fn selected()->i64 {
    let callback=fn(value:i64)->i64 {let text="closure local";value};
    callback(7)
}
@id("app.main") fn main()->i64 {0}
"#;
    assert!(crate::check(owning_closure, "owned-utf8-census.spx")
        .unwrap_err()
        .iter()
        .any(|error| error.code == "SPX-T288"));
    let closure = resolve(&owning_closure.replace("let text=\"closure local\";", ""));
    let functions = super::super::function_value::table_plan(&closure).unwrap();
    assert!(!functions.bodies.is_empty());
    assert!(!crate::wasm::functions_use_strings(
        closure.functions.iter()
    ));
    assert!(!crate::wasm::functions_use_strings(functions.bodies.iter()));
    let bytes = super::super::emit_owned_data_exports(&closure, &[plan()]).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();

    // A structural census must inspect private closure bodies even though the
    // source/ABI profile independently refuses owning closure construction.
    let helper = resolve(
        r#"module test.utf8_private;
@id("selected") fn selected()->i64 {let text="closure local";7}
@id("app.main") fn main()->i64 {0}
"#,
    );
    let string_body = helper
        .functions
        .iter()
        .find(|f| f.id.as_str() == "selected")
        .unwrap()
        .body
        .clone();
    let mut private_bodies = functions.bodies.clone();
    private_bodies[0].body = string_body.clone();
    assert!(crate::wasm::functions_use_strings(private_bodies.iter()));
    let mut forged = closure.clone();
    let selected = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "selected")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut selected.body.kind else {
        panic!("closure block")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("closure binding")
    };
    let hir::ResolvedExprKind::Closure { body, .. } = &mut value.kind else {
        panic!("closure site")
    };
    **body = string_body;
    assert_eq!(
        super::super::emit_owned_data_exports(&forged, &[plan()])
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}
#[test]
fn local_static_text_still_refuses_public_owned_export_authority() {
    let program = resolve(
        r#"module test.utf8_public_refusal;
@id("selected") fn selected()->i64 {let text=str"static";7}
@id("app.main") fn main()->i64 {0}
"#,
    );
    assert!(!crate::wasm::functions_use_strings(
        program.functions.iter()
    ));
    assert_eq!(
        super::super::emit_owned_data_exports(&program, &[plan()])
            .unwrap_err()
            .code,
        "SPX-W119"
    );
}
