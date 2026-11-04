//! Mechanical separation of recursively retained expression families.
//! Charging stays in Evaluator::evaluate; these borrowed helpers introduce no
//! evaluation strategy, carriers, allocation, fuel, or cleanup reorder.
use super::*;

impl Evaluator<'_> {
    pub(super) fn evaluate_binary_expression(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        match &expression.kind {
            ResolvedExprKind::Binary {
                op, left, right, ..
            } => {
                let lhs = self.evaluate(left, environment, depth)?;
                match op {
                    BinaryOp::And => match lhs {
                        Value::Bool(false) => Ok(Value::Bool(false)),
                        Value::Bool(true) => match self.evaluate(right, environment, depth)? {
                            Value::Bool(rhs) => Ok(Value::Bool(rhs)),
                            _ => Err(Flow::Guard("ill-typed conjunction operand")),
                        },
                        _ => Err(Flow::Guard("ill-typed conjunction operand")),
                    },
                    BinaryOp::Or => match lhs {
                        Value::Bool(true) => Ok(Value::Bool(true)),
                        Value::Bool(false) => match self.evaluate(right, environment, depth)? {
                            Value::Bool(rhs) => Ok(Value::Bool(rhs)),
                            _ => Err(Flow::Guard("ill-typed disjunction operand")),
                        },
                        _ => Err(Flow::Guard("ill-typed disjunction operand")),
                    },
                    _ => {
                        let rhs = self.evaluate(right, environment, depth)?;
                        self.combine_completed(expression, lhs, rhs, environment)
                    }
                }
            }
            _ => unreachable!("binary expression dispatched by exact kind"),
        }
    }

    pub(super) fn evaluate_conditional_expression(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        match &expression.kind {
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let branch = match self.evaluate(condition, environment, depth)? {
                    Value::Bool(true) => then_branch,
                    Value::Bool(false) => else_branch,
                    _ => return Err(Flow::Guard("non-boolean condition")),
                };
                self.evaluate(branch, environment, depth)
            }
            _ => unreachable!("conditional expression dispatched by exact kind"),
        }
    }

    pub(super) fn evaluate_call_expression(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        match &expression.kind {
            ResolvedExprKind::Call {
                callee,
                instance,
                type_arguments,
                args,
            } => {
                let vec_intrinsic = owned_vec::is_intrinsic_call(callee, instance, type_arguments);
                let box_intrinsic = owned_box::is_intrinsic_call(callee, instance, type_arguments);
                if !vec_intrinsic
                    && !(instance.is_none()
                        && crate::iterator_ops::by_id(callee.as_str()).is_some())
                    && !(instance.is_none() && crate::map_ops::by_id(callee.as_str()).is_some())
                    && !box_intrinsic
                    && instance.is_some() != !type_arguments.is_empty()
                {
                    return Err(Flow::Guard("generic call identity is incomplete"));
                }
                if let Some(op) = crate::map_ops::by_id(callee.as_str()) {
                    return self.evaluate_typed_map(op, type_arguments, args, environment, depth);
                }
                if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                    // Compiler-owned string operations evaluate in place;
                    // their byte semantics match the native and Wasm backends.
                    return self.evaluate_string_op(op, args, environment, depth);
                }
                if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                    self.charge()?;
                    let mut values = Vec::with_capacity(args.len());
                    for argument in args {
                        values.push(self.evaluate(argument, environment, depth)?);
                    }
                    return match op {
                        crate::str_ops::StrOp::ByteAt => match values.as_slice() {
                            [Value::BorrowedStr(text), Value::Usize(index)] => Ok(Value::OptionU8(
                                usize::try_from(*index)
                                    .ok()
                                    .and_then(|index| text.bytes.as_ref().get(index))
                                    .copied(),
                            )),
                            _ => Err(Flow::Guard("ill-typed borrowed string byte read")),
                        },
                        crate::str_ops::StrOp::LenBytes => borrowed_text(&values[0])
                            .map(|value| Value::Int(value.len() as i64))
                            .ok_or(Flow::Guard("ill-typed borrowed string operand")),
                        crate::str_ops::StrOp::IsEmpty => borrowed_text(&values[0])
                            .map(|value| Value::Bool(value.is_empty()))
                            .ok_or(Flow::Guard("ill-typed borrowed string operand")),
                        crate::str_ops::StrOp::StartsWith => {
                            match (borrowed_text(&values[0]), borrowed_text(&values[1])) {
                                (Some(value), Some(prefix)) => {
                                    Ok(Value::Bool(value.starts_with(prefix)))
                                }
                                _ => Err(Flow::Guard("ill-typed borrowed string operand")),
                            }
                        }
                        crate::str_ops::StrOp::Contains => {
                            match (borrowed_text(&values[0]), borrowed_text(&values[1])) {
                                (Some(value), Some(needle)) => {
                                    Ok(Value::Bool(crate::str_ops::contains(value, needle)))
                                }
                                _ => Err(Flow::Guard("ill-typed borrowed string operand")),
                            }
                        }
                    };
                }
                if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                    byte_partition::refuse_ordinary_operation(op)?;
                    self.charge()?;
                    let mut values = Vec::with_capacity(args.len());
                    for argument in args {
                        values.push(self.evaluate(argument, environment, depth)?);
                    }
                    if op.is_owned_buffer_chain() {
                        return self.evaluate_owned_buffer_operation(expression, op, values);
                    }
                    return match (op, values.as_slice()) {
                        (crate::byte_ops::ByteOp::Len, [Value::BorrowedSlice(value)]) => Ok(
                            Value::Usize(value.read(&mut self.fused, |bytes| bytes.len() as u64)?),
                        ),
                        (
                            crate::byte_ops::ByteOp::Get,
                            [Value::BorrowedSlice(value), Value::Usize(index)],
                        ) => {
                            let byte = value.read(&mut self.fused, |bytes| {
                                usize::try_from(*index)
                                    .ok()
                                    .and_then(|index| bytes.get(index))
                                    .copied()
                            })?;
                            Ok(Value::OptionU8(byte))
                        }
                        (crate::byte_ops::ByteOp::ZeroPrefix, [Value::BorrowedSlice(value)]) => {
                            let count = value.read(&mut self.fused, |bytes| {
                                bytes.iter().take_while(|byte| **byte == 0).count()
                            })?;
                            Ok(Value::Usize(count as u64))
                        }
                        (
                            crate::byte_ops::ByteOp::PrefixClass,
                            [Value::BorrowedSlice(value), Value::Usize(index), Value::Usize(limit), Value::Uint8(low), Value::Uint8(high), Value::Uint8(stop1), Value::Uint8(stop2)],
                        ) => {
                            let count = value.read(&mut self.fused, |bytes| {
                                let Ok(index) = usize::try_from(*index) else {
                                    return 0;
                                };
                                let Some(rest) = bytes.get(index..) else {
                                    return 0;
                                };
                                let limit = usize::try_from(*limit)
                                    .unwrap_or(usize::MAX)
                                    .min(rest.len());
                                rest[..limit]
                                    .iter()
                                    .take_while(|byte| {
                                        **byte >= *low
                                            && **byte <= *high
                                            && **byte != *stop1
                                            && **byte != *stop2
                                    })
                                    .count()
                            })?;
                            Ok(Value::Usize(count as u64))
                        }
                        (
                            crate::byte_ops::ByteOp::ReadLe,
                            [Value::BorrowedSlice(value), Value::Usize(index), Value::Usize(width)],
                        ) => {
                            let decoded = value.read(&mut self.fused, |bytes| {
                                let index = usize::try_from(*index).ok()?;
                                let width = usize::try_from(*width).ok()?;
                                if !(1..=8).contains(&width)
                                    || index > bytes.len()
                                    || width > bytes.len() - index
                                {
                                    return None;
                                }
                                let mut word = [0u8; 8];
                                word[..width].copy_from_slice(&bytes[index..index + width]);
                                Some(u64::from_le_bytes(word))
                            })?;
                            let (case, fields) = match decoded {
                                Some(value) => (
                                    hir::DeclarationId::new(crate::prelude::OPTION_SOME_ID),
                                    BTreeMap::from([(
                                        hir::DeclarationId::new(
                                            crate::prelude::OPTION_SOME_VALUE_ID,
                                        ),
                                        Value::Usize(value),
                                    )]),
                                ),
                                None => (
                                    hir::DeclarationId::new(crate::prelude::OPTION_NONE_ID),
                                    BTreeMap::new(),
                                ),
                            };
                            Ok(Value::Variant(Arc::new(OwnedVariantValue {
                                ty: expression.ty.clone(),
                                variant: hir::DeclarationId::compiler(
                                    hir::CompilerDeclaration::Option,
                                ),
                                case,
                                fields,
                            })))
                        }
                        (crate::byte_ops::ByteOp::Copy, [Value::BorrowedSlice(value)]) => {
                            self.copy_bytes(value).map(Value::Bytes)
                        }
                        (crate::byte_ops::ByteOp::Range, _) => Err(Flow::Guard(
                            "byte_range reached interpreter as an ordinary call",
                        )),
                        _ => Err(Flow::Guard("ill-typed borrowed byte operation operand")),
                    };
                }
                if let Some(op) = crate::iterator_ops::by_id(callee.as_str()) {
                    return self.evaluate_iterator_op(op, type_arguments, args, environment, depth);
                }
                if let Some(op) = crate::list_ops::by_id(callee.as_str()) {
                    return self.evaluate_list_op(op, type_arguments, args, environment, depth);
                }
                if let Some(slot) = crate::vec_ops::string_slot::inspect(expression) {
                    return self.evaluate_string_slot(expression, slot, environment, depth);
                }
                if let Some(op) = crate::vec_ops::by_id(callee.as_str()) {
                    return self.evaluate_vec_op(
                        expression,
                        op,
                        callee.as_str(),
                        type_arguments,
                        args,
                        environment,
                        depth,
                    );
                }
                if let Some(op) = crate::box_ops::by_id(callee.as_str()) {
                    return self.evaluate_box_op(op, type_arguments, args, environment, depth);
                }
                if crate::host_io_ops::by_id(callee.as_str()).is_some() {
                    self.charge()?;
                    let [argument] = args.as_slice() else {
                        return Err(Flow::Guard("invalid stdout_write arity"));
                    };
                    let value = self.evaluate(argument, environment, depth)?;
                    let Value::BorrowedSlice(value) = value else {
                        return Err(Flow::Guard("ill-typed stdout_write operand"));
                    };
                    let stderr_length = self.stderr_transcript.as_ref().map_or(0, Vec::len);
                    let transcript = self
                        .stdout_transcript
                        .as_mut()
                        .ok_or(Flow::Guard("stdout_write reached effect-free interpreter"))?;
                    let next = transcript
                        .len()
                        .checked_add(value.bytes().len())
                        .ok_or(Flow::Guard("stdout transcript length overflowed"))?;
                    if next.checked_add(stderr_length).is_none_or(|combined| {
                        combined > crate::host_io_ops::MAX_STDOUT_TRANSCRIPT_BYTES as usize
                    }) {
                        return Err(Flow::Guard("stdout transcript exceeds verified capacity"));
                    }
                    transcript.extend_from_slice(value.bytes());
                    return Ok(Value::Usize(value.bytes().len() as u64));
                }
                let execution = instance
                    .as_ref()
                    .map_or_else(|| callee.as_str(), hir::FunctionInstanceId::as_str);
                let Some(function) = self.admitted.get(execution) else {
                    return Err(Flow::Guard("call outside the admitted closure"));
                };
                let mut values: Vec<(ValueId, Value)> = Vec::with_capacity(args.len());
                for (param, argument) in function.params.iter().zip(args.iter()) {
                    let value = if let ResolvedExprKind::RowLoan { loan, args } = &argument.kind {
                        self.evaluate_row_loan(param, loan, args, environment, depth)?
                    } else if param.ownership == hir::OwnershipMode::Borrow
                        && param.ty == ResolvedType::Bytes
                    {
                        self.borrow_bytes_call_argument(environment, argument, depth)?
                    } else if param.ownership == hir::OwnershipMode::Borrow
                        && matches!(param.ty, ResolvedType::Nominal { .. })
                    {
                        // The resolved argument expression retains the owned
                        // place's type/mode. Parameter ownership is the call
                        // boundary authority: charge the argument node, then
                        // stage an alias without tombstoning its caller slot.
                        self.charge()?;
                        self.borrow_nominal_call_argument(environment, argument)?
                    } else {
                        self.evaluate(argument, environment, depth)?
                    };
                    values.push((param.id.clone(), value));
                }
                if values.len() != function.params.len() {
                    return Err(Flow::Guard("argument arity mismatch"));
                }
                self.call_frame(function, values, depth + 1)
            }
            _ => unreachable!("call expression dispatched by exact kind"),
        }
    }

    pub(super) fn evaluate_block_expression(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        match &expression.kind {
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                let fused_scope = self.fused.enter()?;
                let base = environment.len();
                let temporary_base = lexical_temporary::checkpoint(environment);
                let mut interrupted = None;
                'statements: for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            match self.resolve_let_value(&binding.id, value, environment, depth) {
                                Ok(value) => environment.push((binding.id.clone(), value)),
                                Err(flow) => {
                                    interrupted = Some(flow);
                                    break;
                                }
                            }
                        }
                        ResolvedStatement::CopyDiscard { value, .. } => {
                            if let Err(flow) = self.evaluate(value, environment, depth) {
                                interrupted = Some(flow);
                                break 'statements;
                            }
                        }
                        ResolvedStatement::Assign {
                            binding,
                            field,
                            value,
                            ..
                        } => {
                            let stored =
                                self.evaluate(value, environment, depth).and_then(|value| {
                                    match field {
                                        // Explicit Mutation v1 replaces the whole
                                        // scalar binding.
                                        None => match environment.get_mut(&binding.id) {
                                            Some(slot) => {
                                                *slot = value;
                                                Ok(())
                                            }
                                            None => {
                                                Err(Flow::Guard("assignment to an unknown binding"))
                                            }
                                        },
                                        Some(field) => self.store_record_field(
                                            environment,
                                            binding,
                                            field,
                                            value,
                                        ),
                                    }
                                });
                            if let Err(flow) = stored {
                                interrupted = Some(flow);
                                break;
                            }
                        }
                        ResolvedStatement::Unsafe { .. } => {
                            interrupted =
                                Some(Flow::Guard("unsafe boundary outside the admitted surface"));
                            break;
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        }
                        | ResolvedStatement::LexicalWhile {
                            condition, body, ..
                        } => {
                            if let Err(flow) =
                                self.evaluate_while(condition, body, environment, depth)
                            {
                                interrupted = Some(flow);
                                break 'statements;
                            }
                        }
                    }
                }
                let outcome = match interrupted {
                    Some(flow) => Err(flow),
                    None => self.evaluate(tail, environment, depth),
                };
                let retain_root = matches!(
                    &outcome,
                    Ok(_) | Err(Flow::Lexical(lexical_control::ExitFlow::Return { .. }))
                ) && environment
                    .control
                    .as_deref()
                    .is_some_and(|control| control.body == expression.id);
                if outcome.is_ok() && !retain_root {
                    self.settle_completed_string_scope(environment, temporary_base)?;
                }
                let fused_cleanup = self.fused.leave(fused_scope);
                if outcome.is_ok() {
                    fused_cleanup?;
                }
                if !retain_root {
                    let fused_cleanup = self.settle_fused_environment(environment, base);
                    if outcome.is_ok() {
                        fused_cleanup?;
                    }
                    environment.truncate(base);
                    lexical_temporary::truncate(environment, temporary_base);
                }
                outcome
            }
            _ => unreachable!("block expression dispatched by exact kind"),
        }
    }
}
