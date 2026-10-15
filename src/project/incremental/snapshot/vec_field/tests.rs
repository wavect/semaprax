use super::*;
use crate::hir::{self, DeclarationId, ResolvedExprKind, ResolvedStatement};
const SOURCE: &str = r#"module replay.fields;
@id("row") record Row { @id("row.left") left:string,@id("row.right") right:string }
@id("reader") fn reader(values:borrow Vec<Row>)->i64 {
 let view=vec_field<Row>(values,0usize,"left");str_len_bytes(view)
}
@id("app.main") fn main()->i64 {0}
"#;
#[test]
fn same_typed_field_substitution_cannot_acquire_source_or_cache_authority() {
    let ast = crate::check(SOURCE, "source.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let mut retained = hir::resolve(&ast).unwrap();
    replay_source(&canonical, &ast, &retained).unwrap();
    let reader = retained
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "reader")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &mut reader.body.kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("view")
    };
    let ResolvedExprKind::VecFieldRead { field, .. } = &mut value.kind else {
        panic!("field")
    };
    *field = DeclarationId::new("row.right");
    // Both fields have the same declared type and legal live root. HIR alone
    // can authenticate either operation; source replay must select the authored one.
    hir::validate(&retained).unwrap();
    let wire = crate::cache_codec::encode(&retained).unwrap();
    let restored = crate::cache_codec::decode(&wire).unwrap();
    let errors = replay_source(&canonical, &ast, &restored).unwrap_err();
    assert!(errors
        .iter()
        .any(|d| d.message.contains("HIR disagrees with canonical source")));
    let changed = crate::check(
        &SOURCE.replace("0usize,\"left\"", "0usize,\"right\""),
        "changed.spx",
    )
    .unwrap();
    let original = hir::resolve(&ast).unwrap();
    assert!(replay_source(&crate::format::canonical(&changed), &changed, &original).is_err());
}

#[test]
fn explicit_and_row_read_origin_substitutions_have_no_cache_authority() {
    let source = SOURCE.replace("let view=vec_field<Row>(values,0usize,\"left\");str_len_bytes(view)", "for borrow row at index in values { let derived=row.left; let explicit=vec_field<Row>(values,index,\"right\");str_len_bytes(derived)+str_len_bytes(explicit) } 0");
    assert_ne!(source, SOURCE);
    let ast = crate::check(&source, "origin-cache.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let original = hir::resolve(&ast).unwrap();
    replay_source(&canonical, &ast, &original).unwrap();
    fn change(expression: &mut crate::hir::ResolvedExpr, target: hir::VecFieldReadOrigin) -> bool {
        if let ResolvedExprKind::VecFieldRead { origin, .. } = &mut expression.kind {
            if *origin == target {
                *origin = match target {
                    hir::VecFieldReadOrigin::Explicit => hir::VecFieldReadOrigin::Traversal,
                    hir::VecFieldReadOrigin::Traversal => hir::VecFieldReadOrigin::Explicit,
                };
                return true;
            }
        }
        if let ResolvedExprKind::Block { statements, tail } = &mut expression.kind {
            for statement in statements {
                match statement {
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => {
                        if change(condition, target) {
                            return true;
                        }
                        if change(body, target) {
                            return true;
                        }
                    }
                    _ => {
                        if change(statement.value_mut(), target) {
                            return true;
                        }
                    }
                }
            }
            return change(tail, target);
        }
        if let ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } = &mut expression.kind
        {
            return change(then_branch, target) || change(else_branch, target);
        }
        false
    }
    for target in [
        hir::VecFieldReadOrigin::Explicit,
        hir::VecFieldReadOrigin::Traversal,
    ] {
        let mut forged = original.clone();
        let reader = forged
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "reader")
            .unwrap();
        assert!(change(&mut reader.body, target));
        let wire = crate::cache_codec::encode(&forged).unwrap();
        let restored = crate::cache_codec::decode(&wire).unwrap();
        assert!(replay_source(&canonical, &ast, &restored).is_err());
    }
    let mut origin_wire = crate::cache_codec::encode(&hir::VecFieldReadOrigin::Traversal).unwrap();
    *origin_wire.last_mut().unwrap() = 255;
    assert!(crate::cache_codec::decode::<hir::VecFieldReadOrigin>(&origin_wire).is_err());
}
