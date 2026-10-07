//! `while` admission rules for the iterative verifier: the fail-closed
//! rejection of expression forms that are not yet admitted inside a loop.

use crate::ast::{Expr, ExprKind, ParamMode, Statement, Type};
use crate::source_verify::diagnostics::error;
use crate::source_verify::IterativeVerifier;

impl<'a, 'p> IterativeVerifier<'a, 'p> {
    pub(super) fn reject_for_body_disallowed(
        &mut self,
        body: &'p Expr,
        source: &str,
    ) -> Result<(), ()> {
        enum Item<'a> {
            Expr(&'a Expr),
            Statement(&'a Statement),
        }
        let mut pending = vec![Item::Expr(body)];
        while let Some(item) = pending.pop() {
            match item {
                Item::Statement(Statement::Assign { name, span, .. }) if name == source => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T284",
                        "for traversal source cannot be rebound or mutated in the loop body",
                        *span,
                    ));
                    return Err(());
                }
                Item::Statement(Statement::For { span, .. } | Statement::ForOwn { span, .. }) => {
                    self.diagnostics.push(error(
                        self.program,
                        "SPX-T284",
                        "nested for traversal is not admitted in this bounded profile",
                        *span,
                    ));
                    return Err(());
                }
                Item::Statement(statement) => {
                    for index in (0..statement.child_count()).rev() {
                        if let Some(child) = statement.child(index) {
                            pending.push(Item::Expr(child));
                        }
                    }
                }
                Item::Expr(expression) => {
                    if let ExprKind::Call { name, args, .. } = &expression.kind {
                        let consumes_source = crate::vec_ops::by_name(name).is_some_and(|op| {
                            op.param_ownership(0) == crate::hir::OwnershipMode::Own
                                && matches!(args.first().map(|arg| &arg.kind), Some(ExprKind::Var(name)) if name == source)
                        });
                        if consumes_source {
                            self.diagnostics.push(error(
                                self.program, "SPX-T284",
                                "for traversal source cannot be consumed or mutated in the loop body",
                                expression.span,
                            ));
                            return Err(());
                        }
                    }
                    if let ExprKind::Block { statements, tail } = &expression.kind {
                        pending.extend(statements.iter().rev().map(Item::Statement));
                        pending.push(Item::Expr(tail));
                    }
                }
            }
        }
        Ok(())
    }

    /// Bounded While-Loops v1 plus Indexed Byte Loop v2 admission profile: a
    /// loop condition or body may contain Copy-scalar operations — scalar
    /// literals, names, checked
    /// scalar arithmetic and comparisons, nested `if`s over scalars, blocks
    /// with scalar statements, scalar `let`/assignment statements, nested
    /// while loops, monomorphic calls to scalar-value functions, exact
    /// read-only `byte_len`/`byte_get`, and one guard-free direct
    /// `byte_get`/`Option<u8>` match. Every other construct is rejected
    /// fail-closed so loop cleanup stays edge-free.
    /// Owned String Loops v1: a `while` condition creates no owned String.
    pub(super) fn reject_owned_string_condition(&mut self, condition: &'p Expr) {
        let functions = &self.functions;
        if let Some(span) = crate::string_ops::owned_string_in_condition(condition, &|name| {
            functions.get(name).is_some_and(|function| {
                function.return_type == Type::String
                    || function.params.iter().any(|param| param.ty == Type::String)
            })
        }) {
            self.diagnostics.push(error(
                self.program,
                "SPX-T252",
                crate::string_ops::OWNED_STRING_CONDITION_MESSAGE,
                span,
            ));
        }
    }

    pub(super) fn reject_while_disallowed(&mut self, expression: &'p Expr) -> Result<(), ()> {
        self.reject_iterator_body(expression, None)
    }
    pub(super) fn reject_iterator_body(
        &mut self,
        expression: &'p Expr,
        owned_item: Option<&str>,
    ) -> Result<(), ()> {
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
            FieldsNext {
                fields: &'a [crate::ast::FieldInitializer],
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
                    Statement::For { span, .. } | Statement::ForOwn { span, .. } => {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T284",
                            "nested for traversal is not admitted in this bounded profile",
                            *span,
                        ));
                        return Err(());
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
                Frame::FieldsNext { fields, next } => {
                    if next != 0 && results.pop().is_none_or(|result| result.is_err()) {
                        results.push(Err(()));
                        continue;
                    }
                    if let Some(field) = fields.get(next) {
                        frames.push(Frame::FieldsNext {
                            fields,
                            next: next + 1,
                        });
                        frames.push(Frame::Expression(&field.value));
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
                        if let Some(guard) = &arm.guard {
                            frames.push(Frame::JoinAll(2));
                            frames.push(Frame::Expression(&arm.value));
                            frames.push(Frame::Expression(guard));
                        } else {
                            frames.push(Frame::Expression(&arm.value));
                        }
                    } else {
                        results.push(Ok(()));
                    }
                    continue;
                }
            };

            match &expression.kind {
                ExprKind::Closure { .. } => results.push(Ok(())),

                ExprKind::Int(_)
                | ExprKind::Int32(_)
                | ExprKind::Char(_)
                | ExprKind::Uint8(_)
                | ExprKind::Usize(_)
                | ExprKind::Float32(_)
                | ExprKind::Float64(_)
                | ExprKind::Bool(_)
                | ExprKind::Var(_) => results.push(Ok(())),
                // Owned String Loops v1: a literal allocates one owned
                // String in the per-iteration body region.
                ExprKind::String(_) => results.push(Ok(())),
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
                    let vec_operation = crate::vec_ops::by_name(name);
                    if !type_arguments.is_empty() && vec_operation.is_none() {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T252",
                            "generic calls are not yet admitted in while bodies",
                            expression.span,
                        ));
                        results.push(Err(()));
                        continue;
                    }
                    if let Some(operation) = vec_operation {
                        if !operation.admitted_in_while()
                            || type_arguments.len() != 1
                            || !crate::vec_ops::ast_element_is_admitted(&type_arguments[0])
                            || args.len() != operation.arity()
                        {
                            self.diagnostics.push(error(
                                self.program,
                                "SPX-T283",
                                "only exact typed scalar Vec update/read operations are admitted in while bodies",
                                expression.span,
                            ));
                            results.push(Err(()));
                            continue;
                        }
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
                        if !operation.admitted_in_while() || args.len() != operation.arity() {
                            self.diagnostics.push(error(
                            self.program,
                            "SPX-T252",
                            format!(
                                "byte operation `{name}` is not admitted in while bodies; only exact byte_len, byte_get and byte_range reads and the loop-carried bytes_set/bytes_set5/bytes_set1_or5_from_slice/bytes_set1_or6_or48_from_slice fills qualify"
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
                        let scalar_signature = crate::stdin_stream_ops::ast_forward_signature(declared) || ( crate::loop_calls::effects_admitted(&declared.effects)
                            && crate::loop_calls::ast_result_admitted(&declared.return_type)
                            && declared.params.iter().zip(args).all(|(param, argument)| {
                                crate::loop_calls::ast_param_admitted(param.mode, &param.ty)
                                    || (param.mode == ParamMode::Own && param.ty == Type::Bytes
                                        && owned_item.is_some_and(|item| matches!(&argument.kind, ExprKind::Var(name) if name == item)))
                            }));
                        if !scalar_signature {
                            self.diagnostics.push(error(
                            self.program,
                            "SPX-T252",
                            format!(
                                "call `{name}` is not admitted in loop bodies; use scalar/text signatures with read-only input effects. For output, build one string in the loop and write it once afterwards"
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
                ExprKind::ConstructVariant {
                    type_name,
                    type_arguments,
                    fields,
                    ..
                } => {
                    let ty = Type::Named {
                        name: type_name.clone(),
                        arguments: type_arguments.clone(),
                    };
                    if crate::source_verify::variant_guards::copy_variant(self.types, &ty) {
                        frames.push(Frame::FieldsNext { fields, next: 0 });
                    } else {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T252",
                            "variant construction in a loop requires only Copy scalar payloads",
                            expression.span,
                        ));
                        results.push(Err(()));
                    }
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
                // Owned String Loops v2: a match is scanned like any
                // branch; the verifier's typed scrutinee rule and HIR
                // validation narrow it to Copy scalars and Copy-payload
                // variants.
                ExprKind::Match {
                    scrutinee, arms, ..
                } => {
                    frames.push(Frame::MatchNext { arms, next: 0 });
                    frames.push(Frame::Expression(scrutinee));
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
                // Resumable Effects control profile (issue #296):
                // `parser::yields` admits a `yield` here only as a direct
                // statement value; its request is checked like any operand.
                ExprKind::Yield { request } => frames.push(Frame::Expression(request)),
            }
        }
        results.pop().unwrap_or(Err(()))
    }
}
