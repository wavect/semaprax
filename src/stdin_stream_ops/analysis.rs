//! Bounded, source-independent streaming lifetime and helper derivation.
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, ExpressionId, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedStatement, ValueId,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) const MAX_WORK: usize = 1_000_000;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FunctionFacts {
    pub(crate) open_bound: u8,
    pub(crate) forwarding_parameter: Option<ValueId>,
    pub(crate) opens: Vec<ExpressionId>,
    pub(crate) nexts: Vec<ExpressionId>,
    pub(crate) chunks: Vec<ExpressionId>,
    pub(crate) eofs: Vec<ExpressionId>,
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}
fn charge(work: &mut usize) -> Result<(), Diagnostic> {
    *work += 1;
    if *work > MAX_WORK {
        Err(error("streaming admission exceeds bounded work"))
    } else {
        Ok(())
    }
}

pub(crate) fn derive(
    program: &ResolvedProgram,
) -> Result<BTreeMap<DeclarationId, FunctionFacts>, Diagnostic> {
    if !super::resolved_program_uses(program) {
        return Ok(BTreeMap::new());
    }
    if !program.agents.is_empty() {
        return Err(error(
            "streaming stdin does not compose with agent execution",
        ));
    }
    if program.functions.len() > crate::byte_data_capacity::MAX_FUNCTIONS {
        return Err(error("streaming admission exceeds function capacity"));
    }
    let functions = program
        .functions
        .iter()
        .map(|f| (f.id.clone(), f))
        .collect::<BTreeMap<_, _>>();
    let mut reverse = BTreeMap::<DeclarationId, BTreeSet<DeclarationId>>::new();
    let mut relevant = BTreeSet::new();
    let mut work = 0;
    for function in &program.functions {
        if super::is_reader(&function.return_type)
            || function.params.iter().any(|p| super::is_reader(&p.ty))
        {
            relevant.insert(function.id.clone());
        }
        let mut pending = vec![&function.body];
        while let Some(expression) = pending.pop() {
            charge(&mut work)?;
            if super::is_reader(&expression.ty)
                || matches!(&expression.kind, ResolvedExprKind::HostCommandCall(call) if super::is_host(call.operation))
                || matches!(&expression.kind, ResolvedExprKind::BorrowPlace { operation, .. } if operation.as_str() == super::CHUNK_ID)
                || matches!(&expression.kind, ResolvedExprKind::Call { callee, .. } if callee.as_str() == super::EOF_ID)
            {
                relevant.insert(function.id.clone());
            }
            if let ResolvedExprKind::Call {
                callee,
                instance: None,
                ..
            } = &expression.kind
            {
                if functions.contains_key(callee) {
                    reverse
                        .entry(callee.clone())
                        .or_default()
                        .insert(function.id.clone());
                }
            }
            if matches!(
                expression.kind,
                ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::Invoke { .. }
                    | ResolvedExprKind::FunctionReference { .. }
            ) {
                return Err(error(
                    "streaming stdin does not compose with function values",
                ));
            }
            if let ResolvedExprKind::Closure { body, .. } = &expression.kind {
                if super::resolved_expression_uses(body) {
                    return Err(error("streaming stdin cannot execute in a closure"));
                }
            }
            hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
        }
    }
    let mut pending = relevant.iter().cloned().collect::<Vec<_>>();
    while let Some(callee) = pending.pop() {
        for caller in reverse.get(&callee).into_iter().flatten() {
            charge(&mut work)?;
            if relevant.insert(caller.clone()) {
                pending.push(caller.clone());
            }
        }
    }
    let mut facts = BTreeMap::new();
    let mut active = BTreeSet::new();
    for root in &relevant {
        let mut pending = vec![(root.clone(), false)];
        while let Some((id, finishing)) = pending.pop() {
            charge(&mut work)?;
            if facts.contains_key(&id) {
                continue;
            }
            let function = functions
                .get(&id)
                .ok_or_else(|| error("streaming helper identity is not indexed"))?;
            if finishing {
                let open_bound = expression_bound(&function.body, &facts, &mut work)?;
                let forwarding_parameter = if super::is_reader(&function.return_type) {
                    if !super::resolved_forward_signature(function) {
                        return Err(error(
                            "streaming reader result requires one owned forwarding parameter",
                        ));
                    }
                    authenticate_forward_tail(program, function)?;
                    Some(function.params[0].id.clone())
                } else {
                    None
                };
                if function
                    .params
                    .iter()
                    .any(|p| super::is_reader(&p.ty) && p.ownership == hir::OwnershipMode::Own)
                    && forwarding_parameter.is_none()
                {
                    return Err(error("owned reader calls must forward the same reader"));
                }
                if function
                    .params
                    .iter()
                    .any(|param| super::is_reader(&param.ty))
                    && open_bound != 0
                {
                    return Err(error(
                        "reader forwarding helpers cannot open a second reader",
                    ));
                }
                let mut entry = FunctionFacts {
                    open_bound,
                    forwarding_parameter,
                    opens: Vec::new(),
                    nexts: Vec::new(),
                    chunks: Vec::new(),
                    eofs: Vec::new(),
                };
                let mut expressions = vec![&function.body];
                while let Some(expression) = expressions.pop() {
                    charge(&mut work)?;
                    match &expression.kind {
                        ResolvedExprKind::HostCommandCall(call)
                            if call.operation
                                == hir::ResolvedHostCommandOperation::StdinStreamOpen =>
                        {
                            entry.opens.push(expression.id.clone())
                        }
                        ResolvedExprKind::HostCommandCall(call)
                            if call.operation
                                == hir::ResolvedHostCommandOperation::StdinStreamNext =>
                        {
                            entry.nexts.push(expression.id.clone())
                        }
                        ResolvedExprKind::BorrowPlace { operation, .. }
                            if operation.as_str() == super::CHUNK_ID =>
                        {
                            entry.chunks.push(expression.id.clone())
                        }
                        ResolvedExprKind::Call { callee, .. }
                            if callee.as_str() == super::EOF_ID =>
                        {
                            entry.eofs.push(expression.id.clone())
                        }
                        _ => {}
                    }
                    hir::push_resolved_expression_children_in_authored_order(
                        expression,
                        &mut expressions,
                    );
                }
                facts.insert(id.clone(), entry);
                active.remove(&id);
                continue;
            }
            if !active.insert(id.clone()) {
                return Err(error("streaming stdin call graph must be acyclic"));
            }
            pending.push((id.clone(), true));
            let mut expressions = vec![&function.body];
            while let Some(expression) = expressions.pop() {
                charge(&mut work)?;
                if let ResolvedExprKind::Call {
                    callee,
                    instance: None,
                    ..
                } = &expression.kind
                {
                    if relevant.contains(callee) && !facts.contains_key(callee) {
                        pending.push((callee.clone(), false));
                    }
                }
                hir::push_resolved_expression_children_in_authored_order(
                    expression,
                    &mut expressions,
                );
            }
        }
    }
    Ok(facts)
}
fn authenticate_forward_tail(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut tail = &function.body;
    while let ResolvedExprKind::Block { tail: next, .. } = &tail.kind {
        tail = next;
    }
    let owner = &function.params[0].id;
    if matches!(&tail.kind, ResolvedExprKind::Place(place) if &place.root == owner && place.projections.is_empty())
    {
        return Ok(());
    }
    if !super::hir_reopen(tail, owner) {
        return Err(error(
            "reader forwarding tail must return its exact parameter or successor",
        ));
    }
    if let ResolvedExprKind::Call { callee, .. } = &tail.kind {
        if !program
            .resolve_call_target(callee, None)
            .is_some_and(super::resolved_forward_signature)
        {
            return Err(error(
                "reader forwarding tail calls an unauthenticated carrier helper",
            ));
        }
    }
    Ok(())
}
fn expression_bound(
    expression: &ResolvedExpr,
    functions: &BTreeMap<DeclarationId, FunctionFacts>,
    work: &mut usize,
) -> Result<u8, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedExpr),
        Finish(&'a ResolvedExpr, usize),
    }
    let mut pending = vec![Frame::Enter(expression)];
    let mut values = Vec::<u8>::new();
    while let Some(frame) = pending.pop() {
        charge(work)?;
        match frame {
            Frame::Enter(expression) => {
                let mut children = Vec::new();
                hir::push_resolved_expression_children_in_authored_order(expression, &mut children);
                pending.push(Frame::Finish(expression, children.len()));
                pending.extend(children.into_iter().map(Frame::Enter));
            }
            Frame::Finish(expression, count) => {
                let offset = values
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| error("streaming bound child accounting disagrees"))?;
                let children = &values[offset..];
                let cost = match &expression.kind {
                    ResolvedExprKind::If { .. } => {
                        children[0].saturating_add(children[1].max(children[2]))
                    }
                    ResolvedExprKind::Match { arms, .. } => {
                        let mut index = 1;
                        let mut guards = 0u8;
                        let mut arm_max = 0u8;
                        for arm in arms {
                            if arm.guard.is_some() {
                                guards = guards.saturating_add(children[index]);
                                index += 1;
                            }
                            arm_max = arm_max.max(children[index]);
                            index += 1;
                        }
                        children[0].saturating_add(guards).saturating_add(arm_max)
                    }
                    ResolvedExprKind::HostCommandCall(call)
                        if call.operation == hir::ResolvedHostCommandOperation::StdinStreamOpen =>
                    {
                        children.iter().copied().fold(1, u8::saturating_add)
                    }
                    ResolvedExprKind::Call { callee, .. } => children.iter().copied().fold(
                        functions.get(callee).map_or(0, |facts| facts.open_bound),
                        u8::saturating_add,
                    ),
                    ResolvedExprKind::Block { statements, .. } => {
                        let mut child = 0;
                        for statement in statements {
                            let count = statement.child_count();
                            if matches!(statement, ResolvedStatement::While { .. })
                                && children[child..child + count]
                                    .iter()
                                    .any(|bound| *bound != 0)
                            {
                                return Err(error("streaming stdin Open cannot execute in a loop"));
                            }
                            child += count;
                        }
                        children.iter().copied().fold(0, u8::saturating_add)
                    }
                    _ => children.iter().copied().fold(0, u8::saturating_add),
                };
                if cost > 1 {
                    return Err(error(
                        "streaming stdin Open may execute at most once per invocation path",
                    ));
                }
                values.truncate(offset);
                values.push(cost);
            }
        }
    }
    Ok(values.pop().unwrap_or(0))
}
