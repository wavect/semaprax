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
    fn is_owned_iterator_record_item(
        &self,
        expression: &ResolvedExpr,
        owned_item: Option<&ResolvedBinding>,
    ) -> bool {
        owned_item.is_some_and(|item| {
            item.ownership == OwnershipMode::Own
                && expression.ownership == OwnershipMode::Own
                && expression.ty == item.ty
                && crate::hir::owned_record_collection::is_admitted_owned_record_collection_element(
                    &self.program.declarations,
                    &item.ty,
                )
                && matches!(&expression.kind, ResolvedExprKind::Place(place)
                    if place.root == item.id && place.projections.is_empty())
        })
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
                        ) || crate::map_ops::is_collection(&expression.ty) || (matches!(expression.ty, ResolvedType::Nominal { .. })
                            && crate::loop_calls::resolved_match_scrutinee_admitted(
                                &self.program.declarations,
                                &expression.ty,
                            )))
                            && place.projections.is_empty();
                    let named_str = expression.ty == ResolvedType::Str
                        && expression.ownership == OwnershipMode::Borrow
                        && place.projections.is_empty();
                    let cursor_borrow = expression.ownership == OwnershipMode::Borrow
                        && place.projections.is_empty()
                        && crate::hir::iterator_loop::is_owner_renewal_record(
                            &self.program.declarations,
                            &expression.ty,
                        );
                    if !whole_string
                        && !named_str
                        && !cursor_borrow
                        && !self.is_owned_iterator_record_item(expression, owned_item)
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
                            && expression.ty == ResolvedType::SliceU8)
                        || (operation.as_str() == crate::stdin_stream_ops::CHUNK_ID
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
                        let renewal = if let ResolvedStatement::Assign {
                            binding,
                            field: None,
                            value,
                            ..
                        } = statement
                        {
                            crate::hir::iterator_loop::is_record_owner_renewal(
                                self.program,
                                binding,
                                value,
                            )
                            .then_some(value)
                        } else {
                            None
                        };
                        if let Some(value) = renewal {
                            let ResolvedExprKind::Call { callee, args, .. } = &value.kind else {
                                unreachable!("record renewal admission requires a call")
                            };
                            let target = self
                                .program
                                .resolve_call_target(callee, None)
                                .ok_or_else(|| hir_error("record renewal target disappeared"))?;
                            // The consumed owner and exact whole-record borrows
                            // were authenticated by `is_record_owner_renewal`.
                            // Copy arguments remain ordinary loop expressions;
                            // replay them so a pure renewal wrapper cannot hide
                            // a disallowed nested/effectful computation.
                            pending.extend(target.params.iter().zip(args).rev().filter_map(
                                |(parameter, argument)| {
                                    (parameter.ownership == OwnershipMode::Value)
                                        .then_some(argument)
                                },
                            ));
                            continue;
                        }
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
                    if callee.as_str() == crate::stdin_stream_ops::EOF_ID {
                        if instance.is_some()
                            || !type_arguments.is_empty()
                            || expression.ty != ResolvedType::Bool
                            || !matches!(args.as_slice(), [argument] if crate::stdin_stream_ops::is_reader(&argument.ty) && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()))
                        {
                            return Err(hir_error("invalid streaming EOF inspection in while"));
                        }
                        continue;
                    }
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
                            || args.iter().zip(operation.param_types()).any(|(argument, ty)| {
                                argument.ty != *ty || if *ty == ResolvedType::Str {
                                    argument.ownership != OwnershipMode::Borrow
                                    || !matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
                                } else { argument.ownership != OwnershipMode::Value }
                            })
                        {
                            return Err(hir_error("loop text reads require exact immutable named borrowed-str inputs"));
                        }
                        // Full expression replay authenticates each binding and
                        // immutable borrowed-str origin; these closed readers
                        // return only Copy data and cannot retain their inputs.
                        pending.extend(
                            args.iter()
                                .filter(|argument| argument.ty != ResolvedType::Str),
                        );
                        continue;
                    }
                    if let Some(operation)=crate::map_ops::by_id(callee.as_str()) {
                        let (params,ty)=operation.resolved_signature(type_arguments).ok_or_else(||hir_error("loop collection signature is invalid"))?;
                        if args.len()!=params.len()||expression.ty!=ty||args.iter().zip(params).any(|(arg,param)|arg.ty!=param.ty){return Err(hir_error("loop collection operands are invalid"));}
                        pending.extend(args);continue;
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
                    if crate::stdin_stream_ops::resolved_forward_signature(target) {
                        if !crate::stdin_stream_ops::hir_reopen(
                            expression,
                            &match args.as_slice() {
                                [argument] => match &argument.kind {
                                    ResolvedExprKind::Place(place) => place.root.clone(),
                                    _ => {
                                        return Err(hir_error(
                                            "forwarding helper requires a named reader",
                                        ))
                                    }
                                },
                                _ => {
                                    return Err(hir_error("forwarding helper requires one reader"))
                                }
                            },
                        ) {
                            return Err(hir_error("invalid reader forwarding call"));
                        }
                        continue;
                    }
                    let scalar_signature = crate::loop_calls::effects_admitted(&target.effects)
                        && crate::loop_calls::resolved_result_admitted(&self.program.declarations, &target.return_type)
                        && target.params.iter().zip(args).all(|(param, argument)| {
                            crate::loop_calls::resolved_param_admitted(&self.program.declarations, param.ownership, &param.ty)
                                || (param.ownership == OwnershipMode::Borrow
                                    && crate::hir::iterator_loop::is_owner_renewal_record(
                                        &self.program.declarations,
                                        &param.ty,
                                    ))
                                || (param.ownership == OwnershipMode::Own
                                    && argument.ownership == OwnershipMode::Own
                                    && param.ty == argument.ty
                                    && owned_item.is_some_and(|item| item.ty == param.ty
                                        && matches!(&argument.kind, ResolvedExprKind::Place(place)
                                            if place.root == item.id && place.projections.is_empty())))
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
                        } else if parameter.ownership == OwnershipMode::Borrow
                            && crate::hir::iterator_loop::is_owner_renewal_record(
                                &self.program.declarations,
                                &parameter.ty,
                            )
                        {
                            if !matches!(&argument.kind, ResolvedExprKind::Place(place)
                                if place.projections.is_empty()
                                    && matches!(argument.ownership,
                                        OwnershipMode::Own | OwnershipMode::Borrow))
                            {
                                return Err(hir_error(
                                    "while loop owner observer requires a whole named cursor",
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
                ResolvedExprKind::ConstructVariant { fields, .. } => {
                    if expression.ownership != OwnershipMode::Value
                        || !crate::variant_guards::copy_variant(
                            &self.program.declarations,
                            &expression.ty,
                        )
                    {
                        return Err(hir_error(
                            "while loop variant construction requires Copy scalar payloads",
                        ));
                    }
                    pending.extend(fields.iter().rev().map(|field| &field.value));
                }
                ResolvedExprKind::UpdateRecord { .. } => {
                    return Err(hir_error("while loops cannot update records"));
                }
                // Owned String Loops v2: a match over a Copy scalar or a
                // Copy-payload variant binds only Copy values, so it is
                // cleanup-inert; its arms may yield a `string`, which joins
                // like any branch result in the body region.
                // Consuming record traversal also admits `match own` on the
                // exact protocol item. Ordinary replay transfers its Bytes
                // leaves into the selected arm and settles them in that region.
                ResolvedExprKind::Match {
                    mode,
                    scrutinee,
                    arms,
                    ..
                } => {
                    let owned_record = *mode == ResolvedMatchMode::Own
                        && self.is_owned_iterator_record_item(scrutinee, owned_item);
                    if !(owned_record
                        || crate::loop_calls::resolved_match_scrutinee_admitted(
                            &self.program.declarations,
                            &scrutinee.ty,
                        ))
                        || !crate::loop_calls::resolved_result_admitted(
                            &self.program.declarations,
                            &expression.ty,
                        )
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
        || !(matches!(target.ty, ResolvedType::String | ResolvedType::StringMap)||crate::map_ops::is_collection(&target.ty))
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
