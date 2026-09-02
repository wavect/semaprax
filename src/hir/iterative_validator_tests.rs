use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::*;
use crate::{hir, parse};

#[test]
fn hostile_declaration_index_rejects_reserved_host_id_for_every_authored_kind() {
    let source = r#"
module test.reserved_host_index;
@id("token") resource Token { @id("token.drop") drop trivial; }
@id("pair") record Pair { @id("pair.x") x: i64, }
@id("choice") variant Choice { @id("choice.a") A { @id("choice.a.x") x: i64, }, }
@id("class") class Class { @id("class.x") x: i64, @id("class.value") fn value(self: Class) -> i64 { self.x } }
@id("host") interface Host permits {} {
@id("host.echo") import rust fn echo(value: i64) -> i64 effects {} failure infallible;
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = parse(source, Path::new("reserved-host-index.spx")).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    for identity in [
        "token",
        "token.drop",
        "pair",
        "pair.x",
        "choice",
        "choice.a",
        "choice.a.x",
        "class",
        "class.x",
        "class.value",
        "host",
        "host.echo",
    ] {
        let mut hostile = resolved.clone();
        let original = DeclarationId::new(identity);
        let mut declaration = hostile
            .declarations
            .declarations
            .remove(&original)
            .expect("fixture declaration is indexed");
        declaration.id = DeclarationId::new(crate::host_io_ops::STDOUT_WRITE_ID);
        hostile
            .declarations
            .declarations
            .insert(declaration.id.clone(), declaration);
        let diagnostic = hir::validate(&hostile).unwrap_err();
        assert!(
            diagnostic
                .message
                .contains("aliases a compiler-owned host I/O operation"),
            "reserved hostile declaration kind `{identity}` was not rejected first: {diagnostic:?}"
        );
    }
}

#[test]
fn iterative_resolver_matches_recursive_reference_outside_builder_accounting() {
    let source = r#"
module test.resolver_oracle;
permit { host.echo }
@id("choice")
variant Choice {
  @id("choice.a") A { @id("choice.a.v") v: i64, },
  @id("choice.b") B,
}

@id("pair")
record Pair {
  @id("pair.a") a: i64,
  @id("pair.b") b: i64,
}
@id("host.echo.interface")
interface HostEcho permits { host.echo } {
  @id("host.echo") import rust fn host_echo(value: i64) -> i64
effects { host.echo }
failure status "host.echo.v1";
}
@id("callee") fn callee(a: i64, b: i64) -> i64 { a + b }
@id("identity") fn identity<T>(value: T) -> T { value }
@id("option_use") fn option_use(value: Option<i64>) -> Option<bool> {
  let checked = value?;
  Option<bool>::Some { value: checked > 0 }
}
@id("result_use") fn result_use(value: Result<i64, bool>) -> Result<bool, bool> {
  let checked = value?;
  Result<bool, bool>::Ok { value: checked > 0 }
}
@id("match_value") fn match_value(value: i64) -> i64 {
  match value { _ => value, }
}
@id("match_own") fn match_own(value: Choice) -> i64 {
  match value { Choice::A { v } => v, Choice::B {} => 0, }
}
@id("match_borrow") fn match_borrow(value: Choice) -> i64 {
  match value { Choice::A { v } => v, Choice::B {} => 0, }
}
@id("exercise") fn exercise(flag: bool, choice: Choice, pair: Pair) -> i64
  uses { host.echo }
{
  let x = callee(1, 2);
  let mut total = x;
  total = total + x;
  let native = host_echo(identity<i64>(total));
  let rebuilt = if flag && !false { Choice::A { v: Pair { a: native, b: 3 }.a } } else { choice };
  let y = pair with { b: 4 }.b;
  match rebuilt { Choice::A { v } => y + v, Choice::B {} => -y, }
}
@id("main") fn main() -> i64 { 0 }
"#;
    let parsed = parse(source, Path::new("resolver-oracle.spx")).unwrap();
    let resolved = hir::resolve(&parsed).unwrap();
    let resolver = Resolver {
        program: &parsed,
        declarations: DeclarationIndex::from_verified(&parsed).unwrap(),
    };
    for source_function in &parsed.functions {
        let Some(resolved_function) = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == source_function.stable_id)
        else {
            continue;
        };
        let execution = FunctionExecutionId::Monomorphic(resolved_function.id.clone());
        let bindings = source_function
            .params
            .iter()
            .zip(&resolved_function.params)
            .map(|(source, resolved)| {
                (
                    source.name.clone(),
                    Binding {
                        id: resolved.id.clone(),
                        ty: resolved.ty.clone(),
                        ownership: resolved.ownership,
                        mutable: false,
                    },
                )
            })
            .collect();
        let iterative =
            resolver.resolve_expr_iterative(&execution, &source_function.body, &bindings, "body");
        let recursive = resolver.resolve_expr_recursive_reference(
            &execution,
            &source_function.body,
            &bindings,
            "body",
        );
        match (iterative, recursive) {
            (Ok(iterative), Ok(recursive)) => assert_eq!(iterative, recursive),
            (Err(iterative), Err(recursive)) => {
                assert_eq!(iterative.code, recursive.code);
                assert_eq!(iterative.severity, recursive.severity);
                assert_eq!(iterative.message, recursive.message);
                assert_eq!(iterative.path, recursive.path);
                assert_eq!(iterative.span, recursive.span);
                assert_eq!(iterative.help, recursive.help);
            }
            (iterative, recursive) => panic!(
                "resolver oracle outcome differs: iterative={iterative:?}, recursive={recursive:?}"
            ),
        }
    }

    for (function_id, expected_mode) in [
        ("match_value", ResolvedMatchMode::Value),
        ("match_own", ResolvedMatchMode::Value),
        ("match_borrow", ResolvedMatchMode::Value),
    ] {
        let function = resolved
            .functions
            .iter()
            .find(|function| function.id.as_str() == function_id)
            .expect("match-mode fixture is resolved");
        let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("match-mode fixture body is a block with a tail")
        };
        let ResolvedExprKind::Match { mode, .. } = &tail.kind else {
            panic!("match-mode fixture tail is a match")
        };
        assert_eq!(*mode, expected_mode);
    }

    let invalid = parse(
        "module test.resolver_invalid; @id(\"main\") fn main() -> i64 { missing }",
        Path::new("resolver-invalid.spx"),
    )
    .unwrap();
    let execution = FunctionExecutionId::Monomorphic(DeclarationId::new("main"));
    let iterative = resolver.resolve_expr_iterative(
        &execution,
        &invalid.functions[0].body,
        &BTreeMap::new(),
        "body",
    );
    let recursive = resolver.resolve_expr_recursive_reference(
        &execution,
        &invalid.functions[0].body,
        &BTreeMap::new(),
        "body",
    );
    let (Err(iterative), Err(recursive)) = (iterative, recursive) else {
        panic!("unresolved-value oracle must fail in both evaluators")
    };
    assert_eq!(iterative.code, recursive.code);
    assert_eq!(iterative.severity, recursive.severity);
    assert_eq!(iterative.message, recursive.message);
    assert_eq!(iterative.path, recursive.path);
    assert_eq!(iterative.span, recursive.span);
    assert_eq!(iterative.help, recursive.help);
}

const SOURCE: &str = r#"
module test.validator_oracle_hostiles;
permit { host.echo }

@id("token.type")
resource Token { @id("token.drop") drop trivial; }

@id("owned.box")
record OwnedBox { @id("owned.box.token") token: Token, }

@id("choice.type")
variant Choice { @id("choice.a") A, @id("choice.b") B, }

@id("host.echo.interface")
interface HostEcho permits { host.echo } {
@id("host.echo")
import rust fn host_echo(value: i64) -> i64
    effects { host.echo }
    failure status "host.echo.v1";
}

@id("token.consume")
fn consume(token: own Token) -> i64 { 1 }

@id("token.consume_bool")
fn consume_bool(token: own Token) -> bool { true }

@id("hostile.call")
fn call_hostile(token: own Token) -> i64 { consume(token) }

@id("hostile.native")
fn native_hostile(token: own Token, value: i64) -> i64
uses { host.echo }
{ host_echo(value) }

@id("hostile.construct")
fn construct_hostile(token: own Token) -> OwnedBox { OwnedBox { token: token } }

@id("hostile.update")
fn update_hostile(input: own OwnedBox, token: own Token) -> OwnedBox {
input with { token: token }
}

@id("hostile.block_statement")
fn block_statement_hostile(token: own Token) -> i64 {
let used = consume(token);
used
}

@id("hostile.block_tail")
fn block_tail_hostile(token: own Token) -> i64 {
let zero = 0;
consume(token)
}

@id("hostile.if")
fn if_hostile(flag: bool, token: own Token) -> i64 {
if flag { consume(token) } else { 0 }
}

@id("hostile.lazy")
fn lazy_hostile(flag: bool, token: own Token) -> bool {
flag && consume_bool(token)
}

@id("hostile.match")
fn match_hostile(choice: Choice, token: own Token) -> i64 {
match choice { Choice::A {} => consume(token), Choice::B {} => 0, }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn program() -> ResolvedProgram {
    hir::resolve(&parse(SOURCE, Path::new("validator-oracle-hostiles.spx")).unwrap()).unwrap()
}

fn function_index(program: &ResolvedProgram, id: &str) -> usize {
    program
        .functions
        .iter()
        .position(|function| function.id.as_str() == id)
        .unwrap()
}

fn tail_mut(function: &mut ResolvedFunction) -> &mut ResolvedExpr {
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        panic!("fixture function body must remain a block")
    };
    tail
}

fn validation_scope(function: &ResolvedFunction) -> BTreeMap<ValueId, ValidationBinding> {
    function
        .params
        .iter()
        .map(|param| {
            (
                param.id.clone(),
                ValidationBinding {
                    ty: param.ty.clone(),
                    ownership: param.ownership,
                    availability: Availability::Available,
                    active_loans: BTreeSet::new(),
                    moved_places: BTreeMap::new(),
                    definitely_partial: BTreeSet::new(),
                },
            )
        })
        .collect()
}

fn validate_expression_hostile(
    program: &ResolvedProgram,
    function_id: &str,
    expression: &ResolvedExpr,
    path: &str,
) -> BTreeMap<String, Availability> {
    let function = &program.functions[function_index(program, function_id)];
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let mut scope = validation_scope(function);
    let mut recursive_scope = scope.clone();
    let mut validator = HirValidator::new(program).unwrap();
    let mut recursive_validator = validator.clone();
    let allowed_effects = function.effects.iter().cloned().collect();
    let recursive = recursive_validator.validate_expr_recursive_reference(
        &execution,
        expression,
        &mut recursive_scope,
        path,
        true,
        Some(&allowed_effects),
    );
    let iterative = validator.validate_expr_iterative(
        &execution,
        expression,
        &mut scope,
        path,
        true,
        Some(&allowed_effects),
    );
    HirValidator::assert_validation_oracle(
        &iterative,
        &recursive,
        &validator,
        &recursive_validator,
        &scope,
        &recursive_scope,
        path,
    );
    let diagnostic = iterative.unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006", "{function_id}");
    function
        .params
        .iter()
        .map(|param| {
            (
                param.name.clone(),
                scope.get(&param.id).unwrap().availability,
            )
        })
        .collect()
}

#[test]
fn validator_oracle_preserves_direct_child_scope_on_late_errors() {
    for (function_id, expected_token, expected_input) in [
        ("hostile.call", Availability::Moved, None),
        ("hostile.native", Availability::Available, None),
        ("hostile.construct", Availability::Moved, None),
        (
            "hostile.update",
            Availability::Moved,
            Some(Availability::Moved),
        ),
    ] {
        let mut hostile = program();
        let index = function_index(&hostile, function_id);
        tail_mut(&mut hostile.functions[index]).ownership = match function_id {
            "hostile.construct" | "hostile.update" => OwnershipMode::Value,
            "hostile.call" | "hostile.native" => OwnershipMode::Borrow,
            _ => unreachable!(),
        };
        let expression = tail_mut(&mut hostile.functions[index]).clone();
        let scope = validate_expression_hostile(&hostile, function_id, &expression, "body.tail");
        assert_eq!(scope["token"], expected_token);
        if let Some(expected) = expected_input {
            assert_eq!(scope["input"], expected);
        }
    }
}

#[test]
fn validator_oracle_suppresses_failed_block_branch_lazy_and_match_child_scopes() {
    for function_id in [
        "hostile.block_statement",
        "hostile.block_tail",
        "hostile.if",
        "hostile.lazy",
        "hostile.match",
    ] {
        let mut hostile = program();
        let index = function_index(&hostile, function_id);
        let body = &mut hostile.functions[index].body;
        match function_id {
            "hostile.block_statement" => {
                let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
                    unreachable!()
                };
                let ResolvedStatement::Let { binding, .. } = &mut statements[0] else {
                    unreachable!("fixture statement is a let")
                };
                binding.ty = ResolvedType::Bool;
            }
            "hostile.block_tail" => {
                tail_mut(&mut hostile.functions[index]).ownership = OwnershipMode::Borrow;
            }
            "hostile.if" => {
                let ResolvedExprKind::If { then_branch, .. } =
                    &mut tail_mut(&mut hostile.functions[index]).kind
                else {
                    unreachable!()
                };
                then_branch.ownership = OwnershipMode::Borrow;
            }
            "hostile.lazy" => {
                let ResolvedExprKind::Binary { right, .. } =
                    &mut tail_mut(&mut hostile.functions[index]).kind
                else {
                    unreachable!()
                };
                right.ownership = OwnershipMode::Borrow;
            }
            "hostile.match" => {
                let ResolvedExprKind::Match { arms, .. } =
                    &mut tail_mut(&mut hostile.functions[index]).kind
                else {
                    unreachable!()
                };
                arms[0].value.ownership = OwnershipMode::Borrow;
            }
            _ => unreachable!(),
        }
        let expression = hostile.functions[index].body.clone();
        let scope = validate_expression_hostile(&hostile, function_id, &expression, "body");
        assert_eq!(scope["token"], Availability::Available, "{function_id}");
    }
}

#[test]
fn validator_oracle_handles_an_exact_depth_512_late_error_with_a_nonempty_scope() {
    fn run() {
        const UNARY_NODES: usize = 510;
        let source = format!(
            "module test.validator_depth; @id(\"token.type\") resource Token {{ @id(\"token.drop\") drop trivial; }} @id(\"token.consume\") fn consume(token: own Token) -> i64 {{ 1 }} @id(\"hostile.depth\") fn deep(token: own Token) -> i64 {{ {}consume(token) }} @id(\"app.main\") fn main() -> i64 {{ 0 }}",
            "-".repeat(UNARY_NODES)
        );
        let mut hostile =
            hir::resolve(&parse(&source, Path::new("validator-depth-hostile.spx")).unwrap())
                .unwrap();
        let index = function_index(&hostile, "hostile.depth");
        let expression = tail_mut(&mut hostile.functions[index]);
        let mut depth = 0;
        let mut cursor = &*expression;
        loop {
            depth += 1;
            match &cursor.kind {
                ResolvedExprKind::Unary { value, .. } => cursor = value,
                ResolvedExprKind::Call { args, .. } => cursor = &args[0],
                ResolvedExprKind::Place(_) => break,
                _ => panic!("unexpected exact-depth fixture shape"),
            }
        }
        assert_eq!(depth, 512);
        expression.ownership = OwnershipMode::Borrow;
        let expression = expression.clone();
        let scope =
            validate_expression_hostile(&hostile, "hostile.depth", &expression, "body.tail");
        assert_eq!(scope["token"], Availability::Moved);
    }

    std::thread::Builder::new()
        .name("validator-depth-oracle".to_owned())
        .stack_size(16 * 1024 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
