// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_17 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_7<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            function: &FunctionExecutionId,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            allow_moves: bool,
            allowed_effects: Option<effect_keys::Allowed<'_>>,
        ) -> Result<(), Diagnostic> {
            match frame {
                $Frame::Enter {
                    expression,
                    scope,
                    path,
                } => {
                    reject_nul_identity("resolved expression", expression.id.as_str())?;
                    if !expression.id.matches_display(function, path.len(), path) {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "expression `{}` has a non-canonical identity",
                            expression.id
                        )));
                    }
                    if !identity_insert::expression(&mut validator.expression_ids, &expression.id)?
                    {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "duplicate resolved expression identity `{}`",
                            expression.id
                        )));
                    }
                    validator.validate_type(&expression.ty)?;
                    let expected_path = path;
                    let path = continuation_path::after_header(expression, expected_path.len());
                    match &expression.kind {
                        ResolvedExprKind::LexicalExit { .. }
                        | ResolvedExprKind::Closure { .. }
                        | ResolvedExprKind::FunctionReference { .. }
                        | ResolvedExprKind::Invoke { .. }
                        | ResolvedExprKind::Int { .. }
                        | ResolvedExprKind::Int32 { .. }
                        | ResolvedExprKind::Char { .. }
                        | ResolvedExprKind::Uint8 { .. }
                        | ResolvedExprKind::Usize { .. } => $enter_phase_0(
                            validator,
                            function,
                            expression,
                            scope,
                            path,
                            frames,
                            scopes,
                            allow_moves,
                        )?,
                        ResolvedExprKind::BorrowPlace { .. }
                        | ResolvedExprKind::ByteRange { .. }
                        | ResolvedExprKind::ArrayU8 { .. }
                        | ResolvedExprKind::RepeatArrayU8 { .. }
                        | ResolvedExprKind::Float32 { .. }
                        | ResolvedExprKind::Float64 { .. }
                        | ResolvedExprKind::Bool { .. }
                        | ResolvedExprKind::BorrowedUtf8Literal { .. }
                        | ResolvedExprKind::String { .. } => {
                            $enter_phase_4(validator, expression, scope, path, frames, scopes)?
                        }
                        ResolvedExprKind::Place { .. } | ResolvedExprKind::Unary { .. } => {
                            $enter_phase_5(validator, expression, scope, path, frames, scopes)?
                        }
                        ResolvedExprKind::Call { .. }
                        | ResolvedExprKind::VecFieldRead { .. }
                        | ResolvedExprKind::RowLoan { .. }
                        | ResolvedExprKind::LiteralFormat { .. }
                        | ResolvedExprKind::HostCommandCall { .. } => $enter_phase_1(
                            validator,
                            function,
                            expression,
                            scope,
                            path,
                            frames,
                            allow_moves,
                            allowed_effects,
                        )?,
                        ResolvedExprKind::NativeRustImportCall { .. }
                        | ResolvedExprKind::Binary { .. }
                        | ResolvedExprKind::If { .. }
                        | ResolvedExprKind::Block { .. }
                        | ResolvedExprKind::VecConstructor { .. }
                        | ResolvedExprKind::ConstructRecord { .. }
                        | ResolvedExprKind::ConstructVariant { .. }
                        | ResolvedExprKind::UpdateRecord { .. }
                        | ResolvedExprKind::Match { .. }
                        | ResolvedExprKind::Project { .. }
                        | ResolvedExprKind::Upcast { .. }
                        | ResolvedExprKind::Yield { .. } => $enter_phase_2(
                            validator,
                            expression,
                            scope,
                            path,
                            frames,
                            allowed_effects,
                        )?,
                        ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. } => {
                            $enter_phase_3(expression, scope, path, frames)?
                        }
                    }
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
