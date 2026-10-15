//! Exact same-owner String append receiving facts without Debug formatting.
use super::{DEPTH, prepay};
use super::{Refusal, error};
use crate::cleanup_plan::ReplayPhysicalReservation;
use crate::hir::{OwnershipMode, ResolvedType};
use crate::hir::ResolvedBinding;
pub(crate) fn admitted(
    binding: &ResolvedBinding,
    value: &crate::hir::ResolvedExpr,
    physical: &mut ReplayPhysicalReservation,
    charge: &mut dyn FnMut(usize) -> Result<(), Refusal>,
) -> Result<bool, Refusal> {
    prepay::<bool>(physical, charge)?;
    let crate::hir::ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return Ok(false);
    };
    if callee.as_str() != crate::string_ops::CONCAT_ID {
        return Ok(false);
    }
    let [left, right] = args.as_slice() else {
        return Err(error());
    };
    if binding.ty != ResolvedType::String
        || binding.ownership != OwnershipMode::Own
        || value.ty != ResolvedType::String
        || value.ownership != OwnershipMode::Own
        || !type_arguments.is_empty()
        || args
            .iter()
            .any(|a| a.ty != ResolvedType::String || a.ownership != OwnershipMode::Own)
    {
        return Err(error());
    }
    let crate::hir::ResolvedExprKind::Place(place) = &left.kind else {
        return Ok(false);
    };
    if place.root != binding.id || !place.projections.is_empty() {
        return Ok(false);
    }
    type Frame<'a> = Option<crate::hir::lexical_exit::ChildCursor<'a>>;
    physical
        .reserve(std::mem::size_of::<[Frame<'_>; DEPTH]>())
        .map_err(|_| error())?;
    let mut frames = [None; DEPTH];
    frames[0] = Some(crate::hir::lexical_exit::ChildCursor::new(right));
    let mut depth = 1;
    while depth != 0 {
        charge(1)?;
        let cursor = frames[depth - 1].as_mut().ok_or_else(error)?;
        if !cursor.is_started() {
            let node = cursor.expression();
            charge(binding.id.as_str().len() + 1)?;
            if matches!(&node.kind,crate::hir::ResolvedExprKind::Place(p) if p.root==binding.id) {
                return Err(error());
            }
        }
        if let Some(child) = cursor.next() {
            if depth == DEPTH {
                return Err(error());
            }
            frames[depth] = Some(crate::hir::lexical_exit::ChildCursor::new(child));
            depth += 1;
        } else {
            frames[depth - 1] = None;
            depth -= 1;
        }
    }
    Ok(true)
}
