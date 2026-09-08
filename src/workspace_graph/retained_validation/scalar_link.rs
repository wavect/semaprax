//! Checked scalar private-link boundary and retained native authority.
use super::*;

impl ScalarNativeImports {
    /// Whether every one of these declared effects or module permits is
    /// carried by a retained Native Rust import. With nothing retained only
    /// the empty declaration is admitted, which is the historical rule.
    pub(in crate::workspace_graph) fn effects_admitted(&self, declared: &[String]) -> bool {
        declared.iter().all(|effect| self.effects.contains(effect))
    }

    /// Link one scalar closure, retaining the selected interfaces and their
    /// imports in the linked declaration index. With nothing retained this is
    /// the unchanged pure scalar linker.
    pub(in crate::workspace_graph) fn link(
        self,
        module: String,
        entrypoint: hir::DeclarationId,
        functions: Vec<hir::LinkedScalarFunction>,
        scalar: super::super::owned_generics::RetainedScalarParts,
        declarations: &BTreeMap<String, WorkspaceDeclarationFact>,
        require_main_display_name: bool,
    ) -> Result<hir::ResolvedProgram, Diagnostic> {
        let super::super::owned_generics::RetainedScalarParts {
            types,
            function_templates,
            function_instances,
        } = scalar;
        let mut declaration_facts = BTreeMap::new();
        for linked in &functions {
            let owner = declarations
                .get(linked.function.id.as_str())
                .and_then(|fact| fact.owner.as_deref())
                .map(hir::DeclarationId::new);
            retain_linked_fact(
                declarations,
                &mut declaration_facts,
                &linked.function.id,
                hir::DeclarationKind::Function,
                owner.as_ref(),
            )?;
        }
        for template in &function_templates {
            retain_linked_fact(
                declarations,
                &mut declaration_facts,
                &template.id,
                hir::DeclarationKind::Function,
                None,
            )?;
        }
        for declaration in &types {
            let kind = match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Record { .. } => hir::DeclarationKind::Record,
                hir::ResolvedTypeDeclarationKind::Class { .. } => hir::DeclarationKind::Class,
                hir::ResolvedTypeDeclarationKind::Variant { .. } => hir::DeclarationKind::Variant,
                hir::ResolvedTypeDeclarationKind::Resource { .. } => hir::DeclarationKind::Resource,
            };
            retain_linked_fact(
                declarations,
                &mut declaration_facts,
                &declaration.id,
                kind,
                None,
            )?;
            match &declaration.kind {
                hir::ResolvedTypeDeclarationKind::Record { fields }
                | hir::ResolvedTypeDeclarationKind::Class { fields, .. } => {
                    for field in fields {
                        retain_linked_fact(
                            declarations,
                            &mut declaration_facts,
                            &field.id,
                            hir::DeclarationKind::Field,
                            Some(&declaration.id),
                        )?;
                    }
                }
                hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        retain_linked_fact(
                            declarations,
                            &mut declaration_facts,
                            &case.id,
                            hir::DeclarationKind::VariantCase,
                            Some(&declaration.id),
                        )?;
                        for field in &case.fields {
                            retain_linked_fact(
                                declarations,
                                &mut declaration_facts,
                                &field.id,
                                hir::DeclarationKind::CaseField,
                                Some(&case.id),
                            )?;
                        }
                    }
                }
                hir::ResolvedTypeDeclarationKind::Resource { drop } => {
                    retain_linked_fact(
                        declarations,
                        &mut declaration_facts,
                        &drop.id,
                        hir::DeclarationKind::ResourceDrop,
                        Some(&declaration.id),
                    )?;
                }
            }
        }
        for interface in &self.interfaces {
            retain_linked_fact(
                declarations,
                &mut declaration_facts,
                &interface.id,
                hir::DeclarationKind::Interface,
                None,
            )?;
            for import in &interface.imports {
                retain_linked_fact(
                    declarations,
                    &mut declaration_facts,
                    &import.id,
                    hir::DeclarationKind::Import,
                    Some(&interface.id),
                )?;
            }
        }
        let private_callable_functions =
            super::super::owned_generics::private_callable_link_ids(&functions, declarations);
        let parts = hir::LinkedScalarProjectParts {
            private_callable_functions,
            types,
            interfaces: self.interfaces,
            function_templates,
            function_instances,
            declaration_facts,
        };
        if require_main_display_name {
            hir::link_scalar_project_workspace(module, entrypoint, functions, parts)
        } else {
            hir::link_scalar_project_exports(module, entrypoint, functions, parts)
        }
    }
}
