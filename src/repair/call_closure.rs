//! Conservative checked call closure for bounded repair analysis.
use super::*;

pub(super) fn call_graph(program: &hir::ResolvedProgram) -> Result<CallGraph, Vec<Diagnostic>> {
    let known = program
        .functions
        .iter()
        .map(|function| function.id.clone())
        .collect::<BTreeSet<_>>();
    let mut graph = BTreeMap::new();
    let mut call_sites = 0usize;
    for function in &program.functions {
        let mut calls = BTreeSet::new();
        collect_calls(&function.body, &known, &mut calls, &mut call_sites);
        hir::function_value::walk(function, |expression| {
            if let ResolvedExprKind::Invoke { callable, .. } = &expression.kind {
                calls.extend(
                    hir::function_value::compatible_targets(program, &callable.ty)
                        .into_iter()
                        .map(|target| target.id.clone()),
                );
            }
        });
        if call_sites > MAX_CALL_SITES {
            return Err(vec![repair_query_error(format!(
                "diagnostic repair program exceeds {MAX_CALL_SITES} call sites"
            ))]);
        }
        graph.insert(function.id.clone(), calls);
    }
    Ok(CallGraph {
        edges: graph,
        call_sites,
    })
}

pub(super) fn has_call_cycle(graph: &BTreeMap<DeclarationId, BTreeSet<DeclarationId>>) -> bool {
    let mut indegree = graph
        .keys()
        .cloned()
        .map(|id| (id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for callees in graph.values() {
        for callee in callees {
            if let Some(count) = indegree.get_mut(callee) {
                *count += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<Vec<_>>();
    let mut visited = 0usize;
    while let Some(id) = ready.pop() {
        visited += 1;
        if let Some(callees) = graph.get(&id) {
            for callee in callees {
                let count = indegree
                    .get_mut(callee)
                    .expect("call graph contains known functions only");
                *count -= 1;
                if *count == 0 {
                    ready.push(callee.clone());
                }
            }
        }
    }
    visited != graph.len()
}

fn collect_calls(
    expression: &ResolvedExpr,
    known: &BTreeSet<DeclarationId>,
    calls: &mut BTreeSet<DeclarationId>,
    call_sites: &mut usize,
) {
    match &expression.kind {
        ResolvedExprKind::Closure { captures, body, .. } => {
            // Creation evaluates snapshots only. Its private body is not an
            // immediate execution, but its already-checked direct calls are
            // still dependencies and bounded repair call sites whenever the
            // closure is later invoked or escapes through this function.
            for capture in captures {
                collect_calls(&capture.value, known, calls, call_sites);
            }
            collect_calls(body, known, calls, call_sites);
        }
        ResolvedExprKind::FunctionReference { target } => {
            if known.contains(target) {
                calls.insert(target.clone());
            }
        }
        ResolvedExprKind::Invoke { callable, args } => {
            *call_sites = call_sites.saturating_add(1);
            collect_calls(callable, known, calls, call_sites);
            for arg in args {
                collect_calls(arg, known, calls, call_sites);
            }
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_calls(source, known, calls, call_sites);
            collect_calls(start, known, calls, call_sites);
            collect_calls(end, known, calls, call_sites);
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::BorrowPlace { .. }
        | ResolvedExprKind::Place(_) => {}
        ResolvedExprKind::Call { callee, args, .. } => {
            *call_sites = call_sites.saturating_add(1);
            if known.contains(callee) {
                calls.insert(callee.clone());
            }
            for argument in args {
                collect_calls(argument, known, calls, call_sites);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_calls(argument, known, calls, call_sites);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                collect_calls(argument, known, calls, call_sites);
            }
        }
        ResolvedExprKind::Unary { value, .. } => collect_calls(value, known, calls, call_sites),
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_calls(left, known, calls, call_sites);
            collect_calls(right, known, calls, call_sites);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    collect_calls(
                        statement
                            .child(index)
                            .expect("resolved statement child count is canonical"),
                        known,
                        calls,
                        call_sites,
                    );
                }
            }
            collect_calls(tail, known, calls, call_sites);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_calls(condition, known, calls, call_sites);
            collect_calls(then_branch, known, calls, call_sites);
            collect_calls(else_branch, known, calls, call_sites);
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_calls(&field.value, known, calls, call_sites);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_calls(scrutinee, known, calls, call_sites);
            for arm in arms {
                collect_calls(&arm.value, known, calls, call_sites);
            }
        }
        ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
            collect_calls(operand, known, calls, call_sites);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_calls(base, known, calls, call_sites);
            for field in fields {
                collect_calls(&field.value, known, calls, call_sites);
            }
        }
        ResolvedExprKind::Project { base, .. } => collect_calls(base, known, calls, call_sites),
        ResolvedExprKind::Upcast { source } => collect_calls(source, known, calls, call_sites),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_body_calls_contribute_dependencies_and_call_site_budget_without_counting_creation() {
        let source = r#"
module repair.closure_budget;
@id("repair.helper") fn helper(value:i64)->i64 {value+1}
@id("app.main") fn main()->i64 {
 let offset=40;
 let callback=fn(value:i64)->i64 {helper(offset+value)};
 callback(1)
}
"#;
        let parsed = crate::check(source, "repair-closure-budget.spx").unwrap();
        let resolved = hir::resolve(&parsed).unwrap();
        let graph = call_graph(&resolved).unwrap();
        assert_eq!(graph.call_sites, 2, "closure creation is not a call site");
        assert_eq!(
            graph.edges[&DeclarationId::new("app.main")],
            BTreeSet::from([DeclarationId::new("repair.helper")])
        );
    }
}
