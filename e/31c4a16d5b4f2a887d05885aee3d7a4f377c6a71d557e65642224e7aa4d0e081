// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_9 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_8<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
        ) -> Result<(), Diagnostic> {
            match frame {
                $Frame::ByteRangeAfterSource {
                    expression,
                    start,
                    end,
                    path,
                } => {
                    let scope = scopes.pop().expect("byte range source scope retained");
                    frames.push($Frame::ByteRangeAfterStart {
                        expression,
                        end,
                        path,
                    })?;
                    frames.push($Frame::Enter {
                        expression: start,
                        scope,
                        path: path_label::Label::child(path, ".arg.1", [0, 0])?,
                    })?;
                }
                $Frame::ByteRangeAfterStart {
                    expression,
                    end,
                    path,
                } => {
                    let scope = scopes.pop().expect("byte range start scope retained");
                    frames.push($Frame::ByteRangeAfterEnd { expression })?;
                    frames.push($Frame::Enter {
                        expression: end,
                        scope,
                        path: path_label::Label::child(path, ".arg.2", [0, 0])?,
                    })?;
                }
                $Frame::ByteRangeAfterEnd { expression } => {
                    let scope = scopes.pop().expect("byte range end scope retained");
                    let ResolvedExprKind::ByteRange {
                        source, start, end, ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    validator.require_type(
                        &source.ty,
                        &ResolvedType::SliceU8,
                        "byte range source",
                    )?;
                    validator.require_type(&start.ty, &ResolvedType::Usize, "byte range start")?;
                    validator.require_type(&end.ty, &ResolvedType::Usize, "byte range end")?;
                    if source.ownership != OwnershipMode::Borrow
                        || start.ownership != OwnershipMode::Value
                        || end.ownership != OwnershipMode::Value
                    {
                        return Err(hir_error("byte range operands have invalid ownership"));
                    }
                    validator.finish_expr(
                        expression,
                        &ResolvedType::SliceU8,
                        OwnershipMode::Borrow,
                    )?;
                    scopes.push(scope)?;
                }
                $Frame::CallNext {
                    expression,
                    args,
                    params,
                    return_type,
                    return_ownership,
                    index,
                    scope,
                    path,
                } => {
                    if index == args.len() {
                        validator.finish_expr(expression, &return_type, return_ownership)?;
                        scopes.push(scope)?;
                    } else {
                        frames.push($Frame::CallAfterArg {
                            expression,
                            args,
                            params,
                            return_type,
                            return_ownership,
                            index,
                            path,
                        })?;
                        frames.push($Frame::Enter {
                            expression: &args[index],
                            scope,
                            path: path_label::Label::argument(expression, path, index)?,
                        })?;
                    }
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
