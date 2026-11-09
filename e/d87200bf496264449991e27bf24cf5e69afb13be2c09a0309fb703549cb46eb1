// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_14 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_4<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            function: &FunctionExecutionId,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            publication: &mut scope_checkpoint::Publication<'_>,
        ) -> Result<(), Diagnostic> {
            let program = validator.program;
            match frame {
                $Frame::MatchScrutinee {
                    expression,
                    arms,
                    path,
                } => {
                    let mut outer = scopes.pop().expect("match scrutinee scope retained");
                    publication.publish(&outer)?;
                    let ResolvedExprKind::Match {
                        mode, scrutinee, ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    // Refutable Match v1: Copy-scalar scrutinees validate
                    // through the decision-chain twins below.
                    if matches!(
                        scrutinee.ty,
                        ResolvedType::I64
                            | ResolvedType::I32
                            | ResolvedType::U8
                            | ResolvedType::Usize
                            | ResolvedType::Char
                            | ResolvedType::Bool
                    ) {
                        if *mode != ResolvedMatchMode::Value
                            || scrutinee.ownership != OwnershipMode::Value
                        {
                            return Err(hir_error(
                                "resolved refutable match scrutinee is not Copy",
                            ));
                        }
                        if arms.is_empty() {
                            return Err(hir_error("resolved match has no arms"));
                        }
                        let outer_ids = outer.ids();
                        frames.push($Frame::ScalarMatchNext {
                            expression,
                            arms,
                            index: 0,
                            outer,
                            outer_ids,
                            arm_scopes: scope_checkpoint::Storage::with_capacity(arms.len())?,
                            result: None,
                            path,
                        })?;
                        return Ok(());
                    }
                    let ResolvedType::Nominal {
                        declaration: matched,
                        arguments: _,
                    } = &scrutinee.ty
                    else {
                        return Err(hir_error("resolved match scrutinee is not nominal"));
                    };
                    let kind = validator
                        .program
                        .declarations
                        .declaration(matched)
                        .map(|item| item.kind);
                    let outer_ids = outer.ids();
                    if kind == Some(DeclarationKind::Record) {
                        let facts = validator
                            .program
                            .declarations
                            .type_flags(&scrutinee.ty)
                            .ok_or_else(|| hir_error("record match has no exact type facts"))?;
                        match mode {
                            ResolvedMatchMode::Value
                                if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                            ResolvedMatchMode::Own
                                if facts.needs_drop
                                    && !facts.copy
                                    && scrutinee.ownership == OwnershipMode::Own
                                    && (!crate::hir::type_reachability::is_nested_nonflat_owned_byte_record(
                                        &validator.program.declarations,
                                        &scrutinee.ty,
                                    ) || matches!(
                                        &scrutinee.kind,
                                        ResolvedExprKind::Place(place)
                                            if place.projections.is_empty()
                                    )) =>
                            {
                                validator.mark_value_sources_moved(scrutinee, &mut outer)?;
                            }
                            ResolvedMatchMode::Borrow
                                if facts.needs_drop
                                    && !facts.copy
                                    && matches!(
                                        scrutinee.ownership,
                                        OwnershipMode::Own | OwnershipMode::Borrow
                                    )
                                    && matches!(
                                        &scrutinee.kind,
                                        ResolvedExprKind::Place(place)
                                            if place.projections.is_empty()
                                    ) => {}
                            _ => {
                                return Err(hir_error(
                                    "resolved record match mode disagrees with its scrutinee",
                                ));
                            }
                        }
                        let [arm] = arms else {
                            return Err(hir_error(
                                "resolved irrefutable record match must have exactly one arm",
                            ));
                        };
                        let mut arm_scope = outer.clone();
                        let borrowed_root = if *mode == ResolvedMatchMode::Borrow {
                            let ResolvedExprKind::Place(place) = &scrutinee.kind else {
                                unreachable!("borrow mode authenticated a place above")
                            };
                            if let Some(owner) = arm_scope.get_mut(&place.root)? {
                                owner.active_loans.extend(validator.exact_loan_id(
                                    function,
                                    &expression.id,
                                    &LoanCause::MatchBorrow { arm: 0 },
                                )?);
                            }
                            Some(place.root.clone())
                        } else {
                            None
                        };
                        match &arm.pattern {
                            ResolvedMatchPattern::Wildcard if *mode != ResolvedMatchMode::Value => {
                                return Err(hir_error(
                                    "resolved explicit ownership match lacks an exact record pattern",
                                ));
                            }
                            ResolvedMatchPattern::Wildcard => {}
                            ResolvedMatchPattern::Record {
                                record,
                                instance,
                                fields,
                            } => validator
                                .validate_record_match_pattern(
                                    function,
                                    &scrutinee.ty,
                                    record,
                                    instance,
                                    fields,
                                    &mut arm_scope,
                                    path_label::Label::child(path, ".arm.0.record", [0, 0])?,
                                    *mode,
                                )
                                .map_err(|diagnostic| {
                                    hir_diagnostic_at_span(diagnostic, expression.span)
                                })?,
                            ResolvedMatchPattern::Variant { .. } => {
                                return Err(hir_error(
                                    "resolved variant pattern has a record scrutinee",
                                ));
                            }
                            ResolvedMatchPattern::Literal(_)
                            | ResolvedMatchPattern::Or(_)
                            | ResolvedMatchPattern::Binding(_) => {
                                return Err(hir_error(
                                    "resolved refutable pattern has an aggregate record \
                                     scrutinee",
                                ));
                            }
                        }
                        frames.push($Frame::RecordMatchArm {
                            expression,
                            arm,
                            outer,
                            outer_ids,
                            borrowed_root,
                        })?;
                        let enabled = publication.enabled;
                        publication.enabled = false;
                        frames.push($Frame::RestorePublication(enabled))?;
                        frames.push($Frame::Enter {
                            expression: &arm.value,
                            scope: arm_scope,
                            path: path_label::Label::child(path, ".arm.0.value", [0, 0])?,
                        })?;
                    } else {
                        if kind != Some(DeclarationKind::Variant) {
                            return Err(hir_error(
                                "resolved match scrutinee is not a concrete variant",
                            ));
                        }
                        let facts = validator
                            .program
                            .declarations
                            .type_flags(&scrutinee.ty)
                            .ok_or_else(|| hir_error("variant match has no exact type facts"))?;
                        match mode {
                            ResolvedMatchMode::Value
                                if facts.copy && scrutinee.ownership == OwnershipMode::Value => {}
                            ResolvedMatchMode::Own
                                if (resolved_type_is_flat_owned_byte_variant(
                                    validator.program,
                                    &scrutinee.ty,
                                ) || resolved_type_is_direct_owned_variant(
                                    validator.program,
                                    &scrutinee.ty,
                                )) && facts.needs_drop
                                    && !facts.copy
                                    && scrutinee.ownership == OwnershipMode::Own =>
                            {
                                validator.mark_value_sources_moved(scrutinee, &mut outer)?;
                            }
                            ResolvedMatchMode::Borrow
                                if (resolved_type_is_flat_owned_byte_variant(
                                    validator.program,
                                    &scrutinee.ty,
                                ) || resolved_type_is_direct_owned_variant(
                                    validator.program,
                                    &scrutinee.ty,
                                ) || regex_result(validator.program, &scrutinee.ty))
                                    && facts.needs_drop
                                    && !facts.copy
                                    && matches!(
                                        scrutinee.ownership,
                                        OwnershipMode::Own | OwnershipMode::Borrow
                                    )
                                    && matches!(&scrutinee.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()) =>
                                {}
                            _ => {
                                return Err(hir_error(
                                    "resolved variant match mode disagrees with its scrutinee",
                                ));
                            }
                        }
                        let cases = aggregate_metadata::variant_cases(program, matched)
                            .ok_or_else(|| {
                                scope_checkpoint::paths::diagnostic(format_args!(
                                    "variant `{matched}` has no cases"
                                ))
                            })?;
                        if arms.is_empty() {
                            return Err(hir_error("resolved match has no arms"));
                        }
                        frames.push($Frame::VariantMatchNext {
                            expression,
                            cases,
                            index: 0,
                            outer,
                            outer_ids,
                            arm_scopes: scope_checkpoint::Storage::with_capacity(arms.len())?,
                            covered: variant_or::Covered::new(),
                            wildcard_seen: false,
                            result: None,
                            path,
                        })?;
                    }
                }
                $Frame::RecordMatchArm {
                    expression,
                    arm,
                    mut outer,
                    outer_ids,
                    borrowed_root,
                } => {
                    let mut arm_scope = scopes.pop().expect("record match arm scope retained");
                    if let Some(root) = borrowed_root {
                        if let Some(owner) = arm_scope.get_mut(&root)? {
                            if let Some(loan) = validator.exact_loan_id(
                                function,
                                &expression.id,
                                &LoanCause::MatchBorrow { arm: 0 },
                            )? {
                                owner.active_loans.remove(&loan);
                            }
                        }
                    }
                    validator.validate_generic_record_match_result(function, expression, arm)?;
                    for id in scope_checkpoint::Ids::ids(&outer_ids) {
                        outer.copy_binding_from(&arm_scope, id)?;
                    }
                    publication.publish(&outer)?;
                    validator.finish_expr(expression, &arm.value.ty, arm.value.ownership)?;
                    scopes.push(outer)?;
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
