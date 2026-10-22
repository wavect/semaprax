// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_3 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $enter_phase_4<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            expression: &'e ResolvedExpr,
            scope: scope_checkpoint::Scope,
            path: &'e str,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
        ) -> Result<(), Diagnostic> {
            match &expression.kind {
                ResolvedExprKind::BorrowPlace { operation, place } => {
                    let ty = validator.validate_borrowed_view(
                        operation,
                        place,
                        expression.span,
                        &scope,
                    )?;
                    validator.finish_expr(expression, &ty, OwnershipMode::Borrow)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::ByteRange {
                    operation,
                    source,
                    start,
                    end,
                } => {
                    if operation.as_str() != crate::byte_ops::RANGE_ID {
                        return Err(hir_error(
                            "byte range has an invalid compiler-owned operation",
                        ));
                    }
                    validator.byte_range_origin(source, &scope)?;
                    frames.push($Frame::ByteRangeAfterSource {
                        expression,
                        start,
                        end,
                        path,
                    })?;
                    frames.push($Frame::Enter {
                        expression: source,
                        scope,
                        path: path_label::Label::child(path, ".arg.0", [0, 0])?,
                    })?;
                }
                ResolvedExprKind::ArrayU8(values) => {
                    let length = u32::try_from(values.len())
                        .map_err(|_| hir_error("fixed-array literal length does not fit u32"))?;
                    validator.finish_expr(
                        expression,
                        &ResolvedType::ArrayU8(length),
                        OwnershipMode::Value,
                    )?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::RepeatArrayU8 { count, .. } => {
                    if u64::from(*count) > crate::byte_data_capacity::MAX_ARRAY_BYTES {
                        return Err(hir_error("fixed-array repeat exceeds the exact byte bound"));
                    }
                    validator.finish_expr(
                        expression,
                        &ResolvedType::ArrayU8(*count),
                        OwnershipMode::Value,
                    )?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::Float32(bits) => {
                    validator.validate_finite_f32(*bits)?;
                    validator.finish_expr(expression, &ResolvedType::F32, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::Float64(bits) => {
                    validator.validate_finite_f64(*bits)?;
                    validator.finish_expr(expression, &ResolvedType::F64, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::Bool(_) => {
                    validator.finish_expr(expression, &ResolvedType::Bool, OwnershipMode::Value)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::BorrowedUtf8Literal(value) => {
                    validator.finish_static_text(expression, value)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::String(value) => {
                    validator.record_static_pool(value.len(), false)?;
                    validator.finish_expr(expression, &ResolvedType::String, OwnershipMode::Own)?;
                    scopes.push(scope)?;
                }
                _ => unreachable!("expression was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
