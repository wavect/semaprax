//! Checked caller continuation for a returned child, copy and mutation.
use super::*;
use crate::{
    ast::BinaryOp,
    construction_fusion::domain::owner_core::Runtime,
    hir::{ResolvedMatchMode, ResolvedMatchPattern},
    interpreter::{
        byte_partition::{funding::BorrowedAccount, values},
        Flow,
    },
    loan_plan::{
        LoanId,
        LoanPointPhase::{After, Before},
    },
    prelude,
};

#[derive(Clone, Copy)]
struct View {
    point: u16,
    loan: LoanId,
}
struct Read {
    argument_after: u16,
    scrutinee_after: u16,
    arm_before: [u16; 2],
    match_after: u16,
    index: u64,
    expected: u8,
}
impl View {
    fn prove(
        function: &hir::ResolvedFunction,
        expression: &ResolvedExpr,
        root_id: &hir::ValueId,
        work: &mut usize,
    ) -> Result<Self, Refusal> {
        let K::BorrowPlace { operation, place } = &expression.kind else {
            return Err(Refusal::Identity);
        };
        pay(work, operation.as_str().len().saturating_add(1))?;
        if operation.as_str() != crate::byte_ops::BYTES_AS_SLICE_ID {
            return Err(Refusal::Identity);
        }
        root(expression, root_id, work)?;
        let plan = &function.loan_plan;
        Ok(Self {
            point: iterator::endpoint(plan, expression, Before, work)?,
            loan: iterator::slice_loan(plan, expression, place, work)?,
        })
    }
}
fn byte(expression: &ResolvedExpr) -> Result<u8, Refusal> {
    match expression.kind {
        K::Uint8(value) => Ok(value),
        _ => Err(Refusal::Identity),
    }
}
impl Read {
    fn prove(
        function: &hir::ResolvedFunction,
        expression: &ResolvedExpr,
        view: &hir::ValueId,
        work: &mut usize,
    ) -> Result<Self, Refusal> {
        let K::Match {
            mode: ResolvedMatchMode::Value,
            scrutinee,
            arms,
        } = &expression.kind
        else {
            return Err(Refusal::Identity);
        };
        let [slice, index] = call(scrutinee, crate::byte_ops::GET_ID, work)? else {
            return Err(Refusal::Identity);
        };
        root(slice, view, work)?;
        let [some, none] = arms.as_slice() else {
            return Err(Refusal::Identity);
        };
        let (
            ResolvedMatchPattern::Variant {
                variant,
                case,
                fields,
            },
            ResolvedMatchPattern::Variant {
                variant: absent_variant,
                case: absent_case,
                fields: absent_fields,
            },
        ) = (&some.pattern, &none.pattern)
        else {
            return Err(Refusal::Identity);
        };
        for id in [variant, case, absent_variant, absent_case] {
            pay(work, id.as_str().len().saturating_add(1))?;
        }
        let [field] = fields.as_slice() else {
            return Err(Refusal::Identity);
        };
        pay(work, field.field.as_str().len().saturating_add(1))?;
        if variant.as_str() != prelude::OPTION_ID
            || absent_variant != variant
            || case.as_str() != prelude::OPTION_SOME_ID
            || absent_case.as_str() != prelude::OPTION_NONE_ID
            || field.field.as_str() != prelude::OPTION_SOME_VALUE_ID
            || !absent_fields.is_empty()
            || some.guard.is_some()
            || none.guard.is_some()
            || !matches!(none.value.kind, K::Bool(false))
        {
            return Err(Refusal::Identity);
        }
        let K::Binary {
            op: BinaryOp::Eq,
            left,
            right,
        } = &some.value.kind
        else {
            return Err(Refusal::Identity);
        };
        root(left, &field.binding.id, work)?;
        let plan = &function.loan_plan;
        Ok(Self {
            argument_after: iterator::endpoint(plan, slice, After, work)?,
            scrutinee_after: iterator::endpoint(plan, scrutinee, After, work)?,
            arm_before: [
                iterator::endpoint(plan, &some.value, Before, work)?,
                iterator::endpoint(plan, &none.value, Before, work)?,
            ],
            match_after: iterator::endpoint(plan, expression, After, work)?,
            index: literal(index)?,
            expected: byte(right)?,
        })
    }
    fn execute(
        &self,
        loans: &mut super::super::loans::Loans<'_, BorrowedAccount<'_>>,
        view: super::super::loans::ViewId,
    ) -> Result<bool, Refusal> {
        // A slice operand is consumed at its own After point. Capture its
        // length/indexed byte before the next edge may terminate the loan.
        loans.advance_straight_to(self.argument_after)?;
        let byte = loans.read(view, |slice| {
            usize::try_from(self.index)
                .ok()
                .and_then(|i| slice.get(i))
                .copied()
        })?;
        loans.advance_straight_to(self.scrutinee_after)?;
        loans.advance(self.arm_before[usize::from(byte.is_none())])?;
        loans.advance_straight_to(self.match_after)?;
        Ok(byte == Some(self.expected))
    }
}
struct Caller<'a> {
    producer: returned_child::Producer<'a>,
    before_copy: View,
    copy_operand_after: u16,
    store_before: u16,
    store_index: u64,
    store_byte: u8,
    after_store: View,
    copy_view: View,
    reads: [Read; 3],
    result: ResultBranch,
    success_copy_first: bool,
    failure_copy_first: bool,
}

struct ResultBranch {
    first_after: u16,
    second_before: u16,
    pair_after: u16,
    third_before: u16,
    condition_after: u16,
    arm_before: [u16; 2],
    body_after: u16,
    values: [i64; 2],
}

impl ResultBranch {
    fn execute(
        &self,
        loans: &mut super::super::loans::Loans<'_, BorrowedAccount<'_>>,
        first: bool,
        second: bool,
        third: bool,
    ) -> Result<i64, Refusal> {
        loans.advance_straight_to(self.first_after)?;
        if first {
            loans.advance(self.second_before)?;
            loans.advance_straight_to(self.pair_after)?;
        } else {
            loans.advance(self.pair_after)?;
        }
        let pair = first && second;
        if pair {
            loans.advance(self.third_before)?;
            loans.advance_straight_to(self.condition_after)?;
        } else {
            loans.advance(self.condition_after)?;
        }
        let selected = usize::from(pair && third);
        loans.advance(self.arm_before[selected])?;
        loans.advance_straight_to(self.body_after)?;
        Ok(self.values[selected])
    }
}

fn cleanup_order(
    exit: &ExitTarget,
    item: &hir::ValueId,
    copied: &hir::ValueId,
    staged: Option<(&ResolvedExpr, &ResolvedExpr)>,
    work: &mut usize,
) -> Result<bool, Refusal> {
    let [first, second] = exit.finalize_in_order.as_slice() else {
        return Err(Refusal::Identity);
    };
    let classify = |action: &crate::cleanup_plan::FinalizeAction, work: &mut usize| {
        pay(work, action.lifecycle_id.as_str().len().saturating_add(1))?;
        if action.lifecycle_id.as_str() != crate::cleanup::BYTES_DROP_LIFECYCLE_ID
            || action.active_case.is_some()
            || !action.source.projections.is_empty()
        {
            return Err(Refusal::Identity);
        }
        match (&action.source.storage, staged) {
            (StorageId::Value(id), _) => {
                pay(work, id.as_str().len().saturating_mul(2).saturating_add(1))?;
                if id == copied {
                    Ok(true)
                } else if staged.is_none() && id == item {
                    Ok(false)
                } else {
                    Err(Refusal::Identity)
                }
            }
            (
                StorageId::CallArgument {
                    call,
                    parameter_index: 0,
                    value_expression,
                },
                Some((store, owner)),
            ) => {
                pay(
                    work,
                    call.as_str()
                        .len()
                        .saturating_add(value_expression.as_str().len())
                        .saturating_add(1),
                )?;
                if call == &store.id && value_expression == &owner.id {
                    Ok(false)
                } else {
                    Err(Refusal::Identity)
                }
            }
            _ => Err(Refusal::Identity),
        }
    };
    let a = classify(first, work)?;
    (a != classify(second, work)?)
        .then_some(a)
        .ok_or(Refusal::Identity)
}

impl<'a> Caller<'a> {
    fn prove(
        source: &'a hir::byte_partition::ValidatedPartitionProgram<'_>,
        function: &'a hir::ResolvedFunction,
        success: &ExitTarget,
        work: &mut usize,
    ) -> Result<Self, Refusal> {
        if !function.ensures.is_empty() {
            return Err(Refusal::Identity);
        }
        let K::Block { statements, tail } = &function.body.kind else {
            return Err(Refusal::Identity);
        };
        let [S::Let {
            binding: item,
            mutable: true,
            value: receive,
            ..
        }, S::Let {
            binding: view,
            mutable: false,
            value: before_copy,
            ..
        }, S::Let {
            binding: copied,
            mutable: false,
            value: copy,
            ..
        }, S::Assign {
            binding: target,
            field: None,
            value: store,
            ..
        }, S::Let {
            binding: original_view,
            mutable: false,
            value: after_store,
            ..
        }, S::Let {
            binding: copied_view,
            mutable: false,
            value: copy_view,
            ..
        }, S::Let {
            binding: changed,
            mutable: false,
            value: first,
            ..
        }, S::Let {
            binding: independent,
            mutable: false,
            value: second,
            ..
        }, S::Let {
            binding: preserved,
            mutable: false,
            value: third,
            ..
        }] = statements.as_slice()
        else {
            return Err(Refusal::Identity);
        };
        let K::Call {
            callee,
            instance: None,
            type_arguments,
            args,
        } = &receive.kind
        else {
            return Err(Refusal::Identity);
        };
        if !args.is_empty() || !type_arguments.is_empty() || target.id != item.id {
            return Err(Refusal::Identity);
        }
        let mut producer = None;
        for candidate in &source.program().functions {
            pay(work, candidate.id.as_str().len().saturating_add(1))?;
            if &candidate.id == callee && producer.replace(candidate).is_some() {
                return Err(Refusal::Identity);
            }
        }
        let producer = returned_child::Producer::prove(producer.ok_or(Refusal::Identity)?, work)?;
        let [copy_operand] = call(copy, crate::byte_ops::COPY_ID, work)? else {
            return Err(Refusal::Identity);
        };
        root(copy_operand, &view.id, work)?;
        let [store_owner, index, value] = call(store, crate::byte_ops::SET_ID, work)? else {
            return Err(Refusal::Identity);
        };
        root(store_owner, &item.id, work)?;
        let mut failure_copy_first = None;
        for exit in &function.cleanup_plan.exits {
            pay(work, 1)?;
            if let ExitContinuation::ReturnFailure { source } = &exit.continuation {
                pay(work, source.expression.as_str().len().saturating_add(1))?;
                if source.expression == store.id
                    && source.lane == StatusLane::OperationFailure
                    && failure_copy_first
                        .replace(cleanup_order(
                            exit,
                            &item.id,
                            &copied.id,
                            Some((store, store_owner)),
                            work,
                        )?)
                        .is_some()
                {
                    return Err(Refusal::Identity);
                }
            }
        }
        let K::If {
            condition,
            then_branch,
            else_branch,
        } = &tail.kind
        else {
            return Err(Refusal::Identity);
        };
        let K::Binary {
            op: BinaryOp::And,
            left,
            right,
        } = &condition.kind
        else {
            return Err(Refusal::Identity);
        };
        let K::Binary {
            op: BinaryOp::And,
            left: first_test,
            right: second_test,
        } = &left.kind
        else {
            return Err(Refusal::Identity);
        };
        root(first_test, &changed.id, work)?;
        root(second_test, &independent.id, work)?;
        root(right, &preserved.id, work)?;
        let arm_value = |arm: &ResolvedExpr| match &arm.kind {
            K::Block { statements, tail } if statements.is_empty() => match tail.kind {
                K::Int(value) => Ok(value),
                _ => Err(Refusal::Identity),
            },
            _ => Err(Refusal::Identity),
        };
        let yes = arm_value(then_branch)?;
        let no = arm_value(else_branch)?;
        let plan = &function.loan_plan;
        Ok(Self {
            producer,
            before_copy: View::prove(function, before_copy, &item.id, work)?,
            copy_operand_after: iterator::endpoint(plan, copy_operand, After, work)?,
            store_before: iterator::endpoint(plan, store, Before, work)?,
            store_index: literal(index)?,
            store_byte: byte(value)?,
            after_store: View::prove(function, after_store, &item.id, work)?,
            copy_view: View::prove(function, copy_view, &copied.id, work)?,
            reads: [
                Read::prove(function, first, &original_view.id, work)?,
                Read::prove(function, second, &copied_view.id, work)?,
                Read::prove(function, third, &copied_view.id, work)?,
            ],
            result: ResultBranch {
                first_after: iterator::endpoint(plan, first_test, After, work)?,
                second_before: iterator::endpoint(plan, second_test, Before, work)?,
                pair_after: iterator::endpoint(plan, left, After, work)?,
                third_before: iterator::endpoint(plan, right, Before, work)?,
                condition_after: iterator::endpoint(plan, condition, After, work)?,
                arm_before: [
                    iterator::endpoint(plan, else_branch, Before, work)?,
                    iterator::endpoint(plan, then_branch, Before, work)?,
                ],
                body_after: iterator::endpoint(plan, &function.body, After, work)?,
                values: [no, yes],
            },
            success_copy_first: cleanup_order(success, &item.id, &copied.id, None, work)?,
            failure_copy_first: failure_copy_first.ok_or(Refusal::Identity)?,
        })
    }
}

pub(super) fn execute<'a>(
    source: &'a hir::byte_partition::ValidatedPartitionProgram<'_>,
    function: &'a hir::ResolvedFunction,
    success: &ExitTarget,
    work: &mut usize,
    mut account: BorrowedAccount<'a>,
) -> Result<Outcome, Refusal> {
    account.reserve_metadata(Layout::new::<Caller<'_>>())?;
    let caller = Caller::prove(source, function, success, work)?;
    let mut invocation = Invocation::prepare(source, function, work, account)?;
    let child = match caller.producer.execute(source, &mut invocation, work)? {
        Ok(child) => child,
        Err(outcome) => return Ok(outcome),
    };
    run_caller(&caller, &mut invocation, child)
}

fn run_caller(
    caller: &Caller<'_>,
    invocation: &mut Invocation<'_>,
    child: OwnedBytesValue,
) -> Result<Outcome, Refusal> {
    let account = invocation.account;
    let loans = &mut invocation.loans;
    let mut child = match child.bytes {
        Backing::Partition(owner) => owner,
        Backing::Whole(whole) => loans.hold_whole(whole.bytes).map_err(|(error, _)| error)?,
        _ => return Err(Refusal::Identity),
    };
    loans.advance_straight_to(caller.before_copy.point)?;
    let view = loans.borrow(&child, caller.before_copy.loan)?;
    loans.advance_straight_to(caller.copy_operand_after)?;
    let copied = loans.read(view, |bytes| account.copy(bytes))?;
    let (_, copied) = match copied {
        Ok(copied) => copied,
        Err(AllocationError::Logical) => return Ok(Outcome::AllocationFailure),
        Err(AllocationError::Authority(error)) => return Err(error),
    };
    let mut copy = loans.hold_whole(copied).map_err(|(error, _)| error)?;
    loans.advance_straight_to(caller.store_before)?;
    let stored = values::store_owner_byte(&mut child, loans, caller.store_index, caller.store_byte);
    let selected = (|| match stored {
        Ok(()) => {
            loans.advance_straight_to(caller.after_store.point)?;
            let original = loans.borrow(&child, caller.after_store.loan)?;
            loans.advance_straight_to(caller.copy_view.point)?;
            let copied = loans.borrow(&copy, caller.copy_view.loan)?;
            let first = caller.reads[0].execute(loans, original)?;
            let second = caller.reads[1].execute(loans, copied)?;
            let third = caller.reads[2].execute(loans, copied)?;
            Ok((
                Outcome::Returned(caller.result.execute(loans, first, second, third)?),
                caller.success_copy_first,
            ))
        }
        Err(Flow::Failure(status))
            if status.domain_id() == crate::byte_ops::SET_STATUS_DOMAIN
                && status.code() == crate::byte_ops::SET_INDEX_OUT_OF_BOUNDS_CODE =>
        {
            Ok((Outcome::ByteStoreOutOfBounds, caller.failure_copy_first))
        }
        Err(_) => Err(Refusal::Identity),
    })();
    if selected.is_err() {
        // Administrative refusal has no language contour. End all frame-local
        // loans before releasing the two staged owners.
        let _ = loans.abandon_views();
    }
    let copy_first = selected
        .as_ref()
        .map_or(caller.failure_copy_first, |(_, order)| *order);
    let (first, second) = if copy_first {
        (loans.settle(&mut copy), loans.settle(&mut child))
    } else {
        (loans.settle(&mut child), loans.settle(&mut copy))
    };
    let (outcome, _) = selected?;
    first?;
    second?;
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_source_returns_copies_and_mutates_partition_child() {
        let fixture = include_str!("../../../../tests/fixtures/byte-partition-private-clone.spx");
        for (text, expected) in [
            (fixture.to_owned(), Outcome::Returned(1)),
            (fixture.replace("77u8);", "76u8);"), Outcome::Returned(0)),
            (
                fixture.replace("bytes_set(item, 1usize", "bytes_set(item, 2usize"),
                Outcome::ByteStoreOutOfBounds,
            ),
        ] {
            let ast = crate::parse(&text, "partition-clone.spx").unwrap();
            let owner =
                hir::byte_partition::ValidatedPartitionProgram::compile_source(&ast, &mut {
                    usize::MAX
                })
                .unwrap();
            let (mut events, mut payload) = (0, 0);
            let account = Account::funded(&mut events, &mut payload, 8_388_608).unwrap();
            for count in 1..=2 {
                let report = super::super::execute(&owner, &account, &mut { usize::MAX });
                assert_eq!(report.outcome, expected, "{report:?}");
                assert!(report.cleanup_complete);
                assert_eq!(
                    (report.allocations, report.payload),
                    (count * 2, u64::from(count) * 8)
                );
            }

            let entry = owner
                .program()
                .functions
                .iter()
                .find(|function| function.id == owner.program().entrypoint)
                .unwrap();
            let exit = entry
                .cleanup_plan
                .exits
                .iter()
                .find(|exit| {
                    matches!(&exit.continuation, ExitContinuation::CommitResult {
                    source: CleanupResultSource::Scalar { expression },
                } if expression == &entry.body.id)
                })
                .unwrap();
            let mut work = usize::MAX;
            let caller = Caller::prove(&owner, entry, exit, &mut work).unwrap();
            let mut invocation =
                Invocation::prepare(&owner, entry, &mut work, account.borrowed()).unwrap();
            let child = caller
                .producer
                .execute(&owner, &mut invocation, &mut work)
                .unwrap()
                .unwrap();
            // Measure runtime allocations after compiler proof. The producer's
            // backing already exists, so confirm its release via the registry.
            // Include invocation teardown for any directory growth during the copy.
            let (outcome, metrics) = crate::test_allocation_observer::measure(|| {
                let outcome = run_caller(&caller, &mut invocation, child);
                assert!(invocation.loans.all_owners_settled());
                drop(invocation);
                outcome
            });
            assert_eq!(outcome.unwrap(), expected);
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
    }
}
