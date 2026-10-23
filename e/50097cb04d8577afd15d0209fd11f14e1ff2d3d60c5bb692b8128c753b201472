use super::super::*;
impl PersistentCallIndex {
    pub(super) fn build_before(program: &ResolvedProgram) -> Result<Self, Diagnostic> {
        hir::validate(program)?;
        let mut index = Self {
            sites_by_expression: BTreeMap::new(),
            calls_by_owner: BTreeMap::new(),
            callers_by_callee: BTreeMap::new(),
            kinds_by_owner: BTreeMap::new(),
            origins_by_owner: BTreeMap::new(),
        };
        for function in &program.functions {
            index.add_owner(
                program,
                &function.id,
                PersistentCallableKind::Function,
                &function.requires,
                &function.body,
                &function.ensures,
            )?;
        }
        for template in &program.function_templates {
            index.add_owner(
                program,
                &template.id,
                PersistentCallableKind::FunctionTemplate,
                &template.requires,
                &template.body,
                &template.ensures,
            )?;
        }

        // Graph v36 call closure includes conservative indirect candidates.
        // Direct call-site records stay direct: one invocation is not forged
        // into several independent source call expressions.
        for function in &program.functions {
            let mut dependencies = BTreeSet::new();
            hir::function_value::walk(function, |expression| match &expression.kind {
                // Persistent dependencies retain body callees for linking and
                // review. Creation is still an atomic Copy operation; exact
                // execution edges belong to the derived closure body graph.
                ResolvedExprKind::Closure { body, .. } => {
                    hir::visit_resolved_calls(body, &mut |callee, _, _| {
                        dependencies.insert(callee.clone());
                    });
                }
                ResolvedExprKind::FunctionReference { target } => {
                    dependencies.insert(target.clone());
                }
                ResolvedExprKind::Invoke { callable, .. } => dependencies.extend(
                    hir::function_value::compatible_targets(program, &callable.ty)
                        .into_iter()
                        .map(|target| target.id.clone()),
                ),
                _ => {}
            });
            index
                .calls_by_owner
                .get_mut(&function.id)
                .expect("registered function")
                .extend(dependencies);
        }

        let owners = index
            .kinds_by_owner
            .keys()
            .cloned()
            .collect::<BTreeSet<_>>();
        for callees in index.calls_by_owner.values_mut() {
            callees.retain(|callee| owners.contains(callee));
        }
        index.callers_by_callee = owners
            .iter()
            .cloned()
            .map(|owner| (owner, BTreeSet::new()))
            .collect();
        for (caller, callees) in &index.calls_by_owner {
            for callee in callees {
                if owners.contains(callee) {
                    index
                        .callers_by_callee
                        .get_mut(callee)
                        .expect("known callable has a reverse-index entry")
                        .insert(caller.clone());
                }
            }
        }
        Ok(index)
    }
}
