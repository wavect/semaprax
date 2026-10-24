// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_11 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_1<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            publication: &mut scope_checkpoint::Publication<'_>,
            allow_moves: bool,
        ) -> Result<(), Diagnostic> {
            match frame {
                $Frame::BinaryRight {
                    expression,
                    op,
                    left,
                    baseline,
                } => {
                    let mut scope = scopes.pop().expect("binary right scope retained");
                    let direct = baseline.is_none();
                    if let Some((ids, mut parent)) = baseline {
                        scope_checkpoint::merge::conditional(&mut parent, &scope, &ids)?;
                        scope = parent;
                    }
                    if direct || matches!(op, BinaryOp::And | BinaryOp::Or) {
                        publication.publish(&scope)?;
                    }
                    let ResolvedExprKind::Binary { right, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let output = match op {
                        BinaryOp::Add
                        | BinaryOp::Sub
                        | BinaryOp::Mul
                        | BinaryOp::Div
                        | BinaryOp::Rem => {
                            // Arithmetic keeps the operand type: i64 and u8
                            // arithmetic is checked, IEEE-754 float
                            // arithmetic is total and never selects a status.
                            if !matches!(
                                &left.ty,
                                ResolvedType::I64
                                    | ResolvedType::I32
                                    | ResolvedType::U8
                                    | ResolvedType::Usize
                                    | ResolvedType::F32
                                    | ResolvedType::F64
                            ) || (matches!(op, BinaryOp::Rem)
                                && !matches!(
                                    left.ty,
                                    ResolvedType::I64
                                        | ResolvedType::I32
                                        | ResolvedType::U8
                                        | ResolvedType::Usize
                                ))
                            {
                                return Err(hir_error(
                                    "binary operand has inconsistent resolved types",
                                ));
                            }
                            validator.require_type(&left.ty, &right.ty, "binary operand")?;
                            left.ty.clone()
                        }
                        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                            // Ordered comparison compares scalar values: i64,
                            // IEEE-754 numerics, and Unicode scalar chars.
                            if !matches!(
                                &left.ty,
                                ResolvedType::I64
                                    | ResolvedType::I32
                                    | ResolvedType::Char
                                    | ResolvedType::U8
                                    | ResolvedType::Usize
                                    | ResolvedType::F32
                                    | ResolvedType::F64
                                    | ResolvedType::String
                            ) {
                                return Err(hir_error(
                                    "comparison operand has inconsistent resolved types",
                                ));
                            }
                            validator.require_type(&left.ty, &right.ty, "comparison operand")?;
                            ResolvedType::Bool
                        }
                        BinaryOp::And | BinaryOp::Or => {
                            validator.require_type(
                                &left.ty,
                                &ResolvedType::Bool,
                                "boolean operand",
                            )?;
                            validator.require_type(
                                &right.ty,
                                &ResolvedType::Bool,
                                "boolean operand",
                            )?;
                            ResolvedType::Bool
                        }
                        BinaryOp::Eq | BinaryOp::Ne => {
                            if left.ty.is_callable() || right.ty.is_callable() {
                                return Err(hir_error(
                                    "function value equality is outside the admitted profile",
                                ));
                            }
                            validator.require_type(&left.ty, &right.ty, "equality operands")?;
                            ResolvedType::Bool
                        }
                    };
                    validator.finish_expr(expression, &output, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                $Frame::LexicalReturn {
                    expression,
                    operand,
                } => {
                    let mut scope = scopes.pop().expect("return operand scope retained");
                    validator.validate_lexical_return_operand(operand, &mut scope)?;
                    validator.finish_expr(
                        expression,
                        &ResolvedType::Never,
                        OwnershipMode::Value,
                    )?;
                    scopes.push(scope)?;
                }
                $Frame::IfCondition {
                    expression,
                    then_branch,
                    else_branch,
                    path,
                } => {
                    let outer = scopes.pop().expect("if condition scope retained");
                    publication.publish(&outer)?;
                    let ResolvedExprKind::If { condition, .. } = &expression.kind else {
                        unreachable!()
                    };
                    validator.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                    let outer_ids = outer.ids();
                    frames.push($Frame::IfThen {
                        expression,
                        else_branch,
                        path,
                        outer: outer.clone(),
                        outer_ids,
                    })?;
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push($Frame::RestorePublication(enabled))?;
                    frames.push($Frame::Enter {
                        expression: then_branch,
                        scope: outer,
                        path: path_label::Label::child(path, ".then", [0, 0])?,
                    })?;
                }
                $Frame::IfThen {
                    expression,
                    else_branch,
                    path,
                    outer,
                    outer_ids,
                } => {
                    let then_scope = scopes.pop().expect("if then scope retained");
                    frames.push($Frame::IfElse {
                        expression,
                        then_scope,
                        outer: outer.clone(),
                        outer_ids,
                    })?;
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push($Frame::RestorePublication(enabled))?;
                    frames.push($Frame::Enter {
                        expression: else_branch,
                        scope: outer,
                        path: path_label::Label::child(path, ".else", [0, 0])?,
                    })?;
                }
                $Frame::IfElse {
                    expression,
                    then_scope,
                    mut outer,
                    outer_ids,
                } => {
                    let else_scope = scopes.pop().expect("if else scope retained");
                    let ResolvedExprKind::If {
                        then_branch,
                        else_branch,
                        ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    validator.finish_lexical_if_checkpoint(
                        expression,
                        then_branch,
                        else_branch,
                        &mut outer,
                        &then_scope,
                        &else_scope,
                        &outer_ids,
                    )?;
                    publication.publish(&outer)?;
                    scopes.push(outer)?;
                }
                $Frame::BlockNext {
                    expression,
                    statements,
                    tail,
                    index,
                    scope,
                    outer,
                    outer_ids,
                    path,
                } => {
                    if index == statements.len() {
                        frames.push($Frame::BlockTail {
                            expression,
                            outer_ids,
                            outer,
                        })?;
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push($Frame::RestorePublication(enabled))?;
                        frames.push($Frame::Enter {
                            expression: tail,
                            scope,
                            path: path_label::Label::child(path, ".tail", [0, 0])?,
                        })?;
                    } else {
                        match &statements[index] {
                            ResolvedStatement::Let { value, .. } => {
                                frames.push($Frame::BlockAfterLet {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path,
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push($Frame::RestorePublication(enabled))?;
                                frames.push($Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: path_label::Label::child(path, ".s{}.value", [index, 0])?,
                                })?;
                            }
                            ResolvedStatement::CopyDiscard { value, .. } => {
                                frames.push($Frame::BlockAfterUnsafe {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path,
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push($Frame::RestorePublication(enabled))?;
                                frames.push($Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: path_label::Label::child(path, ".s{}.value", [index, 0])?,
                                })?;
                            }
                            ResolvedStatement::Assign { binding, value, .. } => {
                                if crate::byte_ops::is_same_owner_set_hir(value, &binding.id) {
                                    validator.buffer_reopen_sites.insert(value.id.clone());
                                }
                                // The target must be a previously declared
                                // mutable scalar binding in this block's scope.
                                if !scope.contains_key(&binding.id) {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "assignment target `{}` has no enclosing binding",
                                        binding.id
                                    )));
                                }
                                let target_scope = scope.clone();
                                frames.push($Frame::BlockAfterAssign {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    scope: target_scope,
                                    outer,
                                    outer_ids,
                                    path,
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push($Frame::RestorePublication(enabled))?;
                                frames.push($Frame::Enter {
                                    expression: value,
                                    scope,
                                    path: path_label::Label::child(path, ".s{}.value", [index, 0])?,
                                })?;
                            }
                            ResolvedStatement::Unsafe { body, .. } => {
                                // Contract expressions stay pure; ordinary
                                // blocks resolve the body like any nested
                                // block and bind nothing outside it.
                                if !allow_moves {
                                    return Err(hir_error(
                                        "contract expressions cannot contain unsafe boundary statements",
                                    ));
                                }
                                frames.push($Frame::BlockAfterUnsafe {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path,
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push($Frame::RestorePublication(enabled))?;
                                frames.push($Frame::Enter {
                                    expression: body,
                                    scope,
                                    path: path_label::Label::child(path, ".s{}.body", [index, 0])?,
                                })?;
                            }
                            ResolvedStatement::While {
                                condition, body, ..
                            }
                            | ResolvedStatement::LexicalWhile {
                                condition, body, ..
                            } => {
                                // Bounded While-Loops v1: re-check admission
                                // at the trust boundary and require an exact
                                // `bool` condition before validating the body
                                // like any nested block.
                                validator.validate_loop_pair(condition, body)?;
                                if condition.ty != ResolvedType::Bool {
                                    return Err(hir_error("`while` condition must be bool"));
                                }
                                frames.push($Frame::BlockAfterWhileCondition {
                                    expression,
                                    statements,
                                    tail,
                                    index,
                                    outer,
                                    outer_ids,
                                    path,
                                    body,
                                    entry: scope.clone(),
                                })?;
                                let enabled = publication.enabled;
                                publication.enabled = false;
                                frames.push($Frame::RestorePublication(enabled))?;
                                frames.push($Frame::Enter {
                                    expression: condition,
                                    scope,
                                    path: path_label::Label::child(
                                        path,
                                        ".s{}.condition",
                                        [index, 0],
                                    )?,
                                })?;
                            }
                        }
                    }
                }
                $Frame::BlockAfterUnsafe {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                } => {
                    // The body block validated like any nested block; its
                    // merged scope continues into the enclosing block.
                    let scope = scopes.pop().expect("unsafe body scope retained");
                    match &statements[index] {
                        ResolvedStatement::CopyDiscard { value, .. } => {
                            crate::hir::copy_discard::require(value)?
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            if body.ownership != OwnershipMode::Value
                                || !crate::hir::is_scalar_resolved_type(&body.ty)
                            {
                                return Err(hir_error(
                                    "unsafe boundary bodies must produce a scalar Copy value",
                                ));
                            }
                        }
                        _ => unreachable!("no-binding statement continuation"),
                    }
                    frames.push($Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope,
                        outer,
                        outer_ids,
                        path,
                    })?;
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
