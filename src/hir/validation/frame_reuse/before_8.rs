// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_8 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_0<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            publication: &mut scope_checkpoint::Publication<'_>,
            allow_moves: bool,
        ) -> Result<(), Diagnostic> {
            match frame {
                $Frame::InvokeNext {
                    expression,
                    index,
                    mut scope,
                    path,
                } => {
                    let ResolvedExprKind::Invoke { callable, args } = &expression.kind else {
                        unreachable!()
                    };
                    let child = if index == 0 {
                        Some(callable.as_ref())
                    } else {
                        args.get(index - 1)
                    };
                    if let Some(child) = child {
                        let child_path = if index == 0 {
                            path_label::Label::child(path, ".callable", [0, 0])?
                        } else {
                            path_label::Label::child(path, ".arg.{}", [index - 1, 0])?
                        };
                        frames.push($Frame::InvokeAfter {
                            expression,
                            index,
                            path,
                        })?;
                        frames.push($Frame::Enter {
                            expression: child,
                            scope,
                            path: child_path,
                        })?;
                    } else {
                        validator.finish_affine_invocation(expression, &mut scope, allow_moves)?;
                        scopes.push(scope)?;
                    }
                }
                $Frame::InvokeAfter {
                    expression,
                    index,
                    path,
                } => {
                    let scope = scopes.pop().expect("invocation child scope");
                    frames.push($Frame::InvokeNext {
                        expression,
                        index: index + 1,
                        scope,
                        path,
                    })?;
                }
                $Frame::RestorePublication(enabled) => publication.enabled = enabled,
                $Frame::Unary { expression, op } => {
                    let scope = scopes.pop().expect("unary scope retained");
                    let ResolvedExprKind::Unary { value, .. } = &expression.kind else {
                        unreachable!()
                    };
                    // Negation keeps a numeric operand type; i64 negation is
                    // checked while IEEE-754 negation is total.
                    if matches!(op, UnaryOp::Neg)
                        && !matches!(
                            &value.ty,
                            ResolvedType::I64
                                | ResolvedType::I32
                                | ResolvedType::F32
                                | ResolvedType::F64
                        )
                    {
                        return Err(hir_error("unary operand has inconsistent resolved types"));
                    }
                    let expected = match op {
                        UnaryOp::Neg => value.ty.clone(),
                        UnaryOp::Not => ResolvedType::Bool,
                    };
                    validator.require_type(&value.ty, &expected, "unary operand")?;
                    validator.finish_expr(expression, &expected, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
