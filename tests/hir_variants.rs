use std::path::Path;

use semaprax::hir::{
    self, DeclarationId, DeclarationKind, OwnershipMode, ResolvedExprKind, ResolvedMatchPattern,
    ResolvedType, ResolvedTypeDeclarationKind,
};
use semaprax::parse;

const SOURCE: &str = r#"
module test.hir_variants;
@id("test.choice")
variant Choice {
    @id("test.choice.none") None,
    @id("test.choice.number") Number {
        @id("test.choice.number.value") value: i64,
    },
    @id("test.choice.flag") Flag {
        @id("test.choice.flag.value") value: bool,
    },
}
@id("app.main")
fn main() -> i64 {
    let choice = Choice::Number { value: 42 };
    match choice {
        Choice::Number { value: number } => number,
        Choice::Flag { value: flag } => if flag { 1 } else { 0 },
        Choice::None {} => 0,
    }
}
"#;

fn resolved() -> hir::ResolvedProgram {
    let program = parse(SOURCE, Path::new("hir-variants.spx")).unwrap();
    hir::resolve(&program).unwrap()
}

fn match_expr(program: &hir::ResolvedProgram) -> &hir::ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &program.functions[0].body.kind else {
        panic!("main body must be a block");
    };
    tail
}

fn match_expr_mut(program: &mut hir::ResolvedProgram) -> &mut hir::ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &mut program.functions[0].body.kind else {
        panic!("main body must be a block");
    };
    tail
}

#[test]
fn variants_resolve_to_stable_case_field_constructor_and_pattern_identities() {
    let first = resolved();
    assert_eq!(first, resolved());
    let declaration = &first.types[0];
    let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind else {
        panic!("Choice must resolve as a variant");
    };
    assert_eq!(declaration.id.as_str(), "test.choice");
    assert_eq!(cases.len(), 3);
    assert_eq!(cases[0].id.as_str(), "test.choice.none");
    assert_eq!(cases[0].index, 0);
    assert!(cases[0].fields.is_empty());
    assert_eq!(cases[1].id.as_str(), "test.choice.number");
    assert_eq!(cases[1].index, 1);
    assert_eq!(cases[1].fields[0].id.as_str(), "test.choice.number.value");
    assert_eq!(cases[1].fields[0].ty, ResolvedType::I64);

    let case = first
        .declarations
        .declaration(&cases[1].id)
        .expect("case must be indexed");
    assert_eq!(case.kind, DeclarationKind::VariantCase);
    assert_eq!(case.owner.as_ref().unwrap().as_str(), "test.choice");
    let field = first
        .declarations
        .declaration(&cases[1].fields[0].id)
        .expect("case field must be indexed");
    assert_eq!(field.kind, DeclarationKind::CaseField);
    assert_eq!(field.owner.as_ref().unwrap().as_str(), "test.choice.number");

    let ResolvedExprKind::Block { statements, .. } = &first.functions[0].body.kind else {
        unreachable!();
    };
    let hir::ResolvedStatement::Let { value, .. } = &statements[0] else {
        panic!("first statement must be a let")
    };
    let ResolvedExprKind::ConstructVariant {
        variant,
        case,
        fields,
    } = &value.kind
    else {
        panic!("local initializer must resolve as variant construction");
    };
    assert_eq!(variant.as_str(), "test.choice");
    assert_eq!(case.as_str(), "test.choice.number");
    assert_eq!(fields[0].field.as_str(), "test.choice.number.value");

    let ResolvedExprKind::Match { scrutinee, arms } = &match_expr(&first).kind else {
        panic!("tail must resolve as match");
    };
    assert_eq!(scrutinee.ownership, OwnershipMode::Value);
    assert_eq!(arms.len(), 3);
    let ResolvedMatchPattern::Variant {
        variant,
        case,
        fields,
    } = &arms[0].pattern
    else {
        panic!("first arm must be a variant pattern");
    };
    assert_eq!(variant.as_str(), "test.choice");
    assert_eq!(case.as_str(), "test.choice.number");
    assert_eq!(fields[0].field.as_str(), "test.choice.number.value");
    assert_eq!(fields[0].binding.name, "number");
    assert_eq!(fields[0].binding.ty, ResolvedType::I64);
    assert!(fields[0]
        .binding
        .id
        .as_str()
        .ends_with("body.tail.arm.0.binding.0"));
    assert!(arms[0].value.id.as_str().ends_with("body.tail.arm.0.value"));
}

#[test]
fn independent_hir_validation_rejects_case_order_exhaustiveness_and_binding_mutations() {
    let mut reordered = resolved();
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut reordered.types[0].kind else {
        unreachable!();
    };
    cases.swap(0, 1);
    assert_eq!(hir::validate(&reordered).unwrap_err().code, "SPX-H006");

    let mut non_exhaustive = resolved();
    let ResolvedExprKind::Match { arms, .. } = &mut match_expr_mut(&mut non_exhaustive).kind else {
        unreachable!();
    };
    arms.pop();
    assert_eq!(hir::validate(&non_exhaustive).unwrap_err().code, "SPX-H006");

    let mut duplicate_case = resolved();
    let ResolvedExprKind::Match { arms, .. } = &mut match_expr_mut(&mut duplicate_case).kind else {
        unreachable!();
    };
    let ResolvedMatchPattern::Variant { case, .. } = &mut arms[1].pattern else {
        unreachable!();
    };
    *case = DeclarationId::new("test.choice.number");
    assert_eq!(hir::validate(&duplicate_case).unwrap_err().code, "SPX-H006");

    let mut hostile_binding = resolved();
    let ResolvedExprKind::Match { arms, .. } = &mut match_expr_mut(&mut hostile_binding).kind
    else {
        unreachable!();
    };
    let ResolvedMatchPattern::Variant { fields, .. } = &mut arms[0].pattern else {
        unreachable!();
    };
    fields[0].binding.ty = ResolvedType::Bool;
    assert_eq!(
        hir::validate(&hostile_binding).unwrap_err().code,
        "SPX-H006"
    );
}
