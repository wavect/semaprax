use super::*;

pub(super) fn collect_arena_facts<'a, P: errors::Policy>(
    function: &'a ResolvedFunction,
    expression: &'a ResolvedExpr,
    facts: &mut Arena<'a>,
    mut capacity: Option<&mut capacity::FactCapacity<'a, '_>>,
    budget: &mut ReplayBudget,
) -> Result<(), P::Error> {
    // One borrowed continuation per ancestor keeps the original pre-order walk
    // and avoids rescanning wide Block/Match prefixes for each child.
    let mut stack = [None; 514];
    stack[0] = Some((
        expression_children::ReplayChildCursor::new(expression),
        false,
    ));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (mut cursor, started) = stack[len].take().expect("expression-fact frame retained");
        let current = cursor.expression();
        if !started {
            if let Some(capacity) = capacity.as_deref_mut() {
                facts.prepare_growth_work(function, budget)?;
                facts
                    .reserve_next(capacity)
                    .map_err(|failure| P::capacity(function, failure))?;
                capacity
                    .arena_expression(current)
                    .map_err(|failure| P::capacity(function, failure))?;
            }
            let fact = match &current.kind {
                _ if crate::hir::closure::once::call(current).is_some() => {
                    let (callee, args) = if capacity.is_some() {
                        crate::hir::closure::once::call(current).expect("same closed affine call")
                    } else {
                        let (callee, args, _) = crate::hir::function_value::cleanup_call(current)?;
                        (callee, args)
                    };
                    Some(CallFact {
                        callee: std::borrow::Cow::Borrowed(callee),
                        instance: None,
                        arguments: call_payload::Arguments::Slice(args),
                        type_arguments: call_payload::Types::Slice(&[]),
                    })
                }
                ResolvedExprKind::Invoke { callable, args } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(&crate::hir::function_value::INVOKE_ID),
                    instance: None,
                    arguments: call_payload::Arguments::Slice(args),
                    type_arguments: call_payload::Types::One(&callable.ty),
                }),
                ResolvedExprKind::VecFieldRead { args, .. } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(crate::vec_field::operation_id()),
                    instance: None,
                    arguments: call_payload::Arguments::Slice(args),
                    type_arguments: call_payload::Types::Slice(&[]),
                }),
                ResolvedExprKind::LiteralFormat { args, .. } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(crate::literal_format::operation_id()),
                    instance: None,
                    arguments: call_payload::Arguments::Slice(args),
                    type_arguments: call_payload::Types::Slice(&[]),
                }),
                ResolvedExprKind::Call {
                    callee,
                    instance,
                    args,
                    type_arguments,
                } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(callee),
                    instance: instance.as_ref(),
                    arguments: call_payload::Arguments::Slice(args),
                    type_arguments: call_payload::Types::Slice(type_arguments),
                }),
                ResolvedExprKind::NativeRustImportCall(call)
                    if crate::cleanup_plan::native_rust::owns(current) =>
                {
                    Some(CallFact {
                        callee: std::borrow::Cow::Borrowed(&call.import),
                        instance: None,
                        arguments: call_payload::Arguments::Slice(&call.args),
                        type_arguments: call_payload::Types::Slice(&[]),
                    })
                }
                ResolvedExprKind::HostCommandCall(call) => Some(CallFact {
                    callee: std::borrow::Cow::Owned(DeclarationId::compiler(
                        crate::hir::CompilerDeclaration::Command(call.operation),
                    )),
                    instance: None,
                    arguments: call_payload::Arguments::Slice(&call.args),
                    type_arguments: call_payload::Types::Slice(&[]),
                }),
                ResolvedExprKind::ByteRange {
                    operation,
                    source,
                    start,
                    end,
                } => Some(CallFact {
                    callee: std::borrow::Cow::Borrowed(operation),
                    instance: None,
                    arguments: call_payload::Arguments::Range([source, start, end]),
                    type_arguments: call_payload::Types::Slice(&[]),
                }),
                _ => None,
            };
            let capacity = capacity.as_deref_mut().expect("arena owns its payer");
            facts.prepare_insert_work(function, &current.id, budget)?;
            if !facts
                .insert(current.id.clone(), fact, capacity)
                .map_err(|failure| P::capacity(function, failure))?
            {
                return Err(P::duplicate(function, &current.id));
            }
        }
        if let Some(child) = cursor.next() {
            if len + 2 > stack.len() {
                return Err(P::capacity(
                    function,
                    capacity::Failure {
                        detail: "HIR expression fact traversal exceeds the admitted depth",
                        physical: false,
                    },
                ));
            }
            stack[len] = Some((cursor, true));
            stack[len + 1] = Some((expression_children::ReplayChildCursor::new(child), false));
            len += 2;
        }
    }
    Ok(())
}
