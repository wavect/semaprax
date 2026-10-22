//! Carrier-free root census: independent AST and resolved-HIR position oracles,
//! a complete-forecast differential against the position-blind formula, exact
//! and one-short limits, malformed-census refusal, and a resolver/replay
//! allocation highwater gate. Variable roots keep all storage and closure
//! subtrees keep every variable auxiliary carrier. This source batch awaits
//! executable qualification. Nothing here mirrors `declaration_cost`.
use super::super::call_carriers_original::historical_scoped as synthetic_builder_bytes_scoped;
mod cached_exact {
    pub(super) use super::super::super::cached_exact::fixed_upper;
    pub(super) use super::super::super::call_carriers_original::historical_cached::{
        summed, Retention,
    };
}
use super::super::dependency_identity_max;
use super::StructuralCost;
use crate::ast::{Expr, ExprKind, Program};
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedStatement};

/// Independent actual-layout oracle for the six omitted carriers.
fn carriers() -> usize {
    use std::mem::size_of;
    size_of::<crate::hir::ResolvedBinding>()
        + size_of::<crate::hir::ResolvedStatement>()
        + size_of::<crate::hir::ResolvedFieldInitializer>()
        + size_of::<crate::hir::ResolvedMatchArm>()
        + size_of::<crate::hir::ResolvedMatchPatternField>()
        + size_of::<crate::hir::ResolvedRecordMatchPatternField>()
}

#[derive(Debug, Default, Eq, PartialEq)]
struct Census {
    kinds: [usize; 3],
    free: usize,
    /// Free `if` roots: carrier-free, but never storage-free.
    ifs: usize,
    /// Free variable roots outside authored closures: never storage-free.
    vars: usize,
    /// Free blocks outside closures, retaining storage and every child carrier.
    blocks: usize,
}

fn ast_kind(kind: &ExprKind) -> Option<usize> {
    match kind {
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_) => Some(0),
        ExprKind::Unary { .. } => Some(1),
        ExprKind::Binary { .. } => Some(2),
        _ => None,
    }
}

/// Raw AST oracle: a root is free only as a unary/binary operand, an `if`
/// condition/branch, a block tail, or a free-function call argument.
fn ast_census(expression: &Expr, free: bool, outside_closure: bool, census: &mut Census) {
    if let Some(kind) = ast_kind(&expression.kind) {
        census.kinds[kind] += 1;
        census.free += usize::from(free);
    }
    if matches!(expression.kind, ExprKind::If { .. }) {
        census.ifs += usize::from(free);
    }
    if outside_closure && matches!(expression.kind, ExprKind::Var(_)) {
        census.vars += usize::from(free);
    }
    if outside_closure && matches!(expression.kind, ExprKind::Block { .. }) {
        census.blocks += usize::from(free);
    }
    let outside_children = outside_closure && !matches!(expression.kind, ExprKind::Closure { .. });
    let free_children: Vec<&Expr> = match &expression.kind {
        ExprKind::Unary { value, .. } => vec![value],
        ExprKind::Binary { left, right, .. } => vec![left, right],
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => vec![condition, then_branch, else_branch],
        ExprKind::Call { args, .. } => args.iter().collect(),
        ExprKind::Block { tail, .. } => vec![tail],
        _ => Vec::new(),
    };
    let mut index = 0;
    while let Some(child) = expression.child(index) {
        let is_free = free_children
            .iter()
            .any(|candidate| std::ptr::eq(*candidate, child));
        ast_census(child, is_free, outside_children, census);
        index += 1;
    }
}

fn hir_kind(kind: &ResolvedExprKind) -> Option<usize> {
    match kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_) => Some(0),
        ResolvedExprKind::Unary { .. } => Some(1),
        ResolvedExprKind::Binary { .. } => Some(2),
        _ => None,
    }
}

/// Resolved-HIR oracle over the actual containers: a root is free only when
/// it is held by a `Box` of a unary/binary/`if`/block-tail node or by a call's
/// argument vector. Statement children are carried by `ResolvedStatement`.
/// Returns the number of auxiliary carriers actually constructed.
fn hir_census(
    expression: &ResolvedExpr,
    free: bool,
    outside_closure: bool,
    census: &mut Census,
) -> usize {
    if let Some(kind) = hir_kind(&expression.kind) {
        census.kinds[kind] += 1;
        census.free += usize::from(free);
    }
    if matches!(expression.kind, ResolvedExprKind::If { .. }) {
        census.ifs += usize::from(free);
    }
    if outside_closure
        && matches!(
            expression.kind,
            ResolvedExprKind::Place(_) | ResolvedExprKind::FunctionReference { .. }
        )
    {
        census.vars += usize::from(free);
    }
    if outside_closure && matches!(expression.kind, ResolvedExprKind::Block { .. }) {
        census.blocks += usize::from(free);
    }
    match &expression.kind {
        ResolvedExprKind::Unary { value, .. } => hir_census(value, true, outside_closure, census),
        ResolvedExprKind::Binary { left, right, .. } => {
            hir_census(left, true, outside_closure, census)
                + hir_census(right, true, outside_closure, census)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            hir_census(condition, true, outside_closure, census)
                + hir_census(then_branch, true, outside_closure, census)
                + hir_census(else_branch, true, outside_closure, census)
        }
        ResolvedExprKind::Call { args, .. } => args
            .iter()
            .map(|argument| hir_census(argument, true, outside_closure, census))
            .sum(),
        ResolvedExprKind::Block { statements, tail } => {
            let mut built = 0;
            for statement in statements {
                built += 1 + usize::from(matches!(
                    statement,
                    ResolvedStatement::Let { .. } | ResolvedStatement::Assign { .. }
                ));
                for index in 0..statement.child_count() {
                    built += hir_census(
                        statement.child(index).unwrap(),
                        false,
                        outside_closure,
                        census,
                    );
                }
            }
            built + hir_census(tail, true, outside_closure, census)
        }
        ResolvedExprKind::Closure {
            parameters,
            captures,
            body,
        } => {
            // Captures are synthetic place reads and bindings, never authored
            // variable roots. The private body inherits the closure exclusion.
            parameters.len() + captures.len() + hir_census(body, false, false, census)
        }
        ResolvedExprKind::Invoke { callable, args } => {
            // The callable place comes from the source call name, not a Var.
            hir_census(callable, false, outside_closure, census)
                + args
                    .iter()
                    .map(|arg| hir_census(arg, true, outside_closure, census))
                    .sum::<usize>()
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => fields
            .iter()
            .map(|field| 1 + hir_census(&field.value, false, outside_closure, census))
            .sum(),
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            hir_census(scrutinee, false, outside_closure, census)
                + arms
                    .iter()
                    .map(|arm| {
                        1 + usize::from(matches!(
                            arm.pattern,
                            crate::hir::ResolvedMatchPattern::Binding(_)
                        )) + arm
                            .guard
                            .as_ref()
                            .map_or(0, |guard| hir_census(guard, false, outside_closure, census))
                            + hir_census(&arm.value, false, outside_closure, census)
                    })
                    .sum::<usize>()
        }
        ResolvedExprKind::Place(_)
        | ResolvedExprKind::FunctionReference { .. }
        | ResolvedExprKind::String(_) => 0,
        _ if hir_kind(&expression.kind) == Some(0) => 0,
        other => panic!("fixture admits no other resolved kind: {other:?}"),
    }
}

fn cost_census(program: &Program) -> StructuralCost {
    let mut cost = StructuralCost::raw_ast(true).with_inline_values(true);
    for function in &program.functions {
        super::super::declaration_cost::ast_function_cost(function, &mut cost).unwrap();
    }
    cost
}

// Literal, checked unary/binary, lazy `&&`/`||`, comparisons and call arguments
// in carrier-free positions beside `let`/assignment values that stay delegated.
const GATE: &str = r#"module carrier.gate;
@id("carrier.gate.add") fn add(a: i64, b: i64) -> i64 { a + b }
@id("carrier.gate.main") fn main() -> i64 {
 let mut total = 1 + 2;
 total = total * 3;
 let flag = !(total < 4) && (total >= 5 || total != 6);
 if flag && add(7, 8 - 9) == 10 { -(11 / 12) } else { add(total, 13) % 14 }
}
"#;

// Every named type is authored; ownership-bearing roots retain their storage.
const VALUES: &str = r#"module carrier.values;

@id("carrier.values.packet")
record Packet {
    @id("carrier.values.packet.text")
    text: string,
    @id("carrier.values.packet.bytes")
    bytes: Bytes,
}

@id("carrier.values.add")
fn add(value: i64) -> i64
{
    value + 1
}

@id("carrier.values.text")
fn text(value: string) -> string
{
    { value }
}

@id("carrier.values.bytes")
fn bytes(value: own Bytes) -> Bytes
{
    { value }
}

@id("carrier.values.record")
fn move_packet(value: own Packet) -> Packet
{
    { value }
}

@id("carrier.values.reference")
fn reference(flag: bool) -> fn(i64) -> i64
{
    if flag { add } else { add }
}

@id("carrier.values.main")
fn main() -> i64
{
    let text_value = text("x");
    let bytes_value = bytes(bytes_zeroed(0usize));
    let copied_text = text(text_value);
    let copied_bytes = bytes(bytes_value);
    let record_value = move_packet(Packet { text: copied_text, bytes: copied_bytes });
    let moved_record = move_packet(record_value);
    let callback = reference(true);
    let direct = add;
    let mut x = 1;
    let y = x;
    x = y;
    let z = match x { n => n, };
    if true { 0 } else { z }
}
"#;

const CLOSURE: &str = r#"module carrier.closure;

@id("carrier.closure.helper")
fn helper(value: i64) -> i64
{
    value
}

@id("carrier.closure.main")
fn main() -> i64
{
    let offset = 1;
    let callback = fn(value: i64) -> i64 { let saved = value; let mut copied = saved; copied = value; if copied > offset { helper(copied) } else { offset } };
    callback(0)
}
"#;

#[test]
fn variables_keep_storage_for_owned_values_references_and_closure_captures() {
    for (source, expected_vars) in [(VALUES, 11), (CLOSURE, 1)] {
        let parsed = crate::parse(source, "values.spx").unwrap();
        let mut ast = Census::default();
        for function in &parsed.functions {
            ast_census(&function.body, false, true, &mut ast);
        }
        assert_eq!(ast.vars, expected_vars);
        let cost = cost_census(&parsed);
        assert_eq!(cost.exact_absent_roots, ast.kinds);
        assert_eq!(cost.exact_carrier_free_roots, ast.free);
        assert_eq!(cost.exact_carrier_free_ifs, ast.ifs);
        assert_eq!(cost.exact_carrier_free_vars, ast.vars);
        assert_eq!(cost.exact_carrier_free_blocks, ast.blocks);
        let checked = crate::check(source, "values.spx").unwrap();
        let resolved = crate::hir::resolve(&checked).unwrap();
        crate::hir::validate(&resolved).unwrap();
        let mut hir = Census::default();
        for function in &resolved.functions {
            hir_census(&function.body, false, true, &mut hir);
        }
        assert_eq!(hir, ast, "authored roots and actual resolved containers");
        let storage = std::mem::size_of::<crate::cleanup::CleanupStorageSlot>()
            + std::mem::size_of::<crate::cleanup::CleanupFlag>()
            + std::mem::size_of::<crate::cleanup::CleanupPlace>()
            + std::mem::size_of::<crate::cleanup_plan::CleanupSlot>();
        assert_eq!(
            super::exact_absent_storage(&cost).unwrap(),
            ast.kinds.iter().sum::<usize>() * storage
                + (ast.free + ast.ifs + ast.vars + ast.blocks) * carriers(),
            "variables omit only the six auxiliary carriers"
        );
    }
}

#[test]
fn delegated_children_and_nested_closures_cannot_reset_variable_exclusion() {
    // Parse-only adversarial cost input: includes closed runtime shapes so the
    // forecast must be conservative before later admission refuses them.
    for body in [
        "let x = value; x = value; value",
        "let x = { value }; { value }",
        "Packet { text: { value }, bytes: { value } }",
        "match value { n if { value } => { value }, _ => { value }, }",
        "let nested = fn(inner: i64) -> i64 { inner + value }; nested(value)",
        "let x = make(value); value.method(value)",
        "let x = value with { field: { value } }; value.field",
        "let x = vec_of<i64>({ value }); value",
        "while value > 0 { let x = { value }; value } value",
    ] {
        let source = format!(
            "module carrier.exclusion; @id(\"carrier.exclusion.main\") fn main() -> i64 {{ let callback = fn(value: i64) -> i64 {{ {body} }}; 0 }}"
        );
        let parsed = crate::parse(&source, "exclusion.spx").unwrap();
        let mut ast = Census::default();
        ast_census(&parsed.functions[0].body, false, true, &mut ast);
        assert_eq!(ast.vars, 0, "{body}");
        assert_eq!(cost_census(&parsed).exact_carrier_free_vars, 0, "{body}");
        assert_eq!(cost_census(&parsed).exact_carrier_free_blocks, 0, "{body}");
    }
}

#[test]
fn malformed_variable_censuses_fail_closed_and_runtime_censuses_are_ignored() {
    let parsed = crate::parse(VALUES, "values.spx").unwrap();
    let raw = cost_census(&parsed);
    let mut runtime = StructuralCost::new();
    runtime.exact_carrier_free_vars = usize::MAX;
    runtime.exact_carrier_free_blocks = usize::MAX;
    let ordinary = StructuralCost::new();
    for mode in [5, 6] {
        assert_eq!(
            super::fixed_hir_discount(mode, &raw, &runtime).unwrap(),
            super::fixed_hir_discount(mode, &raw, &ordinary).unwrap()
        );
    }
    for vars in [raw.exact_expressions, usize::MAX] {
        let mut malformed = cost_census(&parsed);
        malformed.exact_carrier_free_vars = vars;
        for mode in [5, 6] {
            assert!(super::fixed_hir_discount(mode, &malformed, &ordinary)
                .unwrap_err()
                .iter()
                .all(|error| error.code == "SPX-G171"));
        }
        for retention in [
            cached_exact::Retention::Parsed,
            cached_exact::Retention::Semantic,
        ] {
            assert!(
                cached_exact::fixed_upper(1 << 24, &malformed, &ordinary, retention)
                    .unwrap_err()
                    .iter()
                    .all(|error| error.code == "SPX-G171")
            );
        }
    }
    for blocks in [raw.exact_expressions, usize::MAX] {
        let mut malformed = cost_census(&parsed);
        malformed.exact_carrier_free_blocks = blocks;
        for mode in [5, 6] {
            assert!(super::fixed_hir_discount(mode, &malformed, &ordinary)
                .unwrap_err()
                .iter()
                .all(|error| error.code == "SPX-G171"));
        }
    }
    // Even a valid disjoint count refuses if conversion to carrier bytes overflows.
    let mut huge = StructuralCost::new();
    huge.exact_expressions = usize::MAX;
    huge.exact_carrier_free_vars = usize::MAX / carriers() + 1;
    let mut block_overflow = StructuralCost::new();
    block_overflow.exact_expressions = usize::MAX;
    block_overflow.exact_carrier_free_blocks = usize::MAX / carriers() + 1;
    assert!(super::exact_absent_storage(&block_overflow)
        .unwrap_err()
        .iter()
        .all(|error| error.code == "SPX-G171"));
    assert!(super::exact_absent_storage(&huge)
        .unwrap_err()
        .iter()
        .all(|error| error.code == "SPX-G171"));
    for mode in [0, 4, 7, 8] {
        assert!(super::fixed_hir_discount(mode, &huge, &ordinary).is_ok());
    }
    for retention in [
        cached_exact::Retention::Parsed,
        cached_exact::Retention::Semantic,
    ] {
        assert_eq!(
            cached_exact::fixed_upper(1 << 24, &raw, &runtime, retention).unwrap(),
            cached_exact::fixed_upper(1 << 24, &raw, &ordinary, retention).unwrap()
        );
    }
}

#[test]
fn raw_ast_resolved_and_cost_censuses_agree_on_every_carrier_free_root() {
    let parsed = crate::parse(GATE, std::path::Path::new("gate.spx")).unwrap();
    let mut ast = Census::default();
    for function in &parsed.functions {
        ast_census(&function.body, false, true, &mut ast);
    }
    // Hand count: 14 literals, 2 unary, 13 binary; `1 + 2`, `total * 3` and
    // the `let flag` conjunction are the only delegated censused roots. The
    // tail `if` is the one free `if` root. Its scalar operands and those of
    // `add` contribute eight free variables, each retaining all storage.
    assert_eq!(
        ast,
        Census {
            kinds: [14, 2, 13],
            free: 26,
            ifs: 1,
            vars: 8,
            blocks: 2,
        }
    );
    // Both admitted braced if arms are real free Block nodes, even though
    // neither has statements. Keep their full AST/HIR/cost census below.
    // The separate add body has no free block: retain the zero boundary too.
    let mut helper = Census::default();
    ast_census(&parsed.functions[0].body, false, true, &mut helper);
    assert_eq!(helper.blocks, 0);
    assert_eq!(ast.blocks, 2);
    let cost = cost_census(&parsed);
    assert_eq!(cost.exact_absent_roots, ast.kinds);
    assert_eq!(cost.exact_carrier_free_roots, ast.free);
    assert_eq!(cost.exact_carrier_free_ifs, ast.ifs);
    assert_eq!(cost.exact_carrier_free_vars, ast.vars);
    assert_eq!(cost.exact_carrier_free_blocks, ast.blocks);
    // Legacy and runtime accumulators never count.
    let mut legacy = StructuralCost::new();
    for function in &parsed.functions {
        super::super::declaration_cost::ast_function_cost(function, &mut legacy).unwrap();
    }
    assert_eq!(legacy.exact_carrier_free_roots, 0);
    assert_eq!(legacy.exact_carrier_free_ifs, 0);
    assert_eq!(legacy.exact_carrier_free_vars, 0);
    assert_eq!(legacy.exact_carrier_free_blocks, 0);

    let checked = crate::check(GATE, "gate.spx").unwrap();
    let resolved = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let mut hir = Census::default();
    let mut built = 0;
    for name in ["add", "main"] {
        let function = resolved
            .functions
            .iter()
            .find(|function| function.name == name)
            .unwrap();
        assert!(function.requires.is_empty() && function.ensures.is_empty());
        built += hir_census(&function.body, false, true, &mut hir);
    }
    let mut helper_hir = Census::default();
    let add = resolved.functions.iter().find(|f| f.name == "add").unwrap();
    hir_census(&add.body, false, true, &mut helper_hir);
    assert_eq!(helper_hir.blocks, 0);
    assert_eq!(hir.blocks, 2);
    // No literal/unary/binary root is folded, elided, or moved into a carrier.
    assert_eq!(hir, ast);
    // Exactly the three authored statements (two with bindings) exist, all
    // paid by delegated bundles that this change leaves complete.
    assert_eq!(built, 3 + 3);
    assert!(
        built
            <= cost.exact_expressions
                - cost.exact_carrier_free_roots
                - cost.exact_carrier_free_ifs
                - cost.exact_carrier_free_vars
                - cost.exact_carrier_free_blocks
    );
}

#[test]
fn delegated_parents_keep_complete_bundles_and_guards_are_charged() {
    let census = |body: &str| {
        let source = format!("module m; @id(\"m.main\") fn main(left: i64) -> i64 {{ {body} }}");
        let program = crate::parse(&source, std::path::Path::new("m.spx")).unwrap();
        let mut oracle = Census::default();
        ast_census(&program.functions[0].body, false, true, &mut oracle);
        let cost = cost_census(&program);
        assert_eq!(cost.exact_absent_roots, oracle.kinds, "{body}");
        assert_eq!(cost.exact_carrier_free_roots, oracle.free, "{body}");
        assert_eq!(cost.exact_carrier_free_ifs, oracle.ifs, "{body}");
        assert_eq!(cost.exact_carrier_free_vars, oracle.vars, "{body}");
        assert_eq!(cost.exact_carrier_free_blocks, oracle.blocks, "{body}");
        (oracle, cost)
    };
    // Arm values and guards embed or box under a `ResolvedMatchArm`; the
    // `let` value is embedded in its statement. Only the tail-free match
    // root's literal operands inside are free (here: `left + 1` has `1`).
    let (guarded, with_guard) = census("let x = 3; match left { n if n >= 0 => n + 1, _ => 2, }");
    assert_eq!(guarded.kinds, [4, 0, 2]);
    assert_eq!(
        guarded.free, 2,
        "`0` and `1` are operands; `3`, `2`, arms are not"
    );
    let (_, without_guard) = census("let x = 3; match left { n => n + 1, _ => 2, }");
    // The guard subtree (`n >= 0`, `n`, `0`) is now a complete charged tree.
    assert!(
        with_guard.total - without_guard.total >= 3 * std::mem::size_of::<Expr>(),
        "guard bytes must be charged"
    );
    assert_eq!(
        with_guard.exact_expressions - without_guard.exact_expressions,
        3
    );
    let slots = |body: &str| {
        let source = format!("module m; @id(\"m.main\") fn main(left: i64) -> i64 {{ {body} }}");
        let program = crate::parse(&source, std::path::Path::new("m.spx")).unwrap();
        super::super::identity_slots::ast_function_identity_slots(&program.functions[0]).unwrap()
    };
    assert!(
        slots("match left { n if n >= 0 => n + 1, _ => 2, }")
            > slots("match left { n => n + 1, _ => 2, }")
    );
}

/// Same node multiset, strings, identities and calls; only the position of
/// the `n` moved roots differs (let values versus call arguments).
fn pair(n: usize, free: bool, moved: fn(usize) -> String) -> Vec<Program> {
    vec![crate::parse(
        &pair_source(n, free, moved),
        std::path::Path::new("pair.spx"),
    )
    .unwrap()]
}

/// A moved literal root: carrier- and storage-free as an argument.
fn literal(index: usize) -> String {
    index.to_string()
}

/// A moved variable root: carrier-free as an argument, never storage-free.
fn variable(_: usize) -> String {
    "seed".to_owned()
}

/// A moved `if` root: carrier-free as an argument, never storage-free. Its
/// condition and branch-tail literals are free in both positions.
fn branch(index: usize) -> String {
    format!("if true {{ {index} }} else {{ 0 }}")
}
/// The root block changes position; its binding and nested tail retain their
/// own charges in both twins.
fn block(index: usize) -> String {
    format!("{{ let saved = {index}; {{ saved }} }}")
}

fn pair_source(n: usize, free: bool, moved: fn(usize) -> String) -> String {
    let params = (0..n)
        .map(|index| format!("a{index}: i64"))
        .collect::<Vec<_>>()
        .join(", ");
    let mut body = String::from("let seed = 1; ");
    for index in 0..n {
        let value = if free { "f()".to_owned() } else { moved(index) };
        body.push_str(&format!("let v{index} = {value}; "));
    }
    let args = (0..n)
        .map(|index| if free { moved(index) } else { "f()".to_owned() })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "module pair; @id(\"pair.f\") fn f() -> i64 {{ 0 }} @id(\"pair.g\") fn g({params}) -> i64 {{ 0 }} @id(\"pair.main\") fn main() -> i64 {{ {body}g({args}) }}"
    )
}

fn retained(programs: &[Program], mode: u8) -> (usize, usize) {
    let authored = crate::workspace_graph::index_authored(programs).unwrap();
    let maximum = Some(dependency_identity_max(&programs[0], &authored, programs).unwrap());
    let costs =
        synthetic_builder_bytes_scoped(&programs[0], &authored, programs, maximum, mode, true)
            .unwrap();
    (costs.retained_hir, costs.retained_clone_and_hir)
}

struct LimitRestore(usize);
impl Drop for LimitRestore {
    fn drop(&mut self) {
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(self.0));
    }
}

#[test]
fn complete_forecast_differs_from_position_blind_formula_by_exact_carriers() {
    for moved in [literal as fn(usize) -> String, branch, variable, block] {
        moved_roots_differ_by_exact_carriers(moved);
    }
}

fn moved_roots_differ_by_exact_carriers(moved: fn(usize) -> String) {
    const N: usize = 12;
    let (free, delegated) = (pair(N, true, moved), pair(N, false, moved));
    let (mut census, mut twin) = (Census::default(), Census::default());
    ast_census(&free[0].functions[2].body, false, true, &mut census);
    ast_census(&delegated[0].functions[2].body, false, true, &mut twin);
    // Exactly N roots change position. Branch/block children retain their
    // independent identical positions and carriers in both twins.
    assert_eq!(census.kinds, twin.kinds);
    assert_eq!(
        (census.free + census.ifs + census.vars + census.blocks)
            - (twin.free + twin.ifs + twin.vars + twin.blocks),
        N,
        "{census:?} {twin:?}"
    );
    // The old formula is position-blind: every earlier mode is identical.
    for mode in 0..5 {
        assert_eq!(
            retained(&free, mode),
            retained(&delegated, mode),
            "mode {mode}"
        );
    }
    for mode in [5, 6] {
        let (free_hir, free_total) = retained(&free, mode);
        let (delegated_hir, delegated_total) = retained(&delegated, mode);
        assert_eq!(delegated_hir - free_hir, N * carriers());
        assert_eq!(delegated_total - free_total, N * carriers());
    }
    for (retention, copies) in [
        (cached_exact::Retention::Parsed, 1),
        (cached_exact::Retention::Semantic, 2),
    ] {
        let sum = |programs: &[Program]| {
            let authored = crate::workspace_graph::index_authored(programs).unwrap();
            cached_exact::summed(programs, &authored, retention).unwrap()
        };
        let (free_sum, delegated_sum) = (sum(&free), sum(&delegated));
        assert_eq!(delegated_sum.0 - free_sum.0, copies * N * carriers());
        assert_eq!(delegated_sum.1 - free_sum.1, copies * N * carriers());
        // Exact limit admits, one short refuses, and the delegated twin
        // (same bytes under the old formula) refuses at the free bound.
        let previous =
            crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.replace(free_sum.1));
        let _restore = LimitRestore(previous);
        let authored = crate::workspace_graph::index_authored(&free).unwrap();
        assert_eq!(
            cached_exact::summed(&free, &authored, retention).unwrap(),
            free_sum
        );
        let twin = crate::workspace_graph::index_authored(&delegated).unwrap();
        assert!(cached_exact::summed(&delegated, &twin, retention)
            .unwrap_err()
            .iter()
            .any(|error| error.code == "SPX-G171"));
        crate::workspace_graph::ACTIVE_BUILDER_LIMIT.with(|limit| limit.set(free_sum.1 - 1));
        assert!(cached_exact::summed(&free, &authored, retention)
            .unwrap_err()
            .iter()
            .any(|error| error.code == "SPX-G171"));
    }
}

/// Final gate (actual allocation, not a mirror): resolving and replaying each
/// fixture through the real resolver and HIR validator, with the resolved
/// program live, never requests more backing than its reduced mode-5 forecast.
#[test]
fn resolver_and_replay_highwater_stays_within_the_reduced_forecast() {
    for (source, path) in [
        (GATE.to_owned(), "gate.spx"),
        (pair_source(12, true, literal), "pair.spx"),
        (pair_source(12, true, branch), "branch.spx"),
        (pair_source(12, true, variable), "variable.spx"),
        (pair_source(12, true, block), "block.spx"),
        (VALUES.to_owned(), "values.spx"),
        (CLOSURE.to_owned(), "closure.spx"),
    ] {
        let parsed = vec![crate::parse(&source, std::path::Path::new(path)).unwrap()];
        let census = cost_census(&parsed[0]);
        if matches!(path, "values.spx" | "closure.spx") {
            assert!(
                census.exact_carrier_free_vars > 0,
                "gate exercises variables"
            );
        } else if path == "variable.spx" {
            assert!(
                census.exact_carrier_free_vars >= 12,
                "twelve named argument roots exercise the variable-carrier discount"
            );
        } else if path == "block.spx" {
            assert_eq!(
                census.exact_carrier_free_blocks, 24,
                "twelve free outer and independently free inner blocks"
            );
        } else {
            assert!(
                census.exact_carrier_free_roots >= 12,
                "gate exercises the discount"
            );
        }
        let (_, forecast) = retained(&parsed, 5);
        let checked = crate::check(&source, path).unwrap();
        let (resolved, allocation) = crate::test_allocation_observer::measure(|| {
            let resolved = crate::hir::resolve(&checked).unwrap();
            crate::hir::validate(&resolved).unwrap();
            resolved
        });
        assert!(!allocation.invalid, "{path}: observer capacity");
        assert!(allocation.live > 0, "{path}: resolved program stays live");
        assert!(
            allocation.highwater <= forecast,
            "{path}: highwater {} exceeds reduced forecast {forecast}",
            allocation.highwater
        );
        drop(resolved);
    }
}
