//! Lowering pre-scan retains all old restrictions inside an admitted byte body.
use super::super::{
    Binding, DeclarationId, DeclarationIndex, FunctionExecutionId, OwnershipMode, ResolvedType,
    Resolver, ValueId,
};
use crate::ast::{Expr, ExprKind, Statement};
use std::{collections::BTreeMap, path::Path};

fn program(body: &str) -> crate::ast::Program {
    crate::parse(
        &format!("module gate.nested; @id(\"gate.main\") fn main()->i64 {{ {body} }}"),
        Path::new("gate-nested.spx"),
    )
    .unwrap()
}
fn resolver(program: &crate::ast::Program) -> Resolver<'_, '_> {
    Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(program)),
        program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(program),
        declarations: DeclarationIndex::from_verified(program).unwrap(),
        reuse: None,
        function_work: super::super::FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    }
}
fn scope() -> BTreeMap<String, Binding> {
    let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("gate.main"));
    BTreeMap::from([(
        "view".to_owned(),
        Binding {
            traversal_row: None,
            id: ValueId::parameter(&owner, 0),
            ty: ResolvedType::SliceU8,
            ownership: OwnershipMode::Borrow,
            mutable: false,
        },
    )])
}
fn body(program: &crate::ast::Program) -> &Expr {
    &program.functions[0].body
}
fn full(error: crate::diagnostic::Diagnostic) -> String {
    // Diagnostic's Debug includes all six public fields, including span/path.
    format!("{error:?}")
}
#[test]
fn nested_byte_lowering_gate_scans_complete_inner_subtrees_and_keeps_old_priority() {
    let bindings = scope();
    for source in [
        "for item in view { 0 } 0",
        "while true { for item in view { 0 } 0 } 0",
    ] {
        let ast = program(source);
        let resolver = resolver(&ast);
        resolver
            .reject_while_disallowed_in_scope(body(&ast), None, Some(&bindings))
            .unwrap();
        let old = resolver.reject_while_disallowed(body(&ast)).unwrap_err();
        assert_eq!(old.code, "SPX-H006");
        assert_eq!(
            old.message,
            "nested for traversal reached bounded-loop HIR admission"
        );
    }
    for inner in [
        "@audit(\"nested-gate\") unsafe { 0 } 0",
        "[1u8,2u8]",
        "bytes_copy(view)",
        "unknown<i64>(1)",
        "value.method()",
    ] {
        let ast = program(&format!("for item in view {{ {inner} }} 0"));
        let resolver = resolver(&ast);
        let ExprKind::Block { statements, .. } = &body(&ast).kind else {
            panic!("block")
        };
        let Statement::For {
            body: inner_body, ..
        } = &statements[0]
        else {
            panic!("for")
        };
        let expected = resolver.reject_while_disallowed(inner_body).unwrap_err();
        let actual = resolver
            .reject_while_disallowed_in_scope(body(&ast), None, Some(&bindings))
            .unwrap_err();
        assert_eq!(full(actual), full(expected));
    }
}
#[test]
fn nested_byte_lowering_gate_requires_exact_immutable_entry_binding_and_strict_legacy() {
    for kind in [ResolvedType::SliceU8, ResolvedType::I64] {
        for mutable in [false, true] {
            let mut bindings = scope();
            let value = bindings.get_mut("view").unwrap();
            value.ty = kind.clone();
            value.mutable = mutable;
            let ast = program("for item in view { 0 } 0");
            let resolver = resolver(&ast);
            let actual =
                resolver.reject_while_disallowed_in_scope(body(&ast), None, Some(&bindings));
            if kind == ResolvedType::SliceU8 && !mutable {
                actual.unwrap();
            } else {
                assert_eq!(
                    full(actual.unwrap_err()),
                    full(resolver.reject_while_disallowed(body(&ast)).unwrap_err())
                );
            }
        }
    }
    let ast = program("for item in late { 0 } 0");
    let resolver = resolver(&ast);
    assert_eq!(
        full(
            resolver
                .reject_while_disallowed_in_scope(body(&ast), None, Some(&scope()))
                .unwrap_err()
        ),
        full(resolver.reject_while_disallowed(body(&ast)).unwrap_err())
    );
}
#[test]
fn nested_byte_source_lowering_bridge_resolves_and_independent_hir_replays() {
    let source = include_str!("../../../tests/fixtures/nested-byte-slice-traversal.spx");
    let ast = crate::check(source, "nested-byte-slice-traversal.spx").unwrap();
    let hir = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&hir).unwrap();
    // Original complete source/HIR owner controls remain registered separately;
    // this regression pins the actual lowering gate that prevented reaching them.
    assert!(hir.functions.iter().any(|f| f.name == "main"));
}

#[test]
fn nested_byte_actual_resolver_keeps_ancestry_local_and_entry_scope_fixed() {
    const PRE: &str = "let data=[1u8,2u8]; let view=array_as_slice(data);";
    for loops in [
        "for outer in view { for inner in view { 0 } 0 } 0",
        "for outer in view { while false { for inner in view { 0 } 0 } 0 } 0",
        "for outer in view { for inner in view { if inner == 1u8 { break; } else { 0 }; 0 } if outer == 1u8 { continue; } else { 0 }; 0 } 0",
        "for outer in view { for inner in view { if inner == 1u8 { return 0; } else { 0 }; 0 } 0 } 0",
    ] {
        let ast = program(&format!("{PRE} {loops}"));
        let resolver = resolver(&ast);
        resolver.resolve_function(&ast.functions[0]).unwrap();
    }
    // Retain both original unreachable fixtures. The first actual P209 was
    // the inner Break tail; the Return tail has the same independent gate.
    for (loops, stopped) in [
        (
            "for outer in view { for inner in view { break; 0 } continue; 0 } 0",
            "break; ",
        ),
        (
            "for outer in view { for inner in view { return 0; 0 } 0 } 0",
            "return 0; ",
        ),
    ] {
        let unreachable =
            format!("module gate.nested; @id(\"gate.main\") fn main()->i64 {{ {PRE} {loops} }}");
        let error = crate::parse(&unreachable, Path::new("gate-nested.spx")).unwrap_err();
        let span_start = unreachable.find(stopped).unwrap() + stopped.len();
        assert_eq!(
            error,
            crate::diagnostic::Diagnostic {
                code: "SPX-P209",
                severity: crate::diagnostic::Severity::Error,
                message: "code after an unconditional return is unreachable".into(),
                path: Some("gate-nested.spx".to_owned()),
                span: Some(crate::ast::Span {
                    start: span_start,
                    end: span_start + 1,
                    line: 1,
                    column: span_start + 1,
                }),
                help: Some(
                    "remove the unreachable code or put it on a non-returning guard path".into()
                ),
            }
        );
    }
    // A completed byte region must not lend ancestry to a later ordinary While.
    for loops in [
        "while false { for inner in view { 0 } 0 } 0",
        "for outer in view { 0 } while false { for inner in view { 0 } 0 } 0",
        "for outer in view { let late=byte_range(view,0usize,1usize); for inner in late { 0 } 0 } 0",
        "for outer in view { for borrow inner in view { 0 } 0 } 0",
        "for outer in view { for own inner in view { 0 } 0 } 0",
    ] {
        let ast = program(&format!("{PRE} {loops}"));
        let resolver = resolver(&ast);
        let error = resolver.resolve_function(&ast.functions[0]).unwrap_err();
        assert_eq!(error.code, "SPX-H006", "{loops}: {error:?}");
        assert_eq!(
            error.message,
            "nested for traversal reached bounded-loop HIR admission"
        );
    }
    // A shadow may be rejected by ordinary binding admission even before its
    // nested traversal is scheduled; it can never be accepted from entry text.
    for shadow in [
        "let view=vec_with_capacity<i64>(0usize);",
        "let mut view=1;",
        "let view=byte_range(view,0usize,1usize);",
    ] {
        let ast = program(&format!(
            "{PRE} for outer in view {{ {{ {shadow} for inner in view {{ 0 }} 0 }} }} 0"
        ));
        assert!(resolver(&ast).resolve_function(&ast.functions[0]).is_err());
    }
    for loops in [
        "let mut view2=view; for outer in view2 { 0 } 0",
        "for view in view { 0 } 0",
        "let rows=vec_with_capacity<i64>(0usize); for outer in rows { for inner in view { 0 } 0 } 0",
    ] {
        let ast = program(&format!("{PRE} {loops}"));
        let resolver = resolver(&ast);
        let function = FunctionExecutionId::Monomorphic(DeclarationId::new("gate.main"));
        let old = super::super::borrowed_root_oracle::Resolver(&resolver)
            .expression(&function, body(&ast), &BTreeMap::new(), "body")
            .unwrap_err();
        let current = resolver.resolve_function(&ast.functions[0]).unwrap_err();
        assert_eq!(full(current), full(old), "full old/current: {loops}");
    }
}

#[test]
fn nested_byte_actual_schedule_rejects_same_name_foreign_binding_before_any_body_lowering() {
    use super::super::resolve_expr::path_arena::{PathArena, PathId};
    let ast = program("for inner in view { 0 } 0");
    let resolver = resolver(&ast);
    let ExprKind::Block { statements, tail } = &body(&ast).kind else {
        panic!("block")
    };
    let Statement::For {
        item,
        values,
        body: inner,
        ..
    } = &statements[0]
    else {
        panic!("for")
    };
    let entry = scope();
    let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("gate.main"));
    for mutable in [false, true] {
        for ty in [ResolvedType::SliceU8, ResolvedType::I64] {
            let mut current = entry.clone();
            let binding = current.get_mut("view").unwrap();
            binding.id = ValueId::local(&owner, "shadow");
            binding.ty = ty;
            binding.mutable = mutable;
            let mut frames = Vec::new();
            let mut paths = PathArena::new("body");
            let error = super::super::resolve_for::schedule(
                &resolver,
                &owner,
                &mut frames,
                &mut paths,
                body(&ast).span,
                PathId::ROOT,
                statements,
                tail,
                0,
                super::super::resolver_scope::borrowed(&current),
                Vec::new(),
                item,
                values,
                inner,
                Some(&entry),
            )
            .unwrap_err();
            assert_eq!(error.code, "SPX-H006");
            assert_eq!(
                error.message,
                "nested for traversal reached bounded-loop HIR admission"
            );
            assert!(frames.is_empty());
        }
    }
}
