use std::path::Path;

use semaprax::hir::{self, DeclarationId, OwnershipMode, Place, ResolvedExprKind, ResolvedType};
use semaprax::{codegen, parse, wasm};

const SOURCE: &str = r#"
module test.hir_validation;
@id("math.answer")
fn answer(value: i64) -> i64 { value }
@id("app.main")
fn main() -> i64 { answer(42) }
"#;

fn resolved() -> hir::ResolvedProgram {
    let program = parse(SOURCE, Path::new("hir-validation.spx")).unwrap();
    hir::resolve(&program).unwrap()
}

fn assert_both_backends_reject(program: &hir::ResolvedProgram) {
    assert_eq!(hir::validate(program).unwrap_err().code, "SPX-H006");
    assert_eq!(codegen::emit_hir_c(program).unwrap_err().code, "SPX-H006");
    assert_eq!(
        wasm::emit_resolved_module(program).unwrap_err().code,
        "SPX-H006"
    );
}

#[test]
fn direct_hir_backends_reject_inconsistent_return_types() {
    let mut program = resolved();
    let entrypoint = program.entrypoint.clone();
    program
        .functions
        .iter_mut()
        .find(|function| function.id == entrypoint)
        .unwrap()
        .return_type = ResolvedType::Bool;

    assert_both_backends_reject(&program);
}

#[test]
fn direct_hir_backends_reject_duplicate_declaration_identities() {
    let mut program = resolved();
    program.functions[0].id = program.entrypoint.clone();

    assert_both_backends_reject(&program);
}

#[test]
fn direct_hir_backends_reject_function_name_index_disagreement() {
    let mut program = resolved();
    program.functions[0].name = "forged_display_name".to_owned();

    assert_both_backends_reject(&program);
}

#[test]
fn direct_hir_backends_reject_type_name_index_disagreement() {
    let source = r#"
module test.hir_validation_resource_name;
@id("buffer.type")
resource Buffer;
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let ast = parse(source, Path::new("hir-validation-resource-name.spx")).unwrap();
    let mut program = hir::resolve(&ast).unwrap();
    program.types[0].name = "ForgedBuffer".to_owned();

    assert_both_backends_reject(&program);
}

#[test]
fn direct_hir_backends_reject_unknown_nominal_types() {
    let mut program = resolved();
    program.functions[0].params[0].ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("missing.type"),
        arguments: Vec::new(),
    };

    assert_both_backends_reject(&program);
}

#[test]
fn direct_hir_backends_reject_undeclared_generic_arguments() {
    let source = r#"
module test.hir_validation_resource;
@id("buffer.type")
resource Buffer;
@id("buffer.keep")
fn keep(value: borrow Buffer) -> i64 { 1 }
@id("app.main")
fn main() -> i64 { 42 }
"#;
    let ast = parse(source, Path::new("hir-validation-resource.spx")).unwrap();
    let mut program = hir::resolve(&ast).unwrap();
    let ResolvedType::Nominal { arguments, .. } = &mut program.functions[0].params[0].ty else {
        panic!("resource parameter must be nominal");
    };
    arguments.push(ResolvedType::I64);

    assert_both_backends_reject(&program);
}

#[test]
fn direct_hir_backends_reject_out_of_scope_places() {
    let mut program = resolved();
    let foreign_result = program.functions[0].result_id.clone();
    let entrypoint = program.entrypoint.clone();
    let main = program
        .functions
        .iter_mut()
        .find(|function| function.id == entrypoint)
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut main.body.kind else {
        panic!("main body must be a block");
    };
    let ResolvedExprKind::Call { args, .. } = &mut tail.kind else {
        panic!("main tail must be a call");
    };
    args[0].kind = ResolvedExprKind::Place(Place {
        root: foreign_result,
        projections: Vec::new(),
    });
    args[0].ty = ResolvedType::I64;
    args[0].ownership = OwnershipMode::Value;

    assert_both_backends_reject(&program);
}

#[test]
fn same_signature_substitution_cannot_move_one_place_twice() {
    let source = r#"
module test.hir_double_move;
@id("buffer.type")
resource Buffer;
@id("buffer.inspect")
fn inspect(value: borrow Buffer) -> i64 { 1 }
@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }
@id("buffer.use_once")
fn use_once(value: own Buffer) -> i64 { inspect(value) + consume(value) }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let ast = parse(source, Path::new("hir-double-move.spx")).unwrap();
    let mut program = hir::resolve(&ast).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "buffer.use_once")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("function body must be a block");
    };
    let ResolvedExprKind::Binary { left, .. } = &mut tail.kind else {
        panic!("function tail must be binary");
    };
    let ResolvedExprKind::Call { callee, .. } = &mut left.kind else {
        panic!("left operand must be a call");
    };
    *callee = DeclarationId::new("buffer.consume");

    assert_both_backends_reject(&program);
}

#[test]
fn conditional_own_substitution_marks_the_place_maybe_moved() {
    let source = r#"
module test.hir_conditional_move;
@id("buffer.type")
resource Buffer;
@id("buffer.inspect")
fn inspect(value: borrow Buffer) -> i64 { 1 }
@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }
@id("buffer.conditional")
fn conditional(flag: bool, value: own Buffer) -> i64 {
    (if flag { inspect(value) } else { 0 }) + consume(value)
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let ast = parse(source, Path::new("hir-conditional-move.spx")).unwrap();
    let mut program = hir::resolve(&ast).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "buffer.conditional")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("function body must be a block");
    };
    let ResolvedExprKind::Binary { left, .. } = &mut tail.kind else {
        panic!("function tail must be binary");
    };
    let ResolvedExprKind::If { then_branch, .. } = &mut left.kind else {
        panic!("left operand must be conditional");
    };
    let ResolvedExprKind::Block {
        tail: then_tail, ..
    } = &mut then_branch.kind
    else {
        panic!("then branch must be a block");
    };
    let ResolvedExprKind::Call { callee, .. } = &mut then_tail.kind else {
        panic!("then tail must be a call");
    };
    *callee = DeclarationId::new("buffer.consume");

    assert_both_backends_reject(&program);
}

#[test]
fn nested_owned_block_results_validate_but_resource_backends_fail_closed() {
    let source = r#"
module test.hir_nested_block;
@id("buffer.type")
resource Buffer;
@id("buffer.passthrough")
fn passthrough(value: own Buffer) -> Buffer {
    let outer = {
        let inner = value;
        inner
    };
    outer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let ast = parse(source, Path::new("hir-nested-block.spx")).unwrap();
    let program = hir::resolve(&ast).unwrap();

    hir::validate(&program).unwrap();
    let native_hir = codegen::emit_hir_c(&program).unwrap_err();
    assert_eq!(native_hir.code, "SPX-B104");
    assert!(native_hir.message.contains("verified cleanup ABI"));
    let wasm_hir = wasm::emit_resolved_module(&program).unwrap_err();
    assert_eq!(wasm_hir.code, "SPX-W111");
    assert!(wasm_hir.message.contains("verified cleanup ABI"));

    assert_eq!(codegen::emit_c(&ast).unwrap_err().code, "SPX-B104");
    assert_eq!(wasm::emit_module(&ast).unwrap_err().code, "SPX-W111");
}

#[test]
fn same_signature_substitution_cannot_transfer_from_a_contract() {
    let source = r#"
module test.hir_contract_move;
@id("buffer.type")
resource Buffer;
@id("buffer.inspect")
fn inspect(value: borrow Buffer) -> i64 { 1 }
@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }
@id("buffer.guarded")
fn guarded(value: own Buffer) -> i64 requires inspect(value) == 1 { 1 }
@id("app.main")
fn main() -> i64 { 0 }
"#;
    let ast = parse(source, Path::new("hir-contract-move.spx")).unwrap();
    let mut program = hir::resolve(&ast).unwrap();
    let function = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "buffer.guarded")
        .unwrap();
    let ResolvedExprKind::Binary { left, .. } = &mut function.requires[0].kind else {
        panic!("precondition must be binary");
    };
    let ResolvedExprKind::Call { callee, .. } = &mut left.kind else {
        panic!("left operand must be a call");
    };
    *callee = DeclarationId::new("buffer.consume");

    assert_both_backends_reject(&program);
}

const EFFECT_SOURCE: &str = r#"
module test.hir_effects;
permit { clock.read }
@id("math.pure")
fn pure(value: i64) -> i64 { value }
@id("clock.tick")
fn tick(value: i64) -> i64 uses { clock.read } { value }
@id("math.wrapper")
fn wrapper() -> i64 { pure(1) }
@id("math.guarded")
fn guarded() -> i64 requires pure(1) == 1 { 1 }
@id("app.main")
fn main() -> i64 { wrapper() }
"#;

#[test]
fn same_signature_substitution_cannot_add_an_undeclared_effect() {
    let ast = parse(EFFECT_SOURCE, Path::new("hir-effects.spx")).unwrap();
    let mut program = hir::resolve(&ast).unwrap();
    let wrapper = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "math.wrapper")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut wrapper.body.kind else {
        panic!("wrapper body must be a block");
    };
    let ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
        panic!("wrapper tail must be a call");
    };
    *callee = DeclarationId::new("clock.tick");
    assert_both_backends_reject(&program);

    let mut missing_permit = hir::resolve(&ast).unwrap();
    missing_permit.permits.clear();
    assert_both_backends_reject(&missing_permit);
}

#[test]
fn same_signature_substitution_cannot_make_a_contract_effectful() {
    let ast = parse(EFFECT_SOURCE, Path::new("hir-effects.spx")).unwrap();
    let mut program = hir::resolve(&ast).unwrap();
    let guarded = program
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "math.guarded")
        .unwrap();
    let ResolvedExprKind::Binary { left, .. } = &mut guarded.requires[0].kind else {
        panic!("precondition must be binary");
    };
    let ResolvedExprKind::Call { callee, .. } = &mut left.kind else {
        panic!("left operand must be a call");
    };
    *callee = DeclarationId::new("clock.tick");

    assert_both_backends_reject(&program);
}
