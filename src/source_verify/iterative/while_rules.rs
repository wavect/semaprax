//! `while` admission rules for the iterative verifier: the fail-closed
//! rejection of expression forms that are not yet admitted inside a loop.

use crate::ast::{Expr, ExprKind, ParamMode, Statement, Type};
use crate::source_verify::diagnostics::error;
use crate::source_verify::IterativeVerifier;

impl<'a, 'v, 'p> IterativeVerifier<'a, 'v, 'p> {
    pub(super) fn reject_for_body_in_scope(
        &mut self,
        body: &'p Expr,
        source: &str,
        byte_entry_scope: Option<usize>,
    ) -> Result<(), ()> {
        #[cfg(test)]
        if crate::source_verify::byte_traversal::nested::before_selected() {
            return self.reject_for_body_disallowed_before_nested(body, source);
        }
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
                Item::Statement(Statement::For { values, body, .. })
                    if byte_entry_scope
                        .is_some_and(|scope| self.nested_byte_source(scope, values)) =>
                {
                    pending.push(Item::Expr(body));
                }
                Item::Statement(
                    Statement::For { span, .. }
                    | Statement::ForBorrow { span, .. }
                    | Statement::ForOwn { span, .. },
                ) => {
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
    /// fail-closed outside the admitted per-iteration lifetime profile.
    /// Computed String conditions have a separate per-iteration cleanup region.
    pub(super) fn reject_while_disallowed(&mut self, expression: &'p Expr) -> Result<(), ()> {
        if crate::finite_owned_record_loop::source(self.current.payload(), expression).is_some() {
            return Ok(());
        }
        self.reject_iterator_body(expression, None)
    }
    pub(super) fn reject_iterator_body(
        &mut self,
        expression: &'p Expr,
        owned_item: Option<&str>,
    ) -> Result<(), ()> {
        let scope = self.active_byte_entry_scope();
        self.reject_iterator_body_in_scope(expression, owned_item, scope)
    }
    pub(super) fn reject_iterator_body_in_scope(
        &mut self,
        expression: &'p Expr,
        owned_item: Option<&str>,
        byte_entry_scope: Option<usize>,
    ) -> Result<(), ()> {
        #[cfg(test)]
        if crate::source_verify::byte_traversal::nested::before_selected() {
            return self.reject_iterator_body_before_nested(expression, owned_item);
        }
        let finite_scratch = owned_item
            .and_then(|_| crate::finite_byte_scratch::source(self.current.payload(), expression));
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
                    Statement::Assign {
                        name,
                        field: None,
                        value,
                        ..
                    } if self.admits_record_owner_renewal(name, value) => {
                        let ExprKind::Call { args, .. } = &value.kind else {
                            unreachable!("record renewal admission requires a call")
                        };
                        // The outer call's owning result is the authenticated
                        // renewal operation. Its arguments remain ordinary loop
                        // expressions: skipping them would let a pure renewal
                        // wrapper hide a disallowed nested/effectful call.
                        frames.push(Frame::CallNext { args, next: 0 });
                        continue;
                    }
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
                    Statement::For { values, body, .. }
                        if byte_entry_scope
                            .is_some_and(|scope| self.nested_byte_source(scope, values)) =>
                    {
                        frames.push(Frame::Expression(body));
                        continue;
                    }
                    Statement::For { span, .. }
                    | Statement::ForBorrow { span, .. }
                    | Statement::ForOwn { span, .. } => {
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
                ExprKind::LexicalExit { kind, value } => {
                    if let Some(value) = value {
                        if *kind == crate::ast::LexicalExitKind::Return
                            && crate::hir::lexical_exit::return_operand::source_named_handoff(value)
                        {
                            results.push(Ok(()));
                        } else {
                            frames.push(Frame::Expression(value));
                        }
                    } else {
                        results.push(Ok(()));
                    }
                }
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
                ExprKind::BorrowedUtf8Literal(text) => {
                    match crate::source_verify::static_text::checked(
                        self.program,
                        expression.span,
                        text,
                    ) {
                        Ok(_) => results.push(Ok(())),
                        Err(error) => {
                            self.diagnostics.push(error);
                            results.push(Err(()));
                        }
                    }
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
                ExprKind::VecConstructor { value, .. } | ExprKind::Unary { value, .. } => {
                    frames.push(Frame::Expression(value))
                }
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
                    if !type_arguments.is_empty()
                        && vec_operation.is_none()
                        && name != crate::vec_field::NAME
                        && crate::map_ops::by_generic_name(name, type_arguments).is_none()
                    {
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
                            || (if operation == crate::vec_ops::VecOp::StrAt {
                                !crate::source_verify::string_slot::while_shape(
                                    type_arguments,
                                    args,
                                )
                            } else {
                                type_arguments.len() != 1
                            || !(crate::vec_ops::ast_element_is_admitted(&type_arguments[0])
                                || crate::source_verify::declared_type::copy_record_collection::source_admitted(self.program, &type_arguments[0]) || crate::source_verify::declared_type::owned_leaf_collection::source_admitted(self.program, &type_arguments[0]))
                            })
                            || args.len()
                                != if operation == crate::vec_ops::VecOp::UpdateField {
                                    // The source-only literal selector is removed before HIR.
                                    4
                                } else {
                                    operation.arity()
                                }
                        {
                            #[cfg(test)]
                            crate::source_verify::string_slot::trace_while_refusal(
                                "iterative",
                                operation,
                                type_arguments,
                                args,
                            );
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
                        if (!operation.admitted_in_while()
                            && !crate::source_verify::finite_row_allocation(
                                self.program,
                                expression,
                            )
                            && !self
                                .fused_producer
                                .is_some_and(|producer| std::ptr::eq(producer, expression))
                            && !finite_scratch
                                .as_ref()
                                .is_some_and(|proof| proof.permits(expression)))
                            || args.len() != operation.arity()
                        {
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
                        let scalar_signature = self.byte_helper_target(declared) || crate::stdin_stream_ops::ast_forward_signature(declared.payload()) || ( crate::loop_calls::effects_admitted(&declared.payload().effects)
                            && crate::loop_calls::ast_result_admitted(self.program, &declared.payload().return_type)
                            && declared.payload().params.iter().zip(args).all(|(param, argument)| {
                                crate::loop_calls::ast_param_admitted(self.program, param.mode, &param.ty)
                                    || (param.mode == ParamMode::Borrow
                                        && crate::source_verify::declared_type::owned_record_collection::is_owner_renewal_record(self.types, &param.ty))
                                    || (param.mode == ParamMode::Own
                                        && (param.ty == Type::Bytes
                                            || crate::source_verify::declared_type::owned_leaf_collection::runtime_element(self.types, &param.ty))
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
                ExprKind::Project { base, .. } => frames.push(Frame::Expression(base)),
                ExprKind::ConstructRecord {
                    type_name,
                    type_arguments,
                    fields,
                    ..
                } => {
                    let ty = Type::Named {
                        name: type_name.clone(),
                        arguments: type_arguments.clone(),
                    };
                    if crate::source_verify::declared_type::owned_leaf_collection::copy_or_leaf_admitted(
                        self.types, &ty,
                    ) {
                        frames.push(Frame::FieldsNext { fields, next: 0 });
                    } else {
                        self.diagnostics.push(error(
                            self.program,
                            "SPX-T252",
                            "record construction is not yet admitted in while bodies",
                            expression.span,
                        ));
                        results.push(Err(()));
                    }
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

    pub(in crate::source_verify) fn admits_record_owner_renewal(
        &self,
        binding: &str,
        value: &Expr,
    ) -> bool {
        if self.byte_helper_assignment(binding, value) {
            return true;
        }
        let ExprKind::Call {
            name,
            type_arguments,
            args,
            ..
        } = &value.kind
        else {
            return false;
        };
        let Some(target) = self.functions.get(name.as_str()) else {
            return false;
        };
        type_arguments.is_empty()
            && target.payload().type_parameters.is_empty()
            && target.payload().effects.is_empty()
            && crate::source_verify::declared_type::owned_record_collection::is_owner_renewal_record(
                self.types,
                &target.payload().return_type,
            )
            && target.payload().params.len() == args.len()
            && target
                .payload()
                .params
                .iter()
                .zip(args)
                .filter(|(parameter, argument)| {
                    parameter.mode == ParamMode::Own
                        && parameter.ty == target.payload().return_type
                        && matches!(&argument.kind, ExprKind::Var(name) if name == binding)
                })
                .count()
                == 1
            && target
                .payload()
                .params
                .iter()
                .zip(args)
                .all(|(parameter, argument)| {
                    match parameter.mode {
                    ParamMode::Own => {
                        parameter.ty == target.payload().return_type
                            && matches!(&argument.kind, ExprKind::Var(name) if name == binding)
                    }
                    ParamMode::Borrow => {
                        (crate::source_verify::declared_type::owned_record_collection::
                            is_owner_renewal_record(self.types, &parameter.ty)
                            || crate::loop_calls::ast_renewal_view(&parameter.ty))
                            && matches!(argument.kind, ExprKind::Var(_))
                    }
                    ParamMode::Value => true,
                    ParamMode::Shared => false,
                }
                })
    }
}

#[test]
fn record_renewal_named_view_admission_agrees_with_iterative_verifier() {
    use crate::diagnostic::Diagnostic;
    use crate::source_verify::oracle::while_oracle::reject_while_disallowed_oracle;
    use crate::source_verify::type_table::TypeTable;
    use std::collections::HashMap;
    let source = include_str!("../../../tests/language/while_loops/record_borrow_renewal.spx");
    for (setup, arguments) in [
        (
            "bytes_as_slice(source)",
            "matcher, input_alias, text, 1usize",
        ),
        (
            "bytes_as_slice(matcher.storage)",
            "matcher, input_alias, text, 1usize",
        ),
        (
            "bytes_as_slice(source)",
            "matcher, bytes_as_slice(source), text, 1usize",
        ),
        (
            "bytes_as_slice(source)",
            "matcher, input_alias, string_as_str(label), 1usize",
        ),
    ] {
        let source = source
            .replace("bytes_as_slice(source)", setup)
            .replace("matcher, input_alias, text, 1usize", arguments);
        let program = crate::parse(&source, std::path::Path::new("renewal-admission.spx")).unwrap();
        let current = program.functions.iter().find(|f| f.name == "run").unwrap();
        let function_cells = crate::source_verify::capacity::source_capacity_functions(&program);
        let functions = function_cells
            .iter()
            .take(program.functions.len())
            .map(|(_, function)| (function.name(), function))
            .collect::<crate::source_verify::function_map::FunctionMap<'_>>();
        let types = TypeTable::new(&program);
        let ExprKind::Block { statements, .. } = &current.body.kind else {
            panic!("run block")
        };
        let body = statements
            .iter()
            .find_map(|statement| match statement {
                Statement::While { body, .. } => Some(body.as_ref()),
                _ => None,
            })
            .unwrap();
        let mut oracle_diagnostics = Vec::new();
        let expected = reject_while_disallowed_oracle(
            &program,
            body,
            &functions,
            &types,
            &mut oracle_diagnostics,
        );
        let mut actual_diagnostics = Vec::new();
        let mut verifier = crate::source_verify::IterativeVerifier::new(
            &program,
            functions
                .get(current.name.as_str())
                .expect("frozen current declaration cell"),
            HashMap::new(),
            &functions,
            &types,
            None,
            true,
            &mut actual_diagnostics,
        );
        let actual = verifier.reject_while_disallowed(body);
        assert_eq!(actual, expected);
        let key = |diagnostics: Vec<Diagnostic>| {
            diagnostics
                .into_iter()
                .map(|d| (d.code, d.severity, d.message, d.path, d.span, d.help))
                .collect::<Vec<_>>()
        };
        assert_eq!(key(actual_diagnostics), key(oracle_diagnostics));
    }
}

#[cfg(test)]
fn string_slot_loop_admission(source: &str, admitted: bool) {
    use crate::source_verify::oracle::while_oracle::reject_while_disallowed_oracle;
    use crate::source_verify::type_table::TypeTable;
    use std::collections::HashMap;
    let program = crate::parse(source, std::path::Path::new("slots-loop.spx")).unwrap();
    let current = program
        .functions
        .iter()
        .find(|f| f.name == "inspect")
        .unwrap();
    let function_cells = crate::source_verify::capacity::source_capacity_functions(&program);
    let functions = function_cells
        .iter()
        .take(program.functions.len())
        .map(|(_, function)| (function.name(), function))
        .collect::<crate::source_verify::function_map::FunctionMap<'_>>();
    let types = TypeTable::new(&program);
    let ExprKind::Block { statements, .. } = &current.body.kind else {
        panic!("inspect block")
    };
    let body = statements
        .iter()
        .find_map(|statement| match statement {
            Statement::While { body, .. } => Some(body.as_ref()),
            _ => None,
        })
        .unwrap();
    let mut expected_diagnostics = Vec::new();
    let expected = reject_while_disallowed_oracle(
        &program,
        body,
        &functions,
        &types,
        &mut expected_diagnostics,
    );
    let mut actual_diagnostics = Vec::new();
    let mut verifier = crate::source_verify::IterativeVerifier::new(
        &program,
        functions
            .get(current.name.as_str())
            .expect("frozen current declaration cell"),
        HashMap::new(),
        &functions,
        &types,
        None,
        true,
        &mut actual_diagnostics,
    );
    let actual = verifier.reject_while_disallowed(body);
    assert_eq!(actual, expected);
    assert_eq!(actual.is_ok(), admitted);
    let keys = |items: Vec<crate::diagnostic::Diagnostic>| {
        items
            .into_iter()
            .map(|d| (d.code, d.severity, d.message, d.path, d.span, d.help))
            .collect::<Vec<_>>()
    };
    if !admitted {
        assert!(actual_diagnostics.iter().any(|d| d.code == "SPX-T283"));
    }
    assert_eq!(keys(actual_diagnostics), keys(expected_diagnostics));
}

#[test]
fn string_slot_named_root_fused_loop_source_matches_independent_oracle() {
    let source = include_str!("../../../tests/fixtures/string-slot-views.spx");
    crate::check(source, "slots-loop.spx").unwrap();
    string_slot_loop_admission(source, true);
}

#[test]
fn string_slot_explicit_types_and_computed_roots_keep_exact_loop_refusal() {
    let source = include_str!("../../../tests/fixtures/string-slot-views.spx");
    for replacement in [
        "str_as_bytes(vec_str_at<string>(values,0usize))",
        "str_as_bytes(vec_str_at({values},0usize))",
        "str_as_bytes(vec_str_at(values))",
    ] {
        let changed = source.replace("str_as_bytes(vec_str_at(values,0usize))", replacement);
        assert_ne!(changed, source);
        string_slot_loop_admission(&changed, false);
    }
}

#[cfg(test)]
#[allow(
    dead_code,
    reason = "complete independently compiled pre-nesting loop admission oracle"
)]
mod before_nested;
