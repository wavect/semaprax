//! Program-level AST lowering.
//!
//! `hir::resolve` runs source verification first and fails closed, so the
//! lowering diagnostics below are unreachable from a `.spx` file that the
//! verifier already rejects. These tests drive the resolver directly, which is
//! the seam a hostile or machine-authored AST reaches, and pin the identities,
//! order, and stable codes it must produce there.

use std::path::Path;

use crate::hir::DeclarationIndex;

use super::*;

fn parsed(source: &str, path: &str) -> crate::ast::Program {
    crate::parse(source, Path::new(path)).expect("fixture parses")
}

fn resolver(program: &crate::ast::Program) -> Resolver<'_> {
    Resolver {
        program,
        declarations: DeclarationIndex::from_verified(program).expect("declarations index"),
        reuse: None,
        function_work: super::super::FunctionResolutionWork::default(),
    }
}

fn named(name: &str, arguments: Vec<crate::ast::Type>) -> crate::ast::Type {
    crate::ast::Type::Named {
        name: name.to_owned(),
        arguments,
    }
}

#[test]
fn lowering_a_module_without_an_entry_point_is_spx_h005() {
    let program = parsed(
        r#"
module test.hir_resolve_no_entry;

@id("app.helper")
fn helper(value: i64) -> i64
{
    value
}
"#,
        "hir-resolve-no-entry.spx",
    );
    // The resolved program's `entrypoint` is not optional, so a module with no
    // `main` must be refused here rather than resolved with a placeholder.
    let Err(diagnostic) = resolver(&program).resolve() else {
        panic!("a module without `main` has no entry point")
    };
    assert_eq!(diagnostic.code, "SPX-H005");
}

#[test]
fn a_mutually_recursive_record_layout_never_reaches_lowering() {
    let program = parsed(
        r#"
module test.hir_resolve_recursive;

@id("data.left")
record Left {
    @id("data.left.right")
    right: Right,
}

@id("data.right")
record Right {
    @id("data.right.left")
    left: Left,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#,
        "hir-resolve-recursive.spx",
    );
    // Neither record names itself; only the cycle through the other one makes
    // the by-value layout infinite. It is refused under the same stable code
    // as direct self-recursion, and before any HIR exists.
    let diagnostics = crate::hir::resolve(&program).expect_err("a record cycle has no layout");
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T217"),
        "{diagnostics:#?}"
    );
    // The declaration index is the first gate, so the resolver's own layout
    // check is never reached for this input.
    assert_eq!(
        DeclarationIndex::from_verified(&program)
            .expect_err("the index refuses the cycle too")
            .code,
        "SPX-T217"
    );
}

#[test]
fn shared_record_fields_are_a_diamond_and_not_a_recursive_layout() {
    let program = parsed(
        r#"
module test.hir_resolve_diamond;

@id("data.leaf")
record Leaf {
    @id("data.leaf.count")
    count: i64,
}

@id("data.pair")
record Pair {
    @id("data.pair.first")
    first: Leaf,
    @id("data.pair.second")
    second: Leaf,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#,
        "hir-resolve-diamond.spx",
    );
    // Two fields of the same record type visit `Leaf` twice. A layout check
    // that treated a repeated visit as a cycle would reject ordinary
    // aggregates, so the admitting direction is pinned as well.
    assert!(resolver(&program).validate_record_layouts().is_ok());
}

const TYPES: &str = r#"
module test.hir_resolve_types;

@id("data.point")
record Point {
    @id("data.point.x")
    x: i64,
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

#[test]
fn resolving_an_undeclared_named_type_is_spx_h001() {
    let program = parsed(TYPES, "hir-resolve-types.spx");
    let resolver = resolver(&program);
    let diagnostic = resolver
        .resolve_type(&named("Missing", Vec::new()), crate::ast::Span::default())
        .expect_err("an undeclared type has no identity");
    assert_eq!(diagnostic.code, "SPX-H001");

    // A declared type still resolves to its stable identity, not its name.
    assert_eq!(
        resolver
            .resolve_type(&named("Point", Vec::new()), crate::ast::Span::default())
            .unwrap(),
        ResolvedType::Nominal {
            declaration: DeclarationId::new("data.point"),
            arguments: Vec::new(),
        }
    );
}

#[test]
fn generic_type_arguments_are_limited_to_copy_scalars_and_the_owned_byte_prelude() {
    let program = parsed(TYPES, "hir-resolve-types.spx");
    let resolver = resolver(&program);
    let resolve = |ty: crate::ast::Type| resolver.resolve_type(&ty, crate::ast::Span::default());

    assert!(resolve(named("Option", vec![crate::ast::Type::I64])).is_ok());
    assert!(resolve(named("Option", vec![crate::ast::Type::Bool])).is_ok());
    // `Option<Bytes>` and the two mixed `Result` shapes are the exact
    // compiler-owned carriers admitted past the copy-scalar rule.
    assert!(resolve(named("Option", vec![crate::ast::Type::Bytes])).is_ok());
    assert!(resolve(named(
        "Result",
        vec![crate::ast::Type::Bytes, crate::ast::Type::I64]
    ))
    .is_ok());
    assert!(resolve(named(
        "Result",
        vec![crate::ast::Type::I64, crate::ast::Type::Bytes]
    ))
    .is_ok());

    // Anything else, including a second owned carrier in the same instance,
    // leaves the admitted profile.
    for rejected in [
        named("Option", vec![crate::ast::Type::Usize]),
        named("Option", vec![crate::ast::Type::String]),
        named(
            "Result",
            vec![crate::ast::Type::Bytes, crate::ast::Type::Bytes],
        ),
        // Arity is checked in the same place: `Option` takes exactly one.
        named("Option", Vec::new()),
        named("Option", vec![crate::ast::Type::I64, crate::ast::Type::I64]),
    ] {
        let diagnostic = resolve(rejected.clone())
            .expect_err("type outside the admitted generic profile is refused");
        assert_eq!(diagnostic.code, "SPX-H006", "{rejected:?}");
    }
}

#[test]
fn function_type_parameters_resolve_to_their_declaration_index() {
    let program = parsed(
        r#"
module test.hir_resolve_type_parameters;

@id("app.pick")
fn pick<Left, Right>(left: Left, right: Right, flag: bool) -> Left
{
    if flag { left } else { left }
}

@id("app.main")
fn main() -> i64
{
    pick<i64, bool>(1, true, true)
}
"#,
        "hir-resolve-type-parameters.spx",
    );
    let resolver = resolver(&program);
    let pick = program
        .functions
        .iter()
        .find(|function| function.name == "pick")
        .expect("generic function parsed");
    let resolve = |name: &str| {
        resolver.resolve_function_type(pick, &named(name, Vec::new()), crate::ast::Span::default())
    };

    // Index is the declaration position, so reordering the parameter list is a
    // breaking change the instance identity must see.
    assert_eq!(
        resolve("Left").unwrap(),
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("app.pick"),
            index: 0,
        }
    );
    assert_eq!(
        resolve("Right").unwrap(),
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("app.pick"),
            index: 1,
        }
    );
    // A name that is not in scope as a parameter falls through to ordinary
    // type resolution and is rejected there.
    assert_eq!(
        resolve("Missing").expect_err("not a parameter").code,
        "SPX-H001"
    );
}

#[test]
fn discovered_instances_follow_call_order_and_collapse_repeats() {
    let program = parsed(
        r#"
module test.hir_resolve_instances;

@id("app.identity")
fn identity<T>(value: T) -> T
{
    value
}

@id("app.main")
fn main() -> i64
{
    let first = identity<bool>(true);
    let second = identity<i64>(4);
    let third = identity<i64>(5);
    if first { second + third } else { 0 }
}
"#,
        "hir-resolve-instances.spx",
    );
    let instances = resolver(&program)
        .discover_function_instances()
        .expect("instances discover");
    // `bool` is called first even though `i64` sorts earlier, and the second
    // `i64` call reuses the first instance rather than adding one.
    assert_eq!(
        instances
            .iter()
            .map(|instance| instance.type_arguments.clone())
            .collect::<Vec<_>>(),
        vec![vec![ResolvedType::Bool], vec![ResolvedType::I64]]
    );
    assert!(instances
        .iter()
        .all(|instance| instance.template == DeclarationId::new("app.identity")));
    // Two resolutions of the same source agree, identities included.
    let repeated = resolver(&program)
        .discover_function_instances()
        .expect("instances discover");
    assert_eq!(
        instances
            .iter()
            .map(|instance| instance.id.as_str().to_owned())
            .collect::<Vec<_>>(),
        repeated
            .iter()
            .map(|instance| instance.id.as_str().to_owned())
            .collect::<Vec<_>>()
    );
}

#[test]
fn resolved_functions_keep_source_order_and_place_class_methods_last() {
    let source = r#"
module test.hir_resolve_order;

@id("data.counter")
class Counter {
    @id("data.counter.value")
    value: i64,

    @id("data.counter.bumped")
    fn bumped(self: Counter, amount: i64) -> Counter
{
        Counter { value: self.value + amount }
    }
}

@id("app.zed")
fn zed(value: i64) -> i64
{
    value
}

@id("app.alpha")
fn alpha(value: i64) -> i64
{
    zed(value)
}

@id("app.main")
fn main() -> i64
{
    alpha(1)
}
"#;
    let ast = parsed(source, "hir-resolve-order.spx");
    let program = crate::hir::resolve(&ast).expect("fixture resolves");
    // Free functions first in declaration order (`zed` before `alpha`, which
    // is not alphabetical), then class methods. Positional consumers and the
    // graph projection both depend on this being source order, not name order.
    assert_eq!(
        program
            .functions
            .iter()
            .map(|function| function.id.as_str().to_owned())
            .collect::<Vec<_>>(),
        vec![
            "app.zed".to_owned(),
            "app.alpha".to_owned(),
            "app.main".to_owned(),
            "data.counter.bumped".to_owned(),
        ]
    );
    assert_eq!(program.entrypoint, DeclarationId::new("app.main"));
}
