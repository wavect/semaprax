//! Ordinary status reconstruction compares borrowed operands in the original order.
//! A mismatch is retained until the complete walk succeeds: later HIR errors
//! retain their original priority over the final table mismatch. No plan row is
//! authority for discovering a failure producer.
use super::*;
use crate::cleanup_plan::CleanupPlan;
pub(in crate::cleanup_plan) mod allocation;
#[cfg(test)]
mod old;
#[cfg(test)]
mod tests;

struct BorrowedId<'a> {
    expression: &'a ExpressionId,
    lane: StatusLane,
}
struct BorrowedStatus<'a> {
    id: BorrowedId<'a>,
    producer: BorrowedProducer<'a>,
}
enum BorrowedProducer<'a> {
    CheckedAllocation {
        operation: super::super::AllocationOperation,
    },
    PropagatedCall {
        callee: &'a str,
    },
    CheckedArithmetic {
        operation: super::super::CheckedOperation,
        normalized_cases: &'a [StatusCase],
    },
    ContractFalse {
        phase: super::super::ContractPhase,
        ordinal: u32,
    },
}
struct Comparison<'a> {
    actual: &'a [StatusSource],
    next: usize,
    equal: bool,
}
impl<'a> Comparison<'a> {
    fn new(actual: &'a [StatusSource]) -> Self {
        Self {
            actual,
            next: 0,
            equal: true,
        }
    }
    fn push(&mut self, expected: BorrowedStatus<'_>) {
        let equal = self.actual.get(self.next).is_some_and(|actual| {
            actual.id.expression == *expected.id.expression
                && actual.id.lane == expected.id.lane
                && match (&actual.producer, expected.producer) {
                    (
                        StatusProducer::CheckedAllocation {
                            operation,
                            normalized_cases,
                        },
                        BorrowedProducer::CheckedAllocation {
                            operation: expected,
                        },
                    ) => {
                        *operation == expected
                            && *normalized_cases
                                == [
                                    super::super::AllocationCase::PayloadRefused,
                                    super::super::AllocationCase::SizeOverflow,
                                ]
                    }
                    (
                        StatusProducer::PropagatedCall { callee },
                        BorrowedProducer::PropagatedCall { callee: expected },
                    ) => callee.as_str() == expected,
                    (
                        StatusProducer::CheckedArithmetic {
                            operation,
                            normalized_cases,
                        },
                        BorrowedProducer::CheckedArithmetic {
                            operation: expected,
                            normalized_cases: cases,
                        },
                    ) => *operation == expected && normalized_cases.as_slice() == cases,
                    (
                        StatusProducer::ContractFalse { phase, ordinal },
                        BorrowedProducer::ContractFalse {
                            phase: expected,
                            ordinal: count,
                        },
                    ) => *phase == expected && *ordinal == count,
                    _ => false,
                }
        });
        self.equal &= equal;
        // An impossible cardinality overflow is still a table mismatch, never
        // an earlier failure that could suppress a later source diagnostic.
        match self.next.checked_add(1) {
            Some(next) => self.next = next,
            None => self.equal = false,
        }
    }
    fn complete(&self) -> bool {
        self.equal && self.next == self.actual.len()
    }
}
fn borrowed_checked<'a>(
    expression: &'a ResolvedExpr,
    operation: super::super::CheckedOperation,
    normalized_cases: &'a [StatusCase],
) -> BorrowedStatus<'a> {
    BorrowedStatus {
        id: BorrowedId {
            expression: &expression.id,
            lane: StatusLane::OperationFailure,
        },
        producer: BorrowedProducer::CheckedArithmetic {
            operation,
            normalized_cases,
        },
    }
}

pub(super) fn validate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    validate_rows(
        program,
        function,
        &function.cleanup_plan.status_sources,
        None,
    )
}

/// Independent status discovery only. Passing this receiver does not validate
/// allocation failure cleanup, skeleton paths or an allocator/lease receiver.
/// The current-function token comes only from full original-source preparation.
#[cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "private checked allocation skeleton and engine receivers are not activated"
    )
)]
pub(in crate::cleanup_plan) fn validate_allocation_projection(
    current: super::super::checked_allocation::Function<'_>,
    plan: &CleanupPlan,
) -> Result<(), Diagnostic> {
    let function = current.function();
    if !current.held_by(current.program(), function)
        || plan.schema != super::super::CLEANUP_PLAN_SCHEMA_V20
    {
        return Err(replay_error(
            function,
            "allocation status projection changed its current subject or schema",
        ));
    }
    let selection = allocation::Selection::new(function)?;
    validate_rows(
        current.program(),
        function,
        &plan.status_sources,
        Some(&selection),
    )
}

fn validate_rows(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    actual: &[StatusSource],
    allocation: Option<&allocation::Selection>,
) -> Result<(), Diagnostic> {
    let mut expected = Comparison::new(actual);
    for (ordinal, contract) in function.requires.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected, allocation)?;
        expected.push(BorrowedStatus {
            id: BorrowedId {
                expression: &contract.id,
                lane: StatusLane::ContractFalse,
            },
            producer: BorrowedProducer::ContractFalse {
                phase: super::super::ContractPhase::Requires,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| replay_error(function, "too many preconditions"))?,
            },
        });
    }
    collect_expression_statuses(program, function, &function.body, &mut expected, allocation)?;
    for (ordinal, contract) in function.ensures.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected, allocation)?;
        expected.push(BorrowedStatus {
            id: BorrowedId {
                expression: &contract.id,
                lane: StatusLane::ContractFalse,
            },
            producer: BorrowedProducer::ContractFalse {
                phase: super::super::ContractPhase::Ensures,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| replay_error(function, "too many postconditions"))?,
            },
        });
    }
    if !expected.complete() {
        return Err(replay_error(
            function,
            "cleanup status sources do not exactly cover typed HIR failure producers",
        ));
    }
    Ok(())
}

fn collect_expression_statuses(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    statuses: &mut Comparison<'_>,
    allocation: Option<&allocation::Selection>,
) -> Result<(), Diagnostic> {
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("status frame retained");
        if let Some(child) = replay_expression_child(expression, next) {
            if len + 2 > stack.len() {
                return Err(replay_error(
                    function,
                    "cleanup status expression depth exceeds 512",
                ));
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
            continue;
        }
        if super::super::owner_admission::required(program, expression) {
            let call = super::super::native_rust::parts(expression).expect("admitted owner call");
            statuses.push(BorrowedStatus {
                id: BorrowedId {
                    expression: &expression.id,
                    lane: StatusLane::OwnerAdmission,
                },
                producer: BorrowedProducer::PropagatedCall {
                    callee: call.callee.as_str(),
                },
            });
        }
        match &expression.kind {
            ResolvedExprKind::Closure { .. } if !expression.ty.is_once_function() => {}
            ResolvedExprKind::FunctionReference { .. } => {}
            ResolvedExprKind::ByteRange { operation, .. } => {
                if operation.as_str() != crate::byte_ops::RANGE_ID {
                    return Err(replay_error(
                        function,
                        "byte range status source carries an unknown operation identity",
                    ));
                }
                statuses.push(BorrowedStatus {
                    id: BorrowedId {
                        expression: &expression.id,
                        lane: StatusLane::OperationFailure,
                    },
                    producer: BorrowedProducer::PropagatedCall {
                        callee: operation.as_str(),
                    },
                });
            }
            ResolvedExprKind::Invoke { .. } | ResolvedExprKind::Closure { .. } => {
                statuses.push(BorrowedStatus {
                    id: BorrowedId {
                        expression: &expression.id,
                        lane: StatusLane::OperationFailure,
                    },
                    producer: BorrowedProducer::PropagatedCall {
                        callee: crate::hir::function_value::cleanup_call(expression)?
                            .0
                            .as_str(),
                    },
                });
            }
            ResolvedExprKind::VecFieldRead { .. } => {
                statuses.push(BorrowedStatus {
                    id: BorrowedId {
                        expression: &expression.id,
                        lane: StatusLane::OperationFailure,
                    },
                    producer: BorrowedProducer::PropagatedCall {
                        callee: crate::vec_field::operation_id().as_str(),
                    },
                });
            }
            ResolvedExprKind::LiteralFormat { .. } => {
                statuses.push(BorrowedStatus {
                    id: BorrowedId {
                        expression: &expression.id,
                        lane: StatusLane::OperationFailure,
                    },
                    producer: BorrowedProducer::PropagatedCall {
                        callee: crate::literal_format::operation_id().as_str(),
                    },
                });
            }
            ResolvedExprKind::Call {
                callee, instance, ..
            } => {
                if instance.is_none()
                    && (super::super::deferred_commit::is_total_str_len_bytes(callee)
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
                    push_allocation(statuses, allocation, expression);
                    continue;
                }
                if instance.is_none()
                    && super::super::deferred_commit::is_infallible_vec_operation(
                        crate::vec_ops::by_id(callee.as_str()),
                    )
                {
                    push_allocation(statuses, allocation, expression);
                    continue;
                }
                if instance.is_none()
                    && super::super::deferred_commit::is_infallible_box_operation(callee)
                {
                    push_allocation(statuses, allocation, expression);
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
                    return Err(replay_error(
                        function,
                        format_args!("status source call has unknown callee `{callee}`"),
                    ));
                }
                statuses.push(BorrowedStatus {
                    id: BorrowedId {
                        expression: &expression.id,
                        lane: StatusLane::OperationFailure,
                    },
                    producer: BorrowedProducer::PropagatedCall {
                        callee: callee.as_str(),
                    },
                });
            }
            ResolvedExprKind::HostCommandCall(call) => {
                if crate::command_io_ops::failure(call.operation)
                    == crate::command_io_ops::CommandIoFailure::Status
                {
                    statuses.push(BorrowedStatus {
                        id: BorrowedId {
                            expression: &expression.id,
                            lane: StatusLane::OperationFailure,
                        },
                        producer: BorrowedProducer::PropagatedCall {
                            callee: crate::command_io_ops::id(call.operation),
                        },
                    });
                }
            }
            ResolvedExprKind::Unary {
                op: UnaryOp::Neg, ..
            } => statuses.push(borrowed_checked(
                expression,
                super::super::CheckedOperation::Neg,
                &[StatusCase::NegationOverflow],
            )),
            ResolvedExprKind::Unary {
                op: UnaryOp::Not, ..
            } => {}
            ResolvedExprKind::Binary { op, .. } => {
                let checked: Option<(super::super::CheckedOperation, &[StatusCase])> = match op {
                    BinaryOp::Add => Some((
                        super::super::CheckedOperation::Add,
                        &[StatusCase::AddOverflow],
                    )),
                    BinaryOp::Sub => Some((
                        super::super::CheckedOperation::Sub,
                        &[StatusCase::SubOverflow],
                    )),
                    BinaryOp::Mul => Some((
                        super::super::CheckedOperation::Mul,
                        &[StatusCase::MulOverflow],
                    )),
                    BinaryOp::Div => Some((
                        super::super::CheckedOperation::Div,
                        &[StatusCase::DivisionByZero, StatusCase::DivisionOverflow],
                    )),
                    BinaryOp::Rem => Some((
                        super::super::CheckedOperation::Rem,
                        &[StatusCase::RemainderByZero, StatusCase::RemainderOverflow],
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
                    statuses.push(borrowed_checked(expression, operation, cases));
                }
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                if super::super::native_rust::owns(expression) {
                    statuses.push(BorrowedStatus {
                        id: BorrowedId {
                            expression: &expression.id,
                            lane: StatusLane::OperationFailure,
                        },
                        producer: BorrowedProducer::PropagatedCall {
                            callee: call.import.as_str(),
                        },
                    });
                }
            }
            ResolvedExprKind::Block { .. }
            | ResolvedExprKind::VecConstructor { .. }
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
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::RowLoan { .. } => {}
        }
        push_allocation(statuses, allocation, expression);
    }
    Ok(())
}

fn push_allocation(
    statuses: &mut Comparison<'_>,
    selection: Option<&allocation::Selection>,
    expression: &ResolvedExpr,
) {
    let Some(operation) = selection.and_then(|selection| selection.operation(expression)) else {
        return;
    };
    statuses.push(BorrowedStatus {
        id: BorrowedId {
            expression: &expression.id,
            lane: StatusLane::AllocationFailure,
        },
        producer: BorrowedProducer::CheckedAllocation { operation },
    });
}
