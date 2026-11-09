// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_16 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_6<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            function: &FunctionExecutionId,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            publication: &mut scope_checkpoint::Publication<'_>,
        ) -> Result<(), Diagnostic> {
            match frame {
                $Frame::ScalarMatchNext {
                    expression,
                    arms,
                    index,
                    outer,
                    outer_ids,
                    arm_scopes,
                    result,
                    path,
                } => {
                    let ResolvedExprKind::Match { scrutinee, .. } = &expression.kind else {
                        unreachable!()
                    };
                    if index == arms.len() {
                        // Admission already required the trailing catch-all;
                        // re-check it here against hostile HIR.
                        let last = arms.last().expect("non-empty checked above");
                        let catch_all = matches!(
                            &last.pattern,
                            ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Binding(_)
                        );
                        if !catch_all || last.guard.is_some() {
                            return Err(hir_error(
                                "resolved refutable match lacks a trailing irrefutable \
                                 guard-free catch-all",
                            ));
                        }
                        #[cfg(test)]
                        lexical_exit::observe_witness_frontier(
                            "finish",
                            lexical_exit::WITNESS_FINISH,
                        );
                        scope_checkpoint::charge::reserve(Some(lexical_exit::WITNESS_FINISH))?;
                        #[cfg(test)]
                        lexical_exit::observe_witness_role("finish");
                        let witness =
                            result.ok_or_else(|| hir_error("resolved match has no result"))?;
                        let mut final_scope = outer;
                        if let Some((first, rest)) = arm_scopes.split_first() {
                            let mut joined = first.clone();
                            for arm_scope in rest {
                                scope_checkpoint::merge::conditional(
                                    &mut joined,
                                    arm_scope,
                                    &outer_ids,
                                )?;
                            }
                            scope_checkpoint::merge::availability(
                                &mut final_scope,
                                &joined,
                                &outer_ids,
                            )?;
                            scope_checkpoint::merge::borrows(
                                &mut final_scope,
                                &joined,
                                &outer_ids,
                            )?;
                        }
                        publication.publish(&final_scope)?;
                        validator.finish_expr(expression, &witness.ty, witness.ownership)?;
                        scopes.push(final_scope)?;
                        return Ok(());
                    }
                    let arm = &arms[index];
                    let mut arm_scope = outer.clone();
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {}
                        ResolvedMatchPattern::Binding(binding) => {
                            if !binding.id.matches_local_label(
                                function,
                                path_label::Label::child(path, ".arm.{}.binding", [index, 0])?,
                            ) || binding.ty != scrutinee.ty
                                || binding.ownership != OwnershipMode::Value
                            {
                                return Err(hir_error("resolved scalar match binding is invalid"));
                            }
                            validator.insert_value(&binding.id)?;
                            validator.validate_type(&binding.ty)?;
                            if arm_scope.contains_key(&binding.id) {
                                return Err(hir_error(
                                    "resolved match pattern binding shadows an existing value",
                                ));
                            }
                            scope_checkpoint::charge::clone_type(&binding.ty)?;
                            arm_scope.insert(
                                binding.id.clone(),
                                ValidationBinding {
                                    contract_local: false,
                                    mutable: false,
                                    ty: binding.ty.clone(),
                                    ownership: OwnershipMode::Value,
                                    availability: Availability::Available,
                                    active_loans: BTreeSet::new(),
                                    moved_places: BTreeMap::new(),
                                    definitely_partial: BTreeSet::new(),
                                },
                            )?;
                        }
                        ResolvedMatchPattern::Literal(value) => {
                            if value.ty() != scrutinee.ty {
                                return Err(hir_error(
                                    "resolved literal pattern disagrees with its scrutinee type",
                                ));
                            }
                        }
                        ResolvedMatchPattern::Or(alternatives) => {
                            if alternatives.is_empty() {
                                return Err(hir_error("resolved or-pattern has no alternatives"));
                            }
                            for alternative in alternatives {
                                let ResolvedMatchPattern::Literal(value) = alternative else {
                                    return Err(hir_error(
                                        "resolved or-pattern contains a non-literal alternative",
                                    ));
                                };
                                if value.ty() != scrutinee.ty {
                                    return Err(hir_error(
                                        "resolved or-pattern alternative disagrees with its \
                                         scrutinee type",
                                    ));
                                }
                            }
                        }
                        ResolvedMatchPattern::Variant { .. }
                        | ResolvedMatchPattern::Record { .. } => {
                            return Err(hir_error(
                                "resolved aggregate pattern has a Copy-scalar scrutinee",
                            ));
                        }
                    }
                    if let Some(guard) = &arm.guard {
                        frames.push($Frame::ScalarMatchAfterGuard {
                            expression,
                            arms,
                            index,
                            outer,
                            outer_ids,
                            arm_scopes,
                            result,
                            path,
                        })?;
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push($Frame::RestorePublication(enabled))?;
                        frames.push($Frame::Enter {
                            expression: guard.as_ref(),
                            scope: arm_scope,
                            path: path_label::Label::child(path, ".arm.{}.guard", [index, 0])?,
                        })?;
                    } else {
                        frames.push($Frame::ScalarMatchAfterArm {
                            expression,
                            arms,
                            index,
                            outer,
                            outer_ids,
                            arm_scopes,
                            result,
                            path,
                        })?;
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push($Frame::RestorePublication(enabled))?;
                        frames.push($Frame::Enter {
                            expression: &arm.value,
                            scope: arm_scope,
                            path: path_label::Label::child(path, ".arm.{}.value", [index, 0])?,
                        })?;
                    }
                }
                $Frame::ScalarMatchAfterGuard {
                    expression,
                    arms,
                    index,
                    outer,
                    outer_ids,
                    arm_scopes,
                    result,
                    path,
                } => {
                    let arm = &arms[index];
                    let Some(guard) = &arm.guard else {
                        unreachable!("guard continuation retains a guarded arm")
                    };
                    let scope_after_guard =
                        scopes.pop().expect("scalar match guard scope retained");
                    if guard.ty != ResolvedType::Bool || guard.ownership != OwnershipMode::Value {
                        return Err(hir_error("resolved match guard is not exactly bool"));
                    }
                    // The arm value observes the guard's moves within this
                    // arm only, so it resolves under the post-guard scope.
                    let enabled = publication.enabled;
                    publication.enabled = false;
                    frames.push($Frame::ScalarMatchAfterArm {
                        expression,
                        arms,
                        index,
                        outer,
                        outer_ids,
                        arm_scopes,
                        result,
                        path,
                    })?;
                    frames.push($Frame::RestorePublication(enabled))?;
                    frames.push($Frame::Enter {
                        expression: &arm.value,
                        scope: scope_after_guard,
                        path: path_label::Label::child(path, ".arm.{}.value", [index, 0])?,
                    })?;
                }
                $Frame::ScalarMatchAfterArm {
                    expression,
                    arms,
                    index,
                    outer,
                    outer_ids,
                    mut arm_scopes,
                    mut result,
                    path,
                } => {
                    let arm_scope = scopes.pop().expect("scalar match arm scope retained");
                    let arm = &arms[index];
                    #[cfg(test)]
                    lexical_exit::observe_witness_frontier("join", lexical_exit::WITNESS_JOIN);
                    scope_checkpoint::charge::reserve(Some(lexical_exit::WITNESS_JOIN))?;
                    #[cfg(test)]
                    lexical_exit::observe_witness_role("join");
                    validator.join_lexical_match_witness(&mut result, &arm.value)?;
                    #[cfg(test)]
                    lexical_exit::witness_unwind_point();
                    if !crate::hir::lexical_exit::returns_only(&arm.value) {
                        arm_scopes.push(arm_scope)?;
                    }
                    frames.push($Frame::ScalarMatchNext {
                        expression,
                        arms,
                        index: index + 1,
                        outer,
                        outer_ids,
                        arm_scopes,
                        result,
                        path,
                    })?;
                }
                $Frame::Project { expression, field } => {
                    let scope = scopes.pop().expect("projection scope retained");
                    let ResolvedExprKind::Project { base, .. } = &expression.kind else {
                        unreachable!()
                    };
                    let projected = validator.field_type_for_type(&base.ty, field)?;
                    let ownership = validator.expected_ownership(&projected, base.ownership)?;
                    validator.finish_expr(expression, &projected, ownership)?;
                    scopes.push(scope)?;
                }
                $Frame::Upcast { expression } => {
                    let scope = scopes.pop().expect("upcast scope retained");
                    let ResolvedExprKind::Upcast { source } = &expression.kind else {
                        unreachable!()
                    };
                    validator.validate_upcast(expression, source)?;
                    let ownership =
                        validator.expected_ownership(&expression.ty, OwnershipMode::Own)?;
                    validator.finish_expr(expression, &expression.ty, ownership)?;
                    scopes.push(scope)?;
                }
                $Frame::Yield { expression } => {
                    let scope = scopes.pop().expect("yield scope retained");
                    if !matches!(expression.kind, ResolvedExprKind::Yield { .. }) {
                        unreachable!()
                    }
                    // Re-derive `hir::resolve_yield`'s own admission: scalar,
                    // or (issue #296 R20) a bounded aggregate.
                    let (d, ty) = (&validator.program.declarations, &expression.ty);
                    let bounded = matches!(ty, ResolvedType::Nominal { .. })
                        && crate::hir::yield_aggregate::bounded_aggregate_refusal(d, ty).is_ok();
                    if !crate::hir::is_scalar_resolved_type(ty) && !bounded {
                        return Err(hir_error("resolved `yield` type is not scalar or bounded"));
                    }
                    let ownership =
                        validator.expected_ownership(&expression.ty, OwnershipMode::Value)?;
                    validator.finish_expr(expression, &expression.ty, ownership)?;
                    scopes.push(scope)?;
                }
                $Frame::Try {
                    expression,
                    path,
                    option,
                } => {
                    let mut scope = scopes.pop().expect("try scope retained");
                    if !option {
                        let ResolvedExprKind::Try { operand, .. } = &expression.kind else {
                            unreachable!()
                        };
                        if owned_result_try::expression_owns_exact_operand(operand) {
                            validator.mark_value_sources_moved(operand, &mut scope)?;
                        }
                    }
                    validator.finish_try_expr(function, expression, &scope, path, option)?;
                    scopes.push(scope)?;
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
