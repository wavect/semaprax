use std::path::Path;

use semaprax::ast::BinaryOp;
use semaprax::hir::{self, ResolvedExprKind, ResolvedStatement, ResolvedType};
use semaprax::{format, graph, parse, verify};

const SOURCE: &str = r#"
module test.vec_for;

@id("for.i64") fn walk_i64() -> usize { let mut building = vec_with_capacity<i64>(1usize); building = vec_push<i64>(building, 1); let values = building; for item in values { let seen = item; 0usize } 0usize }
@id("for.i32") fn walk_i32() -> usize { let mut building = vec_with_capacity<i32>(1usize); building = vec_push<i32>(building, 1i32); let values = building; for item in values { let seen = item; 0usize } 0usize }
@id("for.u8") fn walk_u8() -> usize { let mut building = vec_with_capacity<u8>(1usize); building = vec_push<u8>(building, 1u8); let values = building; for item in values { let seen = item; 0usize } 0usize }
@id("for.usize") fn walk_usize() -> usize { let mut building = vec_with_capacity<usize>(1usize); building = vec_push<usize>(building, 1usize); let values = building; for item in values { let seen = item; 0usize } 0usize }
@id("for.char") fn walk_char() -> usize { let mut building = vec_with_capacity<char>(1usize); building = vec_push<char>(building, 'x'); let values = building; for item in values { let seen = item; 0usize } 0usize }
@id("for.f32") fn walk_f32() -> usize { let mut building = vec_with_capacity<f32>(1usize); building = vec_push<f32>(building, 1.0f32); let values = building; for item in values { let seen = item; 0usize } 0usize }
@id("for.f64") fn walk_f64() -> usize { let mut building = vec_with_capacity<f64>(1usize); building = vec_push<f64>(building, 1.0); let values = building; for item in values { let seen = item; 0usize } 0usize }
@id("for.bool") fn walk_bool() -> usize { let mut building = vec_with_capacity<bool>(1usize); building = vec_push<bool>(building, true); let values = building; for item in values { let seen = item; 0usize } 0usize }

@id("app.main") fn main() -> i64 { let a = walk_i64(); let b = walk_i32(); let c = walk_u8(); let d = walk_usize(); let e = walk_char(); let f = walk_f32(); let g = walk_f64(); let h = walk_bool(); 0 }
"#;

fn parsed(source: &str) -> semaprax::ast::Program {
    parse(source, Path::new("vec-for-traversal.spx")).unwrap()
}

#[test]
fn vec_for_preserves_source_and_lowers_all_copy_scalars() {
    let program = parsed(SOURCE);
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let canonical = format::canonical(&program);
    assert_eq!(canonical.matches("for item in values").count(), 8);
    assert_eq!(format::canonical(&parsed(&canonical)), canonical);

    let resolved = hir::resolve(&program).unwrap();
    for (name, element) in [
        ("walk_i64", ResolvedType::I64),
        ("walk_i32", ResolvedType::I32),
        ("walk_u8", ResolvedType::U8),
        ("walk_usize", ResolvedType::Usize),
        ("walk_char", ResolvedType::Char),
        ("walk_f32", ResolvedType::F32),
        ("walk_f64", ResolvedType::F64),
        ("walk_bool", ResolvedType::Bool),
    ] {
        let function = resolved
            .functions
            .iter()
            .find(|function| function.name == name)
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
            panic!("function block")
        };
        assert_eq!(statements.len(), 4);
        let ResolvedStatement::Let {
            binding: wrapper,
            mutable: false,
            value,
            ..
        } = &statements[3]
        else {
            panic!("lowered wrapper")
        };
        assert_eq!(wrapper.name, "#for");
        assert_eq!(wrapper.ty, ResolvedType::Usize);
        let ResolvedExprKind::Block {
            statements,
            tail: wrapper_tail,
        } = &value.kind
        else {
            panic!("lowered block")
        };
        assert_eq!(statements.len(), 3);
        assert!(matches!(wrapper_tail.kind, ResolvedExprKind::Usize(0)));
        let ResolvedStatement::Let {
            binding: length,
            mutable: false,
            value: length_value,
            ..
        } = &statements[0]
        else {
            panic!("length snapshot")
        };
        assert_eq!(length.name, "#for-length");
        let ResolvedExprKind::Call {
            callee,
            type_arguments: length_arguments,
            instance: None,
            args: length_args,
        } = &length_value.kind
        else {
            panic!("length call")
        };
        assert_eq!(callee.as_str(), "core.vec.len");
        assert_eq!(length_arguments, std::slice::from_ref(&element));
        assert_eq!(length_args.len(), 1);
        let ResolvedExprKind::Place(source_place) = &length_args[0].kind else {
            panic!("length source")
        };
        assert!(source_place.projections.is_empty());

        let ResolvedStatement::Let {
            binding: index,
            mutable: true,
            value: index_value,
            ..
        } = &statements[1]
        else {
            panic!("index initializer")
        };
        assert_eq!(index.name, "#for-index");
        assert_eq!(index.ty, ResolvedType::Usize);
        assert!(matches!(index_value.kind, ResolvedExprKind::Usize(0)));

        let ResolvedStatement::While {
            condition, body, ..
        } = &statements[2]
        else {
            panic!("lowered while")
        };
        let ResolvedExprKind::Binary {
            op: BinaryOp::Lt,
            left,
            right,
        } = &condition.kind
        else {
            panic!("index bound")
        };
        assert!(matches!(&left.kind, ResolvedExprKind::Place(place)
            if place.root == index.id && place.projections.is_empty()));
        assert!(matches!(&right.kind, ResolvedExprKind::Place(place)
            if place.root == length.id && place.projections.is_empty()));

        let ResolvedExprKind::Block {
            statements,
            tail: body_tail,
        } = &body.kind
        else {
            panic!("while body")
        };
        assert_eq!(statements.len(), 3);
        assert!(matches!(body_tail.kind, ResolvedExprKind::Usize(0)));
        let ResolvedStatement::Let {
            binding: item,
            mutable: false,
            value: item_value,
            ..
        } = &statements[0]
        else {
            panic!("item binding")
        };
        assert_eq!(item.name, "item");
        assert_eq!(item.ty, element);
        let ResolvedExprKind::Call {
            callee,
            type_arguments: get_arguments,
            instance: None,
            args: get_args,
        } = &item_value.kind
        else {
            panic!("item get")
        };
        assert_eq!(callee.as_str(), "core.vec.get");
        assert_eq!(get_arguments, &[element]);
        assert_eq!(get_args.len(), 2);
        assert!(matches!(&get_args[0].kind, ResolvedExprKind::Place(place)
            if place.root == source_place.root && place.projections.is_empty()));
        assert!(matches!(&get_args[1].kind, ResolvedExprKind::Place(place)
            if place.root == index.id && place.projections.is_empty()));

        let ResolvedStatement::Let {
            binding: authored,
            mutable: false,
            ..
        } = &statements[1]
        else {
            panic!("discarded authored body")
        };
        assert_eq!(authored.name, "#for-body");
        let ResolvedStatement::Assign {
            binding: incremented,
            field: None,
            value: increment,
            ..
        } = &statements[2]
        else {
            panic!("index increment")
        };
        assert_eq!(incremented.id, index.id);
        let ResolvedExprKind::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } = &increment.kind
        else {
            panic!("index increment value")
        };
        assert!(matches!(&left.kind, ResolvedExprKind::Place(place)
            if place.root == index.id && place.projections.is_empty()));
        assert!(matches!(right.kind, ResolvedExprKind::Usize(1)));
    }
    let rendered = graph::to_json(&program).unwrap();
    assert!(rendered.contains("core.vec.len"));
    assert!(rendered.contains("core.vec.get"));
}

#[test]
fn vec_for_rejects_non_binding_mutable_and_nested_sources_stably() {
    let computed = SOURCE.replacen(
        "for item in values",
        "for item in vec_clear<i64>(values)",
        1,
    );
    assert!(verify::verify(&parsed(&computed))
        .iter()
        .any(|d| d.code == "SPX-T284"));

    let nested = SOURCE.replacen(
        "let seen = item; 0usize",
        "let seen = item; for inner in values { let copy = inner; 0usize } 0usize",
        1,
    );
    assert!(verify::verify(&parsed(&nested))
        .iter()
        .any(|d| d.code == "SPX-T284"));

    let mutable = SOURCE.replacen("let values = building", "let mut values = building", 1);
    let mutable = parsed(&mutable);
    assert!(verify::verify(&mutable)
        .iter()
        .any(|d| d.code == "SPX-T284"));
    assert!(hir::resolve(&mutable).is_err());

    let consumed = SOURCE.replacen("let seen = item; 0usize", "let stolen = values; 0usize", 1);
    let consumed = parsed(&consumed);
    assert!(verify::verify(&consumed)
        .iter()
        .any(|d| d.code == "SPX-T284"));
    assert!(hir::resolve(&consumed).is_err());

    let reassigned_item = SOURCE.replacen("let seen = item; 0usize", "item = item; 0usize", 1);
    assert!(verify::verify(&parsed(&reassigned_item))
        .iter()
        .any(|d| d.code == "SPX-U101"));

    let unsupported = SOURCE.replacen("<i64>", "<Bytes>", 1);
    assert!(verify::verify(&parsed(&unsupported))
        .iter()
        .any(|d| matches!(d.code, "SPX-T281" | "SPX-T284")));

    let shadowed = SOURCE.replacen("for item in values", "let item = 0; for item in values", 1);
    let shadowed = parsed(&shadowed);
    assert!(verify::verify(&shadowed)
        .iter()
        .any(|d| d.code == "SPX-T209"));
    assert!(hir::resolve(&shadowed).is_err());
}
