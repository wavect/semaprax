//! Receive a checked early return from a consuming Bytes iterator.
use super::*;
use crate::{
    ast::LexicalExitKind,
    hir::{ControlTarget, FunctionExecutionId, LoopLatchKind, ResolvedMatchPattern, ResolvedType},
    interpreter::byte_partition::{authority::LoanCapacity, values},
    iterator_ops,
};

pub(super) struct Producer<'a> {
    function: &'a hir::ResolvedFunction,
    precondition: bool,
    seed: &'a [u8],
    count: u64,
    width: u64,
    fallback_extent: usize,
}

fn returns_item<'a>(
    expression: &'a ResolvedExpr,
    item: &hir::ValueId,
    work: &mut usize,
) -> Result<&'a hir::ExpressionId, Refusal> {
    let K::Block { statements, tail } = &expression.kind else {
        return Err(Refusal::Identity);
    };
    if !matches!(tail.kind, K::Int(0)) {
        return Err(Refusal::Identity);
    }
    let [S::Let {
        mutable: false,
        value,
        ..
    }] = statements.as_slice()
    else {
        return Err(Refusal::Identity);
    };
    let K::If {
        condition,
        then_branch,
        ..
    } = &value.kind
    else {
        return Err(Refusal::Identity);
    };
    if !matches!(condition.kind, K::Bool(true)) {
        return Err(Refusal::Identity);
    }
    let K::Block { statements, tail } = &then_branch.kind else {
        return Err(Refusal::Identity);
    };
    let terminal = match statements.as_slice() {
        [] => tail.as_ref(),
        [S::CopyDiscard { value, .. }] => value,
        _ => return Err(Refusal::Identity),
    };
    let K::LexicalExit {
        kind: LexicalExitKind::Return,
        value: Some(value),
        ..
    } = &terminal.kind
    else {
        return Err(Refusal::Identity);
    };
    root(value, item, work)?;
    Ok(&terminal.id)
}

impl<'a> Producer<'a> {
    pub(super) fn prove(
        function: &'a hir::ResolvedFunction,
        work: &mut usize,
    ) -> Result<Self, Refusal> {
        if function.return_type != ResolvedType::Bytes
            || !function.params.is_empty()
            || !function.ensures.is_empty()
        {
            return Err(Refusal::Identity);
        }
        let K::Block { statements, tail } = &function.body.kind else {
            return Err(Refusal::Identity);
        };
        let [S::Let {
            binding: seed_root,
            value: seed,
            mutable: false,
            ..
        }, S::Let {
            binding: input,
            value: array_view,
            mutable: false,
            ..
        }, S::Let {
            binding: whole,
            value: copy,
            mutable: false,
            ..
        }, S::Let {
            binding: children,
            value: partition,
            mutable: false,
            ..
        }, S::Let {
            value: traversal,
            mutable: false,
            ..
        }] = statements.as_slice()
        else {
            return Err(Refusal::Identity);
        };
        let K::ArrayU8(seed) = &seed.kind else {
            return Err(Refusal::Identity);
        };
        let K::BorrowPlace { operation, .. } = &array_view.kind else {
            return Err(Refusal::Identity);
        };
        pay(work, operation.as_str().len().saturating_add(1))?;
        if operation.as_str() != crate::byte_ops::ARRAY_AS_SLICE_ID {
            return Err(Refusal::Identity);
        }
        root(array_view, &seed_root.id, work)?;
        let [view] = call(copy, crate::byte_ops::COPY_ID, work)? else {
            return Err(Refusal::Identity);
        };
        root(view, &input.id, work)?;
        let operands = partition::operation::typed(partition, work)?.ok_or(Refusal::Identity)?;
        root(operands.owner, &whole.id, work)?;
        verify_partition_fault_cleanup(&function.cleanup_plan, partition, operands.owner, work)?;
        let [empty_extent] = call(tail, crate::byte_ops::ZEROED_ID, work)? else {
            return Err(Refusal::Identity);
        };

        let K::Block { statements, tail } = &traversal.kind else {
            return Err(Refusal::Identity);
        };
        let [S::Let {
            binding: step,
            value: initial,
            mutable: true,
            ..
        }, S::LexicalWhile {
            condition,
            body,
            latch: LoopLatchKind::OwnedIteratorStep { remainder, .. },
            ..
        }] = statements.as_slice()
        else {
            return Err(Refusal::Identity);
        };
        if !matches!(tail.kind, K::Int(0)) {
            return Err(Refusal::Identity);
        }
        let loop_proof = hir::iterator_loop::recognize(condition, body).ok_or(Refusal::Identity)?;
        let [iter] = call(initial, iterator_ops::NEXT_ID, work)? else {
            return Err(Refusal::Identity);
        };
        let [vector] = call(iter, iterator_ops::INTO_ITER_ID, work)? else {
            return Err(Refusal::Identity);
        };
        root(vector, &children.id, work)?;
        let item = loop_proof.owned_item.ok_or(Refusal::Identity)?;
        pay(work, step.id.as_str().len().saturating_add(1))?;
        if loop_proof.step.id != step.id {
            return Err(Refusal::Identity);
        }
        let site = returns_item(loop_proof.authored_body, &item.id, work)?;
        verify_return(function, site, remainder, work)?;
        verify_empty_return(&function.cleanup_plan, work)?;
        Ok(Self {
            function,
            precondition: prove_precondition(function, work)?,
            seed,
            count: literal(operands.count)?,
            width: literal(operands.width)?,
            fallback_extent: usize::try_from(literal(empty_extent)?)
                .map_err(|_| Refusal::Metadata)?,
        })
    }

    pub(super) fn execute(
        &self,
        source: &'a hir::byte_partition::ValidatedPartitionProgram<'_>,
        invocation: &mut Invocation<'a>,
        work: &mut usize,
    ) -> Result<Result<OwnedBytesValue, Outcome>, Refusal> {
        if !self.precondition {
            return Ok(Err(Outcome::PreconditionFailed));
        }
        let capacity = LoanCapacity::from_source(source, self.function, work)?;
        invocation.loans.enter_frame(capacity)?;
        let result = self.run(invocation);
        let exit = invocation.loans.leave_frame();
        let mut result = result?;
        if let Err(error) = exit {
            if let Ok(child) = &mut result {
                if matches!(child.bytes, Backing::Partition(_)) {
                    let _ = child.settle_partition(&mut invocation.loans);
                }
            }
            return Err(error);
        }
        Ok(result)
    }

    fn run(
        &self,
        invocation: &mut Invocation<'_>,
    ) -> Result<Result<OwnedBytesValue, Outcome>, Refusal> {
        let account = invocation.account;
        let whole = |(allocation, bytes)| OwnedBytesValue {
            allocation,
            bytes: Backing::Whole(WholeBacking {
                account: account.identity(),
                bytes,
            }),
        };
        let input = match account.copy(self.seed) {
            Ok(value) => whole(value),
            Err(AllocationError::Logical) => return Ok(Err(Outcome::AllocationFailure)),
            Err(AllocationError::Authority(error)) => return Err(error),
        };
        let loans = &mut invocation.loans;
        let value = match values::partition_whole(loans, input, self.count, self.width) {
            Ok(value) => value,
            Err((Preparation::Fault(fault), staged)) => {
                drop(staged);
                return Ok(Err(Outcome::PartitionFault(fault)));
            }
            Err((Preparation::Authority(error), _staged)) => return Err(error),
        };
        let mut iterator = iterator::into_iterator(value, loans)?;
        let child = match iterator.next_bytes(loans) {
            Ok(child) => child,
            Err(error) => {
                let _ = iterator.settle(loans);
                return Err(error);
            }
        };
        // The exact lexical return contour owns the suffix and protects the
        // transferred item as the provisional result.
        if let Err(error) = iterator.settle(loans) {
            if let Some(mut child) = child {
                let _ = child.settle_partition(loans);
            }
            return Err(error);
        }
        drop(iterator);
        if let Some(child) = child {
            return Ok(Ok(child));
        }
        match account.allocate_zeroed(self.fallback_extent) {
            Ok(value) => Ok(Ok(whole(value))),
            Err(AllocationError::Logical) => Ok(Err(Outcome::AllocationFailure)),
            Err(AllocationError::Authority(error)) => Err(error),
        }
    }
}

fn verify_return(
    function: &hir::ResolvedFunction,
    site: &hir::ExpressionId,
    remainder: &hir::ValueId,
    work: &mut usize,
) -> Result<(), Refusal> {
    let plan = &function.cleanup_plan;
    let control = plan.lexical_control.as_deref().ok_or(Refusal::Identity)?;
    for certificate in &control.exits {
        pay(work, certificate.site.as_str().len().saturating_add(1))?;
    }
    let (certificate, route) = plan.lexical_exit(site).ok_or(Refusal::Identity)?;
    let ControlTarget::Function(FunctionExecutionId::Monomorphic(function_id)) =
        &certificate.target
    else {
        return Err(Refusal::Identity);
    };
    pay(work, function_id.as_str().len().saturating_add(1))?;
    if function_id != &function.id || certificate.kind != LexicalExitKind::Return {
        return Err(Refusal::Identity);
    }
    let ExitContinuation::FunctionReturn {
        target,
        source: CleanupResultSource::Owned { storage },
        ..
    } = &route.continuation
    else {
        return Err(Refusal::Identity);
    };
    let [action] = route.finalize_in_order.as_slice() else {
        return Err(Refusal::Identity);
    };
    pay(work, remainder.as_str().len().saturating_add(1))?;
    pay(work, action.lifecycle_id.as_str().len().saturating_add(1))?;
    if target != &certificate.target
        || storage.storage != StorageId::ProvisionalResult
        || !storage.projections.is_empty()
        || !matches!(&action.source.storage, StorageId::Value(id) if id == remainder)
        || action.active_case.is_some()
        || !action.source.projections.is_empty()
        || action.lifecycle_id.as_str() != crate::cleanup::ITER_DROP_LIFECYCLE_ID
    {
        return Err(Refusal::Identity);
    }
    let resume = certificate.resume.ok_or(Refusal::Identity)?;
    let certificate = plan.lexical_resume(resume).ok_or(Refusal::Identity)?;
    let mut success = false;
    for exit in &plan.exits {
        pay(work, certificate.blocks.len().saturating_add(1))?;
        if certificate.blocks.contains(&exit.from) {
            if success
                || !exit.finalize_in_order.is_empty()
                || !matches!(&exit.continuation, ExitContinuation::CommitResult {
                    source: CleanupResultSource::Owned { storage: result },
                } if result == storage)
            {
                return Err(Refusal::Identity);
            }
            success = true;
        }
    }
    success.then_some(()).ok_or(Refusal::Identity)
}

fn verify_empty_return(plan: &CleanupPlan, work: &mut usize) -> Result<(), Refusal> {
    let control = plan.lexical_control.as_deref().ok_or(Refusal::Identity)?;
    let mut found = false;
    for id in &control.function_success_exits {
        let exit = plan.exits.get(id.0 as usize).ok_or(Refusal::Identity)?;
        for resume in &control.resumes {
            pay(work, resume.blocks.len().saturating_add(1))?;
        }
        if !plan.lexical_block_in_context(None, exit.from) {
            continue;
        }
        if found
            || !exit.finalize_in_order.is_empty()
            || !matches!(&exit.continuation, ExitContinuation::CommitResult {
                source: CleanupResultSource::Owned { storage },
            } if storage.storage == StorageId::ProvisionalResult && storage.projections.is_empty())
        {
            return Err(Refusal::Identity);
        }
        found = true;
    }
    found.then_some(()).ok_or(Refusal::Identity)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_source_return_preserves_partition_child_after_callee_cleanup() {
        let source = include_str!("../../../../tests/fixtures/byte-partition-private-clone.spx");
        let ast = crate::parse(source, "partition-return.spx").unwrap();
        let mut work = usize::MAX;
        let owner = hir::byte_partition::ValidatedPartitionProgram::compile_source(&ast, &mut work)
            .unwrap();
        let [function, entry] = owner.program().functions.as_slice() else {
            panic!("fixture function inventory");
        };
        let producer = Producer::prove(function, &mut work).unwrap();
        let (mut allocations, mut payload) = (0, 0);
        let account = Account::funded(&mut allocations, &mut payload, 8_388_608).unwrap();
        let mut invocation =
            Invocation::prepare(&owner, entry, &mut work, account.borrowed()).unwrap();
        let mut child = producer
            .execute(&owner, &mut invocation, &mut work)
            .unwrap()
            .unwrap();
        assert_eq!(invocation.account.counters(), (1, 6));
        assert_eq!(child.allocation, 1);
        let Backing::Partition(range) = &mut child.bytes else {
            panic!("actual partition child");
        };
        let copy = invocation.loans.copy_out_and_settle(range).unwrap();
        assert_eq!(copy.as_ref(), &[0, 255]);
        assert_eq!(invocation.account.counters(), (2, 8));
    }
}
