// Independently retained immediate previous local definition.
#[rustfmt::skip]
macro_rules! before_definition_12 {
    ($Frame:ident, $frame_owned_capacity:ident, $enter_phase_0:ident, $enter_phase_4:ident, $enter_phase_5:ident, $enter_phase_1:ident, $enter_phase_2:ident, $enter_phase_3:ident, $frame_phase_0:ident, $frame_phase_8:ident, $frame_phase_9:ident, $frame_phase_1:ident, $frame_phase_2:ident, $frame_phase_3:ident, $frame_phase_4:ident, $frame_phase_5:ident, $frame_phase_6:ident, $frame_phase_7:ident) => {
        #[inline(never)]
        fn $frame_phase_2<'a: 'e, 'e>(
            validator: &mut HirValidator<'a>,
            function: &FunctionExecutionId,
            frame: $Frame<'e>,
            frames: &mut scope_checkpoint::Storage<$Frame<'e>>,
            scopes: &mut scope_checkpoint::Storage<scope_checkpoint::Scope>,
            publication: &mut scope_checkpoint::Publication<'_>,
            allow_moves: bool,
        ) -> Result<(), Diagnostic> {
            match frame {
                $Frame::BlockAfterWhileCondition {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                    body,
                    entry,
                } => {
                    // The condition validated like any expression; it must be
                    // exactly `bool` because it re-evaluates before every
                    // iteration.
                    let scope = scopes.pop().expect("while condition scope retained");
                    let (ResolvedStatement::While { condition, .. }
                    | ResolvedStatement::LexicalWhile { condition, .. }) = &statements[index]
                    else {
                        unreachable!("while condition frame resumes at a while statement")
                    };
                    if scope != entry {
                        return Err(hir_error("while condition changes ownership liveness"));
                    }
                    if condition.ty != ResolvedType::Bool {
                        return Err(hir_error("`while` condition must be bool"));
                    }
                    frames.push($Frame::BlockAfterWhileBody {
                        expression,
                        statements,
                        tail,
                        index,
                        outer,
                        outer_ids,
                        path,
                        entry: scope.clone(),
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
                $Frame::BlockAfterWhileBody {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                    entry,
                } => {
                    // Zero or more iterations run: every route reaching the
                    // continuation must preserve the loop-entry ownership state.
                    let after = scopes.pop().expect("while body scope retained");
                    let body = statements[index].child(1).expect("loop body");
                    if !crate::hir::lexical_exit::returns_only(body) && after != entry {
                        return Err(hir_error("while loop body changes ownership liveness"));
                    }
                    frames.push($Frame::BlockNext {
                        expression,
                        statements,
                        tail,
                        index: index + 1,
                        scope: entry,
                        outer,
                        outer_ids,
                        path,
                    })?;
                }
                $Frame::BlockAfterLet {
                    expression,
                    statements,
                    tail,
                    index,
                    outer,
                    outer_ids,
                    path,
                } => {
                    let mut scope = scopes.pop().expect("block let scope retained");
                    let ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("let frame resumes at a let statement")
                    };
                    let statement_path = path_label::Label::child(path, ".s{}", [index, 0])?;
                    if !binding.id.matches_local_display(
                        function,
                        statement_path.len(),
                        statement_path,
                    ) {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "local `{}` has a non-canonical identity",
                            binding.id
                        )));
                    }
                    validator.insert_value(&binding.id)?;
                    validator.require_type(&binding.ty, &value.ty, "local binding")?;
                    if binding.ownership != value.ownership {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "local `{}` has inconsistent ownership",
                            binding.id
                        )));
                    }
                    validator.validate_declared_ownership(&binding.ty, binding.ownership)?;
                    if binding.ty == ResolvedType::SliceU8 {
                        let (place, origin) = match &value.kind {
                            ResolvedExprKind::Place(place) => {
                                let origin = validator.byte_slice_aliases.get(&place.root).cloned().ok_or_else(|| {
                                    hir_error("byte-slice local alias lacks symbolic parameter-root provenance")
                                })?;
                                (place, origin)
                            }
                            ResolvedExprKind::BorrowPlace { place, .. } => (place, place.clone()),
                            ResolvedExprKind::Call { .. }
                                if crate::vec_ops::string_slot::inspect(value).is_some() =>
                            {
                                let place = crate::vec_ops::string_slot::place(value)
                                    .expect("authenticated slot");
                                (place, place.clone())
                            }
                            ResolvedExprKind::VecFieldRead { args, .. } => {
                                let ResolvedExprKind::Place(place) = &args[0].kind else {
                                    return Err(hir_error("vector view has no carrier place"));
                                };
                                (place, place.clone())
                            }
                            ResolvedExprKind::ByteRange { source, .. } => {
                                validator.byte_range_origin(source, &scope)?
                            }
                            _ => {
                                return Err(hir_error(
                                    "byte-slice local must be a direct immutable alias or authenticated view",
                                ));
                            }
                        };
                        let is_authenticated_view = crate::vec_ops::string_slot::inspect(value)
                            .is_some()
                            || matches!(
                                &value.kind,
                                ResolvedExprKind::BorrowPlace { .. }
                                    | ResolvedExprKind::VecFieldRead { .. }
                            );
                        if *mutable || (!is_authenticated_view && !place.projections.is_empty()) {
                            return Err(hir_error(
                                "byte-slice local alias must be immutable and unprojected",
                            ));
                        }
                        validator
                            .byte_slice_aliases
                            .insert(binding.id.clone(), origin.clone());
                        let borrowed_origin = match &value.kind {
                            ResolvedExprKind::Call { .. }
                                if crate::vec_ops::string_slot::inspect(value).is_some() =>
                            {
                                Some(origin)
                            }
                            ResolvedExprKind::BorrowPlace { .. }
                            | ResolvedExprKind::VecFieldRead { .. }
                            | ResolvedExprKind::ByteRange { .. } => Some(origin),
                            _ => None,
                        };
                        if let Some(origin) = borrowed_origin {
                            let owner = scope.get_mut(&origin.root)?.ok_or_else(|| {
                                hir_error("byte view owner disappeared before lexical borrow")
                            })?;
                            owner.active_loans.extend(validator.exact_loan_id(
                                function,
                                &value.id,
                                &LoanCause::SliceView,
                            )?);
                        }
                    } else if binding.ty == ResolvedType::Str {
                        let (origin, creates_loan) =
                            validator.borrowed_str_let_origin(value, binding)?;
                        if *mutable {
                            return Err(hir_error(
                                "borrowed-str local alias must be immutable and unprojected",
                            ));
                        }
                        validator
                            .borrowed_str_aliases
                            .insert(binding.id.clone(), origin.clone());
                        if creates_loan {
                            let owner = scope.get_mut(&origin.root)?.ok_or_else(|| {
                                hir_error("String view owner disappeared before lexical borrow")
                            })?;
                            owner.active_loans.extend(validator.exact_loan_id(
                                function,
                                &value.id,
                                &LoanCause::StrView,
                            )?);
                        }
                    }
                    let contract_local = !allow_moves && contract_owners::fresh(value, &scope);
                    if validator.is_owned_resource(&binding.ty, binding.ownership)? {
                        if !allow_moves && !contract_owners::fresh(value, &scope) {
                            return Err(hir_error(
                                "contract cannot transfer ownership into a local binding",
                            ));
                        }
                        validator.mark_value_sources_moved(value, &mut scope)?;
                    }
                    scope_checkpoint::charge::clone_type(&binding.ty)?;
                    scope.insert(
                        binding.id.clone(),
                        ValidationBinding {
                            contract_local,
                            mutable: *mutable,
                            ty: binding.ty.clone(),
                            ownership: binding.ownership,
                            availability: Availability::Available,
                            active_loans: BTreeSet::new(),
                            moved_places: BTreeMap::new(),
                            definitely_partial: BTreeSet::new(),
                        },
                    )?;
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
                $Frame::BlockAfterAssign {
                    expression,
                    statements,
                    tail,
                    index,
                    scope,
                    outer,
                    outer_ids,
                    path,
                } => {
                    let assigned_scope = scopes.pop().expect("block assign scope retained");
                    let mut scope = scope;
                    let ResolvedStatement::Assign {
                        binding,
                        field,
                        value: assigned,
                        span: assignment_span,
                        ..
                    } = &statements[index]
                    else {
                        unreachable!("assign frame resumes at an assignment statement")
                    };
                    if crate::stdin_stream_ops::hir_reopen(assigned, &binding.id) {
                        scope = assigned_scope;
                        stdin_stream::reopen_reader(&mut scope, &binding.id)?;
                    } else if crate::hir::iterator_loop::is_step_reassignment(assigned, &binding.id)
                    {
                        scope = assigned_scope;
                        iterator_loops::reopen_step(&mut scope, &binding.id)?;
                    } else if crate::string_ops::is_same_owner_concat_hir(assigned, &binding.id) {
                        scope = assigned_scope;
                        iterator_loops::reopen_string(&mut scope, &binding.id)?;
                    } else if crate::string_ops::replacement::admitted(binding, assigned) {
                        if scope
                            .get(&binding.id)
                            .is_none_or(|target| target.availability != Availability::Available)
                        {
                            return Err(hir_error(
                                "String replacement starts with an unavailable owner",
                            ));
                        }
                        scope = assigned_scope;
                        validator.mark_value_sources_moved(assigned, &mut scope)?;
                        string_replacement::reopen(&mut scope, &binding.id)?;
                    }
                    let target = scope.get(&binding.id).cloned();
                    let Some(target) = target else {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "assignment target `{}` has no enclosing binding",
                            binding.id
                        )));
                    };
                    let assigned_place = Place {
                        root: binding.id.clone(),
                        projections: field.iter().cloned().map(PlaceProjection::Field).collect(),
                    };
                    if validator.binding_has_live_overlapping_loans(
                        &target,
                        &assigned_place,
                        &assigned.id,
                        LoanPointPhase::After,
                    )? {
                        return Err(hir_error(
                            "SPX-T265: assignment would replace lexically borrowed byte storage",
                        ));
                    }
                    match field {
                        Some(field) => {
                            if target.ownership != OwnershipMode::Value {
                                return Err(hir_error(
                                    "field assignment base is not a value-owned aggregate",
                                ));
                            }
                            validator.validate_assign_field(
                                &target.ty,
                                field,
                                assigned,
                                *assignment_span,
                            )?;
                        }
                        None => {
                            validator.require_type(&target.ty, &assigned.ty, "assignment")?;
                            if (target.ownership != OwnershipMode::Value
                                || !crate::hir::is_scalar_resolved_type(&target.ty))
                                && !crate::string_ops::replacement::admitted(binding, assigned)
                                && !owner_renewal::assignment_is_admitted(
                                    validator.program,
                                    validator.byte_helpers,
                                    binding,
                                    assigned,
                                )
                            {
                                return Err(hir_error(
                                    "explicit mutation v1 supports only scalar Copy values",
                                ));
                            }
                        }
                    }
                    if target.availability != Availability::Available {
                        return Err(scope_checkpoint::paths::diagnostic(format_args!(
                            "assignment target `{}` is not available",
                            binding.id
                        )));
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
                $Frame::BlockTail {
                    expression,
                    outer_ids,
                    mut outer,
                } => {
                    let block_scope = scopes.pop().expect("block tail scope retained");
                    scope_checkpoint::merge::availability(&mut outer, &block_scope, &outer_ids)?;
                    publication.publish(&outer)?;
                    let (ResolvedExprKind::Block { tail, .. }
                    | ResolvedExprKind::VecConstructor { tail, .. }) = &expression.kind
                    else {
                        unreachable!()
                    };
                    validator.finish_expr(expression, &tail.ty, tail.ownership)?;
                    scopes.push(outer)?;
                }
                _ => unreachable!("frame was routed to its matching phase"),
            }
            Ok(())
        }
    };
}
