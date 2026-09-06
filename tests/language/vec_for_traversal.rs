use std::path::Path;

use semaprax::hir::{self, ResolvedExprKind, ResolvedStatement};
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
    for function in resolved
        .functions
        .iter()
        .filter(|function| function.name.starts_with("walk_"))
    {
        let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
            panic!("function block")
        };
        let ResolvedStatement::Let { value, .. } = &statements[3] else {
            panic!("lowered wrapper")
        };
        let ResolvedExprKind::Block { statements, .. } = &value.kind else {
            panic!("lowered block")
        };
        assert!(
            matches!(&statements[0], ResolvedStatement::Let { value, .. }
            if matches!(&value.kind, ResolvedExprKind::Call { callee, .. } if callee.as_str() == "core.vec.len"))
        );
        assert!(matches!(
            &statements[1],
            ResolvedStatement::Let { mutable: true, .. }
        ));
        let ResolvedStatement::While { body, .. } = &statements[2] else {
            panic!("lowered while")
        };
        let ResolvedExprKind::Block { statements, .. } = &body.kind else {
            panic!("while body")
        };
        assert!(
            matches!(&statements[0], ResolvedStatement::Let { value, .. }
            if matches!(&value.kind, ResolvedExprKind::Call { callee, .. } if callee.as_str() == "core.vec.get"))
        );
        assert!(matches!(&statements[2], ResolvedStatement::Assign { .. }));
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
