//! Same source-move walk; immutable checkpoints replace owned branch maps.
use super::super::*;
impl HirValidator<'_> {
    pub(in crate::hir::validation) fn mark_value_sources_moved(
        &self,
        expression: &ResolvedExpr,
        scope: &mut dyn ScopeWrite,
    ) -> Result<(), Diagnostic> {
        #[cfg(test)]
        if tests::before_selected() {
            return self.mark_value_sources_moved_before_inert_root(expression, scope);
        }
        if let Some(scope) = scope.old_mut() {
            return self.mark_value_sources_moved_owned_reference(expression, scope);
        }
        // The original walker does nothing for these exact root shapes. This
        // is only a source-move pass: full expression validation already has
        // its own owner, and every propagating root retains the old driver.
        // Fund the additional required-reservation argument/formal and bool
        // return/receiving, plus the borrowed match scrutinee, before formation.
        // The existing outbound Result and static refusal constructor retain
        // their roles; no forwarding Result/helper or owned selector is added.
        {
            #[cfg(test)]
            let _operation = crate::bounded_output::stage_debits::operation_guard(
                crate::bounded_output::stage_debits::Operation::ScopeCheckpoint,
            );
            #[cfg(feature = "unstable-workflow-profiling")]
            let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
                crate::bounded_output::exclusive_debits::Operation::ScopeCheckpoint,
            );
            if !crate::bounded_output::reserve_active_required(INERT_ROOT_CONTROLS) {
                // Same allocation-free diagnostic and complete outbound Result
                // as the original first Storage payer. It is selected before
                // touching scope, including a pre-existing sticky refusal.
                return Err(Diagnostic::io_static(
                    "SPX-H006",
                    "ownership scope checkpoint exceeds the active validation budget",
                ));
            }
        }
        match &expression.kind {
            ResolvedExprKind::Closure { .. }
            | ResolvedExprKind::FunctionReference { .. }
            | ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::BorrowedUtf8Literal(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::ByteRange { .. }
            | ResolvedExprKind::Call { .. }
            | ResolvedExprKind::LiteralFormat { .. }
            | ResolvedExprKind::VecFieldRead { .. }
            | ResolvedExprKind::RowLoan { .. }
            | ResolvedExprKind::NativeRustImportCall(_)
            | ResolvedExprKind::HostCommandCall(_)
            | ResolvedExprKind::Unary { .. }
            | ResolvedExprKind::Binary { .. }
            | ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::Yield { .. }
            | ResolvedExprKind::UpdateRecord { .. } => return Ok(()),
            ResolvedExprKind::LexicalExit { .. }
            | ResolvedExprKind::Invoke { .. }
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::Block { .. }
            | ResolvedExprKind::VecConstructor { .. }
            | ResolvedExprKind::If { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Upcast { .. } => {}
        }
        enum Frame<'a> {
            Enter(&'a ResolvedExpr, usize),
            AfterThen {
                else_branch: &'a ResolvedExpr,
                parent: usize,
                then_scope: usize,
                ids: scope_checkpoint::IdSnapshot,
            },
            AfterElse {
                parent: usize,
                else_scope: usize,
                ids: scope_checkpoint::IdSnapshot,
                then_bindings: scope_checkpoint::Owned,
            },
            AfterMatchArm {
                arms: &'a [ResolvedMatchArm],
                index: usize,
                parent: usize,
                arm_scope: usize,
                ids: scope_checkpoint::IdSnapshot,
                arm_scopes: scope_checkpoint::Storage<scope_checkpoint::Owned>,
            },
        }
        let root = scope.take_owned();
        let mut scopes = scope_checkpoint::Storage::new();
        scopes.push(root)?;
        let mut frames = scope_checkpoint::Storage::new();
        frames.push(Frame::Enter(expression, 0))?;
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(expression, scope_index) => match &expression.kind {
                    ResolvedExprKind::LexicalExit { value, .. } => {
                        if let Some(value) = value {
                            frames.push(Frame::Enter(value, scope_index))?;
                        }
                    }
                    ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::FunctionReference { .. } => {}
                    ResolvedExprKind::Invoke { args, .. } => {
                        for arg in args.iter().rev() {
                            frames.push(Frame::Enter(arg, scope_index))?;
                        }
                    }
                    ResolvedExprKind::Place(place) => {
                        let Some(binding) = scopes[scope_index].get(&place.root) else {
                            continue;
                        };
                        let (place_ty, place_ownership) = self.resolve_place(place, binding)?;
                        let should_move = self.is_owned_resource(&place_ty, place_ownership)?
                            && Self::place_availability(binding, &place.projections)
                                == Availability::Available;
                        if should_move {
                            if self.binding_has_live_overlapping_loans(
                                binding,
                                place,
                                &expression.id,
                                LoanPointPhase::Before,
                            )? {
                                return Err(hir_error(
                                    "SPX-T265: move or transfer would invalidate a lexical byte view",
                                ));
                            }
                            let binding =
                                scopes[scope_index].get_mut(&place.root)?.ok_or_else(|| {
                                    hir_error(format!(
                                    "resolved value `{}` disappeared during ownership validation",
                                    place.root
                                ))
                                })?;
                            if place.projections.is_empty() {
                                binding.availability = Availability::Moved;
                            } else {
                                scope_checkpoint::move_projection(binding, &place.projections)?;
                            }
                        }
                    }
                    ResolvedExprKind::Block { tail, .. }
                    | ResolvedExprKind::VecConstructor { tail, .. } => {
                        frames.push(Frame::Enter(tail, scope_index))?;
                    }
                    ResolvedExprKind::If {
                        then_branch,
                        else_branch,
                        ..
                    } => {
                        let ids = scopes[scope_index].ids();
                        let then_scope = scopes.len();
                        scopes.push(scopes[scope_index].clone())?;
                        frames.push(Frame::AfterThen {
                            else_branch,
                            parent: scope_index,
                            then_scope,
                            ids,
                        })?;
                        frames.push(Frame::Enter(then_branch, then_scope))?;
                    }
                    ResolvedExprKind::Match { arms, .. } => {
                        if let Some(first) = arms.first() {
                            let ids = scopes[scope_index].ids();
                            let arm_scope = scopes.len();
                            scopes.push(scopes[scope_index].clone())?;
                            frames.push(Frame::AfterMatchArm {
                                arms,
                                index: 0,
                                parent: scope_index,
                                arm_scope,
                                ids,
                                arm_scopes: scope_checkpoint::Storage::with_capacity(arms.len())?,
                            })?;
                            frames.push(Frame::Enter(&first.value, arm_scope))?;
                        }
                    }
                    ResolvedExprKind::Project { base, .. }
                    | ResolvedExprKind::Upcast { source: base } => {
                        frames.push(Frame::Enter(base, scope_index))?;
                    }
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::String(_)
                    | ResolvedExprKind::BorrowedUtf8Literal(_)
                    | ResolvedExprKind::ArrayU8(_)
                    | ResolvedExprKind::RepeatArrayU8 { .. }
                    | ResolvedExprKind::BorrowPlace { .. }
                    | ResolvedExprKind::ByteRange { .. }
                    | ResolvedExprKind::Call { .. }
                    | ResolvedExprKind::LiteralFormat { .. }
                    | ResolvedExprKind::VecFieldRead { .. }
                    | ResolvedExprKind::RowLoan { .. }
                    | ResolvedExprKind::NativeRustImportCall(_)
                    | ResolvedExprKind::HostCommandCall(_)
                    | ResolvedExprKind::Unary { .. }
                    | ResolvedExprKind::Binary { .. }
                    | ResolvedExprKind::ConstructRecord { .. }
                    | ResolvedExprKind::ConstructVariant { .. }
                    | ResolvedExprKind::Try { .. }
                    | ResolvedExprKind::TryOption { .. }
                    | ResolvedExprKind::Yield { .. }
                    | ResolvedExprKind::UpdateRecord { .. } => {}
                },
                Frame::AfterThen {
                    else_branch,
                    parent,
                    then_scope,
                    ids,
                } => {
                    debug_assert_eq!(then_scope + 1, scopes.len());
                    let then_bindings = scopes.pop().expect("active move branch retained");
                    let else_scope = scopes.len();
                    scopes.push(scopes[parent].clone())?;
                    frames.push(Frame::AfterElse {
                        parent,
                        else_scope,
                        ids,
                        then_bindings,
                    })?;
                    frames.push(Frame::Enter(else_branch, else_scope))?;
                }
                Frame::AfterElse {
                    parent,
                    else_scope,
                    ids,
                    then_bindings,
                } => {
                    debug_assert_eq!(else_scope + 1, scopes.len());
                    let else_bindings = scopes.pop().expect("active move branch retained");
                    scope_checkpoint::merge::branches(
                        &mut scopes[parent],
                        &then_bindings,
                        &else_bindings,
                        &ids,
                    )?;
                }
                Frame::AfterMatchArm {
                    arms,
                    index,
                    parent,
                    arm_scope,
                    ids,
                    mut arm_scopes,
                } => {
                    debug_assert_eq!(arm_scope + 1, scopes.len());
                    arm_scopes.push(scopes.pop().expect("active match move branch retained"))?;
                    let next = index + 1;
                    if let Some(arm) = arms.get(next) {
                        let arm_scope = scopes.len();
                        scopes.push(scopes[parent].clone())?;
                        frames.push(Frame::AfterMatchArm {
                            arms,
                            index: next,
                            parent,
                            arm_scope,
                            ids,
                            arm_scopes,
                        })?;
                        frames.push(Frame::Enter(&arm.value, arm_scope))?;
                    } else if let Some((first, rest)) = arm_scopes.split_first() {
                        let mut joined = first.clone();
                        for arm_scope in rest {
                            scope_checkpoint::merge::conditional(&mut joined, arm_scope, &ids)?;
                        }
                        scope_checkpoint::merge::availability(&mut scopes[parent], &joined, &ids)?;
                    }
                }
            }
        }
        scope.replace_owned(scopes.pop().expect("root move scope retained"));
        Ok(())
    }
}

// One real primitive site, no new fallible wrapper or helper call. The bool
// branch reads its original primitive result; match creates no bool predicate.
const INERT_ROOT_CONTROLS: usize = 2 * std::mem::size_of::<usize>()
    + 3 * std::mem::size_of::<bool>()
    + std::mem::size_of::<&ResolvedExprKind>();

#[cfg(test)]
mod before;
#[cfg(test)]
mod tests;
