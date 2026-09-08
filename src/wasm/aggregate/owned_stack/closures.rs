//! Exact closure-profile activation weights. Creation copies scalars and has
//! no callee activation; only checked Invoke candidates enter private bodies.
use super::*;
use crate::hir::{self, FunctionExecutionId, ResolvedExprKind};

pub(super) fn derive(
    program: &ResolvedProgram,
    layouts: &VariantLayoutCache,
    roots: &[DeclarationId],
    weight: impl Fn(&FunctionPlan) -> Result<u32, Diagnostic>,
) -> Result<BTreeMap<DeclarationId, u32>, Diagnostic> {
    hir::validate(program)?;
    let bodies = hir::closure::inventory(program)
        .into_iter()
        .map(|site| hir::closure::closure_function(program, site))
        .collect::<Result<Vec<_>, _>>()?;
    let functions = program
        .functions
        .iter()
        .chain(&bodies)
        .map(|function| {
            (
                FunctionExecutionId::Monomorphic(function.id.clone()),
                function,
            )
        })
        .chain(program.function_instances.iter().map(|instance| {
            (
                FunctionExecutionId::Generic(instance.id.clone()),
                &instance.function,
            )
        }))
        .collect::<BTreeMap<_, _>>();
    let mut calls = BTreeMap::new();
    for (execution, function) in &functions {
        let mut targets = BTreeSet::new();
        hir::function_value::walk(function, |expression| match &expression.kind {
            ResolvedExprKind::Call {
                callee, instance, ..
            } => {
                let target = instance.as_ref().map_or_else(
                    || FunctionExecutionId::Monomorphic(callee.clone()),
                    |id| FunctionExecutionId::Generic(id.clone()),
                );
                if functions.contains_key(&target) {
                    targets.insert(target);
                }
            }
            ResolvedExprKind::Invoke { callable, .. } => {
                targets.extend(
                    hir::function_value::compatible_targets(program, &callable.ty)
                        .into_iter()
                        .map(|function| FunctionExecutionId::Monomorphic(function.id.clone())),
                );
                targets.extend(
                    hir::closure::inventory(program)
                        .into_iter()
                        .filter(|site| site.ty == callable.ty)
                        .map(|site| {
                            FunctionExecutionId::Monomorphic(hir::closure::closure_id(&site.id))
                        }),
                );
            }
            _ => {}
        });
        calls.insert(execution.clone(), targets);
    }
    let mut reachable = BTreeSet::new();
    let mut pending = roots
        .iter()
        .cloned()
        .map(FunctionExecutionId::Monomorphic)
        .collect::<Vec<_>>();
    while let Some(owner) = pending.pop() {
        if !reachable.insert(owner.clone()) {
            continue;
        }
        if reachable.len() > crate::project::MAX_PUBLIC_API_CLOSURE_FUNCTIONS {
            return Err(error(
                "owned-data closure execution inventory exceeds its bound",
            ));
        }
        pending.extend(
            calls
                .get(&owner)
                .ok_or_else(|| error("owned-data closure execution is absent"))?
                .iter()
                .cloned(),
        );
    }
    let frames = reachable
        .iter()
        .map(|execution| {
            let function = functions
                .get(execution)
                .ok_or_else(|| error("owned-data closure execution body is absent"))?;
            Ok((
                execution.clone(),
                weight(&FunctionPlan::build(program, function, layouts)?)?,
            ))
        })
        .collect::<Result<BTreeMap<_, _>, Diagnostic>>()?;
    calls.retain(|owner, _| reachable.contains(owner));
    Ok(longest_paths(&frames, &calls)?
        .into_iter()
        .filter_map(|(execution, extent)| match execution {
            FunctionExecutionId::Monomorphic(id) => Some((id, extent)),
            FunctionExecutionId::Generic(_) => None,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closure_creation_alone_does_not_charge_body_activation() {
        let source = r#"
module test.closure_stack_creation;
@id("stack.helper") fn helper(value:i64)->i64 { value+1 }
@id("stack.main") fn main()->i64 {
    let snapshot=7;
    let unused=fn(value:i64)->i64 { helper(snapshot)+value };
    0
}
"#;
        let program = hir::resolve(&crate::check(source, "closure-stack.spx").unwrap()).unwrap();
        let layouts =
            VariantLayoutCache::build(&program, crate::variant_layout::VariantTarget::Wasm32)
                .unwrap();
        let root = DeclarationId::new("stack.main");
        let function = program
            .functions
            .iter()
            .find(|function| function.id == root)
            .unwrap();
        let own_frame = FunctionPlan::build(&program, function, &layouts)
            .unwrap()
            .frame_size;
        let extents = derive(&program, &layouts, std::slice::from_ref(&root), |plan| {
            Ok(plan.frame_size)
        })
        .unwrap();
        assert_eq!(extents[&root], own_frame);
        assert_eq!(extents.len(), 1);
    }
}
