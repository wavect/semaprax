//! Independent reconstruction of the complete record-traversal freeze region.
//! A compiler spelling is a discovery marker only; every structural fact is
//! checked again before it can create loan authority.
use super::*;
use crate::hir::ResolvedBinding;
pub(super) mod paid;
#[cfg(test)]
pub(super) mod paid_legacy;

pub(crate) struct Proof<'a> {
    pub(crate) source: &'a ResolvedExpr,
    pub(crate) region: &'a ResolvedExpr,
}

fn place_is(expression: &ResolvedExpr, binding: &ResolvedBinding) -> bool {
    expression.ty == binding.ty
        && expression.ownership == binding.ownership
        && matches!(&expression.kind, ResolvedExprKind::Place(place) if place.root == binding.id && place.projections.is_empty())
}
// Explicit lookup retains its ordinary bounds-checked index. Row syntax keeps
// its exact snapshot; neither form exempts any root occurrence in the index.
fn field_index(
    origin: crate::hir::VecFieldReadOrigin,
    index: &ResolvedExpr,
    row_index: &ResolvedBinding,
) -> bool {
    match origin {
        crate::hir::VecFieldReadOrigin::Explicit => {
            index.ty == ResolvedType::Usize && index.ownership == OwnershipMode::Value
        }
        crate::hir::VecFieldReadOrigin::Traversal => place_is(index, row_index),
    }
}
fn usize_value(expression: &ResolvedExpr, expected: usize) -> bool {
    expression.ty == ResolvedType::Usize
        && expression.ownership == OwnershipMode::Value
        && matches!(expression.kind, ResolvedExprKind::Usize(value) if value == expected as u64)
}
fn scalar(binding: &ResolvedBinding) -> bool {
    binding.ty == ResolvedType::Usize && binding.ownership == OwnershipMode::Value
}

pub(crate) fn inspect<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    region: &'a ResolvedExpr,
) -> Result<Option<Proof<'a>>, Diagnostic> {
    inspect_counted(
        program,
        region,
        &mut WorkCounter::new(MAX_LOAN_PLAN_WORK_V1),
    )
}

pub(super) fn inspect_counted<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    region: &'a ResolvedExpr,
    work: &mut WorkCounter,
) -> Result<Option<Proof<'a>>, Diagnostic> {
    let fail = || {
        error("record traversal length, index, nominal reads or whole-root freeze proof is invalid")
    };
    let Some(Prefix {
        source,
        owner,
        element,
        counter,
        length,
        row_index,
        authored,
    }) = prefix(region, |element| {
        crate::hir::copy_record_collection::admitted(program.declarations(), element)
            || crate::hir::owned_leaf_collection::layout(program.declarations(), element)
                .is_some_and(|layout| layout.fields.is_some())
    })
    .map_err(|()| fail())?
    else {
        return Ok(None);
    };
    // Every root occurrence must be the exact child of an authenticated read
    // or an ordinary shared-borrow argument. Mutation, transfer, rebind and
    // aggregate storage are not permitted, even when all row fields are Copy.
    let mut allowed = BTreeSet::new();
    let mut pending = vec![authored];
    while let Some(expression) = pending.pop() {
        charge(work)?;
        match &expression.kind {
            ResolvedExprKind::VecFieldRead {
                origin,
                element: read_element,
                field,
                bytes,
                args,
            } => {
                if let [vector, index] = args.as_slice() {
                    if matches!(&vector.kind, ResolvedExprKind::Place(place) if place.root == owner.root)
                    {
                        if vector.ty != source.ty
                            || vector.ownership != source.ownership
                            || !matches!(&vector.kind, ResolvedExprKind::Place(place) if place == owner)
                            || read_element != element
                            || !field_index(*origin, index, row_index)
                            || crate::hir::vec_field::field(program.declarations(), element, field)
                                .and_then(|selected| selected.result_type(*bytes))
                                != Some(expression.ty.clone())
                        {
                            return Err(fail());
                        }
                        allowed.insert(vector.id.clone());
                    }
                }
            }
            // Row Loan v1 names this exact region, bound and row index; the
            // freeze covers the whole call that receives the loan.
            ResolvedExprKind::RowLoan { loan, args } => {
                if let [vector, index] = args.as_slice() {
                    if matches!(&vector.kind, ResolvedExprKind::Place(place) if place.root == owner.root)
                    {
                        if vector.ty != source.ty
                            || vector.ownership != source.ownership
                            || !matches!(&vector.kind, ResolvedExprKind::Place(place) if place == owner)
                            || loan.element != *element
                            || loan.traversal != region.id
                            || loan.bound != length.id
                            || !place_is(index, row_index)
                            || expression.ty != *element
                            || expression.ownership != OwnershipMode::Borrow
                        {
                            return Err(fail());
                        }
                        allowed.insert(vector.id.clone());
                    }
                }
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance: None,
                args,
            } if matches!(
                callee.as_str(),
                crate::vec_ops::GET_ID | crate::vec_ops::LEN_ID
            ) =>
            {
                if let Some(vector) = args.first() {
                    if matches!(&vector.kind, ResolvedExprKind::Place(place) if place.root == owner.root)
                    {
                        if vector.ty != source.ty
                            || vector.ownership != source.ownership
                            || (type_arguments.len() != 1
                                || type_arguments.first() != Some(element))
                            || !matches!(&vector.kind, ResolvedExprKind::Place(place) if place == owner)
                            || (callee.as_str() == crate::vec_ops::GET_ID
                                && (args.len() != 2
                                    || !place_is(&args[1], row_index)
                                    || !crate::hir::copy_record_collection::admitted(
                                        program.declarations(),
                                        element,
                                    )))
                            || (callee.as_str() == crate::vec_ops::LEN_ID && args.len() != 1)
                        {
                            return Err(fail());
                        }
                        allowed.insert(vector.id.clone());
                    }
                }
            }
            ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } => {
                if matches!(statements.first(), Some(ResolvedStatement::Let { binding, .. }) if binding.name == "#for-borrow-length")
                {
                    return Err(fail());
                }
                if statements.iter().any(|statement| matches!(statement, ResolvedStatement::Assign { binding, .. } if binding.id == owner.root || binding.id == counter.id || binding.id == row_index.id || binding.id == length.id)) { return Err(fail()); }
            }
            _ => {}
        }
        for (_, argument) in borrowed_call_arguments(program, expression) {
            if matches!(&argument.kind, ResolvedExprKind::Place(place) if place == owner) {
                allowed.insert(argument.id.clone());
            }
        }
        // Parent expressions are inspected before their children; an exact
        // allowed argument identity cannot authorize another root occurrence.
        if matches!(&expression.kind, ResolvedExprKind::Place(place) if place.root == owner.root)
            && !allowed.contains(&expression.id)
        {
            return Err(fail());
        }
        push_children(expression, &mut pending);
    }
    Ok(Some(Proof { source, region }))
}

pub(super) fn draft(
    program: &dyn crate::cleanup::ReplayContext,
    expression: &ResolvedExpr,
    cfg: &Cfg<'_>,
    work: &mut WorkCounter,
) -> Result<Option<CfgDraft>, Diagnostic> {
    let Some(proof) = inspect_counted(program, expression, work)? else {
        return Ok(None);
    };
    let origin = expression_place(proof.source)
        .ok_or_else(|| error("record traversal source lacks its exact place"))?;
    Ok(Some(CfgDraft {
        site: proof.region.id.clone(),
        origin,
        origin_span: proof.source.span,
        parent_root: None,
        binding: None,
        start: cfg.node(proof.region, LoanPointPhase::Before)?,
        seeds: [cfg.node(proof.region, LoanPointPhase::After)?]
            .into_iter()
            .collect(),
        cause: LoanCause::TraversalFreeze,
    }))
}

/// Borrowed `Slice<u8>` traversal: the view's resolved owner is frozen from
/// before the complete lowered region until after it. The loan is independent
/// of the view's own liveness, which may end at its last read inside the body.
#[cfg(test)]
#[allow(
    dead_code,
    reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
)]
pub(super) fn draft_bytes(
    expression: &ResolvedExpr,
    cfg: &Cfg<'_>,
    aliases: &BTreeMap<ValueId, Place>,
    work: &mut WorkCounter,
) -> Result<Option<CfgDraft>, Diagnostic> {
    let Some(region) = crate::hir::byte_traversal_region(expression)? else {
        return Ok(None);
    };
    charge(work)?;
    let place = expression_place(region.view)
        .ok_or_else(|| error("borrowed byte traversal view lacks its exact place"))?;
    Ok(Some(CfgDraft {
        site: expression.id.clone(),
        origin: resolve_origin(aliases, place, work)?,
        origin_span: region.view.span,
        parent_root: None,
        binding: None,
        start: cfg.node(expression, LoanPointPhase::Before)?,
        seeds: [cfg.node(expression, LoanPointPhase::After)?]
            .into_iter()
            .collect(),
        cause: LoanCause::TraversalFreeze,
    }))
}

pub(super) fn draft_bytes_in<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    census: &mut crate::hir::ByteTraversalSlot<'a>,
    expression: &'a ResolvedExpr,
    cfg: &Cfg<'_>,
    aliases: &BTreeMap<ValueId, Place>,
    work: &mut WorkCounter,
) -> Result<Option<CfgDraft>, Diagnostic> {
    let region = census.region(program, function, expression)?;
    let Some(region) = region else {
        return Ok(None);
    };
    charge(work)?;
    let place = expression_place(region.view)
        .ok_or_else(|| error("borrowed byte traversal view lacks its exact place"))?;
    Ok(Some(CfgDraft {
        site: expression.id.clone(),
        origin: resolve_origin(aliases, place, work)?,
        origin_span: region.view.span,
        parent_root: None,
        binding: None,
        start: cfg.node(expression, LoanPointPhase::Before)?,
        seeds: [cfg.node(expression, LoanPointPhase::After)?]
            .into_iter()
            .collect(),
        cause: LoanCause::TraversalFreeze,
    }))
}

#[cfg(test)]
mod origin_tests;
#[cfg(test)]
mod tests;

/// Row Loan v1 replay facts of one exact lowered owned-leaf traversal region,
/// reconstructed by the same structural prefix as the traversal freeze.
pub(crate) struct RowRegion<'a> {
    pub(crate) source: &'a ResolvedExpr,
    pub(crate) owner: &'a crate::hir::Place,
    pub(crate) element: &'a ResolvedType,
    pub(crate) length: &'a crate::hir::ValueId,
    pub(crate) row_index: &'a crate::hir::ValueId,
}
#[cfg(test)]
pub(crate) fn row_region<'a>(
    index: &crate::hir::DeclarationIndex,
    region: &'a ResolvedExpr,
) -> Option<RowRegion<'a>> {
    let Ok(Some(prefix)) = prefix(region, |element| {
        crate::hir::row_loan::element_admitted(index, element)
    }) else {
        return None;
    };
    Some(RowRegion {
        source: prefix.source,
        owner: prefix.owner,
        element: prefix.element,
        length: &prefix.length.id,
        row_index: &prefix.row_index.id,
    })
}

// Shared allocation-free structural prefix. Callers supply their independently
// authenticated element profile; no attached loan plan participates.
struct Prefix<'a> {
    source: &'a ResolvedExpr,
    owner: &'a crate::hir::Place,
    element: &'a ResolvedType,
    counter: &'a ResolvedBinding,
    length: &'a ResolvedBinding,
    row_index: &'a ResolvedBinding,
    authored: &'a ResolvedExpr,
}
fn prefix<'a>(
    region: &'a ResolvedExpr,
    element_admitted: impl Fn(&ResolvedType) -> bool,
) -> Result<Option<Prefix<'a>>, ()> {
    let ResolvedExprKind::Block { statements, tail } = &region.kind else {
        return Ok(None);
    };
    let Some(ResolvedStatement::Let {
        binding: marker, ..
    }) = statements.first()
    else {
        return Ok(None);
    };
    if marker.name != "#for-borrow-length" {
        return Ok(None);
    }
    let [ResolvedStatement::Let {
        binding: length,
        mutable: false,
        value: length_value,
        ..
    }, ResolvedStatement::Let {
        binding: counter,
        mutable: true,
        value: zero,
        ..
    }, ResolvedStatement::While {
        condition, body, ..
    }
    | ResolvedStatement::LexicalWhile {
        condition, body, ..
    }] = statements.as_slice()
    else {
        return Err(());
    };
    if !scalar(length)
        || !scalar(counter)
        || !usize_value(zero, 0)
        || !usize_value(tail, 0)
        || region.ty != ResolvedType::Usize
        || region.ownership != OwnershipMode::Value
    {
        return Err(());
    }
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &length_value.kind
    else {
        return Err(());
    };
    let [element] = type_arguments.as_slice() else {
        return Err(());
    };
    let [source] = args.as_slice() else {
        return Err(());
    };
    let ResolvedExprKind::Place(owner) = &source.kind else {
        return Err(());
    };
    if callee.as_str() != crate::vec_ops::LEN_ID
        || !owner.projections.is_empty()
        || !matches!(&source.ty, ResolvedType::Nominal { declaration, arguments } if declaration.as_str()==crate::prelude::VEC_ID && arguments.as_slice()==std::slice::from_ref(element))
        || !matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || !element_admitted(element)
        || length_value.ty != ResolvedType::Usize
        || length_value.ownership != OwnershipMode::Value
    {
        return Err(());
    }
    let ResolvedExprKind::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return Err(());
    };
    if !place_is(left, counter) || !place_is(right, length) || condition.ty != ResolvedType::Bool {
        return Err(());
    }
    let ResolvedExprKind::Block {
        statements: iteration,
        tail: iteration_tail,
    } = &body.kind
    else {
        return Err(());
    };
    let [ResolvedStatement::Let {
        binding: row_index,
        mutable: false,
        value: index_snapshot,
        ..
    }, ResolvedStatement::Let {
        binding: discarded,
        mutable: false,
        value: authored,
        ..
    }, ResolvedStatement::Assign {
        binding: incremented,
        field: None,
        value: increment,
        ..
    }] = iteration.as_slice()
    else {
        return Err(());
    };
    if !scalar(row_index)
        || !place_is(index_snapshot, counter)
        || incremented != counter
        || discarded.ty != authored.ty
        || discarded.ownership != authored.ownership
        || !usize_value(iteration_tail, 0)
    {
        return Err(());
    }
    let ids = [
        &owner.root,
        &length.id,
        &counter.id,
        &row_index.id,
        &discarded.id,
    ];
    if ids
        .iter()
        .enumerate()
        .any(|(index, id)| ids[..index].contains(id))
    {
        return Err(());
    }
    let ResolvedExprKind::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = &increment.kind
    else {
        return Err(());
    };
    if !place_is(left, counter) || !usize_value(right, 1) {
        return Err(());
    }
    Ok(Some(Prefix {
        source,
        owner,
        element,
        counter,
        length,
        row_index,
        authored,
    }))
}
mod borrowed;
pub(crate) use borrowed::inspect as inspect_borrowed;

pub(crate) mod row_metered;
