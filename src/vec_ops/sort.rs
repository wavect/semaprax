//! Reachability of the additive Copy-scalar sorting operation.
pub(crate) fn program_uses_sort(program: &crate::ast::Program) -> bool {
    let uses = |function: &crate::ast::Function| {
        function.stable_id == super::wrapper_id(super::VecOp::Sort)
            || function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(|root| {
                    let mut found = false;
                    root.visit_calls(&mut |name, _| found |= name == super::SORT_NAME);
                    found
                })
    };
    program.module_uses.iter().any(|binding| binding.kind == crate::ast::ModuleUseKind::Function && binding.target_module == super::MODULE && binding.persistent_id == super::wrapper_id(super::VecOp::Sort)) || program.functions.iter().any(uses) || program.types.iter().any(|declaration| {
        matches!(&declaration.kind, crate::ast::TypeDeclarationKind::Class { methods, .. } if methods.iter().any(uses))
    })
}
pub(crate) fn resolved_program_uses_sort(program: &crate::hir::ResolvedProgram) -> bool {
    let uses = |function: &crate::hir::ResolvedFunction| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(|root| {
                let mut found = false;
                crate::hir::visit_resolved_calls(root, &mut |callee, _, _| {
                    found |= callee.as_str() == super::SORT_ID
                        || callee.as_str() == super::wrapper_id(super::VecOp::Sort)
                });
                found
            })
    };
    program.functions.iter().any(uses)
        || program
            .function_instances
            .iter()
            .any(|instance| uses(&instance.function))
        || program.function_templates.iter().any(|template| {
            template
                .requires
                .iter()
                .chain(std::iter::once(&template.body))
                .chain(&template.ensures)
                .any(|root| {
                    let mut found = false;
                    crate::hir::visit_resolved_calls(root, &mut |callee, _, _| {
                        found |= callee.as_str() == super::SORT_ID
                            || callee.as_str() == super::wrapper_id(super::VecOp::Sort)
                    });
                    found
                })
        })
}

#[cfg(test)]
mod tests {
    #[test]
    fn forged_sort_of_owned_payload_fails_independent_hir_validation() {
        let program = crate::check("module t; fn main()->i64 { let v=vec_with_capacity<Bytes>(0usize);let w=vec_clear<Bytes>(v);0 }", "forged-sort.spx").unwrap();
        let mut resolved = crate::hir::resolve(&program).unwrap();
        let main = resolved
            .functions
            .iter_mut()
            .find(|f| f.name == "main")
            .unwrap();
        let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
            panic!("block")
        };
        let crate::hir::ResolvedStatement::Let { value, .. } = &mut statements[1] else {
            panic!("let")
        };
        let crate::hir::ResolvedExprKind::Call { callee, .. } = &mut value.kind else {
            panic!("call")
        };
        *callee = crate::hir::DeclarationId::new(crate::vec_ops::SORT_ID);
        assert!(crate::hir::validate(&resolved).is_err());
    }
    #[test]
    fn sort_selects_new_prelude_without_rewriting_older_contracts() {
        let old = crate::check(
            "module t; fn main()->i64 {let v=vec_with_capacity<i64>(0usize);0}",
            "old.spx",
        )
        .unwrap();
        let new=crate::check("module t; fn main()->i64 {let v=vec_with_capacity<i64>(0usize);let w=vec_sort<i64>(v);0}","new.spx").unwrap();
        assert_eq!(
            crate::prelude::selected_for_program(&old).0,
            crate::prelude::SCHEMA_V2
        );
        assert_eq!(
            crate::prelude::selected_for_program(&new).0,
            crate::prelude::SCHEMA_V11
        );
        let old = String::from_utf8(crate::stdin_stream_ops::contract_bytes()).unwrap();
        let expected = old.replacen(crate::prelude::SCHEMA_V10, crate::prelude::SCHEMA_V11, 1);
        let new = String::from_utf8(crate::prelude::contract_bytes_v11()).unwrap();
        assert!(new.starts_with(&expected));
        assert!(new.ends_with("rule sort Copy_scalars_only ascending numeric_char_bool floating_IEEE_total_order no_payload_clones no_capacity_change generation=next\n"));
    }
}
