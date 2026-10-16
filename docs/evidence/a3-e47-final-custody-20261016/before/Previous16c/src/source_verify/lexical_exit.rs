//! Independent authored contour validation before type and ownership inference.
use super::diagnostics::error;
use crate::ast::program_source::ProgramSource;
use crate::ast::{
    Expr, ExprKind, Function, LetSyntax, LexicalExitKind, Statement, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;

pub(super) fn validate(program: &dyn ProgramSource, diagnostics: &mut Vec<Diagnostic>) {
    for function in program
        .functions()
        .iter()
        .chain(program.types().iter().flat_map(|ty| match &ty.kind {
            TypeDeclarationKind::Class { methods, .. } => methods.as_slice(),
            _ => &[],
        }))
    {
        check_function(program, function, diagnostics);
    }
}
fn check_function(
    program: &dyn ProgramSource,
    function: &Function,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // Legacy ASTs keep their owning expression-depth admission. Only an actual
    // exit selects this additional contour contract; presence is independently
    // scanned through every structural child, including contracts and closures.
    if !function.body.contains_lexical_exit()
        && !function
            .requires
            .iter()
            .chain(&function.ensures)
            .any(Expr::contains_lexical_exit)
    {
        return;
    }
    if !crate::ast::expression_statement::projection_roots(
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures),
        function
            .params
            .iter()
            .map(|p| p.name.as_str())
            .chain(function.type_parameters.iter().map(|p| p.name.as_str())),
        crate::bounded_output::reserve_active_required,
    ) {
        diagnostics.push(error(
            program,
            "SPX-T315",
            "expression-statement projection is not hygienic",
            function.span,
        ));
        return;
    }
    for contract in function.requires.iter().chain(&function.ensures) {
        walk(program, contract, false, 0, 0, diagnostics);
    }
    walk(
        program,
        &function.body,
        function.yields.is_none(),
        0,
        0,
        diagnostics,
    );
}
/// Independently of HIR replay, a statement guard completes normally only with
/// the scalar discard zero. Any other normal value (owned, aggregate, borrowed,
/// non-`i64` or nonzero) would be stored, so its exits cross a value boundary.
fn guard_completion(expr: &Expr, depth: usize) -> bool {
    if depth > crate::cache_codec::MAX_DEPTH {
        return false;
    }
    match &expr.kind {
        ExprKind::LexicalExit { .. } | ExprKind::Int(0) => true,
        ExprKind::Block { tail, .. } => guard_completion(tail, depth + 1),
        ExprKind::If {
            then_branch,
            else_branch,
            ..
        } => guard_completion(then_branch, depth + 1) && guard_completion(else_branch, depth + 1),
        ExprKind::Match { arms, .. } => {
            !arms.is_empty()
                && arms
                    .iter()
                    .all(|arm| guard_completion(&arm.value, depth + 1))
        }
        _ => false,
    }
}

fn walk(
    program: &dyn ProgramSource,
    expr: &Expr,
    statement_route: bool,
    loops: usize,
    depth: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if depth > crate::cache_codec::MAX_DEPTH {
        diagnostics.push(error(
            program,
            "SPX-P207",
            "lexical exit contour exceeds source nesting bound",
            expr.span,
        ));
        return;
    }
    match &expr.kind {
        ExprKind::LexicalExit { kind, value } => {
            if !statement_route {
                diagnostics.push(error(program, "SPX-T312", "lexical exit crosses a value-evaluation or callable boundary", expr.span)
                    .with_help("place the exit on a direct function, loop or conditional statement-tail route; initializer, argument, aggregate, return-operand, yield and closure crossings are not admitted"));
            } else if matches!(kind, LexicalExitKind::Break | LexicalExitKind::Continue)
                && loops == 0
            {
                diagnostics.push(
                    error(
                        program,
                        "SPX-T313",
                        "lexical loop exit has no enclosing loop target",
                        expr.span,
                    )
                    .with_help("use break or continue only inside the nearest source loop"),
                );
            }
            if (kind == &LexicalExitKind::Return) != value.is_some() {
                diagnostics.push(error(
                    program,
                    "SPX-T312",
                    "lexical exit value does not match its control kind",
                    expr.span,
                ));
            }
            if let Some(value) = value {
                walk(program, value, false, loops, depth + 1, diagnostics);
            }
        }
        ExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    // The parser marker selects the route only on its own
                    // normalized shape; names and `;` spelling never do.
                    Statement::Let {
                        syntax: LetSyntax::StatementIf(marker),
                        mutable: false,
                        declared: None,
                        value,
                        ..
                    } if !marker.branches().is_empty()
                        && matches!(value.kind, ExprKind::If { .. }) =>
                    {
                        walk(
                            program,
                            value,
                            statement_route && guard_completion(value, depth + 1),
                            loops,
                            depth + 1,
                            diagnostics,
                        )
                    }
                    statement @ Statement::Let { value, .. } => {
                        let discard = crate::ast::expression_statement::value(statement)
                            .is_some_and(|value| {
                                matches!(value.kind, ExprKind::Match { .. })
                                    && value.contains_lexical_exit()
                            });
                        walk(
                            program,
                            value,
                            statement_route && discard,
                            loops,
                            depth + 1,
                            diagnostics,
                        )
                    }
                    Statement::Assign { value, .. } => {
                        walk(program, value, false, loops, depth + 1, diagnostics)
                    }
                    Statement::Unsafe { body, .. } => {
                        walk(program, body, false, loops, depth + 1, diagnostics)
                    }
                    Statement::While {
                        condition, body, ..
                    } => {
                        walk(program, condition, false, loops, depth + 1, diagnostics);
                        walk(
                            program,
                            body,
                            statement_route,
                            loops + 1,
                            depth + 1,
                            diagnostics,
                        );
                    }
                    Statement::For { values, body, .. }
                    | Statement::ForBorrow { values, body, .. } => {
                        walk(program, values, false, loops, depth + 1, diagnostics);
                        walk(
                            program,
                            body,
                            statement_route,
                            loops + 1,
                            depth + 1,
                            diagnostics,
                        );
                    }
                    Statement::ForOwn { values, body, .. } => {
                        walk(program, values, false, loops, depth + 1, diagnostics);
                        walk(
                            program,
                            body,
                            statement_route,
                            loops + 1,
                            depth + 1,
                            diagnostics,
                        );
                    }
                }
            }
            walk(
                program,
                tail,
                statement_route,
                loops,
                depth + 1,
                diagnostics,
            );
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            walk(program, condition, false, loops, depth + 1, diagnostics);
            walk(
                program,
                then_branch,
                statement_route,
                loops,
                depth + 1,
                diagnostics,
            );
            walk(
                program,
                else_branch,
                statement_route,
                loops,
                depth + 1,
                diagnostics,
            );
        }
        ExprKind::Match {
            scrutinee, arms, ..
        } => {
            walk(program, scrutinee, false, loops, depth + 1, diagnostics);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    walk(program, guard, false, loops, depth + 1, diagnostics);
                }
                walk(
                    program,
                    &arm.value,
                    statement_route,
                    loops,
                    depth + 1,
                    diagnostics,
                );
            }
        }
        ExprKind::Closure { body, .. } => walk(program, body, false, 0, depth + 1, diagnostics),
        _ => {
            let mut index = 0;
            while let Some(child) = expr.child(index) {
                walk(program, child, false, loops, depth + 1, diagnostics);
                index += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate;
    use crate::ast::{Expr, ExprKind, LexicalExitKind, Program, UnaryOp};

    fn fixture() -> Program {
        crate::parse(
            "module lexical.contour; @id(\"main\") fn main() -> i64 { 0 }",
            "lexical-contour.spx",
        )
        .unwrap()
    }

    fn wrapped(mut expr: Expr, depth: usize) -> Expr {
        for _ in 0..depth {
            let span = expr.span;
            expr = Expr {
                span,
                kind: ExprKind::Unary {
                    op: UnaryOp::Neg,
                    value: Box::new(expr),
                },
            };
        }
        expr
    }

    #[test]
    fn legacy_depth_512_does_not_select_the_lexical_contour_contract() {
        let mut program = fixture();
        program.functions[0].body = wrapped(program.functions[0].body.clone(), 510);
        let mut diagnostics = Vec::new();
        validate(&program, &mut diagnostics);
        assert!(diagnostics.is_empty());
        assert!(!program.functions[0].body.contains_lexical_exit());
    }

    #[test]
    fn lexical_presence_keeps_deep_contours_and_contract_crossings_closed() {
        let mut program = fixture();
        let value = program.functions[0].body.clone();
        let exit = Expr {
            span: value.span,
            kind: ExprKind::LexicalExit {
                kind: LexicalExitKind::Return,
                value: Some(Box::new(value)),
            },
        };
        program.functions[0].body = wrapped(exit.clone(), crate::cache_codec::MAX_DEPTH + 1);
        let mut diagnostics = Vec::new();
        validate(&program, &mut diagnostics);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-P207"));
        // Contracts select the same independent scan and cannot grant an exit
        // merely because the function's ordinary body contains none.
        program.functions[0].body = fixture().functions[0].body.clone();
        program.functions[0].ensures.push(exit);
        diagnostics.clear();
        validate(&program, &mut diagnostics);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T312"));
    }
}
