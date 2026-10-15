//! Complete previous current-parameter receiver, with namespace adapter only.
use super::{Diagnostic, Function, Recovery};
impl<'p> Recovery<'p> {
    pub(in crate::source_verify) fn before_parameter_signature(
        program: &'p dyn crate::ast::program_source::ProgramSource,
        current: &'p Function,
        variables: &std::collections::HashMap<String, crate::source_verify::binding::Binding>,
        types: &crate::source_verify::type_table::TypeTable<'_>,
        diagnostics: &[Diagnostic],
    ) -> Self {
        let mut recovery = Self::default();
        if !diagnostics
            .iter()
            .any(super::callee_components::unknown_component_receipt)
        {
            return recovery;
        }
        recovery.unknown_declarations = true;
        let mut parameters = None;
        let mut scratch = Vec::new();
        for parameter in &current.params {
            if types.needs_drop(&parameter.ty)
                || variables.get(&parameter.name).is_none_or(|binding| {
                    binding.ty != parameter.ty || binding.mode != parameter.mode
                })
                || !diagnostics.iter().any(|diagnostic| {
                    diagnostic.code == "SPX-T001" && diagnostic.span == Some(parameter.span)
                })
            {
                continue;
            }
            let parameters = parameters.get_or_insert_with(|| {
                current
                    .type_parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect()
            });
            scratch.clear();
            crate::source_verify::declared_type::check_declared_type(
                program,
                &parameter.ty,
                parameter.span,
                types,
                parameters,
                &mut scratch,
            );
            if let Some(origin) = scratch
                .iter()
                .filter(|refusal| refusal.code == "SPX-T001")
                .find_map(|refusal| {
                    diagnostics.iter().position(|original| {
                        original.code == refusal.code
                            && original.severity == refusal.severity
                            && original.message == refusal.message
                            && original.path == refusal.path
                            && original.span == refusal.span
                            && original.help == refusal.help
                    })
                })
            {
                recovery.ensure(0);
                recovery.scopes[0]
                    .names
                    .insert(parameter.name.as_str(), (origin, true, None));
            }
        }
        recovery
    }
}
