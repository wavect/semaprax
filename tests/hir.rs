use std::path::Path;

use semaprax::hir::{
    resolve, DeclarationId, IdentityOrigin, OwnershipMode, ResolvedExprKind, ResolvedStatement,
    ResolvedType,
};
use semaprax::parse;

const SOURCE: &str = r#"
module test.hir;

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}

@id("buffer.consume")
fn consume(value: own Buffer) -> i64 { 1 }

@id("math.answer")
fn answer(value: i64) -> i64
    ensures result == value + 1
{
    let adjusted = value + 1;
    adjusted
}

@id("app.main")
fn main() -> i64 { answer(41) }
"#;

#[test]
fn resolution_is_deterministic_and_calls_use_declaration_ids() {
    let first = resolve(&parse(SOURCE, Path::new("first.spx")).unwrap()).unwrap();
    let second = resolve(&parse(SOURCE, Path::new("second.spx")).unwrap()).unwrap();
    assert_eq!(first, second);

    let main = first
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &main.body.kind else {
        panic!("function body must resolve to a block");
    };
    let ResolvedExprKind::Call { callee, .. } = &tail.kind else {
        panic!("main tail must resolve to a call");
    };
    assert_eq!(callee.as_str(), "math.answer");
    assert_eq!(
        tail.id.as_str(),
        "declaration:8:app.main:expression:9:body.tail"
    );
}

#[test]
fn semantic_identities_ignore_paths_whitespace_and_display_names() {
    let renamed = SOURCE
        .replace("fn answer(value: i64)", "fn calculate(value: i64)")
        .replace("answer(41)", "calculate(41)")
        .replace("module test.hir;", "module   test.hir ;");
    let original = resolve(&parse(SOURCE, Path::new("original/location.spx")).unwrap()).unwrap();
    let renamed = resolve(&parse(&renamed, Path::new("other/location.spx")).unwrap()).unwrap();

    let original_main = original
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let renamed_main = renamed
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let ResolvedExprKind::Block {
        tail: original_tail,
        ..
    } = &original_main.body.kind
    else {
        panic!("main body must be a block");
    };
    let ResolvedExprKind::Block {
        tail: renamed_tail, ..
    } = &renamed_main.body.kind
    else {
        panic!("main body must be a block");
    };
    let ResolvedExprKind::Call {
        callee: original_callee,
        ..
    } = &original_tail.kind
    else {
        panic!("main tail must be a call");
    };
    let ResolvedExprKind::Call {
        callee: renamed_callee,
        ..
    } = &renamed_tail.kind
    else {
        panic!("main tail must be a call");
    };

    assert_eq!(original_main.id, renamed_main.id);
    assert_eq!(original_main.body.id, renamed_main.body.id);
    assert_eq!(original_tail.id, renamed_tail.id);
    assert_eq!(original_callee, renamed_callee);
    assert_eq!(original_callee.as_str(), "math.answer");
}

#[test]
fn resource_names_resolve_to_persistent_type_identities() {
    let program = resolve(&parse(SOURCE, Path::new("resource.spx")).unwrap()).unwrap();
    let consume = program
        .functions
        .iter()
        .find(|function| function.name == "consume")
        .unwrap();
    assert_eq!(consume.params[0].ownership, OwnershipMode::Own);
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &consume.params[0].ty
    else {
        panic!("resource parameter must have a nominal type");
    };
    assert_eq!(declaration.as_str(), "buffer.type");
    assert!(arguments.is_empty());
    assert_eq!(program.declarations.type_id("Buffer").unwrap(), declaration);
    assert_eq!(
        consume.params[0].ty.identity_key(),
        "nominal:11:buffer.type:0:"
    );
    let facts = program
        .declarations
        .type_facts(&consume.params[0].ty)
        .unwrap();
    assert!(!facts.copy);
    assert!(facts.contains_resource);
    assert!(facts.sized);
    assert!(facts.needs_drop);
    assert_eq!(facts.layout_key, "resource:nominal:11:buffer.type:0:");
}

#[test]
fn type_facts_and_layout_keys_survive_display_name_renames() {
    let renamed = SOURCE
        .replace("resource Buffer {", "resource Store {")
        .replace(
            "fn consume(value: own Buffer)",
            "fn consume(value: own Store)",
        );
    let original = resolve(&parse(SOURCE, Path::new("original.spx")).unwrap()).unwrap();
    let renamed = resolve(&parse(&renamed, Path::new("renamed.spx")).unwrap()).unwrap();
    let original_ty = &original
        .functions
        .iter()
        .find(|function| function.name == "consume")
        .unwrap()
        .params[0]
        .ty;
    let renamed_ty = &renamed
        .functions
        .iter()
        .find(|function| function.name == "consume")
        .unwrap()
        .params[0]
        .ty;

    assert_eq!(original_ty, renamed_ty);
    assert_eq!(
        original.declarations.type_facts(original_ty).unwrap(),
        renamed.declarations.type_facts(renamed_ty).unwrap()
    );
}

#[test]
fn generic_identity_includes_parameter_owner_and_argument_tree() {
    let first_owner = DeclarationId::new("package.first");
    let second_owner = DeclarationId::new("package.second");
    let first = ResolvedType::TypeParameter {
        owner: first_owner.clone(),
        index: 0,
    };
    let second = ResolvedType::TypeParameter {
        owner: second_owner,
        index: 0,
    };
    assert_ne!(first.identity_key(), second.identity_key());

    let nominal = ResolvedType::Nominal {
        declaration: DeclarationId::new("core.option"),
        arguments: vec![first],
    };
    assert!(nominal.identity_key().contains("package.first"));
}

#[test]
fn local_references_resolve_to_their_place_identity() {
    let program = resolve(&parse(SOURCE, Path::new("places.spx")).unwrap()).unwrap();
    let answer = program
        .functions
        .iter()
        .find(|function| function.name == "answer")
        .unwrap();
    let ResolvedExprKind::Block { statements, tail } = &answer.body.kind else {
        panic!("answer body must be a block");
    };
    let ResolvedStatement::Let { binding, .. } = &statements[0] else {
        panic!("statement must be a let");
    };
    assert_eq!(
        binding.id.as_str(),
        "declaration:11:math.answer:value:local:7:body.s0"
    );
    let ResolvedExprKind::Place(place) = &tail.kind else {
        panic!("tail must be a resolved place");
    };
    assert_eq!(place.root, binding.id);
    assert!(place.projections.is_empty());
    assert_eq!(
        tail.id.as_str(),
        "declaration:11:math.answer:expression:9:body.tail"
    );
}

#[test]
fn postconditions_use_the_explicit_stable_result_identity() {
    let program = resolve(&parse(SOURCE, Path::new("result.spx")).unwrap()).unwrap();
    let answer = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "math.answer")
        .unwrap();
    let ResolvedExprKind::Binary { left, .. } = &answer.ensures[0].kind else {
        panic!("postcondition must be a binary expression");
    };
    let ResolvedExprKind::Place(result) = &left.kind else {
        panic!("postcondition left operand must be the result place");
    };
    let ResolvedExprKind::Block { statements, .. } = &answer.body.kind else {
        panic!("answer body must be a block");
    };
    let ResolvedStatement::Let { binding, .. } = &statements[0] else {
        panic!("statement must be a let");
    };

    assert_eq!(result.root, answer.result_id);
    assert_ne!(answer.result_id, answer.params[0].id);
    assert_ne!(answer.result_id, binding.id);
    assert_eq!(
        answer.result_id.as_str(),
        "declaration:11:math.answer:value:result:0:"
    );

    let renamed = SOURCE
        .replace("fn answer(value: i64)", "fn calculate(value: i64)")
        .replace("answer(41)", "calculate(41)");
    let renamed = resolve(&parse(&renamed, Path::new("renamed-result.spx")).unwrap()).unwrap();
    let renamed_answer = renamed
        .functions
        .iter()
        .find(|function| function.id.as_str() == "math.answer")
        .unwrap();
    assert_eq!(answer.result_id, renamed_answer.result_id);
}

#[test]
fn sibling_scope_bindings_have_distinct_place_identities() {
    let source = r#"
module test.sibling_places;
@id("scope.choose")
fn choose(flag: bool) -> i64 {
    if flag {
        let value = 1;
        value
    } else {
        let value = 2;
        value
    }
}
@id("app.main")
fn main() -> i64 { choose(true) }
"#;
    let program = resolve(&parse(source, Path::new("siblings.spx")).unwrap()).unwrap();
    let choose = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "scope.choose")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &choose.body.kind else {
        panic!("function body must be a block");
    };
    let ResolvedExprKind::If {
        then_branch,
        else_branch,
        ..
    } = &tail.kind
    else {
        panic!("tail must be an if expression");
    };
    let ResolvedExprKind::Block {
        statements: then_statements,
        tail: then_tail,
    } = &then_branch.kind
    else {
        panic!("then branch must be a block");
    };
    let ResolvedExprKind::Block {
        statements: else_statements,
        tail: else_tail,
    } = &else_branch.kind
    else {
        panic!("else branch must be a block");
    };
    let ResolvedStatement::Let {
        binding: then_binding,
        ..
    } = &then_statements[0]
    else {
        panic!("statement must be a let");
    };
    let ResolvedStatement::Let {
        binding: else_binding,
        ..
    } = &else_statements[0]
    else {
        panic!("statement must be a let");
    };
    let ResolvedExprKind::Place(then_place) = &then_tail.kind else {
        panic!("then tail must be a place");
    };
    let ResolvedExprKind::Place(else_place) = &else_tail.kind else {
        panic!("else tail must be a place");
    };

    assert_ne!(then_binding.id, else_binding.id);
    assert_eq!(then_place.root, then_binding.id);
    assert_eq!(else_place.root, else_binding.id);
}

#[test]
fn invalid_ast_is_rejected_before_hir_resolution() {
    let source = r#"
module test.invalid_hir;
@id("app.main")
fn main() -> i64 { missing(42) }
"#;
    let ast = parse(source, Path::new("invalid-hir.spx")).unwrap();
    let diagnostics = resolve(&ast).unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T203"));
    assert!(!diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.starts_with("SPX-H")));
}

#[test]
fn declaration_index_distinguishes_persistent_and_automatic_identities() {
    let source = r#"
module test.identity_origin;
resource Buffer {
    @id("test.identity_origin.buffer.drop")
    drop trivial;
}
fn main() -> i64 { 42 }
"#;
    let ast = parse(source, Path::new("identity-origin.spx")).unwrap();
    let program = resolve(&ast).unwrap();
    let resource = program
        .declarations
        .declaration(program.declarations.type_id("Buffer").unwrap())
        .unwrap();
    let main = program
        .declarations
        .declaration(program.declarations.function_id("main").unwrap())
        .unwrap();

    assert_eq!(resource.identity_origin, IdentityOrigin::Automatic);
    assert_eq!(main.identity_origin, IdentityOrigin::Automatic);
    assert!(!resource.identity_origin.is_persistent());
    assert_eq!(resource.identity_origin.text(), "automatic");
}
