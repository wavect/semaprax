// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_4 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $enter_phase_5<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            expression: &'e ResolvedExpr,
            scope: scope_checkpoint::Scope,
            path: &'e str,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
        ) -> Result<(), Diagnostic> {
            match &expression.kind {
                ResolvedExprKind::Place(place) => {
                    let binding = scope.get(&place.root).ok_or_else(|| {
                        scope_checkpoint::paths::diagnostic(format_args!(
                            "resolved value `{}` is out of scope",
                            place.root
                        ))
                    })?;
                    match (place.projections.is_empty(), binding.availability) {
                        (true, Availability::Available) => {
                            match HirValidator::place_availability(binding, &[]) {
                                Availability::Available => {}
                                Availability::Moved => {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "resolved value `{}` is partially moved",
                                        place.root
                                    )));
                                }
                                Availability::MaybeMoved => {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "resolved value `{}` may be partially moved",
                                        place.root
                                    )));
                                }
                            }
                        }
                        (true, Availability::Moved) => {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "resolved value `{}` is used after it was moved",
                                place.root
                            )));
                        }
                        (true, Availability::MaybeMoved) => {
                            return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                "resolved value `{}` may have been moved",
                                place.root
                            )));
                        }
                        (false, _) => {
                            match HirValidator::place_availability(binding, &place.projections) {
                                Availability::Available => {}
                                Availability::Moved => {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "resolved place rooted at `{}` is partially moved",
                                        place.root
                                    )));
                                }
                                Availability::MaybeMoved => {
                                    return Err(scope_checkpoint::paths::diagnostic(format_args!(
                                        "resolved place rooted at `{}` may be conditionally moved",
                                        place.root
                                    )));
                                }
                            }
                        }
                    }
                    let (ty, ownership) = validator.resolve_place(place, binding)?;
                    validator.finish_expr(expression, &ty, ownership)?;
                    scopes.push(scope)?;
                }
                ResolvedExprKind::Unary { op, value } => {
                    frames.push($Frame::Unary {
                        expression,
                        op: *op,
                    })?;
                    frames.push($Frame::Enter {
                        expression: value,
                        scope,
                        path: path_label::Label::child(path, ".value", [0, 0])?,
                    })?;
                }
                _ => unreachable!("expression was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
