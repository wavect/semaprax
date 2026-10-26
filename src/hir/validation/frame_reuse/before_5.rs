// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_5 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $enter_phase_1<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            function: &FunctionExecutionId,
            expression: &'e ResolvedExpr,
            scope: scope_checkpoint::Scope,
            path: &'e str,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            allow_moves: bool,
            allowed_effects: Option<effect_keys::Allowed<'_>>,
        ) -> Result<(), Diagnostic> {
            match &expression.kind {
                ResolvedExprKind::Call {
                    callee,
                    type_arguments,
                    instance,
                    args,
                } => {
                    let owned_generic_intrinsic = box_intrinsic::is_call(callee, instance);
                    match instance {
                        None if !type_arguments.is_empty() && !owned_generic_intrinsic => {
                            return Err(hir_error(
                                "monomorphic resolved call carries generic type arguments",
                            ));
                        }
                        Some(actual)
                            if FunctionInstanceId::derive(callee, type_arguments) != *actual =>
                        {
                            return Err(hir_error(
                                "resolved call instance disagrees with its template and arguments",
                            ));
                        }
                        Some(_) if type_arguments.is_empty() => {
                            return Err(hir_error(
                                "generic resolved call has no concrete type arguments",
                            ));
                        }
                        None | Some(_) => {}
                    }
                    if !owned_generic_intrinsic
                        && !generic_instance_arguments_are_admitted(
                            validator.program,
                            callee,
                            type_arguments,
                        )
                    {
                        return Err(hir_error(
                                    "resolved call has a generic type argument outside the direct-scalar or owned-record relay profile",
                                ));
                    }
                    let mut intrinsic_parameters = None;
                    let mut borrowed_parameters = None;
                    let mut held_parameters = None;
                    let retained_program = validator.program;
                    let (params, return_type) = if let Some(signature) =
                        validator.intrinsic_signature(callee, type_arguments, instance, args)?
                    {
                        (signature.0, call_signature::ResultType::Owned(signature.1))
                    } else if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                        string_intrinsic::take_shape(expression, allow_moves)?;
                        if instance.is_some() || !type_arguments.is_empty() {
                            return Err(hir_error("string operation call must be monomorphic"));
                        }
                        if args.len() != op.arity() {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "string operation `{}` expects {} arguments but received {}",
                                op.name(),
                                op.arity(),
                                args.len()
                            )));
                        }
                        intrinsic_parameters = Some(CallParameters::String(op));
                        (
                            Vec::new(),
                            call_signature::ResultType::Owned(op.return_type()),
                        )
                    } else if let Some(op) = crate::str_ops::by_id(callee.as_str()) {
                        if instance.is_some() || !type_arguments.is_empty() {
                            return Err(hir_error(
                                "borrowed string operation call must be monomorphic",
                            ));
                        }
                        if args.len() != op.arity() {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "borrowed string operation `{}` expects {} arguments but received {}",
                                        op.name(),
                                        op.arity(),
                                        args.len()
                                    )));
                        }
                        intrinsic_parameters = Some(CallParameters::Str(op));
                        (
                            Vec::new(),
                            call_signature::ResultType::Owned(op.return_type()),
                        )
                    } else if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                        if instance.is_some() || !type_arguments.is_empty() {
                            return Err(hir_error("byte operation call must be monomorphic"));
                        }
                        if args.len() != op.arity() {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "byte operation `{}` expects {} arguments but received {}",
                                op.name(),
                                op.arity(),
                                args.len()
                            )));
                        }
                        owned_buffer::require_admitted_chain(
                            op,
                            args,
                            validator.buffer_reopen_sites.contains(&expression.id),
                        )?;
                        intrinsic_parameters = Some(CallParameters::Byte(op));
                        (
                            Vec::new(),
                            call_signature::ResultType::Owned(op.return_type()),
                        )
                    } else if let Some(op) = crate::host_io_ops::by_id(callee.as_str()) {
                        if instance.is_some() || !type_arguments.is_empty() {
                            return Err(hir_error("host I/O operation call must be monomorphic"));
                        }
                        if args.len() != op.arity() {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "host I/O operation `{}` expects {} arguments but received {}",
                                op.name(),
                                op.arity(),
                                args.len()
                            )));
                        }
                        match allowed_effects {
                            Some(allowed) if !allowed.contains(op.effect()) => {
                                return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                    "call to `{callee}` requires undeclared effect `{}`",
                                    op.effect()
                                )));
                            }
                            None => {
                                return Err(hir_error(
                                    "contract calls effectful host I/O operation",
                                ));
                            }
                            _ => {}
                        }
                        (
                            crate::host_io_ops::resolved_params(op),
                            call_signature::ResultType::Owned(op.return_type()),
                        )
                    } else {
                        let retained_target = if validator.executable_profiles.is_bound() {
                            held_parameters = validator
                                .executable_profiles
                                .call(callee, instance.as_ref());
                            held_parameters.map(|call| call.function())
                        } else {
                            retained_program.resolve_call_target(callee, instance.as_ref())
                        };
                        let (params, return_type, target_effects) =
                            if let Some(target) = retained_target {
                                borrowed_parameters = Some(target.params.as_slice());
                                (
                                    Vec::new(),
                                    call_signature::ResultType::Borrowed(&target.return_type),
                                    std::borrow::Cow::Borrowed(target.effects.as_slice()),
                                )
                            } else if validator.proof_generic_calls {
                                let (params, result, effects) = generic_template::proof_signature(
                                    validator.program,
                                    callee,
                                    instance.as_ref(),
                                    type_arguments,
                                )?;
                                (
                                    params,
                                    call_signature::ResultType::Owned(result),
                                    std::borrow::Cow::Owned(effects),
                                )
                            } else {
                                return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                    "resolved callee `{callee}` is not indexed"
                                )));
                            };
                        let parameter_count =
                            borrowed_parameters.map_or(params.len(), |parameters| parameters.len());
                        if args.len() != parameter_count {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "call to `{callee}` has {} arguments but expects {}",
                                args.len(),
                                parameter_count
                            )));
                        }
                        match allowed_effects {
                            Some(allowed) => {
                                for effect in target_effects.iter() {
                                    if !allowed.contains(effect) {
                                        return Err(scope_checkpoint::paths::diagnostic(
                                                    format_args!(
                                                        "call to `{callee}` requires undeclared effect `{effect}`"
                                                    ),
                                                ));
                                    }
                                }
                            }
                            None if !target_effects.is_empty() => {
                                return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                    "contract calls effectful function `{callee}`"
                                )));
                            }
                            None => {}
                        }
                        (params, return_type)
                    };
                    let params = if let Some(parameters) = borrowed_parameters {
                        held_parameters
                            .map_or(CallParameters::Borrowed(parameters), CallParameters::Held)
                    } else {
                        intrinsic_parameters.unwrap_or(CallParameters::Owned(params))
                    };
                    let return_ownership = if matches!(
                        callee.as_str(),
                        crate::vec_ops::string_slot::ID | crate::vec_ops::string_slot::BYTES_ID
                    ) {
                        OwnershipMode::Borrow
                    } else {
                        validator.expected_ownership(&return_type, OwnershipMode::Own)?
                    };
                    frames.push($Frame::CallNext {
                        expression,
                        args,
                        params,
                        return_type,
                        return_ownership,
                        index: 0,
                        scope,
                        path,
                    })?;
                }
                ResolvedExprKind::VecFieldRead { args, .. }
                | ResolvedExprKind::RowLoan { args, .. } => {
                    let (return_type, return_ownership, _bytes) =
                        validator.validate_scoped_vec_node(function, expression, &scope)?;
                    frames.push($Frame::CallNext {
                        expression,
                        args,
                        params: CallParameters::VecField(args),
                        return_type: call_signature::ResultType::Owned(return_type),
                        return_ownership,
                        index: 0,
                        scope,
                        path,
                    })?;
                }
                ResolvedExprKind::LiteralFormat { template, args } => {
                    validator.record_static_pool(template.len(), false)?;
                    validator.validate_literal_format_arguments(function, template, args)?;
                    frames.push($Frame::CallNext {
                        expression,
                        args,
                        params: CallParameters::LiteralFormat(args),
                        return_type: call_signature::ResultType::Owned(ResolvedType::String),
                        return_ownership: OwnershipMode::Own,
                        index: 0,
                        scope,
                        path,
                    })?;
                }
                ResolvedExprKind::HostCommandCall(call) => {
                    if call.expression != expression.id
                        || call.args.len() != crate::command_io_ops::arity(call.operation)
                    {
                        return Err(hir_error(
                            "host-command call has a non-canonical identity or shape",
                        ));
                    }
                    host_command::require_effects_view(call.operation, allowed_effects)?;
                    frames.push($Frame::CallNext {
                        expression,
                        args: &call.args,
                        params: CallParameters::Owned(crate::command_io_ops::resolved_params(
                            call.operation,
                        )),
                        return_type: call_signature::ResultType::Owned(
                            crate::command_io_ops::return_type(call.operation),
                        ),
                        return_ownership: crate::command_io_ops::result_ownership(call.operation),
                        index: 0,
                        scope,
                        path,
                    })?;
                }
                _ => unreachable!("expression was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
