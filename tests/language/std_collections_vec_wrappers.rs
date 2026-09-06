use std::path::Path;

use semaprax::hir::{self, DeclarationId, ResolvedExprKind, ResolvedType};
use semaprax::{graph, parse, verify};

const WRAPPERS: &str = r#"
module std.collections;
@id("std.collections.vec.with-capacity")
fn with_capacity<T>(capacity: usize) -> Vec<T> { vec_with_capacity<T>(capacity) }
@id("std.collections.vec.push")
fn push<T>(values: own Vec<T>, value: T) -> Vec<T> { vec_push<T>(values, value) }
@id("std.collections.vec.len")
fn len<T>(values: borrow Vec<T>) -> usize { vec_len<T>(values) }
@id("std.collections.vec.capacity")
fn capacity<T>(values: borrow Vec<T>) -> usize { vec_capacity<T>(values) }
@id("std.collections.vec.get")
fn get<T>(values: borrow Vec<T>, index: usize) -> T { vec_get<T>(values, index) }
@id("std.collections.vec.reserve-exact")
fn reserve_exact<T>(values: own Vec<T>, additional: usize) -> Vec<T> { vec_reserve_exact<T>(values, additional) }
@id("std.collections.vec.set")
fn set<T>(values: own Vec<T>, index: usize, value: T) -> Vec<T> { vec_set<T>(values, index, value) }
@id("std.collections.vec.clear")
fn clear<T>(values: own Vec<T>) -> Vec<T> { vec_clear<T>(values) }
@id("app.main")
fn main() -> i64 {
    let mut values = with_capacity<i64>(2usize);
    values = push<i64>(values, 7);
    values = reserve_exact<i64>(values, 2usize);
    values = set<i64>(values, 0usize, 9);
    let preserved = len<i64>(values) == 1usize && capacity<i64>(values) == 4usize && get<i64>(values, 0usize) == 9;
    values = clear<i64>(values);
    if preserved && len<i64>(values) == 0usize { 0 } else { 1 }
}
"#;

fn parsed(source: &str) -> semaprax::ast::Program {
    parse(source, Path::new("std-collections-vec-wrappers.spx")).unwrap()
}

fn errors(source: &str) -> Vec<&'static str> {
    verify::verify(&parsed(source))
        .into_iter()
        .filter(|diagnostic| diagnostic.severity.is_error())
        .map(|diagnostic| diagnostic.code)
        .collect()
}

fn tail_call(expression: &hir::ResolvedExpr) -> (&DeclarationId, &[ResolvedType], bool) {
    let ResolvedExprKind::Block { tail, .. } = &expression.kind else {
        panic!("wrapper body must be a block")
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance,
        ..
    } = &tail.kind
    else {
        panic!("wrapper tail must be a call")
    };
    (callee, type_arguments, instance.is_none())
}

#[test]
fn exact_wrappers_preserve_authored_and_intrinsic_hir_and_graph_identity() {
    let program = parsed(WRAPPERS);
    assert!(verify::verify(&program).is_empty());
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    for (wrapper, intrinsic) in [
        (
            "std.collections.vec.with-capacity",
            "core.vec.with-capacity",
        ),
        ("std.collections.vec.push", "core.vec.push"),
        ("std.collections.vec.len", "core.vec.len"),
        ("std.collections.vec.capacity", "core.vec.capacity"),
        ("std.collections.vec.get", "core.vec.get"),
        (
            "std.collections.vec.reserve-exact",
            "core.vec.reserve-exact",
        ),
        ("std.collections.vec.set", "core.vec.set"),
        ("std.collections.vec.clear", "core.vec.clear"),
    ] {
        let template = resolved
            .function_templates
            .iter()
            .find(|template| template.id.as_str() == wrapper)
            .unwrap();
        let (callee, arguments, no_instance) = tail_call(&template.body);
        assert_eq!(callee.as_str(), intrinsic);
        assert!(no_instance);
        assert!(matches!(arguments,
            [ResolvedType::TypeParameter { owner, index: 0 }] if owner == &template.id));
        let instance = resolved
            .function_instances
            .iter()
            .find(|instance| instance.template.as_str() == wrapper)
            .unwrap();
        assert_eq!(instance.type_arguments, [ResolvedType::I64]);
        let (callee, arguments, no_instance) = tail_call(&instance.function.body);
        assert_eq!(callee.as_str(), intrinsic);
        assert_eq!(arguments, [ResolvedType::I64]);
        assert!(no_instance);
    }
    let rendered = graph::to_json(&program).unwrap();
    assert!(rendered.contains("semaprax.prelude.v3"));
    for identity in [
        "std.collections.vec.with-capacity",
        "std.collections.vec.push",
        "std.collections.vec.len",
        "std.collections.vec.capacity",
        "std.collections.vec.get",
        "std.collections.vec.reserve-exact",
        "std.collections.vec.set",
        "std.collections.vec.clear",
        "core.vec.with-capacity",
        "core.vec.push",
        "core.vec.len",
        "core.vec.capacity",
        "core.vec.get",
        "core.vec.reserve-exact",
        "core.vec.set",
        "core.vec.clear",
    ] {
        assert!(rendered.contains(identity), "missing {identity} from Graph");
    }
}

#[test]
fn exact_wrappers_admit_all_copy_scalars_only_with_explicit_arguments() {
    for (ty, literal) in [
        ("i64", "7"),
        ("i32", "7i32"),
        ("u8", "7u8"),
        ("usize", "7usize"),
        ("char", "'a'"),
        ("f32", "7.0f32"),
        ("f64", "7.0"),
        ("bool", "true"),
    ] {
        let source = WRAPPERS.replace(
            "let mut values = with_capacity<i64>(2usize);\n    values = push<i64>(values, 7);\n    values = reserve_exact<i64>(values, 2usize);\n    values = set<i64>(values, 0usize, 9);\n    let preserved = len<i64>(values) == 1usize && capacity<i64>(values) == 4usize && get<i64>(values, 0usize) == 9;\n    values = clear<i64>(values);\n    if preserved && len<i64>(values) == 0usize { 0 } else { 1 }",
            &format!("let mut values = with_capacity<{ty}>(2usize);\n    values = push<{ty}>(values, {literal});\n    values = reserve_exact<{ty}>(values, 2usize);\n    values = set<{ty}>(values, 0usize, {literal});\n    let preserved = len<{ty}>(values) == 1usize && capacity<{ty}>(values) == 4usize && get<{ty}>(values, 0usize) == {literal};\n    values = clear<{ty}>(values);\n    if preserved && len<{ty}>(values) == 0usize {{ 0 }} else {{ 1 }}"),
        );
        let program = parsed(&source);
        assert!(verify::verify(&program).is_empty(), "{ty}");
        hir::resolve(&program).unwrap();
    }
    let inferred = WRAPPERS.replace("with_capacity<i64>(2usize)", "with_capacity(2usize)");
    assert!(errors(&inferred).contains(&"SPX-T225"));
    let unsupported =
        WRAPPERS.replace("with_capacity<i64>(2usize)", "with_capacity<Bytes>(2usize)");
    assert!(errors(&unsupported).contains(&"SPX-T225"));
    let inferred_new = WRAPPERS.replace("reserve_exact<i64>(values", "reserve_exact(values");
    assert!(errors(&inferred_new).contains(&"SPX-T225"));
    let oversized = r#"
module test.vec_reserve;
@id("app.main") fn main() -> Vec<i64> {
    let mut values = vec_with_capacity<i64>(1usize);
    values = vec_reserve_exact<i64>(values, 8193usize);
    values
}
"#;
    assert!(errors(oversized).contains(&"SPX-T282"));
    let intrinsic_inference = WRAPPERS.replace(
        "vec_reserve_exact<T>(values, additional)",
        "vec_reserve_exact(values, additional)",
    );
    assert!(errors(&intrinsic_inference).contains(&"SPX-T283"));
}

#[test]
fn wrapper_lookalikes_and_hostile_hir_fail_closed() {
    for malformed in [
        WRAPPERS.replace("std.collections.vec.len", "std.collections.vec.wrong"),
        WRAPPERS.replace("values: borrow Vec<T>", "values: Vec<T>"),
        WRAPPERS.replace("vec_capacity<T>(values)", "vec_len<T>(values)"),
        WRAPPERS.replace("vec_get<T>(values, index)", "vec_get<i64>(values, index)"),
        WRAPPERS.replace(
            "vec_reserve_exact<T>(values, additional)",
            "vec_reserve_exact<T>(values, 1usize)",
        ),
        WRAPPERS.replace(
            "vec_set<T>(values, index, value)",
            "vec_set<T>(values, index, index)",
        ),
        WRAPPERS.replace("vec_clear<T>(values)", "vec_push<T>(values, value)"),
    ] {
        assert!(errors(&malformed).contains(&"SPX-T283"));
    }
    let lookalike = WRAPPERS.replace("module std.collections", "module user.collections");
    assert!(errors(&lookalike).contains(&"SPX-T283"));

    let mut effectful = parsed(WRAPPERS);
    effectful.functions[0].effects.push("host.clock".to_owned());
    assert!(verify::verify(&effectful)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T283"));

    let resolved = hir::resolve(&parsed(WRAPPERS)).unwrap();
    let mut wrong_callee = resolved.clone();
    let template = wrong_callee
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "std.collections.vec.get")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
        panic!()
    };
    let ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
        panic!()
    };
    *callee = DeclarationId::new("core.vec.capacity");
    assert_eq!(hir::validate(&wrong_callee).unwrap_err().code, "SPX-H006");

    let mut wrong_forwarding = resolved;
    let template = wrong_forwarding
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "std.collections.vec.push")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
        panic!()
    };
    let ResolvedExprKind::Call { type_arguments, .. } = &mut tail.kind else {
        panic!()
    };
    type_arguments[0] = ResolvedType::I64;
    assert_eq!(
        hir::validate(&wrong_forwarding).unwrap_err().code,
        "SPX-H006"
    );

    let resolved = hir::resolve(&parsed(WRAPPERS)).unwrap();
    let mut wrong_new_callee = resolved.clone();
    let template = wrong_new_callee
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "std.collections.vec.reserve-exact")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut template.body.kind else {
        panic!()
    };
    let ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
        panic!()
    };
    *callee = DeclarationId::new("core.vec.clear");
    assert_eq!(
        hir::validate(&wrong_new_callee).unwrap_err().code,
        "SPX-H006"
    );

    let mut wrong_new_ownership = resolved;
    let template = wrong_new_ownership
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "std.collections.vec.clear")
        .unwrap();
    template.params[0].ownership = hir::OwnershipMode::Borrow;
    assert_eq!(
        hir::validate(&wrong_new_ownership).unwrap_err().code,
        "SPX-H006"
    );
}
