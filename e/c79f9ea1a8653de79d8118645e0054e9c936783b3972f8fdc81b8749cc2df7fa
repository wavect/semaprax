//! Replays each latch from ordinary typed HIR; the descriptor has no authority.
use crate::ast::BinaryOp;
use crate::cleanup_plan::renewal_profile::{
    child_refusal::Refusal, profile_refusal::ProfileRefusal,
};
use crate::hir::{
    LoopLatchKind, OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind,
    ResolvedMatchPattern, ResolvedStatement, ResolvedType, SourceForKind,
};
fn fail() -> Refusal {
    Refusal::Profile(
        ProfileRefusal::LexicalLoopLatchDiffersFromItsIndependentlyAuthenticatedLowering,
    )
}
fn place(value: &ResolvedExpr, binding: &ResolvedBinding) -> bool {
    value.ty == binding.ty
        && value.ownership == binding.ownership
        && matches!(&value.kind,ResolvedExprKind::Place(p) if p.root==binding.id && p.projections.is_empty())
}
pub(crate) fn validate_latch_inline(
    condition: &ResolvedExpr,
    body: &ResolvedExpr,
    latch: &LoopLatchKind,
) -> Result<(), Refusal> {
    match latch {
        LoopLatchKind::Condition => Ok(()),
        LoopLatchKind::OwnedIteratorStep {
            next,
            assignment,
            step,
            remainder,
        } => {
            let proof = crate::hir::iterator_loop::recognize(condition, body).ok_or_else(fail)?;
            let ResolvedExprKind::Block { statements, .. } = &body.kind else {
                return Err(fail());
            };
            let [ResolvedStatement::Assign { value, .. }] = statements.as_slice() else {
                return Err(fail());
            };
            let ResolvedExprKind::Match { arms, .. } = &value.kind else {
                return Err(fail());
            };
            let ResolvedMatchPattern::Variant { fields, .. } = &arms[1].pattern else {
                return Err(fail());
            };
            let ResolvedExprKind::Block { tail, .. } = &arms[1].value.kind else {
                return Err(fail());
            };
            if &tail.id != next
                || &value.id != assignment
                || &proof.step.id != step
                || &fields[1].binding.id != remainder
            {
                return Err(fail());
            }
            Ok(())
        }
        LoopLatchKind::SourceForStep { kind, step } => {
            if *kind == SourceForKind::OwnedIterator {
                return Err(fail());
            }
            let ResolvedExprKind::Binary {
                op: BinaryOp::Lt,
                left,
                right,
            } = &condition.kind
            else {
                return Err(fail());
            };
            let ResolvedExprKind::Block { statements, tail } = &body.kind else {
                return Err(fail());
            };
            let [
                ResolvedStatement::Let {
                    binding: item,
                    mutable: false,
                    value: read,
                    ..
                },
                ResolvedStatement::Let {
                    binding: discard,
                    mutable: false,
                    value: authored,
                    ..
                },
                ResolvedStatement::Assign {
                    binding: counter,
                    field: None,
                    value: advance,
                    ..
                },
            ] = statements.as_slice()
            else {
                return Err(fail());
            };
            let ResolvedExprKind::Binary {
                op: BinaryOp::Add,
                left: old,
                right: one,
            } = &advance.kind
            else {
                return Err(fail());
            };
            if &advance.id != step
                || counter.ty != ResolvedType::Usize
                || counter.ownership != OwnershipMode::Value
                || !place(left, counter)
                || !place(old, counter)
                || right.ty != ResolvedType::Usize
                || !matches!(&right.kind,ResolvedExprKind::Place(p) if p.projections.is_empty())
                || !matches!(one.kind, ResolvedExprKind::Usize(1))
                || one.ty != ResolvedType::Usize
                || one.ownership != OwnershipMode::Value
                || advance.ty != ResolvedType::Usize
                || advance.ownership != OwnershipMode::Value
                || condition.ty != ResolvedType::Bool
                || condition.ownership != OwnershipMode::Value
                || !matches!(tail.kind, ResolvedExprKind::Usize(0))
                || tail.ty != ResolvedType::Usize
                || tail.ownership != OwnershipMode::Value
                || item.ty != read.ty
                || item.ownership != read.ownership
                || discard.ty != authored.ty
                || discard.ownership != authored.ownership
            {
                return Err(fail());
            }
            match kind {
                SourceForKind::Copy => {
                    let ResolvedExprKind::Call {
                        callee,
                        type_arguments,
                        instance: None,
                        args,
                    } = &read.kind
                    else {
                        return Err(fail());
                    };
                    let [owner, index] = args.as_slice() else {
                        return Err(fail());
                    };
                    if callee.as_str() != crate::vec_ops::GET_ID
                        || type_arguments.as_slice() != std::slice::from_ref(&item.ty)
                        || !crate::vec_ops::resolved_element_is_admitted(&item.ty)
                        || !place(index, counter)
                        || !crate::vec_ops::resolved_vec_matches(&owner.ty, &item.ty)
                        || !matches!(&owner.kind,ResolvedExprKind::Place(p) if p.projections.is_empty())
                    {
                        return Err(fail());
                    }
                }
                SourceForKind::BorrowRecord => {
                    if item.ty != ResolvedType::Usize || !place(read, counter) {
                        return Err(fail());
                    }
                }
                SourceForKind::BorrowBytes => {
                    let proof = crate::hir::validation::byte_traversal::receiver_shape(read)
                        .ok_or_else(fail)?;
                    if item.ty != ResolvedType::U8 || !place(proof.index, counter) {
                        return Err(fail());
                    }
                }
                SourceForKind::OwnedIterator => return Err(fail()),
            }
            Ok(())
        }
    }
}
