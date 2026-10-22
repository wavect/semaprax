// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_15 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_5<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            function: &FunctionExecutionId,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            publication: &mut scope_checkpoint::Publication<'_>,
            allow_moves: bool,
            allowed_effects: Option<effect_keys::Allowed<'_>>,
        ) -> Result<(), Diagnostic> {
            match frame {
                $Frame::VariantMatchNext {
                    expression,
                    cases,
                    index,
                    outer,
                    outer_ids,
                    arm_scopes,
                    mut covered,
                    mut wildcard_seen,
                    result,
                    path,
                } => {
                    #[cfg(test)]
                    lexical_exit::observe_witness_frontier("variant", lexical_exit::VARIANT_VIEWS);
                    scope_checkpoint::charge::reserve(Some(lexical_exit::VARIANT_VIEWS))?;
                    #[cfg(test)]
                    lexical_exit::observe_witness_role("variant");
                    let ResolvedExprKind::Match {
                        scrutinee,
                        mode,
                        arms,
                        ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    let ResolvedType::Nominal {
                        declaration: variant,
                        arguments,
                    } = &scrutinee.ty
                    else {
                        unreachable!("variant continuation follows validated nominal scrutinee")
                    };
                    let arms: &[ResolvedMatchArm] = arms;
                    let arguments: &[ResolvedType] = arguments;
                    if index == arms.len() {
                        if !wildcard_seen
                            && covered.len() != cases.len()
                            && !validator.observed_partial_byte_receiver(expression)
                        {
                            return Err(hir_error("resolved match is not exhaustive"));
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
                    } else {
                        let arm = &arms[index];
                        let mut arm_scope = outer.clone();
                        if *mode == ResolvedMatchMode::Borrow {
                            let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                                unreachable!()
                            };
                            if let Some(owner) = arm_scope.get_mut(&place.root)? {
                                owner.active_loans.extend(validator.exact_loan_id(
                                    function,
                                    &expression.id,
                                    &LoanCause::MatchBorrow {
                                        arm: u16::try_from(index).map_err(|_| {
                                            hir_error("match loan arm index overflows")
                                        })?,
                                    },
                                )?);
                            }
                        }
                        match &arm.pattern {
                            ResolvedMatchPattern::Wildcard => {
                                if *mode != ResolvedMatchMode::Value {
                                    return Err(hir_error(
                                        "resolved explicit ownership match cannot discard an owned variant",
                                    ));
                                }
                                if wildcard_seen || covered.len() == cases.len() {
                                    return Err(hir_error(
                                        "resolved match has an unreachable wildcard",
                                    ));
                                }
                                wildcard_seen = arm.guard.is_none();
                            }
                            ResolvedMatchPattern::Variant {
                                variant: pattern_variant,
                                case,
                                fields,
                            } => {
                                if wildcard_seen
                                    || pattern_variant != variant
                                    || covered.contains(case)
                                    || (arm.guard.is_none() && !covered.insert(case)?)
                                {
                                    return Err(hir_error(
                                        "resolved match has an unreachable or foreign case pattern",
                                    ));
                                }
                                let declared_case = cases
                                    .iter()
                                    .find(|item| item.id == *case)
                                    .ok_or_else(|| {
                                        scope_checkpoint::paths::diagnostic(format_args!(
                                            "resolved match references foreign case `{case}`"
                                        ))
                                    })?;
                                let mut seen = BTreeSet::new();
                                for (field_index, field) in fields.iter().enumerate() {
                                    let declared = declared_case
                                        .fields
                                        .iter()
                                        .find(|item| item.id == field.field)
                                        .ok_or_else(|| {
                                            scope_checkpoint::paths::diagnostic(format_args!(
                                                "resolved pattern contains foreign field `{}`",
                                                field.field
                                            ))
                                        })?;
                                    let binding_ty =
                                        substitute_type(&declared.ty, variant, arguments)?;
                                    let facts = validator
                                        .program
                                        .declarations
                                        .type_flags(&binding_ty)
                                        .ok_or_else(|| {
                                            hir_error("variant binding has no exact type facts")
                                        })?;
                                    let binding_ownership = if facts.copy {
                                        OwnershipMode::Value
                                    } else if *mode == ResolvedMatchMode::Own {
                                        OwnershipMode::Own
                                    } else if *mode == ResolvedMatchMode::Borrow {
                                        OwnershipMode::Borrow
                                    } else {
                                        return Err(hir_error(
                                            "Copy variant pattern contains an owned payload",
                                        ));
                                    };
                                    if !seen.insert(field.field.clone())
                                        || !field.binding.id.matches_local_label(
                                            function,
                                            path_label::Label::child(
                                                path,
                                                ".arm.{}.binding.{}",
                                                [index, field_index],
                                            )?,
                                        )
                                        || field.binding.ty != binding_ty
                                        || field.binding.ownership != binding_ownership
                                    {
                                        return Err(hir_error(
                                            "resolved match pattern field or binding is invalid",
                                        ));
                                    }
                                    validator.insert_value(&field.binding.id)?;
                                    validator.validate_type(&field.binding.ty)?;
                                    if arm_scope.contains_key(&field.binding.id) {
                                        return Err(hir_error(
                                            "resolved match pattern binding shadows an existing value",
                                        ));
                                    }
                                    scope_checkpoint::charge::clone_type(&field.binding.ty)?;
                                    arm_scope.insert(
                                        field.binding.id.clone(),
                                        ValidationBinding {
                                            contract_local: false,
                                            mutable: false,
                                            ty: field.binding.ty.clone(),
                                            ownership: binding_ownership,
                                            availability: Availability::Available,
                                            active_loans: BTreeSet::new(),
                                            moved_places: BTreeMap::new(),
                                            definitely_partial: BTreeSet::new(),
                                        },
                                    )?;
                                }
                                if seen.len() != declared_case.fields.len() {
                                    return Err(hir_error(
                                        "resolved match pattern is missing payload fields",
                                    ));
                                }
                            }
                            ResolvedMatchPattern::Record { .. } => {
                                return Err(hir_error(
                                    "resolved record pattern has a variant scrutinee",
                                ));
                            }
                            ResolvedMatchPattern::Or(alternatives) => {
                                let mut candidate = covered.checkpoint()?;
                                variant_or::cover_borrowed(
                                    alternatives,
                                    variant,
                                    cases,
                                    *mode,
                                    wildcard_seen,
                                    &mut candidate,
                                )?;
                                if arm.guard.is_none() {
                                    covered = candidate;
                                }
                            }
                            ResolvedMatchPattern::Literal(_) | ResolvedMatchPattern::Binding(_) => {
                                return Err(hir_error(
                                    "resolved refutable pattern has an aggregate variant \
                                     scrutinee",
                                ));
                            }
                        }
                        if let Some(guard) = &arm.guard {
                            if !crate::variant_guards::admitted(
                                &validator.program.declarations,
                                &scrutinee.ty,
                                *mode,
                                &arm.pattern,
                                guard,
                            ) {
                                return Err(hir_error(
                                    "resolved Copy variant guard is outside its ordinary Boolean profile",
                                ));
                            }
                            validator.validate_expr_before_frame_reuse(
                                function,
                                guard,
                                &mut arm_scope,
                                path_label::Label::child(path, ".arm.{}.guard", [index, 0])?,
                                allow_moves,
                                allowed_effects,
                            )?;
                            if guard.ty != ResolvedType::Bool {
                                return Err(hir_error("resolved match guard is not bool"));
                            }
                            variant_guard_state::unchanged(&outer, &arm_scope)?;
                        }
                        frames.push($Frame::VariantMatchAfterArm {
                            expression,
                            cases,
                            index,
                            outer,
                            outer_ids,
                            arm_scopes,
                            covered,
                            wildcard_seen,
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
                $Frame::VariantMatchAfterArm {
                    expression,
                    cases,
                    index,
                    outer,
                    outer_ids,
                    mut arm_scopes,
                    covered,
                    wildcard_seen,
                    mut result,
                    path,
                } => {
                    #[cfg(test)]
                    lexical_exit::observe_witness_frontier("arm", lexical_exit::VARIANT_ARM_VIEW);
                    scope_checkpoint::charge::reserve(Some(lexical_exit::VARIANT_ARM_VIEW))?;
                    #[cfg(test)]
                    lexical_exit::observe_witness_role("arm");
                    let ResolvedExprKind::Match { arms, .. } = &expression.kind else {
                        unreachable!("variant arm continuation retains its match")
                    };
                    let arms: &[ResolvedMatchArm] = arms;
                    let mut arm_scope = scopes.pop().expect("variant match arm scope retained");
                    if let ResolvedExprKind::Match {
                        scrutinee,
                        mode: ResolvedMatchMode::Borrow,
                        ..
                    } = &expression.kind
                    {
                        let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                            unreachable!()
                        };
                        if let Some(owner) = arm_scope.get_mut(&place.root)? {
                            if let Some(loan) = validator.exact_loan_id(
                                function,
                                &expression.id,
                                &LoanCause::MatchBorrow {
                                    arm: u16::try_from(index)
                                        .map_err(|_| hir_error("match loan arm index overflows"))?,
                                },
                            )? {
                                owner.active_loans.remove(&loan);
                            }
                        }
                    }
                    let arm = &arms[index];
                    if arm.value.ty != ResolvedType::Never
                        && !crate::hir::collection_outcome::match_arm_execution(
                            validator.program,
                            function,
                            expression,
                            &arm.value,
                        )
                    {
                        return Err(hir_error(
                            "resolved owned variant match arm must produce a Copy scalar",
                        ));
                    }
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
                    frames.push($Frame::VariantMatchNext {
                        expression,
                        cases,
                        index: index + 1,
                        outer,
                        outer_ids,
                        arm_scopes,
                        covered,
                        wildcard_seen,
                        result,
                        path,
                    })?;
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
