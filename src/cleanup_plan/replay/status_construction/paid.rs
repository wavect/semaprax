//! Complete status reconstruction with closed semantic data and same-ledger banks.
use super::{
    checked_status, expression_children, BinaryOp, DeclarationId, ReplayBudget, ResolvedExpr,
    ResolvedExprKind, ResolvedFunction, StatusCase, StatusLane, StatusProducer, StatusSource,
    StatusSourceId, UnaryOp,
};
use crate::cleanup_plan::renewal_profile::{child_refusal::Refusal, status_refusal::StatusRefusal};
use crate::cleanup_plan::replay::selected_context::child::receiving::receive;
use crate::cleanup_plan::replay::selected_context::child::{
    ArrayReceiving, CloneReceiving, Permit, VecReceiving,
};
use crate::cleanup_plan::replay::{semantic_refusal, semantic_refusal::Error as _};
type Error = crate::cleanup_plan::replay::expression_facts::errors::EngineError;

pub(in crate::cleanup_plan::replay) fn validate_required_status_sources(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let permit = Permit::profile::<()>(function, budget)?;
    if !program.interfaces().is_empty() {
        return Err(permit.refuse(Refusal::Status(StatusRefusal::Interfaces)));
    }
    let mut expected = Vec::new();
    for (ordinal, contract) in function.requires.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected, budget)?;
        receive!(
            function,
            budget,
            permit,
            VecReceiving<'_, StatusSource>,
            10,
            budget.child_reserve_vec(function, &mut expected, 1, "contract status vector")
        );
        expected.push(StatusSource {
            id: StatusSourceId {
                expression: contract.id.clone(),
                lane: StatusLane::ContractFalse,
            },
            producer: StatusProducer::ContractFalse {
                phase: crate::cleanup_plan::ContractPhase::Requires,
                ordinal: match u32::try_from(ordinal) {
                    Ok(value) => value,
                    Err(_) => {
                        return Err(permit.refuse(Refusal::Status(StatusRefusal::Preconditions)))
                    }
                },
            },
        });
    }
    collect_expression_statuses(program, function, &function.body, &mut expected, budget)?;
    for (ordinal, contract) in function.ensures.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected, budget)?;
        receive!(
            function,
            budget,
            permit,
            VecReceiving<'_, StatusSource>,
            10,
            budget.child_reserve_vec(function, &mut expected, 1, "contract status vector")
        );
        expected.push(StatusSource {
            id: StatusSourceId {
                expression: contract.id.clone(),
                lane: StatusLane::ContractFalse,
            },
            producer: StatusProducer::ContractFalse {
                phase: crate::cleanup_plan::ContractPhase::Ensures,
                ordinal: match u32::try_from(ordinal) {
                    Ok(value) => value,
                    Err(_) => {
                        return Err(permit.refuse(Refusal::Status(StatusRefusal::Postconditions)))
                    }
                },
            },
        });
    }
    #[cfg(test)]
    let equal = if program.borrowed_helper_lease(function).is_some() {
        super::borrowed::equal(
            function,
            &expected,
            &function.cleanup_plan.status_sources,
            budget,
        )?
    } else {
        function.cleanup_plan.status_sources == expected
    };
    #[cfg(not(test))]
    let equal = function.cleanup_plan.status_sources == expected;
    if !equal {
        return Err(permit.refuse(Refusal::Status(StatusRefusal::Coverage)));
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) fn collect_expression_statuses(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    statuses: &mut Vec<StatusSource>,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    let permit = Permit::profile::<()>(function, budget)?;

    receive!(
        function,
        budget,
        permit,
        ArrayReceiving<'_>,
        9,
        budget.child_reserve_array::<Option<expression_children::ReplayChildCursor<'_>>>(
            function,
            514,
            "status expression cursor",
        )
    );
    #[cfg(test)]
    let borrowed = program.borrowed_helper_lease(function);
    let mut stack = [None; 514];
    stack[0] = Some(expression_children::ReplayChildCursor::new(expression));
    let mut len = 1usize;
    while len != 0 {
        #[cfg(test)]
        if borrowed.is_some() {
            budget.charge(function, 1, "borrowed status actual cursor frame")?;
        }
        len -= 1;
        let mut cursor = stack[len].take().expect("status frame retained");
        let expression = cursor.expression();
        if let Some(child) = cursor.next() {
            if len + 2 > stack.len() {
                return Err(permit.refuse(Refusal::Status(StatusRefusal::Depth)));
            }
            stack[len] = Some(cursor);
            stack[len + 1] = Some(expression_children::ReplayChildCursor::new(child));
            len += 2;
            continue;
        }
        match &expression.kind {
            ResolvedExprKind::Invoke { .. }
            | ResolvedExprKind::Closure { .. }
            | ResolvedExprKind::FunctionReference { .. }
            | ResolvedExprKind::LiteralFormat { .. }
            | ResolvedExprKind::HostCommandCall(_)
            | ResolvedExprKind::NativeRustImportCall(_)
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. } => {
                return Err(permit.refuse(Refusal::Status(StatusRefusal::Delegated)))
            }
            _ => {}
        }
        match &expression.kind {
            ResolvedExprKind::VecFieldRead { .. } => {
                crate::cleanup_plan::replay::selected_profile::paid_children::field(
                    program, function, expression, budget,
                )?;
                receive!(
                    function,
                    budget,
                    permit,
                    VecReceiving<'_, StatusSource>,
                    10,
                    budget.child_reserve_vec(function, statuses, 1, "Vec field status vector")
                );
                receive!(
                    function,
                    budget,
                    permit,
                    CloneReceiving<'_, DeclarationId>,
                    9,
                    budget.child_reserve_clone(
                        function,
                        crate::vec_field::operation_id(),
                        "Vec field status callee",
                    )
                );
                statuses.push(StatusSource {
                    id: StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall {
                        callee: crate::vec_field::operation_id().clone(),
                    },
                });
            }
            ResolvedExprKind::ByteRange { operation, .. } => {
                if operation.as_str() != crate::byte_ops::RANGE_ID {
                    return Err(permit.refuse(Refusal::Status(StatusRefusal::RangeIdentity)));
                }
                receive!(
                    function,
                    budget,
                    permit,
                    CloneReceiving<'_, DeclarationId>,
                    9,
                    budget.child_reserve_clone(function, operation, "range status callee")
                );
                receive!(
                    function,
                    budget,
                    permit,
                    VecReceiving<'_, StatusSource>,
                    10,
                    budget.child_reserve_vec(function, statuses, 1, "operation status vector")
                );
                statuses.push(StatusSource {
                    id: StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall {
                        callee: operation.clone(),
                    },
                });
            }
            ResolvedExprKind::Call {
                callee, instance, ..
            } => {
                #[cfg(test)]
                let held = match &borrowed {
                    Some(lease) => {
                        Some(lease.call_is_total(program, function, expression, budget)?)
                    }
                    None => None,
                };
                #[cfg(not(test))]
                let held: Option<bool> = None;
                if let Some(total) = held {
                    if total {
                        continue;
                    }
                } else {
                    if instance.is_none()
                        && (crate::cleanup_plan::deferred_commit::is_total_str_len_bytes(callee)
                            || crate::string_ops::take::operand(expression).is_some()
                            || crate::byte_ops::by_id(callee.as_str())
                                .is_some_and(|op| !op.is_fallible())
                            || crate::host_io_ops::by_id(callee.as_str()).is_some()
                            || callee.as_str() == crate::stdin_stream_ops::EOF_ID)
                    {
                        // The borrowed Str byte-length read is total after HIR admission.
                        // Byte-data operations are total after HIR admission, with
                        // the bounded `bytes_set`, `bytes_set5`, and `bytes_set1_or5_from_slice` stores, whose computed
                        // element index is checked at run time. Physical allocation
                        // failure stays invariant fail-stop, not a recoverable
                        // operation status.
                        continue;
                    }
                    if instance.is_none()
                        && crate::cleanup_plan::deferred_commit::is_infallible_vec_operation(
                            crate::vec_ops::by_id(callee.as_str()),
                        )
                    {
                        continue;
                    }
                    if instance.is_none()
                        && crate::cleanup_plan::deferred_commit::is_infallible_box_operation(callee)
                    {
                        continue;
                    }
                    if instance.is_none()
                        && (crate::map_ops::by_id(callee.as_str()).is_some()
                            || crate::string_ops::by_id(callee.as_str()).is_some()
                            || crate::str_ops::by_id(callee.as_str()).is_some()
                            || crate::vec_ops::by_id(callee.as_str()).is_some()
                            || crate::iterator_ops::by_id(callee.as_str()).is_some()
                            || crate::list_ops::by_id(callee.as_str()).is_some()
                            || crate::box_ops::by_id(callee.as_str()).is_some()
                            || crate::byte_ops::by_id(callee.as_str()).is_some())
                    {
                        // Closed String/collection/byte operations project like ordinary propagated calls.
                    } else if program
                        .resolve_call_target(callee, instance.as_ref())
                        .is_none()
                    {
                        budget.reserve_array::<(semantic_refusal::Atom, semantic_refusal::Atom)>(
                            function,
                            1,
                            "status semantic atom receiving",
                        )?;
                        budget.charge(function, 2, "status semantic atom receiving")?;
                        return Err(Error::structural(
                            function,
                            semantic_refusal::Atom::StatusUnknownCallee(callee.clone()),
                        ));
                    }
                }
                receive!(
                    function,
                    budget,
                    permit,
                    VecReceiving<'_, StatusSource>,
                    10,
                    budget.child_reserve_vec(function, statuses, 1, "operation status vector")
                );
                receive!(
                    function,
                    budget,
                    permit,
                    CloneReceiving<'_, DeclarationId>,
                    9,
                    budget.child_reserve_clone(function, callee, "call status callee")
                );
                statuses.push(StatusSource {
                    id: StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall {
                        callee: callee.clone(),
                    },
                });
            }
            ResolvedExprKind::Unary {
                op: UnaryOp::Neg, ..
            } => {
                receive!(
                    function,
                    budget,
                    permit,
                    VecReceiving<'_, StatusSource>,
                    10,
                    budget.child_reserve_vec(function, statuses, 1, "arithmetic status vector")
                );
                statuses.push(checked_status(
                    expression,
                    crate::cleanup_plan::CheckedOperation::Neg,
                    cases(function, budget, &[StatusCase::NegationOverflow])?,
                ));
            }
            ResolvedExprKind::Unary {
                op: UnaryOp::Not, ..
            } => {}
            ResolvedExprKind::Binary { op, .. } => {
                let checked = match op {
                    BinaryOp::Add => Some((
                        crate::cleanup_plan::CheckedOperation::Add,
                        cases(function, budget, &[StatusCase::AddOverflow])?,
                    )),
                    BinaryOp::Sub => Some((
                        crate::cleanup_plan::CheckedOperation::Sub,
                        cases(function, budget, &[StatusCase::SubOverflow])?,
                    )),
                    BinaryOp::Mul => Some((
                        crate::cleanup_plan::CheckedOperation::Mul,
                        cases(function, budget, &[StatusCase::MulOverflow])?,
                    )),
                    BinaryOp::Div => Some((
                        crate::cleanup_plan::CheckedOperation::Div,
                        cases(
                            function,
                            budget,
                            &[StatusCase::DivisionByZero, StatusCase::DivisionOverflow],
                        )?,
                    )),
                    BinaryOp::Rem => Some((
                        crate::cleanup_plan::CheckedOperation::Rem,
                        cases(
                            function,
                            budget,
                            &[StatusCase::RemainderByZero, StatusCase::RemainderOverflow],
                        )?,
                    )),
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::And
                    | BinaryOp::Or => None,
                };
                if let Some((operation, cases)) = checked {
                    receive!(
                        function,
                        budget,
                        permit,
                        VecReceiving<'_, StatusSource>,
                        10,
                        budget.child_reserve_vec(function, statuses, 1, "arithmetic status vector")
                    );
                    statuses.push(checked_status(expression, operation, cases));
                }
            }
            ResolvedExprKind::Invoke { .. }
            | ResolvedExprKind::Closure { .. }
            | ResolvedExprKind::FunctionReference { .. }
            | ResolvedExprKind::LiteralFormat { .. }
            | ResolvedExprKind::HostCommandCall(_)
            | ResolvedExprKind::NativeRustImportCall(_) => {
                return Err(permit.refuse(Refusal::Status(StatusRefusal::Delegated)))
            }
            ResolvedExprKind::Block { .. }
            | ResolvedExprKind::VecConstructor { .. }
            | ResolvedExprKind::RowLoan { .. }
            | ResolvedExprKind::LexicalExit { .. }
            | ResolvedExprKind::If { .. }
            | ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::ConstructVariant { .. }
            | ResolvedExprKind::Try { .. }
            | ResolvedExprKind::TryOption { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
            | ResolvedExprKind::Upcast { .. }
            | ResolvedExprKind::Yield { .. }
            | ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::RepeatArrayU8 { .. }
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::String(_)
            | ResolvedExprKind::BorrowedUtf8Literal(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. } => {}
        }
    }
    Ok(())
}

fn cases(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    values: &[StatusCase],
) -> Result<Vec<StatusCase>, Error> {
    let permit = Permit::profile::<Vec<StatusCase>>(function, budget)?;
    receive!(
        function,
        budget,
        permit,
        ArrayReceiving<'_>,
        9,
        budget.child_reserve_array::<StatusCase>(
            function,
            values.len(),
            "arithmetic normalized cases"
        )
    );
    Ok(values.to_vec())
}
