//! A checked consuming step with exact loan edges and suffix cleanup.
use super::*;
use crate::{
    iterator_ops,
    loan_plan::{LoanCause, LoanId, LoanPlan, LoanPointPhase},
};
use hir::{ResolvedMatchMode, ResolvedMatchPattern};

#[derive(Clone, Copy)]
pub(super) struct FirstChild {
    empty_result: i64,
    empty_before: u16,
    scrutinee_after: u16,
    match_after: u16,
    rest_first: bool,
    read: ChildRead,
}

#[derive(Clone, Copy)]
struct ChildRead {
    loan: LoanId,
    arm_before: u16,
    view_before: u16,
    operand_after: u16,
    arm_after: u16,
}

fn endpoint(
    plan: &LoanPlan,
    expression: &ResolvedExpr,
    phase: LoanPointPhase,
    work: &mut usize,
) -> Result<u16, Refusal> {
    let mut point = None;
    for (index, endpoint) in plan.endpoints.iter().enumerate() {
        pay(
            work,
            endpoint.point.expression.as_str().len().saturating_add(1),
        )?;
        if endpoint.point.expression == expression.id
            && endpoint.point.phase == phase
            && point
                .replace(u16::try_from(index).map_err(|_| Refusal::Identity)?)
                .is_some()
        {
            return Err(Refusal::Identity);
        }
    }
    point.ok_or(Refusal::Identity)
}

fn slice_loan(
    plan: &LoanPlan,
    view: &ResolvedExpr,
    place: &hir::Place,
    work: &mut usize,
) -> Result<LoanId, Refusal> {
    let mut id = None;
    for loan in &plan.loans {
        pay(work, loan.site.as_str().len().saturating_add(1))?;
        if loan.site != view.id {
            continue;
        }
        pay(work, loan.origin.root.as_str().len().saturating_add(1))?;
        if loan.origin != *place
            || loan.cause != LoanCause::SliceView
            || loan.parent.is_some()
            || id.replace(loan.id).is_some()
        {
            return Err(Refusal::Identity);
        }
    }
    id.ok_or(Refusal::Identity)
}

pub(super) fn prove(
    function: &hir::ResolvedFunction,
    tail_match: &ResolvedExpr,
    vector: &hir::ValueId,
    work: &mut usize,
) -> Result<FirstChild, Refusal> {
    let K::Match {
        mode: ResolvedMatchMode::Own,
        scrutinee,
        arms,
    } = &tail_match.kind
    else {
        return Err(Refusal::Identity);
    };
    let [iter] = call(scrutinee, iterator_ops::NEXT_ID, work)? else {
        return Err(Refusal::Identity);
    };
    let [values] = call(iter, iterator_ops::INTO_ITER_ID, work)? else {
        return Err(Refusal::Identity);
    };
    root(values, vector, work)?;

    let plan = &function.loan_plan;
    let mut empty = None;
    let mut yield_cleanup = None;
    for arm in arms {
        let ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } = &arm.pattern
        else {
            return Err(Refusal::Identity);
        };
        for id in [variant, case] {
            pay(work, id.as_str().len().saturating_add(1))?;
        }
        if variant.as_str() != iterator_ops::STEP_ID || arm.guard.is_some() {
            return Err(Refusal::Identity);
        }
        match (case.as_str(), fields.as_slice(), &arm.value.kind) {
            (iterator_ops::DONE_ID, [], K::Int(value))
                if empty.is_none()
                    && u64::try_from(*value)
                        .is_ok_and(|value| value <= crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES) =>
            {
                empty = Some((
                    *value,
                    endpoint(plan, &arm.value, LoanPointPhase::Before, work)?,
                ));
            }
            (iterator_ops::YIELD_ID, [first, second], K::Block { statements, tail })
                if yield_cleanup.is_none() =>
            {
                for field in [first, second] {
                    pay(work, field.field.as_str().len().saturating_add(1))?;
                }
                let (item, rest) = match (first.field.as_str(), second.field.as_str()) {
                    (iterator_ops::ITEM_ID, iterator_ops::REST_ID) => {
                        (&first.binding, &second.binding)
                    }
                    (iterator_ops::REST_ID, iterator_ops::ITEM_ID) => {
                        (&second.binding, &first.binding)
                    }
                    _ => return Err(Refusal::Identity),
                };
                let [S::Let {
                    binding,
                    mutable: false,
                    value,
                    ..
                }] = statements.as_slice()
                else {
                    return Err(Refusal::Identity);
                };
                let K::BorrowPlace { operation, place } = &value.kind else {
                    return Err(Refusal::Identity);
                };
                pay(work, operation.as_str().len().saturating_add(1))?;
                pay(work, place.root.as_str().len().saturating_add(1))?;
                if operation.as_str() != crate::byte_ops::BYTES_AS_SLICE_ID
                    || place.root != item.id
                    || !place.projections.is_empty()
                {
                    return Err(Refusal::Identity);
                }
                let [len] = call(tail, crate::string_ops::I64_FROM_USIZE_ID, work)? else {
                    return Err(Refusal::Identity);
                };
                let [view] = call(len, crate::byte_ops::LEN_ID, work)? else {
                    return Err(Refusal::Identity);
                };
                root(view, &binding.id, work)?;
                let read = ChildRead {
                    loan: slice_loan(plan, value, place, work)?,
                    arm_before: endpoint(plan, &arm.value, LoanPointPhase::Before, work)?,
                    view_before: endpoint(plan, value, LoanPointPhase::Before, work)?,
                    operand_after: endpoint(plan, view, LoanPointPhase::After, work)?,
                    arm_after: endpoint(plan, &arm.value, LoanPointPhase::After, work)?,
                };
                let rest_first = prove_cleanup(&function.cleanup_plan, &item.id, &rest.id, work)?;
                yield_cleanup = Some((rest_first, read));
            }
            _ => return Err(Refusal::Identity),
        }
    }
    let (rest_first, read) = yield_cleanup.ok_or(Refusal::Identity)?;
    let (empty_result, empty_before) = empty.ok_or(Refusal::Identity)?;
    Ok(FirstChild {
        empty_result,
        empty_before,
        scrutinee_after: endpoint(plan, scrutinee, LoanPointPhase::After, work)?,
        match_after: endpoint(plan, tail_match, LoanPointPhase::After, work)?,
        rest_first,
        read,
    })
}

fn prove_cleanup(
    plan: &CleanupPlan,
    item: &hir::ValueId,
    rest: &hir::ValueId,
    work: &mut usize,
) -> Result<bool, Refusal> {
    let mut order = None;
    for region in &plan.regions {
        pay(work, 1)?;
        let mut holds_item = false;
        for slot in &region.slots {
            pay(work, 1)?;
            if let StorageId::Value(id) = slot {
                pay(work, id.as_str().len().saturating_add(1))?;
                holds_item |= id == item;
            }
        }
        if !holds_item {
            continue;
        }
        let exit = plan
            .exits
            .get(region.normal_scope_end.0 as usize)
            .filter(|exit| {
                exit.id == region.normal_scope_end
                    && exit.leaves_regions.as_slice() == [region.id]
                    && matches!(exit.continuation, ExitContinuation::Continue(_))
            })
            .ok_or(Refusal::Identity)?;
        let [first, second] = exit.finalize_in_order.as_slice() else {
            return Err(Refusal::Identity);
        };
        let mut drops_rest = [false; 2];
        for (index, action) in [first, second].into_iter().enumerate() {
            let StorageId::Value(id) = &action.source.storage else {
                return Err(Refusal::Identity);
            };
            pay(work, id.as_str().len().saturating_add(1).saturating_mul(2))?;
            pay(work, action.lifecycle_id.as_str().len().saturating_add(1))?;
            if action.active_case.is_some() || !action.source.projections.is_empty() {
                return Err(Refusal::Identity);
            }
            drops_rest[index] = match (id, action.lifecycle_id.as_str()) {
                (id, crate::cleanup::ITER_DROP_LIFECYCLE_ID) if id == rest => true,
                (id, crate::cleanup::BYTES_DROP_LIFECYCLE_ID) if id == item => false,
                _ => return Err(Refusal::Identity),
            };
        }
        if drops_rest[0] == drops_rest[1] || order.replace(drops_rest[0]).is_some() {
            return Err(Refusal::Identity);
        }
    }
    order.ok_or(Refusal::Identity)
}

pub(super) fn execute(
    step: FirstChild,
    value: Value,
    invocation: &mut Invocation<'_>,
) -> Result<i64, Refusal> {
    let loans = &mut invocation.loans;
    let Value::PartitionIter(mut iter) = super::super::values::into_iterator(value, loans)
        .map_err(|(_, mut original)| {
            let _ = super::super::values::settle(&mut original, loans);
            Refusal::Identity
        })?
    else {
        return Err(Refusal::Identity);
    };
    if let Err(error) = loans.advance_straight_to(step.scrutinee_after) {
        let _ = iter.settle(loans);
        return Err(error);
    }
    execute_step(step, &mut iter, loans)
}

fn execute_step(
    step: FirstChild,
    iter: &mut super::super::flat::PartitionIterator,
    loans: &mut super::super::loans::Loans<'_, super::super::funding::BorrowedAccount<'_>>,
) -> Result<i64, Refusal> {
    let next = match iter.next_bytes(loans) {
        Ok(next) => next,
        Err(error) => {
            let _ = iter.settle(loans);
            return Err(error);
        }
    };
    let Some(mut child) = next else {
        iter.settle(loans)?;
        loans.advance(step.empty_before)?;
        loans.advance_straight_to(step.match_after)?;
        return Ok(step.empty_result);
    };
    let Backing::Partition(owner) = &mut child.bytes else {
        unreachable!("typed iterator published a non-partition child");
    };
    let len = (|| {
        let read = step.read;
        loans.advance(read.arm_before)?;
        loans.advance_straight_to(read.view_before)?;
        let view = loans.borrow(owner, read.loan)?;
        loans.advance_straight_to(read.operand_after)?;
        let len = loans.read(view, <[u8]>::len)?;
        loans.advance_straight_to(read.arm_after)?;
        i64::try_from(len).map_err(|_| Refusal::Identity)
    })();
    let (first, second) = if step.rest_first {
        (iter.settle(loans), loans.settle(owner))
    } else {
        (loans.settle(owner), iter.settle(loans))
    };
    let len = len?;
    first?;
    second?;
    loans.advance(step.match_after)?;
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::byte_partition::values;

    #[test]
    fn refused_iterator_step_settles_the_untransferred_suffix() {
        let source = include_str!("../../../../tests/fixtures/byte-partition-private-iterator.spx");
        let ast = crate::parse(source, "partition-iterator.spx").unwrap();
        let mut work = usize::MAX;
        let owner = hir::byte_partition::ValidatedPartitionProgram::compile_source(&ast, &mut work)
            .unwrap();
        let mut events = 0;
        let mut payload = 0;
        let account = Account::funded(&mut events, &mut payload, 8_388_608).unwrap();
        let mut invocation = Invocation::prepare(
            &owner,
            &owner.program().functions[0],
            &mut work,
            account.borrowed(),
        )
        .unwrap();
        let (allocation, bytes) = invocation.account.copy(&[1, 2, 3]).unwrap();
        let whole = OwnedBytesValue {
            allocation,
            bytes: Backing::Whole(WholeBacking {
                account: invocation.account.identity(),
                bytes,
            }),
        };
        let Value::PartitionVec(mut vector) =
            values::partition_whole(&mut invocation.loans, whole, 3, 1).unwrap()
        else {
            panic!("partition vector");
        };
        let mut retired = vector.take_last(&mut invocation.loans).unwrap().unwrap();
        let Value::PartitionIter(mut iter) =
            values::into_iterator(Value::PartitionVec(vector), &mut invocation.loans).unwrap()
        else {
            panic!("partition iterator");
        };
        invocation.loans.settle(&mut retired).unwrap();
        assert!(invocation.loans.extent(&retired).is_err());
        assert_eq!(iter.remaining(), 2);
        let function = &owner.program().functions[0];
        let K::Block { statements, tail } = &function.body.kind else {
            panic!("partition iterator body");
        };
        let S::Let { binding, .. } = &statements[1] else {
            panic!("partition vector");
        };
        let step = prove(function, tail, &binding.id, &mut work).unwrap();
        assert!(execute_step(step, &mut iter, &mut invocation.loans).is_err());
        assert_eq!(iter.remaining(), 0);
    }
}
