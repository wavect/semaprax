//! `while` admission rules for the iterative verifier: the fail-closed
//! rejection of expression forms that are not yet admitted inside a loop.

use crate::ast::{Expr, ExprKind, ParamMode, Statement, Type};
use crate::source_verify::diagnostics::{error, is_scalar_source_type};
use crate::source_verify::IterativeVerifier;

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    /// Bounded While-Loops v1 plus Indexed Byte Loop v2 admission profile: a
    /// loop condition or body may contain Copy-scalar operations — scalar
    /// literals, names, checked
    /// scalar arithmetic and comparisons, nested `if`s over scalars, blocks
    /// with scalar statements, scalar `let`/assignment statements, nested
    /// while loops, monomorphic calls to scalar-value functions, exact
    /// read-only `byte_len`/`byte_get`, and one guard-free direct
    /// `byte_get`/`Option<u8>` match. Every other construct is rejected
    /// fail-closed so loop cleanup stays edge-free.
    pub(super) fn reject_while_disallowed(&mut self, expression: &'p Expr) -> Result<(), ()> {
        enum Frame<'a> {
            Expression(&'a Expr),
            Statement(&'a Statement),
            JoinAll(usize),
            BlockNext {
                statements: &'a [Statement],
                next: usize,
                tail: &'a Expr,
            },
            CallNext {
                args: &'a [Expr],
                next: usize,
            },
            MatchNext {
                arms: &'a [crate::ast::MatchArm],
                next: usize,
            },
        }

        let mut frames = vec![Frame::Expression(expression)];
        let mut results = Vec::new();
        while let Some(frame) = frames.pop() {
            let expression = match frame {
                Frame::Statement(statement) => match statement {
                    Statement::Let { value, .. } | Statement::Assign { value, .. } => value,
                    Statement::Unsafe { span, .. } => {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T252",
                            "unsafe boundary statements are not yet admitted in while bodies",
                            *span,
                        ));
                        results.push(Err(()));
                        continue;
                    }
                    Statement::While {
                        condition, body, ..
                    } => {
                        // The recursive admission scan visits both children
                        // even when the condition is rejected.
                        frames.push(Frame::JoinAll(2));
                        frames.push(Frame::Expression(body));
                        frames.push(Frame::Expression(condition));
                        continue;
                    }
                },
                Frame::Expression(expression) => expression,
                Frame::JoinAll(count) => {
                    let start = results
                        .len()
                        .checked_sub(count)
                        .expect("child results retained");
                    let accepted = results[start..].iter().all(Result::is_ok);
                    results.truncate(start);
                    results.push(if accepted { Ok(()) } else { Err(()) });
                    continue;
                }
                Frame::BlockNext {
                    statements,
                    next,
                    tail,
                } => {
                    if next != 0 && results.pop().is_none_or(|result| result.is_err()) {
                        results.push(Err(()));
                        continue;
                    }
                    if let Some(statement) = statements.get(next) {
                        frames.push(Frame::BlockNext {
                            statements,
                            next: next + 1,
                            tail,
                        });
                        frames.push(Frame::Statement(statement));
                    } else {
                        frames.push(Frame::Expression(tail));
                    }
                    continue;
                }
                Frame::CallNext { args, next } => {
                    if next != 0 && results.pop().is_none_or(|result| result.is_err()) {
                        results.push(Err(()));
                        continue;
                    }
                    if let Some(argument) = args.get(next) {
                        frames.push(Frame::CallNext {
                            args,
                            next: next + 1,
                        });
                        frames.push(Frame::Expression(argument));
                    } else {
                        results.push(Ok(()));
                    }
                    continue;
                }
                Frame::MatchNext { arms, next } => {
                    // `next == 0` consumes the scrutinee result; later
                    // continuations consume the preceding arm result.
                    if results.pop().is_none_or(|result| result.is_err()) {
                        results.push(Err(()));
                        continue;
                    }
                    if let Some(arm) = arms.get(next) {
                        frames.push(Frame::MatchNext {
                            arms,
                            next: next + 1,
                        });
                        frames.push(Frame::Expression(&arm.value));
                    } else {
                        results.push(Ok(()));
                    }
                    continue;
                }
            };

            match &expression.kind {
                ExprKind::Int(_)
                | ExprKind::Int32(_)
                | ExprKind::Char(_)
                | ExprKind::Uint8(_)
                | ExprKind::Usize(_)
                | ExprKind::Float32(_)
                | ExprKind::Float64(_)
                | ExprKind::Bool(_)
                | ExprKind::Var(_) => results.push(Ok(())),
                ExprKind::String(_) => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "string literals are not yet admitted in while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
                ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "fixed-array literals are not admitted in bounded while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
                ExprKind::Unary { value, .. } => frames.push(Frame::Expression(value)),
                ExprKind::Binary { left, right, .. } => {
                    frames.push(Frame::JoinAll(2));
                    frames.push(Frame::Expression(right));
                    frames.push(Frame::Expression(left));
                }
                ExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    frames.push(Frame::JoinAll(3));
                    frames.push(Frame::Expression(else_branch));
                    frames.push(Frame::Expression(then_branch));
                    frames.push(Frame::Expression(condition));
                }
                ExprKind::Block { statements, tail } => {
                    frames.push(Frame::BlockNext {
                        statements,
                        next: 0,
                        tail,
                    });
                }
                ExprKind::Call {
                    type_arguments,
                    args,
                    name,
                    ..
                } => {
                    if !type_arguments.is_empty() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T252",
                            "generic calls are not yet admitted in while bodies",
                            expression.span,
                        ));
                        results.push(Err(()));
                        continue;
                    }
                    if crate::command_io_ops::by_name(name).is_some_and(|operation| {
                        !crate::command_io_ops::admitted_in_while(operation)
                    }) {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T270",
                            format!(
                                "command I/O operation `{name}` is not admitted in while bodies"
                            ),
                            expression.span,
                        ));
                        results.push(Err(()));
                        continue;
                    }
                    if crate::command_io_ops::by_name(name).is_some_and(|operation| {
                        args.len() != crate::command_io_ops::arity(operation)
                    }) {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T270",
                            format!("invalid command I/O operation `{name}` call shape"),
                            expression.span,
                        ));
                        results.push(Err(()));
                        continue;
                    }
                    if let Some(operation) = crate::byte_ops::by_name(name) {
                        if !matches!(
                            operation,
                            crate::byte_ops::ByteOp::Len
                                | crate::byte_ops::ByteOp::Get
                                | crate::byte_ops::ByteOp::Range
                        ) || args.len() != operation.arity()
                        {
                            self.diagnostics.push(error(
                            self.program,
                            "SPX-T252",
                            format!(
                                "byte operation `{name}` is not admitted in while bodies; only exact byte_len and byte_get reads qualify"
                            ),
                            expression.span,
                        ));
                            results.push(Err(()));
                            continue;
                        }
                    }
                    // Only calls that resolve to a monomorphic function with
                    // by-value scalar parameters and a scalar result keep the
                    // loop cleanup-edge-free; unknown names keep flowing so the
                    // established unresolved-value diagnostic fires instead.
                    if let Some(declared) = self.functions.get(name.as_str()) {
                        let scalar_signature = declared.effects.is_empty()
                            && is_scalar_source_type(&declared.return_type)
                            && declared.params.iter().all(|param| {
                                (param.mode == ParamMode::Value && is_scalar_source_type(&param.ty))
                                    || (param.mode == ParamMode::Borrow
                                        && param.ty == Type::SliceU8)
                            });
                        if !scalar_signature {
                            self.diagnostics.push(error(
                            self.program,
                            "SPX-T252",
                            format!(
                                "call `{name}` is not admitted in while bodies; only scalar functions qualify"
                            ),
                            expression.span,
                        ));
                            results.push(Err(()));
                            continue;
                        }
                    }
                    frames.push(Frame::CallNext { args, next: 0 });
                }
                ExprKind::SuperMethod { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "super method calls are not yet admitted in while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
                ExprKind::MethodCall { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "method calls are not yet admitted in while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
                ExprKind::Project { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "record field projection is not yet admitted in while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
                ExprKind::ConstructRecord { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "record construction is not yet admitted in while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
                ExprKind::ConstructVariant { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "variant construction is not yet admitted in while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
                ExprKind::UpdateRecord { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "record updates are not yet admitted in while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
                ExprKind::Match {
                    scrutinee, arms, ..
                } if crate::byte_ops::is_indexed_byte_option_match_source(expression) => {
                    frames.push(Frame::MatchNext { arms, next: 0 });
                    frames.push(Frame::Expression(scrutinee));
                }
                ExprKind::Match { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "match expressions are not yet admitted in while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
                ExprKind::Try { .. } => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T252",
                        "postfix `?` propagation is not yet admitted in while bodies",
                        expression.span,
                    ));
                    results.push(Err(()));
                }
            }
        }
        results.pop().unwrap_or(Err(()))
    }
}
