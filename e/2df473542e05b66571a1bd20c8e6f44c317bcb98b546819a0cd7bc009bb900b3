// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_10 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_9<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            publication: &mut scope_checkpoint::Publication<'_>,
            allow_moves: bool,
        ) -> Result<(), Diagnostic> {
            match frame {
                $Frame::CallAfterArg {
                    expression,
                    args,
                    params,
                    return_type,
                    return_ownership,
                    index,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("call argument scope retained");
                    publication.publish(&scope)?;
                    let argument = &args[index];
                    let param = params.parameter(index);
                    validator.require_type(&argument.ty, param.ty, "call argument")?;
                    validator.validate_argument_ownership_view(argument, param)?;
                    let param = params.parameter(index);
                    validator.validate_borrowed_bytes_call_argument_fields(
                        expression,
                        argument,
                        (param.ty, param.ownership),
                        index,
                        &scope,
                    )?;
                    if *param.ty == ResolvedType::SliceU8 {
                        validator.validate_byte_slice_call_argument(argument, &scope)?;
                    }
                    if validator.is_owned_resource(param.ty, param.ownership)? {
                        if !allow_moves && !contract_owners::local(argument, &scope) {
                            let callee = match &expression.kind {
                                ResolvedExprKind::Call { callee, .. } => callee.as_str(),
                                ResolvedExprKind::LiteralFormat { .. } => crate::literal_format::ID,
                                ResolvedExprKind::VecFieldRead { .. } => crate::vec_field::ID,
                                _ => unreachable!(),
                            };
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "contract cannot transfer ownership to `{callee}`"
                            )));
                        }
                        validator.mark_value_sources_moved(argument, &mut scope)?;
                        publication.publish(&scope)?;
                    }
                    frames.push($Frame::CallNext {
                        expression,
                        args,
                        params,
                        return_type,
                        return_ownership,
                        index: index + 1,
                        scope,
                        path,
                    })?;
                }
                $Frame::NativeNext {
                    expression,
                    args,
                    params,
                    result,
                    index,
                    scope,
                    path,
                } => {
                    if index == args.len() {
                        let ownership = if let ResolvedExprKind::NativeRustImportCall(call) =
                            &expression.kind
                        {
                            call.result.ownership()
                        } else {
                            OwnershipMode::Value
                        };
                        validator.finish_expr(expression, &result, ownership)?;
                        scopes.push(scope)?;
                    } else {
                        frames.push($Frame::NativeAfterArg {
                            expression,
                            args,
                            params,
                            result,
                            index,
                            path,
                        })?;
                        frames.push($Frame::Enter {
                            expression: &args[index],
                            scope,
                            path: path_label::Label::child(
                                path,
                                ".native-rust-arg.{}",
                                [index, 0],
                            )?,
                        })?;
                    }
                }
                $Frame::NativeAfterArg {
                    expression,
                    args,
                    params,
                    result,
                    index,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("native argument scope retained");
                    publication.publish(&scope)?;
                    let argument = &args[index];
                    let parameter = &params[index];
                    validator.require_type(
                        &argument.ty,
                        &parameter.ty,
                        "native Rust import argument",
                    )?;
                    if argument.ownership != parameter.ownership
                        && !(parameter.ownership == OwnershipMode::Borrow
                            && argument.ownership == OwnershipMode::Own
                            && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()))
                    {
                        return Err(hir_error(
                            "native Rust import argument ownership disagrees with its declaration",
                        ));
                    }
                    if parameter.ownership == OwnershipMode::Own {
                        if !allow_moves {
                            return Err(hir_error(
                                "contract cannot transfer ownership to native Rust",
                            ));
                        }
                        validator.mark_value_sources_moved(argument, &mut scope)?;
                        publication.publish(&scope)?;
                    }
                    frames.push($Frame::NativeNext {
                        expression,
                        args,
                        params,
                        result,
                        index: index + 1,
                        scope,
                        path,
                    })?;
                }
                $Frame::BinaryLeft {
                    expression,
                    op,
                    right,
                    path,
                } => {
                    let left_scope = scopes.pop().expect("binary left scope retained");
                    publication.publish(&left_scope)?;
                    let baseline = if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        Some((left_scope.ids(), left_scope.clone()))
                    } else {
                        None
                    };
                    frames.push($Frame::BinaryRight {
                        expression,
                        op,
                        left: match &expression.kind {
                            ResolvedExprKind::Binary { left, .. } => left,
                            _ => unreachable!(),
                        },
                        baseline,
                    })?;
                    if matches!(op, BinaryOp::And | BinaryOp::Or) {
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push($Frame::RestorePublication(enabled))?;
                    }
                    frames.push($Frame::Enter {
                        expression: right,
                        scope: left_scope,
                        path: path_label::Label::child(path, ".right", [0, 0])?,
                    })?;
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
