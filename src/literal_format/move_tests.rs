//! Owned formatter inputs must match the source move on every lowering.
use super::*;
use crate::cleanup_plan::{CleanupTransition, StorageId};
use crate::hir::{ResolvedExprKind, ResolvedFunction};

const SOURCE: &str = r#"module format.moves;
@id("render") fn render(first:string,second:string)->string {string_format("{}{}",first,second)}
@id("run") fn run()->i64 {string_len(render("a","b"))}
@id("app.main") fn main()->i64 {run()}
"#;

fn operands(function: &ResolvedFunction) -> Vec<&crate::hir::ResolvedExpr> {
    let mut args = Vec::new();
    crate::hir::function_value::walk(function, |expression| {
        if let ResolvedExprKind::LiteralFormat { args: fields, .. } = &expression.kind {
            args.extend(fields);
        }
    });
    args
}

#[test]
fn direct_formatter_places_transfer_into_group_and_forged_clone_plan_is_rejected() {
    let source = format!("{SOURCE}\n@id(\"copy\") fn copy(value:string)->string {{value}}");
    let ast = crate::check(&source, "format-moves.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    crate::cleanup_plan::validate_program(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "render")
        .unwrap();
    let args = operands(function);
    assert_eq!(args.len(), 2);
    assert_eq!(moving_string_operands(function).len(), 2);
    for (index, argument) in args.iter().enumerate() {
        let ResolvedExprKind::Place(place) = &argument.kind else {
            panic!("named owner")
        };
        let transfers = function.cleanup_plan.blocks.iter().flat_map(|b| &b.transitions)
            .filter(|transition| matches!(transition, CleanupTransition::Transfer { at, source, destination }
                if at == &argument.id && source.storage == StorageId::Value(place.root.clone())
                    && source.projections.is_empty()
                    && matches!(&destination.storage, StorageId::CallArgument {parameter_index, value_expression, ..}
                        if *parameter_index == index as u32 && value_expression == &argument.id)))
            .count();
        assert_eq!(transfers, 1);
        assert!(!function.cleanup_plan.blocks.iter().flat_map(|b| &b.transitions)
            .any(|transition| matches!(transition, CleanupTransition::Initialize {at, ..} if at == &argument.id)));
    }
    let copy = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "copy")
        .unwrap();
    assert!(moving_string_operands(copy).is_empty());
    assert!(copy
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|b| &b.transitions)
        .any(
            |transition| matches!(transition, CleanupTransition::Initialize {destination, ..}
            if matches!(destination.storage, StorageId::Temporary(_)))
        ));
    let first = args[0].id.clone();
    let mut forged = program.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "render")
        .unwrap();
    let transfer = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|b| &mut b.transitions)
        .find(
            |transition| matches!(transition, CleanupTransition::Transfer {at, ..} if at == &first),
        )
        .unwrap();
    let CleanupTransition::Transfer { source, .. } = transfer else {
        unreachable!()
    };
    source.storage = StorageId::Temporary(first);
    let wire = crate::cache_codec::encode(&forged).unwrap();
    assert!(crate::cleanup_plan::validate_program(&forged).is_err());
    assert!(crate::hir::validate(&forged).is_err());
    assert_eq!(crate::cache_codec::encode(&forged).unwrap(), wire);
}

#[test]
fn native_and_standalone_wasm_do_not_clone_formatter_place_arguments() {
    let ast = crate::check(SOURCE, "format-native-wasm-moves.spx").unwrap();
    let c = crate::codegen::emit_c(&ast).unwrap();
    assert!(!c.contains("= spx_string_clone("));
    let artifact = crate::wasm::internal_strings::emit_module(
        &ast,
        &["run".into()],
        crate::wasm::internal_strings::InternalStringOptions::default(),
    )
    .unwrap();
    let mut clones = 0;
    for payload in wasmparser::Parser::new(0).parse_all(artifact.wasm_bytes()) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut ops = body.get_operators_reader().unwrap();
            while !ops.eof() {
                // Standalone String import 1 is its frozen clone operation.
                if matches!(
                    ops.read().unwrap(),
                    wasmparser::Operator::Call { function_index: 1 }
                ) {
                    clones += 1;
                }
            }
        }
    }
    assert_eq!(clones, 0);
    assert!(artifact
        .runtime_source()
        .contains("(status===11||status===34)!==(cause!==null)"));
    assert!(artifact.runtime_source().contains("status===34)result=Object.freeze({kind:\"failure\",domain:\"semaprax.string-format.v1\",code:1})"));
}
