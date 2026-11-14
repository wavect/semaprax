//! A checked consuming step with a scalar-only arm and exact suffix cleanup.
use super::*;
use crate::iterator_ops;
use hir::{ResolvedMatchMode, ResolvedMatchPattern};

#[derive(Clone, Copy)]
pub(super) struct FirstChild {
    empty_result: i64,
    rest_first: bool,
}

pub(super) fn prove(
    plan: &CleanupPlan,
    tail: &ResolvedExpr,
    vector: &hir::ValueId,
    work: &mut usize,
) -> Result<FirstChild, Refusal> {
    let K::Match {
        mode: ResolvedMatchMode::Own,
        scrutinee,
        arms,
    } = &tail.kind
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

    let mut empty_result = None;
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
                if empty_result.is_none()
                    && u64::try_from(*value)
                        .is_ok_and(|value| value <= crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES) =>
            {
                empty_result = Some(*value);
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
                yield_cleanup = Some(prove_cleanup(plan, &item.id, &rest.id, work)?);
            }
            _ => return Err(Refusal::Identity),
        }
    }
    Ok(FirstChild {
        empty_result: empty_result.ok_or(Refusal::Identity)?,
        rest_first: yield_cleanup.ok_or(Refusal::Identity)?,
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
    // This fused read executes the exact checked slice/length expression
    // without publishing a view. Only the authenticated extent escapes it.
    let Value::PartitionIter(mut iter) = super::super::values::into_iterator(value, loans)
        .map_err(|(_, mut original)| {
            let _ = super::super::values::settle(&mut original, loans);
            Refusal::Identity
        })?
    else {
        return Err(Refusal::Identity);
    };
    execute_step(step, &mut iter, loans)
}

fn execute_step(
    step: FirstChild,
    iter: &mut super::super::flat::PartitionIterator,
    loans: &mut super::super::loans::Loans<'_, super::super::funding::BorrowedAccount<'_>>,
) -> Result<i64, Refusal> {
    let next = match iter.next(loans) {
        Ok(next) => next,
        Err(error) => {
            let _ = iter.settle(loans);
            return Err(error);
        }
    };
    let Some(mut child) = next else {
        iter.settle(loans)?;
        return Ok(step.empty_result);
    };
    let len = loans
        .extent(&child)
        .and_then(|len| i64::try_from(len).map_err(|_| Refusal::Identity));
    let (first, second) = if step.rest_first {
        (iter.settle(loans), loans.settle(&mut child))
    } else {
        (loans.settle(&mut child), iter.settle(loans))
    };
    let len = len?;
    first?;
    second?;
    Ok(len)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::byte_partition::{flat::PartitionVec, values};

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
        let mut vector = PartitionVec::partition(
            &mut invocation.loans,
            vec![1, 2, 3].into_boxed_slice(),
            3,
            1,
        )
        .unwrap();
        let mut retired = vector.take_last(&mut invocation.loans).unwrap().unwrap();
        let Value::PartitionIter(mut iter) =
            values::into_iterator(Value::PartitionVec(vector), &mut invocation.loans).unwrap()
        else {
            panic!("partition iterator");
        };
        invocation.loans.settle(&mut retired).unwrap();
        assert!(invocation.loans.extent(&retired).is_err());
        assert_eq!(iter.remaining(), 2);
        let step = FirstChild {
            empty_result: 0,
            rest_first: true,
        };
        assert!(execute_step(step, &mut iter, &mut invocation.loans).is_err());
        assert_eq!(iter.remaining(), 0);
    }
}
