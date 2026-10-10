//! Exact operation and child facts used by independent cleanup replay.
use super::*;

pub(super) fn expression_facts(
    function: &ResolvedFunction,
) -> Result<BTreeMap<ExpressionId, Option<CallFact>>, Diagnostic> {
    let mut facts = BTreeMap::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_expression_facts(function, expression, &mut facts)?;
    }
    Ok(facts)
}

fn collect_expression_facts(
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    facts: &mut BTreeMap<ExpressionId, Option<CallFact>>,
) -> Result<(), Diagnostic> {
    // The private replay entry admits at most 512 semantic expression levels. Keep one indexed continuation per ancestor so wide calls, records, blocks,
    // and matches never create a width-sized frontier and callback order stays identical to the former recursive pre-order walk.
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (current, next_child) = stack[len].take().expect("expression-fact frame retained");
        if next_child == 0 {
            let fact = match &current.kind {
                _ if crate::hir::closure::once::call(current).is_some() => {
                    let (callee, args, _) = crate::hir::function_value::cleanup_call(current)?;
                    Some(CallFact {
                        callee: callee.clone(),
                        instance: None,
                        arguments: args.iter().map(|a| a.id.clone()).collect(),
                        type_arguments: vec![],
                    })
                }
                ResolvedExprKind::Invoke { callable, args } => Some(CallFact {
                    callee: crate::hir::function_value::INVOKE_ID.clone(),
                    instance: None,
                    arguments: args.iter().map(|a| a.id.clone()).collect(),
                    type_arguments: vec![callable.ty.clone()],
                }),
                ResolvedExprKind::VecFieldRead { args, .. } => Some(CallFact {
                    callee: crate::vec_field::operation_id().clone(),
                    instance: None,
                    arguments: args.iter().map(|argument| argument.id.clone()).collect(),
                    type_arguments: Vec::new(),
                }),
                ResolvedExprKind::LiteralFormat { args, .. } => Some(CallFact {
                    callee: crate::literal_format::operation_id().clone(),
                    instance: None,
                    arguments: args.iter().map(|argument| argument.id.clone()).collect(),
                    type_arguments: Vec::new(),
                }),
                ResolvedExprKind::Call {
                    callee,
                    instance,
                    args,
                    type_arguments,
                } => Some(CallFact {
                    callee: callee.clone(),
                    instance: instance.clone(),
                    arguments: args.iter().map(|argument| argument.id.clone()).collect(),
                    type_arguments: type_arguments.clone(),
                }),
                ResolvedExprKind::NativeRustImportCall(call)
                    if crate::cleanup_plan::native_rust::owns(current) =>
                {
                    Some(CallFact {
                        callee: call.import.clone(),
                        instance: None,
                        arguments: call
                            .args
                            .iter()
                            .map(|argument| argument.id.clone())
                            .collect(),
                        type_arguments: Vec::new(),
                    })
                }
                ResolvedExprKind::HostCommandCall(call) => Some(CallFact {
                    callee: DeclarationId::new(crate::command_io_ops::id(call.operation)),
                    instance: None,
                    arguments: call
                        .args
                        .iter()
                        .map(|argument| argument.id.clone())
                        .collect(),
                    type_arguments: Vec::new(),
                }),
                ResolvedExprKind::ByteRange {
                    operation,
                    source,
                    start,
                    end,
                } => Some(CallFact {
                    callee: operation.clone(),
                    instance: None,
                    arguments: [source.as_ref(), start.as_ref(), end.as_ref()]
                        .into_iter()
                        .map(|argument| argument.id.clone())
                        .collect(),
                    type_arguments: Vec::new(),
                }),
                _ => None,
            };
            if facts.insert(current.id.clone(), fact).is_some() {
                return Err(replay_error(
                    function,
                    format!("HIR expression identity `{}` is repeated", current.id),
                ));
            }
        }
        if let Some(child) = replay_expression_child(current, next_child) {
            if len + 2 > stack.len() {
                return Err(replay_error(
                    function,
                    "HIR expression fact traversal exceeds the admitted depth",
                ));
            }
            stack[len] = Some((current, next_child + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    Ok(())
}
