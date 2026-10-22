// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_2 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $enter_phase_0<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            function: &FunctionExecutionId,
            expression: &'e ResolvedExpr,
            mut scope: scope_checkpoint::Scope,
            path: &'e str,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            allow_moves: bool,
        ) -> Result<(), Diagnostic> {
            match &expression.kind {
                ResolvedExprKind::LexicalExit { value, .. } => {
                    if let Some(operand) = value {
                        frames.push($Frame::LexicalReturn {
                            expression,
                            operand,
                        })?;
                        frames.push($Frame::Enter {
                            expression: operand,
                            scope,
                            path: path_label::Label::child(path, ".value", [0, 0])?,
                        })?;
                    } else {
                        validator.finish_expr(
                            expression,
                            &ResolvedType::Never,
                            OwnershipMode::Value,
                        )?;
                        scopes.push(scope)?;
                    }
                }
                ResolvedExprKind::Closure { .. } => {
                    validator.validate_closure(
                        function,
                        expression,
                        &mut scope,
                        path,
                        allow_moves,
                    )?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::FunctionReference { target } => {
                    crate::hir::function_value::validate_reference(
                        validator.program,
                        target,
                        &expression.ty,
                    )?;
                    validator.finish_expr(expression, &expression.ty, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::Invoke { .. } => {
                    crate::hir::function_value::validate_invocation(expression)?;
                    frames.push($Frame::InvokeNext {
                        expression,
                        index: 0,
                        scope,
                        path,
                    })?;
                }
                ResolvedExprKind::Int(_) => {
                    validator.finish_expr(expression, &ResolvedType::I64, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::Int32(_) => {
                    validator.finish_expr(expression, &ResolvedType::I32, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::Char(value) => {
                    if char::from_u32(*value).is_none() {
                        return Err(hir_error(
                            "char literal bits are not a Unicode scalar value",
                        ));
                    }
                    validator.finish_expr(expression, &ResolvedType::Char, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::Uint8(_) => {
                    validator.finish_expr(expression, &ResolvedType::U8, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::Usize(_) => {
                    validator.finish_expr(
                        expression,
                        &ResolvedType::Usize,
                        OwnershipMode::Value,
                    )?;
                    scopes.push(scope)?;
                }
                _ => unreachable!("expression was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
