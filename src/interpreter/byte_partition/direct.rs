//! Closed current-source direct construction receiver; no default admission.
use super::{
    funding::{Account, AllocationError},
    invocation::Invocation,
};
use crate::{
    cleanup_plan::{CleanupPlan, CleanupResultSource, ExitContinuation, StatusLane, StorageId},
    construction_fusion::{
        domain::Refusal,
        partition::{self, Preparation},
    },
    hir::{self, ResolvedExpr, ResolvedExprKind as K, ResolvedStatement as S},
    interpreter::{
        construction_fusion::{Backing, WholeBacking},
        OwnedBytesValue, Value,
    },
};
use std::alloc::Layout;
#[derive(Debug, PartialEq)]
pub(crate) enum Outcome {
    Returned(i64),
    PartitionFault(partition::Fault),
    BufferBoundExceeded,
    AllocationFailure,
    Refused,
}
#[derive(Debug, PartialEq)]
pub(crate) struct Report {
    pub(crate) outcome: Outcome,
    pub(crate) cleanup_complete: bool,
    pub(crate) allocations: u32,
    pub(crate) payload: u64,
}
fn literal(expr: &ResolvedExpr) -> Result<u64, Refusal> {
    match expr.kind {
        K::Usize(n) => Ok(n),
        _ => Err(Refusal::Identity),
    }
}
fn call<'a>(
    expr: &'a ResolvedExpr,
    id: &str,
    work: &mut usize,
) -> Result<&'a [ResolvedExpr], Refusal> {
    if let K::Call { callee, .. } = &expr.kind {
        pay(work, callee.as_str().len().saturating_add(1))?;
    }
    match &expr.kind {
        K::Call {
            callee,
            instance: None,
            args,
            ..
        } if callee.as_str() == id => Ok(args),
        _ => Err(Refusal::Identity),
    }
}
fn root(expr: &ResolvedExpr, id: &hir::ValueId, work: &mut usize) -> Result<(), Refusal> {
    if let K::Place(p) | K::BorrowPlace { place: p, .. } = &expr.kind {
        pay(work, p.root.as_str().len().saturating_add(1))?;
    }
    match &expr.kind {
        K::Place(p) | K::BorrowPlace { place: p, .. }
            if &p.root == id && p.projections.is_empty() =>
        {
            Ok(())
        }
        _ => Err(Refusal::Identity),
    }
}
fn pay(work: &mut usize, amount: usize) -> Result<(), Refusal> {
    if amount > *work {
        *work = 0;
        return Err(Refusal::Metadata);
    }
    *work -= amount;
    Ok(())
}

fn verify_unallocated_failure(
    plan: &CleanupPlan,
    producer: &ResolvedExpr,
    work: &mut usize,
) -> Result<(), Refusal> {
    let mut found = false;
    for exit in &plan.exits {
        pay(work, 1)?;
        let ExitContinuation::ReturnFailure { source } = &exit.continuation else {
            continue;
        };
        pay(work, source.expression.as_str().len().saturating_add(1))?;
        if source.lane != StatusLane::OperationFailure || source.expression != producer.id {
            continue;
        }
        if found || !exit.finalize_in_order.is_empty() {
            return Err(Refusal::Identity);
        }
        found = true;
    }
    found.then_some(()).ok_or(Refusal::Identity)
}

fn verify_partition_fault_cleanup(
    plan: &CleanupPlan,
    operation: &ResolvedExpr,
    owner: &ResolvedExpr,
    work: &mut usize,
) -> Result<(), Refusal> {
    let mut found = false;
    for exit in &plan.exits {
        pay(work, 1)?;
        let ExitContinuation::ReturnFailure { source } = &exit.continuation else {
            continue;
        };
        pay(work, source.expression.as_str().len().saturating_add(1))?;
        if source.lane != StatusLane::OperationFailure || source.expression != operation.id {
            continue;
        }
        let [action] = exit.finalize_in_order.as_slice() else {
            return Err(Refusal::Identity);
        };
        let StorageId::CallArgument {
            call,
            parameter_index: 0,
            value_expression,
        } = &action.source.storage
        else {
            return Err(Refusal::Identity);
        };
        for identity in [
            call.as_str(),
            value_expression.as_str(),
            action.lifecycle_id.as_str(),
        ] {
            pay(work, identity.len().saturating_add(1))?;
        }
        if found
            || *call != operation.id
            || *value_expression != owner.id
            || action.lifecycle_id.as_str() != crate::cleanup::BYTES_DROP_LIFECYCLE_ID
            || action.active_case.is_some()
            || !action.source.projections.is_empty()
        {
            return Err(Refusal::Identity);
        }
        found = true;
    }
    found.then_some(()).ok_or(Refusal::Identity)
}

struct PartitionStep {
    extent: u64,
    exceeds_bound: bool,
    count: u64,
    width: u64,
}

fn prove_step(
    plan: &CleanupPlan,
    statements: &[S],
    tail: &ResolvedExpr,
    exit: Option<&crate::cleanup_plan::ExitTarget>,
    work: &mut usize,
) -> Result<PartitionStep, Refusal> {
    let [S::Let {
        binding: whole_binding,
        mutable: false,
        value: producer,
        ..
    }, S::Let {
        binding: vector_binding,
        mutable: false,
        value: operation,
        ..
    }] = statements
    else {
        return Err(Refusal::Identity);
    };
    let K::Call {
        callee,
        instance: None,
        args,
        ..
    } = &producer.kind
    else {
        return Err(Refusal::Identity);
    };
    pay(work, callee.as_str().len().saturating_add(1))?;
    let (extent, exceeds_bound) = match (callee.as_str(), args.as_slice()) {
        (crate::byte_ops::ZEROED_ID, [extent]) => (literal(extent)?, false),
        (crate::byte_ops::ZEROED_BOUNDED_ID, [requested, bound]) => {
            let requested = literal(requested)?;
            let bound = literal(bound)?;
            if bound > crate::byte_ops::MAX_BUFFER_CAPACITY_BYTES {
                return Err(Refusal::Identity);
            }
            verify_unallocated_failure(plan, producer, work)?;
            (requested, requested > bound)
        }
        _ => return Err(Refusal::Identity),
    };
    let operands = partition::operation::typed(operation, work)?.ok_or(Refusal::Identity)?;
    root(operands.owner, &whole_binding.id, work)?;
    let [length] = call(tail, crate::string_ops::I64_FROM_USIZE_ID, work)? else {
        return Err(Refusal::Identity);
    };
    let [values] = call(length, crate::vec_ops::LEN_ID, work)? else {
        return Err(Refusal::Identity);
    };
    root(values, &vector_binding.id, work)?;
    let (count, width) = (literal(operands.count)?, literal(operands.width)?);
    let exit = match exit {
        Some(exit) => exit,
        None => {
            let mut selected = None;
            for region in &plan.regions {
                pay(work, 1)?;
                for slot in &region.slots {
                    pay(work, 1)?;
                    let StorageId::Value(id) = slot else { continue };
                    pay(work, id.as_str().len().saturating_add(1))?;
                    if *id != vector_binding.id {
                        continue;
                    }
                    let exit = plan
                        .exits
                        .get(region.normal_scope_end.0 as usize)
                        .filter(|exit| {
                            exit.id == region.normal_scope_end
                                && region.parent.is_some()
                                && exit.leaves_regions.as_slice() == [region.id]
                                && matches!(exit.continuation, ExitContinuation::Continue(_))
                        })
                        .ok_or(Refusal::Identity)?;
                    if selected.replace(exit).is_some() {
                        return Err(Refusal::Identity);
                    }
                }
            }
            selected.ok_or(Refusal::Identity)?
        }
    };
    let mut vector_actions = 0;
    for action in &exit.finalize_in_order {
        let bytes = match &action.source.storage {
            StorageId::Value(id) => id.as_str().len(),
            StorageId::Temporary(id) => id.as_str().len(),
            StorageId::CallArgument { call, .. } => call.as_str().len(),
            StorageId::ProvisionalResult => 0,
        };
        // Both profile selection and final settlement repeat comparisons;
        // prepay all probes once on the SAME receiving ledger.
        pay(
            work,
            bytes
                .checked_add(1)
                .and_then(|n| n.checked_mul(8))
                .ok_or(Refusal::Metadata)?,
        )?;
        if action.active_case.is_some() || !action.source.projections.is_empty() {
            return Err(Refusal::Identity);
        }
        match &action.source.storage {
            StorageId::Value(id) if *id == vector_binding.id => {
                pay(work, action.lifecycle_id.as_str().len().saturating_add(1))?;
                if action.lifecycle_id.as_str() != crate::cleanup::VEC_DROP_LIFECYCLE_ID {
                    return Err(Refusal::Identity);
                }
                vector_actions += 1;
            }
            StorageId::Value(id) if *id == whole_binding.id => {}
            StorageId::Temporary(id) if *id == producer.id || *id == operation.id => {}
            StorageId::CallArgument {
                call,
                parameter_index: 0,
                ..
            } if *call == operation.id => {}
            _ => return Err(Refusal::Identity),
        }
    }
    if vector_actions != 1 {
        return Err(Refusal::Identity);
    }
    verify_partition_fault_cleanup(plan, operation, operands.owner, work)?;
    Ok(PartitionStep {
        extent,
        exceeds_bound,
        count,
        width,
    })
}

fn execute_step(step: PartitionStep, invocation: &mut Invocation<'_>) -> Result<Outcome, Refusal> {
    let PartitionStep {
        extent,
        exceeds_bound,
        count,
        width,
    } = step;
    let handle = invocation.account;
    if exceeds_bound {
        return Ok(Outcome::BufferBoundExceeded);
    }
    let extent = usize::try_from(extent).map_err(|_| Refusal::Metadata)?;
    let (allocation, bytes) = match handle.allocate_zeroed(extent) {
        Ok(value) => value,
        Err(AllocationError::Logical) => {
            return Ok(Outcome::AllocationFailure);
        }
        Err(AllocationError::Authority(error)) => return Err(error),
    };
    let whole = OwnedBytesValue {
        allocation,
        bytes: Backing::Whole(WholeBacking {
            account: handle.identity(),
            bytes,
        }),
    };
    let mut value = match super::values::partition_whole(&mut invocation.loans, whole, count, width)
    {
        Ok(value) => value,
        Err((Preparation::Fault(fault), whole)) => {
            // Preparation returns the exact staged owner before commit.
            // The checked failure exit finalizes only that argument.
            drop(whole);
            return Ok(Outcome::PartitionFault(fault));
        }
        Err((Preparation::Authority(error), whole)) => {
            drop(whole);
            return Err(error);
        }
    };
    let Value::PartitionVec(vector) = &value else {
        return Err(Refusal::Identity);
    };
    let scalar = i64::try_from(vector.len()).map_err(|_| Refusal::Identity)?;
    super::values::settle(&mut value, &mut invocation.loans).map_err(|_| Refusal::Identity)?;
    Ok(Outcome::Returned(scalar))
}

/// The actual caller Account is borrowed through settlement. No numeric
/// forecast grants funding, and only a scalar result can leave this receiver.
pub(crate) fn execute(
    owner: &hir::byte_partition::ValidatedPartitionProgram<'_>,
    account: &Account<'_>,
    work: &mut usize,
) -> Report {
    let mut report = Report {
        outcome: Outcome::Refused,
        cleanup_complete: false,
        allocations: 0,
        payload: 0,
    };
    let result = (|| -> Result<(), Refusal> {
        owner
            .receive_current_graph(work)
            .map_err(|_| Refusal::Identity)?;
        let program = owner.program();
        let entry_id = &program.entrypoint;
        for function in &program.functions {
            pay(work, function.id.as_str().len().saturating_add(1))?;
        }
        let mut entries = program.functions.iter().filter(|f| &f.id == entry_id);
        let entry = entries.next().ok_or(Refusal::Identity)?;
        if entries.next().is_some()
            || !entry.params.is_empty()
            || !entry.requires.is_empty()
            || !entry.ensures.is_empty()
            || !entry.effects.is_empty()
            || entry.return_type != hir::ResolvedType::I64
        {
            return Err(Refusal::Identity);
        }
        let K::Block { statements, tail } = &entry.body.kind else {
            return Err(Refusal::Identity);
        };
        for exit in &entry.cleanup_plan.exits {
            pay(work, entry.body.id.as_str().len().saturating_add(1))?;
            if let ExitContinuation::CommitResult {
                source: CleanupResultSource::Scalar { expression },
            } = &exit.continuation
            {
                pay(work, expression.as_str().len().saturating_add(1))?;
            }
        }
        let mut exits = entry.cleanup_plan.exits.iter().filter(|exit| {
            matches!(&exit.continuation,
            ExitContinuation::CommitResult { source: CleanupResultSource::Scalar { expression } }
                if *expression == entry.body.id)
        });
        let exit = exits.next().ok_or(Refusal::Identity)?;
        if exits.next().is_some() {
            return Err(Refusal::Identity);
        }
        let mut handle = account.borrowed();
        use crate::construction_fusion::domain::owner_core::Runtime;
        handle.reserve_metadata(Layout::new::<Report>())?;
        handle.reserve_metadata(Layout::new::<(Option<PartitionStep>, PartitionStep)>())?;
        let (first, last) = match statements.as_slice() {
            [S::Let {
                binding,
                mutable: false,
                value,
                ..
            }, rest @ ..]
                if rest.len() == 2 =>
            {
                let K::Block {
                    statements,
                    tail: prefix_result,
                } = &value.kind
                else {
                    return Err(Refusal::Identity);
                };
                let K::Binary {
                    op: crate::ast::BinaryOp::Add,
                    left,
                    right,
                } = &tail.kind
                else {
                    return Err(Refusal::Identity);
                };
                root(left, &binding.id, work)?;
                (
                    Some(prove_step(
                        &entry.cleanup_plan,
                        statements,
                        prefix_result,
                        None,
                        work,
                    )?),
                    prove_step(&entry.cleanup_plan, rest, right, Some(exit), work)?,
                )
            }
            _ => (
                None,
                prove_step(&entry.cleanup_plan, statements, tail, Some(exit), work)?,
            ),
        };
        handle.reserve_metadata(Layout::new::<OwnedBytesValue>())?;
        handle.reserve_metadata(Layout::new::<Value>())?;
        let mut invocation = Invocation::prepare(owner, entry, work, handle)?;
        report.outcome = match first {
            None => execute_step(last, &mut invocation)?,
            Some(step) => match execute_step(step, &mut invocation)? {
                Outcome::Returned(first) => match execute_step(last, &mut invocation)? {
                    Outcome::Returned(last) => {
                        Outcome::Returned(first.checked_add(last).ok_or(Refusal::Identity)?)
                    }
                    fault => fault,
                },
                fault => fault,
            },
        };
        report.cleanup_complete = matches!(
            report.outcome,
            Outcome::Returned(_) | Outcome::PartitionFault(_) | Outcome::BufferBoundExceeded
        );
        Ok(())
    })();
    if result.is_err() {
        report.outcome = Outcome::Refused;
    }
    (report.allocations, report.payload) = account.borrowed().counters();
    report
}
#[cfg(test)]
mod tests;
