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
        enum Item<'a> {
            Expression(&'a ResolvedExpr),
            IndexedMatchNext {
                expression: &'a ResolvedExpr,
                scrutinee: &'a ResolvedExpr,
                arms: &'a [ResolvedMatchArm],
                next: usize,
                some_seen: bool,
                none_seen: bool,
            },
        }

        let mut pending = vec![Item::Expression(expression)];
        while let Some(item) = pending.pop() {
            let expression = match item {
                Item::Expression(expression) => expression,
                Item::IndexedMatchNext {
                    expression,
                    scrutinee,
                    arms,
                    next,
                    mut some_seen,
                    mut none_seen,
                } => {
                    let Some(arm) = arms.get(next) else {
                        if !some_seen || !none_seen {
                            return Err(hir_error(
                                "while loop byte match is not exhaustive over Some and None",
                            ));
                        }
                        pending.push(Item::Expression(scrutinee));
                        continue;
                    };
                    self.validate_indexed_byte_option_match_arm(
                        expression,
                        arm,
                        &mut some_seen,
                        &mut none_seen,
                    )?;
                    pending.push(Item::IndexedMatchNext {
                        expression,
                        scrutinee,
                        arms,
                        next: next + 1,
                        some_seen,
                        none_seen,
                    });
                    pending.push(Item::Expression(&arm.value));
                    continue;
                }
            };
            match &expression.kind {
                ResolvedExprKind::Closure { captures, .. } => {
                    pending.extend(
                        captures
                            .iter()
                            .rev()
                            .map(|capture| Item::Expression(&capture.value)),
                    );
                }
                ResolvedExprKind::FunctionReference { .. } | ResolvedExprKind::Invoke { .. } => {
                    pending.extend(
                        self.while_callable_arguments(expression)?
                            .into_iter()
                            .rev()
                            .map(Item::Expression),
                    );
                }
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_) => {}
                ResolvedExprKind::Place(_) => {
                    if !crate::hir::is_scalar_resolved_type(&expression.ty)
                        || expression.ownership != OwnershipMode::Value
                    {
                        return Err(hir_error(
                            "while loop places must be Copy scalars outside an indexed byte read",
                        ));
                    }
                }
                ResolvedExprKind::String(_) => {
                    return Err(hir_error("while loops cannot contain string literals"));
                }
                ResolvedExprKind::ArrayU8(_) | ResolvedExprKind::RepeatArrayU8 { .. } => {
                    return Err(hir_error("while loops cannot contain fixed-array literals"));
                }
                ResolvedExprKind::BorrowPlace { .. } => {
                    return Err(hir_error("while loops cannot construct byte views"));
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
                    pending.push(Item::Expression(end));
                    pending.push(Item::Expression(start));
                }
                ResolvedExprKind::HostCommandCall(call) => {
                    pending.extend(
                        self.while_host_command_scalar_arguments(expression, call)?
                            .into_iter()
                            .rev()
                            .map(Item::Expression),
                    );
                }
                ResolvedExprKind::Unary { value, .. } => pending.push(Item::Expression(value)),
                ResolvedExprKind::Binary { left, right, .. } => {
                    pending.push(Item::Expression(right));
                    pending.push(Item::Expression(left));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push(Item::Expression(else_branch));
                    pending.push(Item::Expression(then_branch));
                    pending.push(Item::Expression(condition));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    pending.push(Item::Expression(tail));
                    for statement in statements.iter().rev() {
                        for index in (0..statement.child_count()).rev() {
                            let child = statement
                                .child(index)
                                .ok_or_else(|| hir_error("while statement child is missing"))?;
                            pending.push(Item::Expression(child));
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
                        pending.extend(args[1..].iter().rev().map(Item::Expression));
                        continue;
                    }
                    if let Some(operation) = crate::str_ops::by_id(callee.as_str()) {
                        if !crate::environment_ops::program_uses_environment(self.program)
                            || args.len() != operation.arity()
                            || expression.ty != operation.return_type()
                            || expression.ownership != OwnershipMode::Value
                            || args.iter().any(|argument| {
                                argument.ty != ResolvedType::Str
                                    || argument.ownership != OwnershipMode::Borrow
                                    || !matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
                            })
                        {
                            return Err(hir_error("environment loop text reads require exact immutable named borrowed-str inputs"));
                        }
                        // Full expression replay authenticates each binding and
                        // immutable borrowed-str origin; these closed readers
                        // return only a scalar and cannot retain their inputs.
                        continue;
                    }
                    if let Some(operation) = crate::byte_ops::by_id(callee.as_str()) {
                        owned_buffer::require_admitted_while_operation(
                            self, expression, callee, operation, args,
                        )?;
                        pending.extend(args[1..].iter().rev().map(Item::Expression));
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
                    let scalar_signature = target.effects.is_empty()
                        && crate::hir::is_scalar_resolved_type(&target.return_type)
                        && target.params.iter().zip(args).all(|(param, argument)| {
                            (param.ownership == OwnershipMode::Value
                                && crate::hir::is_scalar_resolved_type(&param.ty))
                                || (param.ownership == OwnershipMode::Borrow
                                    && param.ty == ResolvedType::SliceU8)
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
                        } else if parameter.ownership != OwnershipMode::Own {
                            pending.push(Item::Expression(argument));
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
                ResolvedExprKind::Match {
                    scrutinee, arms, ..
                } => {
                    self.validate_indexed_byte_option_match_admission(expression, scrutinee, arms)?;
                    pending.push(Item::IndexedMatchNext {
                        expression,
                        scrutinee,
                        arms,
                        next: 0,
                        some_seen: false,
                        none_seen: false,
                    });
                }
                ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => {
                    return Err(hir_error(
                        "while loops cannot contain postfix `?` propagation",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Authenticate the one aggregate-shaped expression admitted by Indexed
    /// Byte Loop v2. The match is cleanup-inert: its scrutinee is exactly the
    /// compiler byte getter, its case inventory is exactly compiler-owned
    /// `Option<u8>::Some/None`, and every arm still belongs to the existing
    /// Copy-scalar while profile.
    fn validate_indexed_byte_option_match_admission(
        &self,
        expression: &ResolvedExpr,
        scrutinee: &ResolvedExpr,
        arms: &[ResolvedMatchArm],
    ) -> Result<(), Diagnostic> {
        if !crate::hir::is_scalar_resolved_type(&expression.ty)
            || expression.ownership != OwnershipMode::Value
        {
            return Err(hir_error(
                "while loop byte match must produce a Copy scalar value",
            ));
        }
        let ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            args,
        } = &scrutinee.kind
        else {
            return Err(hir_error(
                "while loops cannot match an aggregate other than compiler-owned byte_get",
            ));
        };
        let operation = crate::byte_ops::by_id(callee.as_str());
        if operation != Some(crate::byte_ops::ByteOp::Get)
            || instance.is_some()
            || !type_arguments.is_empty()
            || args.len() != crate::byte_ops::ByteOp::Get.arity()
            || args.iter().enumerate().any(|(index, argument)| {
                !crate::byte_ops::ByteOp::Get.accepts_resolved(index, &argument.ty)
            })
            || scrutinee.ty != crate::byte_ops::ByteOp::Get.return_type()
            || !scrutinee.ty.is_compiler_byte_option()
            || scrutinee.ownership != OwnershipMode::Value
        {
            return Err(hir_error(
                "while loop byte match scrutinee is not the exact compiler-owned byte_get result",
            ));
        }
        for id in [
            crate::prelude::OPTION_ID,
            crate::prelude::OPTION_SOME_ID,
            crate::prelude::OPTION_SOME_VALUE_ID,
            crate::prelude::OPTION_NONE_ID,
        ] {
            let id = DeclarationId::new(id);
            if self
                .program
                .declarations
                .declaration(&id)
                .is_none_or(|declaration| {
                    declaration.identity_origin != IdentityOrigin::CompilerOwned
                })
            {
                return Err(hir_error(format!(
                    "while loop byte match identity `{id}` is not compiler-owned"
                )));
            }
        }
        if arms.len() != 2 {
            return Err(hir_error(
                "while loop byte match must contain exactly Some and None arms",
            ));
        }

        Ok(())
    }

    fn validate_indexed_byte_option_match_arm(
        &self,
        expression: &ResolvedExpr,
        arm: &ResolvedMatchArm,
        some_seen: &mut bool,
        none_seen: &mut bool,
    ) -> Result<(), Diagnostic> {
        if arm.guard.is_some()
            || arm.value.ty != expression.ty
            || arm.value.ownership != OwnershipMode::Value
            || !crate::hir::is_scalar_resolved_type(&arm.value.ty)
        {
            return Err(hir_error(
                "while loop byte match arms must be guard-free Copy-scalar expressions",
            ));
        }
        let ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } = &arm.pattern
        else {
            return Err(hir_error(
                "while loop byte match contains a non-Option case pattern",
            ));
        };
        if variant.as_str() != crate::prelude::OPTION_ID {
            return Err(hir_error(
                "while loop byte match pattern has a foreign variant identity",
            ));
        }
        match case.as_str() {
            crate::prelude::OPTION_SOME_ID
                if !*some_seen
                    && fields.len() == 1
                    && fields[0].field.as_str() == crate::prelude::OPTION_SOME_VALUE_ID
                    && fields[0].binding.ty == ResolvedType::U8
                    && fields[0].binding.ownership == OwnershipMode::Value =>
            {
                *some_seen = true;
            }
            crate::prelude::OPTION_NONE_ID if !*none_seen && fields.is_empty() => {
                *none_seen = true;
            }
            _ => {
                return Err(hir_error(
                    "while loop byte match is not the exact exhaustive Some/None inventory",
                ));
            }
        }
        Ok(())
    }
}

impl HirValidator<'_> {
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
