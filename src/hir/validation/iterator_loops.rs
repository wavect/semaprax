use super::*;

impl HirValidator<'_> {
    /// Bounded While-Loops v1 plus Indexed Byte Loop v2 admission re-check at
    /// the HIR trust boundary. Loop conditions and bodies admit Copy-scalar
    /// operations plus exact read-only `byte_len`/`byte_get` and one direct
    /// guard-free compiler-owned `byte_get`/`Option<u8>` match. Anything else
    /// fails closed as malformed HIR because source resolution rejected it
    /// with `SPX-T252`.
    pub(super) fn validate_while_admission(
        &self,
        expression: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        self.validate_iterator_body(expression, None)
    }
    fn validate_iterator_body(
        &self,
        expression: &ResolvedExpr,
        owned_item: Option<&ResolvedBinding>,
    ) -> Result<(), Diagnostic> {
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            match &expression.kind {
                ResolvedExprKind::Closure { captures, .. } => {
                    pending.extend(captures.iter().rev().map(|capture| &capture.value));
                }
                ResolvedExprKind::FunctionReference { .. } | ResolvedExprKind::Invoke { .. } => {
                    pending.extend(self.while_callable_arguments(expression)?.into_iter().rev());
                }
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_) => {}
                ResolvedExprKind::Place(place) => {
                    // Owned String Loops v1: a whole String binding may be
                    // read or consumed; ordinary ownership replay and the
                    // loop-invariant cleanup state authenticate the use.
                    // String Collections v1 maps are read and reopened the
                    // same way, only as map-operation operands.
                    // Owned String Loops v2: a whole Copy-payload variant
                    // place may be a match scrutinee; moving an outer one
                    // still fails the loop-entry state equality.
                    let whole_string =
                        (matches!(
                            expression.ty,
                            ResolvedType::String | ResolvedType::StringMap
                        ) || (matches!(expression.ty, ResolvedType::Nominal { .. })
                            && crate::loop_calls::resolved_match_scrutinee_admitted(
                                &self.program.declarations,
                                &expression.ty,
                            )))
                            && place.projections.is_empty();
                    let named_str = expression.ty == ResolvedType::Str
                        && expression.ownership == OwnershipMode::Borrow
                        && place.projections.is_empty();
                    if !whole_string
                        && !named_str
                        && (!crate::hir::is_scalar_resolved_type(&expression.ty)
                            || expression.ownership != OwnershipMode::Value)
                    {
                        return Err(hir_error(
                            "while loop places must be Copy scalars outside an indexed byte read",
                        ));
                    }
                }
                // Owned String Loops v1: a literal allocates one owned String
                // in the per-iteration body region.
                ResolvedExprKind::String(_) => {}
                ResolvedExprKind::ArrayU8(_) | ResolvedExprKind::RepeatArrayU8 { .. } => {
                    return Err(hir_error("while loops cannot contain fixed-array literals"));
                }
                ResolvedExprKind::BorrowPlace { operation, place } => {
                    // Ordinary expression replay independently authenticates
                    // the live String owner and its canonical shared loan.
                    let exact_view = (operation.as_str() == crate::byte_ops::STRING_AS_STR_ID
                        && expression.ty == ResolvedType::Str)
                        || (operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID
                            && expression.ty == ResolvedType::SliceU8);
                    if !exact_view
                        || expression.ownership != OwnershipMode::Borrow
                        || !place.projections.is_empty()
                    {
                        return Err(hir_error("while loops cannot construct byte views"));
                    }
                }
                ResolvedExprKind::ByteRange {
                    source, start, end, ..
                } => {
                    let ResolvedExprKind::Place(place) = &source.kind else {
                        return Err(hir_error(
                            "while loop byte ranges require an existing byte-slice alias",
                        ));
                    };
                    if !place.projections.is_empty()
                        || (!self.byte_slice_aliases.contains_key(&place.root)
                            && self
                                .program
                                .declarations
                                .byte_slice_provenance(&place.root)
                                .is_none())
                    {
                        return Err(hir_error(
                            "while loop byte range lacks authenticated slice provenance",
                        ));
                    }
                    pending.push(end);
                    pending.push(start);
                }
                ResolvedExprKind::HostCommandCall(call) => {
                    pending.extend(
                        self.while_host_command_scalar_arguments(expression, call)?
                            .into_iter()
                            .rev(),
                    );
                }
                ResolvedExprKind::Unary { value, .. } => pending.push(value),
                ResolvedExprKind::Binary { left, right, .. } => {
                    pending.push(right);
                    pending.push(left);
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push(else_branch);
                    pending.push(then_branch);
                    pending.push(condition);
                }
                ResolvedExprKind::Block { statements, tail } => {
                    pending.push(tail);
                    for statement in statements.iter().rev() {
                        for index in (0..statement.child_count()).rev() {
                            let child = statement
                                .child(index)
                                .ok_or_else(|| hir_error("while statement child is missing"))?;
                            pending.push(child);
                        }
                    }
                }
                ResolvedExprKind::Call {
                    callee,
                    instance,
                    type_arguments,
                    args,
                } => {
                    let vec_operation = instance
                        .is_none()
                        .then(|| crate::vec_ops::by_id(callee.as_str()))
                        .flatten();
                    if instance.is_some() || (!type_arguments.is_empty() && vec_operation.is_none())
                    {
                        return Err(hir_error("while loops cannot contain generic calls"));
                    }
                    if let Some(operation) = vec_operation {
                        if !operation.admitted_in_while()
                            || type_arguments.len() != 1
                            || !crate::vec_ops::resolved_element_is_admitted(&type_arguments[0])
                            || args.len() != operation.arity()
                            || args.iter().enumerate().any(|(index, argument)| {
                                !operation.accepts_resolved(index, &argument.ty, &type_arguments[0])
                            })
                            || expression.ty != operation.resolved_return_type(&type_arguments[0])
                        {
                            return Err(hir_error(
                                "while loop vector operation is outside Owned Bounded Vec v1",
                            ));
                        }
                        pending.extend(args[1..].iter().rev());
                        continue;
                    }
                    if let Some(operation) = crate::str_ops::by_id(callee.as_str()) {
                        if args.len() != operation.arity()
                            || expression.ty != operation.return_type()
                            || expression.ownership != OwnershipMode::Value
                            || args.iter().any(|argument| {
                                argument.ty != ResolvedType::Str
                                    || argument.ownership != OwnershipMode::Borrow
                                    || !matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
                            })
                        {
                            return Err(hir_error("loop text reads require exact immutable named borrowed-str inputs"));
                        }
                        // Full expression replay authenticates each binding and
                        // immutable borrowed-str origin; these closed readers
                        // return only a scalar and cannot retain their inputs.
                        continue;
                    }
                    if let Some(operation) = crate::string_ops::by_id(callee.as_str()) {
                        if args.len() != operation.arity()
                            || expression.ty != operation.return_type()
                            || args
                                .iter()
                                .zip(operation.param_types())
                                .any(|(argument, ty)| argument.ty != *ty)
                        {
                            return Err(hir_error(
                                "while loop string operation is outside Owned String Loops v1",
                            ));
                        }
                        if operation == crate::string_ops::StringOp::FromStr {
                            // The conversion only copies an existing view; full
                            // expression replay authenticates its immutable loan.
                            // It cannot retain the borrowed input across iterations.
                            if args[0].ownership != OwnershipMode::Borrow
                                || !matches!(&args[0].kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
                            {
                                return Err(hir_error(
                                    "loop string_from_str requires a named borrowed-str input",
                                ));
                            }
                            continue;
                        }
                        pending.extend(args.iter().rev());
                        continue;
                    }
                    if let Some(operation) = crate::byte_ops::by_id(callee.as_str()) {
                        owned_buffer::require_admitted_while_operation(
                            self, expression, callee, operation, args,
                        )?;
                        // Owned-buffer admission authenticates the moved owner
                        // and, for the one-or-five store, its distinct borrowed
                        // source. The generic scalar-place rule must not visit
                        // those two places; every scalar operand is replayed.
                        pending.extend(
                            args.iter()
                                .enumerate()
                                .rev()
                                .filter(|(index, _)| {
                                    *index != 0
                                        && !(matches!(
                                            operation,
                                            crate::byte_ops::ByteOp::Set1Or5
                                                | crate::byte_ops::ByteOp::Set1Or6Or48
                                        ) && *index == 3)
                                })
                                .map(|(_, argument)| argument),
                        );
                        continue;
                    }
                    let target =
                        self.program
                            .resolve_call_target(callee, None)
                            .ok_or_else(|| {
                                hir_error(format!("while loop call `{callee}` is not indexed"))
                            })?;
                    // A loop may call an effect-free bounded-read helper. A
                    // borrowed slice cannot escape because the result remains
                    // scalar; transitive allocation/call cycles are still
                    // rejected by the byte-capacity analysis after HIR replay.
                    let scalar_signature = crate::loop_calls::effects_admitted(&target.effects)
                        && crate::loop_calls::resolved_result_admitted(&target.return_type)
                        && target.params.iter().zip(args).all(|(param, argument)| {
                            crate::loop_calls::resolved_param_admitted(param.ownership, &param.ty)
                                || (param.ownership == OwnershipMode::Own && param.ty == ResolvedType::Bytes
                                    && argument.ownership == OwnershipMode::Own && argument.ty == ResolvedType::Bytes
                                    && owned_item.is_some_and(|item| matches!(&argument.kind, ResolvedExprKind::Place(place) if place.root == item.id && place.projections.is_empty())))
                        });
                    if !scalar_signature {
                        return Err(hir_error(format!(
                            "while loop call `{callee}` is not a scalar-value function"
                        )));
                    }
                    for (argument, parameter) in args.iter().zip(&target.params).rev() {
                        if parameter.ty == ResolvedType::SliceU8 {
                            let ResolvedExprKind::Place(place) = &argument.kind else {
                                return Err(hir_error(
                                    "while loop bounded-read call requires named slice aliases",
                                ));
                            };
                            if !place.projections.is_empty()
                                || (!self.byte_slice_aliases.contains_key(&place.root)
                                    && self
                                        .program
                                        .declarations
                                        .byte_slice_provenance(&place.root)
                                        .is_none())
                            {
                                return Err(hir_error(
                                    format!(
                                        "while loop bounded-read call slice `{}` lacks authenticated provenance",
                                        place.root
                                    ),
                                ));
                            }
                        } else if parameter.ownership != OwnershipMode::Own
                            || parameter.ty == ResolvedType::String
                        {
                            // Loop Calls v1: a consumed String argument is an
                            // ordinary body operand; replay authenticates the
                            // move and the region releases what it leaves.
                            pending.push(argument);
                        }
                    }
                }
                ResolvedExprKind::NativeRustImportCall(_) => {
                    return Err(hir_error(
                        "while loops cannot contain native Rust import calls",
                    ));
                }
                ResolvedExprKind::Upcast { .. } => {
                    return Err(hir_error("while loops cannot contain inheritance upcasts"));
                }
                ResolvedExprKind::Project { .. } => {
                    return Err(hir_error("while loops cannot project record fields"));
                }
                ResolvedExprKind::ConstructRecord { .. } => {
                    return Err(hir_error("while loops cannot construct records"));
                }
                ResolvedExprKind::ConstructVariant { .. } => {
                    return Err(hir_error("while loops cannot construct variants"));
                }
                ResolvedExprKind::UpdateRecord { .. } => {
                    return Err(hir_error("while loops cannot update records"));
                }
                // Owned String Loops v2: a match over a Copy scalar or a
                // Copy-payload variant binds only Copy values, so it is
                // cleanup-inert; its arms may yield a `string`, which joins
                // like any branch result in the body region.
                ResolvedExprKind::Match {
                    scrutinee, arms, ..
                } => {
                    if !crate::loop_calls::resolved_match_scrutinee_admitted(
                        &self.program.declarations,
                        &scrutinee.ty,
                    ) || !crate::loop_calls::resolved_result_admitted(&expression.ty)
                    {
                        return Err(hir_error(
                            "while loop match is outside the Copy-scrutinee profile",
                        ));
                    }
                    for arm in arms.iter().rev() {
                        pending.push(&arm.value);
                        if let Some(guard) = &arm.guard {
                            pending.push(guard);
                        }
                    }
                    pending.push(scrutinee);
                }
                ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => {
                    return Err(hir_error(
                        "while loops cannot contain postfix `?` propagation",
                    ));
                }
                // Resumable Effects control profile (issue #296): a direct
                // statement-value `yield` may suspend inside a loop body; its
                // request is an ordinary scalar operand. Placement is owned by
                // `parser::yields` and re-checked by the control lowering.
                ResolvedExprKind::Yield { request } => pending.push(request),
            }
        }
        Ok(())
    }

    pub(super) fn validate_loop_pair(
        &self,
        condition: &ResolvedExpr,
        body: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        if let Some(protocol) = crate::hir::iterator_loop::recognize(condition, body) {
            if !crate::iterator_ops::step_shape(&self.program.declarations, &protocol.step.ty) {
                return Err(hir_error("iterator loop has unauthenticated Step metadata"));
            }
            self.validate_iterator_body(protocol.authored_body, protocol.owned_item)
        } else {
            // A condition creates no String owner. Exact named length reads
            // inspect a carrier; ordinary HIR replay authenticates the place.
            let reads = crate::string_ops::conditions::condition_reads(condition);
            let mut pending = vec![condition];
            while let Some(expression) = pending.pop() {
                if expression.ty == ResolvedType::String && !reads.contains(&expression.id) {
                    return Err(hir_error(
                        "while loop condition creates an owned String outside the body region",
                    ));
                }
                pending.extend(crate::interpreter::trace_child_expressions(expression));
            }
            self.validate_while_admission(condition)?;
            self.validate_while_admission(body)
        }
    }
}
/// Owned String Loops v1: `text = string_concat(text, …)` consumed the
/// unique current generation of `text` as its first argument; the assignment
/// publishes the next generation into the same binding.
pub(super) fn reopen_string(
    scope: &mut BTreeMap<ValueId, ValidationBinding>,
    id: &ValueId,
) -> Result<(), Diagnostic> {
    let target = scope
        .get_mut(id)
        .ok_or_else(|| hir_error("string append target is missing"))?;
    if target.availability != Availability::Moved
        || !target.active_loans.is_empty()
        || target.ownership != OwnershipMode::Own
        || !matches!(target.ty, ResolvedType::String | ResolvedType::StringMap)
    {
        return Err(hir_error(
            "string append did not consume its unique String owner",
        ));
    }
    target.availability = Availability::Available;
    target.moved_places.clear();
    target.definitely_partial.clear();
    Ok(())
}

pub(super) fn reopen_step(
    scope: &mut BTreeMap<ValueId, ValidationBinding>,
    id: &ValueId,
) -> Result<(), Diagnostic> {
    let target = scope
        .get_mut(id)
        .ok_or_else(|| hir_error("iterator replacement target is missing"))?;
    if target.availability != Availability::Moved
        || !target.active_loans.is_empty()
        || target.ownership != OwnershipMode::Own
        || !crate::iterator_ops::is_step(&target.ty)
    {
        return Err(hir_error(
            "iterator replacement did not consume its unique Step owner",
        ));
    }
    target.availability = Availability::Available;
    target.moved_places.clear();
    target.definitely_partial.clear();
    Ok(())
}
