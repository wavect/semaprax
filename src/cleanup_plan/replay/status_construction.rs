//! Selected plain primitive/user-call status reconstruction.
use super::*;
#[cfg(test)]
mod borrowed;

pub(super) fn validate_required_status_sources(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if !program.interfaces().is_empty() {
        return Err(replay_error(
            function,
            "selected physical status reconstruction does not support interfaces",
        ));
    }
    let mut expected = Vec::new();
    for (ordinal, contract) in function.requires.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected, budget)?;
        budget.reserve_vec(function, &mut expected, 1, "contract status vector")?;
        expected.push(StatusSource {
            id: StatusSourceId {
                expression: contract.id.clone(),
                lane: StatusLane::ContractFalse,
            },
            producer: StatusProducer::ContractFalse {
                phase: super::super::ContractPhase::Requires,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| replay_error(function, "too many preconditions"))?,
            },
        });
    }
    collect_expression_statuses(program, function, &function.body, &mut expected, budget)?;
    for (ordinal, contract) in function.ensures.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected, budget)?;
        budget.reserve_vec(function, &mut expected, 1, "contract status vector")?;
        expected.push(StatusSource {
            id: StatusSourceId {
                expression: contract.id.clone(),
                lane: StatusLane::ContractFalse,
            },
            producer: StatusProducer::ContractFalse {
                phase: super::super::ContractPhase::Ensures,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| replay_error(function, "too many postconditions"))?,
            },
        });
    }
    #[cfg(test)]
    let equal = if program.borrowed_helper_lease(function).is_some() {
        borrowed::equal(
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
    statuses: &mut Vec<StatusSource>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    budget.reserve_array::<Option<expression_children::ReplayChildCursor<'_>>>(
        function,
        514,
        "status expression cursor",
    )?;
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
                return Err(replay_error(
                    function,
                    "cleanup status expression depth exceeds 512",
                ));
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
                return Err(replay_error(
                    function,
                    "selected physical status reconstruction does not support delegated producers",
                ))
            }
            _ => {}
        }
        match &expression.kind {
            ResolvedExprKind::VecFieldRead { .. } => {
                super::selected_profile::collections::field(program, function, expression, budget)?;
                budget.reserve_vec(function, statuses, 1, "Vec field status vector")?;
                budget.reserve_clone(
                    function,
                    crate::vec_field::operation_id(),
                    "Vec field status callee",
                )?;
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
                    return Err(replay_error(
                        function,
                        "byte range status source carries an unknown operation identity",
                    ));
                }
                budget.reserve_clone(function, operation, "range status callee")?;
                budget.reserve_vec(function, statuses, 1, "operation status vector")?;
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
                        continue;
                    }
                    if instance.is_none()
                        && super::super::deferred_commit::is_infallible_vec_operation(
                            crate::vec_ops::by_id(callee.as_str()),
                        )
                    {
                        continue;
                    }
                    if instance.is_none()
                        && super::super::deferred_commit::is_infallible_box_operation(callee)
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
                        return Err(replay_error(
                            function,
                            format_args!("status source call has unknown callee `{callee}`"),
                        ));
                    }
                }
                budget.reserve_vec(function, statuses, 1, "operation status vector")?;
                budget.reserve_clone(function, callee, "call status callee")?;
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
                budget.reserve_vec(function, statuses, 1, "arithmetic status vector")?;
                statuses.push(checked_status(
                    expression,
                    super::super::CheckedOperation::Neg,
                    cases(function, budget, &[StatusCase::NegationOverflow])?,
                ));
            }
            ResolvedExprKind::Unary {
                op: UnaryOp::Not, ..
            } => {}
            ResolvedExprKind::Binary { op, .. } => {
                let checked = match op {
                    BinaryOp::Add => Some((
                        super::super::CheckedOperation::Add,
                        cases(function, budget, &[StatusCase::AddOverflow])?,
                    )),
                    BinaryOp::Sub => Some((
                        super::super::CheckedOperation::Sub,
                        cases(function, budget, &[StatusCase::SubOverflow])?,
                    )),
                    BinaryOp::Mul => Some((
                        super::super::CheckedOperation::Mul,
                        cases(function, budget, &[StatusCase::MulOverflow])?,
                    )),
                    BinaryOp::Div => Some((
                        super::super::CheckedOperation::Div,
                        cases(
                            function,
                            budget,
                            &[StatusCase::DivisionByZero, StatusCase::DivisionOverflow],
                        )?,
                    )),
                    BinaryOp::Rem => Some((
                        super::super::CheckedOperation::Rem,
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
                    budget.reserve_vec(function, statuses, 1, "arithmetic status vector")?;
                    statuses.push(checked_status(expression, operation, cases));
                }
            }
            ResolvedExprKind::Invoke { .. }
            | ResolvedExprKind::Closure { .. }
            | ResolvedExprKind::FunctionReference { .. }
            | ResolvedExprKind::LiteralFormat { .. }
            | ResolvedExprKind::HostCommandCall(_)
            | ResolvedExprKind::NativeRustImportCall(_) => {
                return Err(replay_error(
                    function,
                    "selected physical status reconstruction does not support delegated producers",
                ))
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
) -> Result<Vec<StatusCase>, Diagnostic> {
    budget.reserve_array::<StatusCase>(function, values.len(), "arithmetic normalized cases")?;
    Ok(values.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn program() -> ResolvedProgram {
        let source = crate::check(
            r#"module replay.status_owner;
@id("status.add") fn add(x:i64)->i64 requires x>=0 ensures result>=0 {x+1}
@id("status.main") fn main()->i64 {add(41)}
"#,
            "replay-status-owner.spx",
        )
        .unwrap();
        crate::hir::resolve(&source).unwrap()
    }
    fn run(
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        limit: usize,
    ) -> Result<(), Diagnostic> {
        validate_required_status_sources(
            program,
            function,
            &mut ReplayBudget::with_physical(function, limit)?,
        )
    }
    #[test]
    fn primitive_status_sources_match_legacy_exact_and_one_short() {
        let program = program();
        // Expression identity equality is its text, including empty identities
        // with different Arc representation. The receiver preserves that rule.
        let function = &program.functions[0];
        let mut expected = function.cleanup_plan.status_sources[0].clone();
        // Exercise the existing refused-construction empty representation
        // against a separately allocated empty identity, without new API.
        let (empty, refused) =
            crate::bounded_output::with_limit(0, || ExpressionId::from_owned(String::new()));
        assert!(refused);
        expected.id.expression = empty;
        let mut actual = expected.clone();
        actual.id.expression = ExpressionId::from_owned(String::new());
        assert_eq!(expected, actual);
        let mut budget = ReplayBudget::new();
        assert!(borrowed::equal(function, &[expected], &[actual], &mut budget).unwrap());
        for function in program
            .functions
            .iter()
            .filter(|f| f.id.as_str().starts_with("status."))
        {
            super::super::validate_required_status_sources(&program, function).unwrap();
            let (_, refused, bytes) = crate::bounded_output::with_limit_usage(usize::MAX, || {
                run(&program, function, usize::MAX).unwrap()
            });
            assert!(!refused);
            let (exact, refused, used) =
                crate::bounded_output::with_limit_usage(bytes, || run(&program, function, bytes));
            exact.unwrap();
            assert!(!refused);
            assert_eq!(used, bytes);
            let (short, refused, used) = crate::bounded_output::with_limit_usage(bytes - 1, || {
                run(&program, function, usize::MAX)
            });
            assert_eq!(short.unwrap_err().code, "SPX-H006");
            assert!(refused);
            assert!(used < bytes);
        }
    }
    #[test]
    fn missing_typed_status_producer_remains_a_semantic_refusal() {
        let program = program();
        let mut function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "status.add")
            .unwrap()
            .clone();
        function.cleanup_plan.status_sources.clear();
        let error = run(&program, &function, usize::MAX).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error
            .message
            .contains("exactly cover typed HIR failure producers"));
    }
}

pub(in crate::cleanup_plan::replay) mod paid;

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn collect_for_test(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    statuses: &mut Vec<StatusSource>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    collect_expression_statuses(program, function, expression, statuses, budget)
}
