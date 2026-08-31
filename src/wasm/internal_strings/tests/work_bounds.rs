use super::{emit_module, program, InternalStringOptions};
use crate::hir::{ResolvedExprKind, ResolvedStatement, ResolvedType};
use std::fmt::Write as _;

const NESTED_BLOCKS: usize = 32;

fn subject(bindings: usize) -> crate::ast::Program {
    let mut source =
        String::from("module test.string_work;\n@id(\"work.main\") fn main() -> i64 {\n");
    for _ in 0..NESTED_BLOCKS {
        source.push_str("{\n");
    }
    for index in 0..bindings {
        writeln!(source, "let value_{index} = \"x\";").unwrap();
    }
    source.push_str("42\n");
    for _ in 0..=NESTED_BLOCKS {
        source.push_str("}\n");
    }
    let ast = program(&source);
    let resolved = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let (_, closure) = super::super::admission::prepare(&resolved, &["work.main".into()])
        .expect("this source fits the earlier signature, node and depth bounds");
    assert_eq!(closure.len(), 1);

    // Independently inspect the source-derived shape, not the planner's
    // Cells inventory or its work-count result. Every literal and String let
    // owns one cell; all are inside each of the 33 scalar-result blocks.
    let mut blocks = 0;
    let mut literals = 0;
    let mut owned_bindings = 0;
    let mut nodes = 0;
    let mut maximum_depth = 0;
    let mut pending = vec![(&resolved.functions[0].body, 1)];
    while let Some((expression, depth)) = pending.pop() {
        nodes += 1;
        maximum_depth = maximum_depth.max(depth);
        match &expression.kind {
            ResolvedExprKind::Block { statements, tail } => {
                blocks += 1;
                assert_eq!(expression.ty, ResolvedType::I64);
                pending.push((tail, depth + 1));
                for statement in statements {
                    let ResolvedStatement::Let { binding, value, .. } = statement else {
                        panic!("unexpected statement in literal work witness");
                    };
                    assert_eq!(binding.ty, ResolvedType::String);
                    owned_bindings += 1;
                    pending.push((value, depth + 1));
                }
            }
            ResolvedExprKind::String(value) => {
                assert_eq!(value, "x");
                literals += 1;
            }
            ResolvedExprKind::Int(value) => assert_eq!(*value, 42),
            _ => panic!("unexpected expression in literal work witness"),
        }
    }
    assert_eq!(blocks, 33);
    assert_eq!(literals, bindings);
    assert_eq!(owned_bindings, bindings);
    assert_eq!(nodes, bindings + 34);
    assert_eq!(maximum_depth, 34);
    ast
}

#[test]
fn selected_cleanup_work_limit_uses_the_standalone_diagnostic() {
    // 8000 owners, 33 full block sweeps, 4000 literal sweeps and two
    // 8000-owner epilogues require 284000 visits, above the 262144 cap.
    // Only 4034 expressions and depth 34 are present; earlier admission is
    // independently checked above and cannot stand in for this work refusal.
    let ast = subject(4_000);
    let error = emit_module(
        &ast,
        &["work.main".into()],
        InternalStringOptions::default(),
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-W111");
    assert_eq!(
        error.message,
        "standalone String cleanup emission exceeds its work bound"
    );
}

#[test]
fn smaller_selected_cleanup_work_still_emits_a_valid_module() {
    let ast = subject(100);
    let emitted = emit_module(
        &ast,
        &["work.main".into()],
        InternalStringOptions::default(),
    )
    .unwrap();
    wasmparser::Validator::new()
        .validate_all(emitted.wasm_bytes())
        .unwrap();
}
