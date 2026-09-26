//! Resumable Effects v1 (issue #204): structural admission for `yield`.
//!
//! Checked once a function's AST is fully parsed, before any semantic
//! resolution runs. Everything here is purely syntactic -- whether a
//! `yield` expression sits at the direct top level of its *own* enclosing
//! function's body block (a `let`/assignment value, or the block's tail),
//! and whether that function declared a `yields` clause at all. Type-level
//! checks (the operand's type against the declared request type, and the
//! whole `yield` expression's type against the declared response type) are
//! `hir::resolve_yield`'s job once types exist.
//!
//! Issue #296 widens this to structured control: a `yield` may also be the
//! direct `let`/assignment value of a block reached from the body only through
//! `if`/`else` branches, `while` bodies, or block-valued slots. Conditions,
//! operands, nested block tails, `match` arms, `for` and `unsafe` bodies, and
//! closures stay closed. The paragraph below records the original slice.
//!
//! Restricting `yield` to a function's own top-level statement/tail
//! positions -- never nested inside a call's arguments, a binary operator,
//! a record/variant construction, an `if`/`match` arm, or a nested block --
//! is a deliberate first-slice scope decision. It statically guarantees, by
//! construction rather than by a separate check, every deferred case
//! `docs/RESUMABLE-EFFECTS-V1.md` names: no loop can contain a `yield`
//! (`while` bodies are always nested blocks), no owned call's argument
//! staging can observe a suspension (call arguments are always nested
//! sub-expressions), and yield sites execute only in authored top-level
//! sequence.
//! See `docs/RESUMABLE-EFFECTS-V1.md`.

use crate::ast::{Expr, ExprKind, FieldInitializer, Function, MatchArm, Statement};
use crate::diagnostic::Diagnostic;

/// `yield` used somewhere a `yields`-declaring function's own top-level
/// body block does not admit it: nested inside another expression, a
/// loop, a conditional branch, or a function with no `yields` clause.
const MISPLACED_YIELD: &str = "SPX-T297";
/// A `yields`-declaring function's body contains no top-level `yield`
/// expression.
const YIELD_ARITY: &str = "SPX-T298";

pub(super) fn check_function_yield_placement(
    function: &Function,
    path: &str,
) -> Result<(), Diagnostic> {
    let mut top_level_yields: Vec<Expr> = Vec::new();
    scan_top_level(&function.body, &mut top_level_yields, path)?;

    match (&function.yields, top_level_yields.len()) {
        (Some(_), 1..) => Ok(()),
        (Some(clause), 0) => Err(Diagnostic::error(
            YIELD_ARITY,
            format!(
                "function `{}` declares `yields` but its body never yields",
                function.name
            ),
            clause.span,
        )
        .at_path(path)),
        (None, 0) => Ok(()),
        (None, _) => Err(Diagnostic::error(
            MISPLACED_YIELD,
            format!(
                "function `{}` uses `yield` but does not declare a `yields` clause",
                function.name
            ),
            top_level_yields[0].span,
        )
        .at_path(path)),
    }
}

/// Where a scanned slot sits. `Top` is the function body's own block;
/// `Nested` is any block reached from it only through `if`/`else` branches,
/// `while` bodies, or a block-valued slot (Resumable Effects control profile,
/// issue #296).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Level {
    Top,
    Nested,
}

/// Scans the function body's own block. A directly-`Yield` statement value or
/// top-level tail is admitted and recorded, and so is a direct `let`/assign
/// value of any block reached through structured control; everything else
/// -- including every admitted yield's own operand, every condition, and every
/// nested block tail -- is handed to [`forbid_nested_yield`].
fn scan_top_level(body: &Expr, found: &mut Vec<Expr>, path: &str) -> Result<(), Diagnostic> {
    if !matches!(body.kind, ExprKind::Block { .. }) {
        // Every admitted function body is a brace block; a differently
        // shaped body has no top-level position to admit at all.
        return forbid_nested_yield(body, path);
    }
    scan_block(body, Level::Top, found, path)
}

fn scan_block(
    block: &Expr,
    level: Level,
    found: &mut Vec<Expr>,
    path: &str,
) -> Result<(), Diagnostic> {
    let ExprKind::Block { statements, tail } = &block.kind else {
        return forbid_nested_yield(block, path);
    };
    for statement in statements {
        scan_top_level_statement(statement, found, path)?;
    }
    scan_slot(tail, level == Level::Top, found, path)
}

fn scan_top_level_statement(
    statement: &Statement,
    found: &mut Vec<Expr>,
    path: &str,
) -> Result<(), Diagnostic> {
    match statement {
        Statement::Let { value, .. } | Statement::Assign { value, .. } => {
            scan_slot(value, true, found, path)
        }
        // An unsafe body stays closed to `yield`.
        Statement::Unsafe { body, .. } => forbid_nested_yield(body, path),
        // A `while` body is a nested block; its condition never suspends.
        Statement::While {
            condition, body, ..
        } => {
            forbid_nested_yield(condition, path)?;
            scan_block(body, Level::Nested, found, path)
        }
        Statement::For { values, body, .. } | Statement::ForOwn { values, body, .. } => {
            forbid_nested_yield(values, path)?;
            forbid_nested_yield(body, path)
        }
    }
}

/// One slot. `admits_yield` is true for a `let`/assignment value and for the
/// function's own tail. An `if` or block in any slot opens nested blocks.
fn scan_slot(
    slot: &Expr,
    admits_yield: bool,
    found: &mut Vec<Expr>,
    path: &str,
) -> Result<(), Diagnostic> {
    match &slot.kind {
        ExprKind::Yield { request } if admits_yield => {
            forbid_nested_yield(request, path)?;
            found.push(slot.clone());
            Ok(())
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            forbid_nested_yield(condition, path)?;
            scan_slot(then_branch, false, found, path)?;
            scan_slot(else_branch, false, found, path)
        }
        ExprKind::Block { .. } => scan_block(slot, Level::Nested, found, path),
        _ => forbid_nested_yield(slot, path),
    }
}

fn misplaced(span: crate::ast::Span, path: &str) -> Diagnostic {
    Diagnostic::error(
        MISPLACED_YIELD,
        "`yield` is only admitted as a `let`/assignment value of the function's own block or of \
         a block nested in it through `if`/`else` branches or `while` bodies, or as the \
         function's own tail expression; never inside another expression, a condition, a \
         nested block's tail, a `match`, `for` or `unsafe` body",
        span,
    )
    .at_path(path)
}

/// Exhaustive descent over every expression shape that never admits a
/// nested `Yield`. Every child of every variant is visited so a `yield`
/// buried at any depth -- inside a call argument, a binary operand, a
/// match arm, an `if` branch, a nested block, a closure body, and so on --
/// is reported rather than silently accepted.
fn forbid_nested_yield(expr: &Expr, path: &str) -> Result<(), Diagnostic> {
    match &expr.kind {
        ExprKind::Yield { .. } => Err(misplaced(expr.span, path)),
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::Var(_) => Ok(()),
        ExprKind::Closure { body, .. } => forbid_nested_yield(body, path),
        ExprKind::Call { args, .. } => args
            .iter()
            .try_for_each(|arg| forbid_nested_yield(arg, path)),
        ExprKind::MethodCall { receiver, args, .. } => {
            forbid_nested_yield(receiver, path)?;
            args.iter()
                .try_for_each(|arg| forbid_nested_yield(arg, path))
        }
        ExprKind::SuperMethod { args, .. } => args
            .iter()
            .try_for_each(|arg| forbid_nested_yield(arg, path)),
        ExprKind::Unary { value, .. } => forbid_nested_yield(value, path),
        ExprKind::Binary { left, right, .. } => {
            forbid_nested_yield(left, path)?;
            forbid_nested_yield(right, path)
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                forbid_nested_yield_statement(statement, path)?;
            }
            forbid_nested_yield(tail, path)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            forbid_nested_yield(condition, path)?;
            forbid_nested_yield(then_branch, path)?;
            forbid_nested_yield(else_branch, path)
        }
        ExprKind::ConstructRecord { fields, .. } | ExprKind::ConstructVariant { fields, .. } => {
            forbid_nested_yield_fields(fields, path)
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            forbid_nested_yield(scrutinee, path)?;
            arms.iter()
                .try_for_each(|arm| forbid_nested_yield_arm(arm, path))
        }
        ExprKind::Try { operand } => forbid_nested_yield(operand, path),
        ExprKind::UpdateRecord { base, fields } => {
            forbid_nested_yield(base, path)?;
            forbid_nested_yield_fields(fields, path)
        }
        ExprKind::Project { base, .. } => forbid_nested_yield(base, path),
    }
}

fn forbid_nested_yield_fields(fields: &[FieldInitializer], path: &str) -> Result<(), Diagnostic> {
    fields
        .iter()
        .try_for_each(|field| forbid_nested_yield(&field.value, path))
}

fn forbid_nested_yield_arm(arm: &MatchArm, path: &str) -> Result<(), Diagnostic> {
    if let Some(guard) = &arm.guard {
        forbid_nested_yield(guard, path)?;
    }
    forbid_nested_yield(&arm.value, path)
}

fn forbid_nested_yield_statement(statement: &Statement, path: &str) -> Result<(), Diagnostic> {
    match statement {
        Statement::Let { value, .. } | Statement::Assign { value, .. } => {
            forbid_nested_yield(value, path)
        }
        Statement::Unsafe { body, .. } => forbid_nested_yield(body, path),
        Statement::While {
            condition, body, ..
        } => {
            forbid_nested_yield(condition, path)?;
            forbid_nested_yield(body, path)
        }
        Statement::For { values, body, .. } | Statement::ForOwn { values, body, .. } => {
            forbid_nested_yield(values, path)?;
            forbid_nested_yield(body, path)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Diagnostic, MISPLACED_YIELD, YIELD_ARITY};

    fn parse(source: &str) -> Result<crate::ast::Program, Diagnostic> {
        crate::parse(source, Path::new("yields-fixture.spx"))
    }

    #[test]
    fn a_single_top_level_yield_in_a_declaring_function_parses() {
        let source = r#"
module test.yields_ok;
@id("app.ask")
fn ask(seed: i64) -> i64
    yields i64 -> i64
{
    let answer = yield seed + 1;
    answer * 2
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        parse(source).expect("a direct top-level yield is admitted");
    }

    #[test]
    fn yield_as_the_tail_expression_parses() {
        let source = r#"
module test.yields_tail;
@id("app.ask")
fn ask() -> i64
    yields i64 -> i64
{
    yield 1
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        parse(source).expect("yield as the function's own tail is admitted");
    }

    #[test]
    fn yield_without_a_yields_clause_is_refused() {
        let source = r#"
module test.yields_missing_clause;
@id("app.ask")
fn ask() -> i64 {
    yield 1
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = parse(source).unwrap_err();
        assert_eq!(error.code, MISPLACED_YIELD);
    }

    #[test]
    fn yield_nested_inside_a_binary_operand_is_refused() {
        let source = r#"
module test.yields_nested;
@id("app.ask")
fn ask() -> i64
    yields i64 -> i64
{
    let answer = 1 + (yield 1);
    answer
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = parse(source).unwrap_err();
        assert_eq!(error.code, MISPLACED_YIELD);
    }

    #[test]
    fn yields_in_while_bodies_and_if_branches_parse() {
        let source = r#"
module test.yields_in_control;
@id("app.ask")
fn ask(limit: i64) -> i64
    yields i64 -> i64
{
    let mut round = 0;
    while round < limit {
        let _consumed = yield round;
        round = round + 1;
        round > 0
    }
    let bonus = if round > 1 {
        let extra = yield round;
        extra
    } else {
        0
    };
    bonus
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        parse(source).expect("structured-control yields are admitted");
    }

    #[test]
    fn yield_in_a_condition_nested_tail_match_or_for_is_refused() {
        for body in [
            "while (yield 1) > 0 { 0 } 0",
            "let x = if true { yield 1 } else { 0 }; x",
            "let x = if (yield 1) > 0 { 1 } else { 0 }; x",
            "let x = match 1 { _ => yield 1, }; x",
            "let x = { let y = 1; yield y }; x",
        ] {
            let source = format!(
                "module test.yields_misplaced;\n@id(\"app.ask\")\nfn ask() -> i64\n    yields i64 -> i64\n{{\n    {body}\n}}\n@id(\"app.main\")\nfn main() -> i64 {{ 0 }}\n"
            );
            let error = parse(&source).unwrap_err();
            assert_eq!(error.code, MISPLACED_YIELD, "{body}");
        }
    }

    #[test]
    fn a_yields_declaring_function_that_never_yields_is_refused() {
        let source = r#"
module test.yields_zero;
@id("app.ask")
fn ask() -> i64
    yields i64 -> i64
{
    0
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = parse(source).unwrap_err();
        assert_eq!(error.code, YIELD_ARITY);
    }

    #[test]
    fn sequential_top_level_yields_in_a_declaring_function_parse() {
        let source = r#"
module test.yields_twice;
@id("app.ask")
fn ask() -> i64
    yields i64 -> i64
{
    let a = yield 1;
    let b = yield 2;
    a + b
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        parse(source).expect("sequential top-level yields are admitted");
    }

    #[test]
    fn a_class_method_cannot_declare_yields() {
        let source = r#"
module test.yields_method;
@id("app.box")
class Box {
    @id("app.box.ask")
    fn ask(self: Box) -> i64
        yields i64 -> i64
    {
        yield 1
    }
}
@id("app.main")
fn main() -> i64 { 0 }
"#;
        let error = parse(source).unwrap_err();
        assert_eq!(error.code, "SPX-T304");
    }
}
