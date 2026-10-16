//! A generic profile cannot classify the shape of a missing type component.
//! The ordinary declared-type owner still publishes the authoritative receipt
//! in body preparation. This scratch result neither admits source nor erases
//! independent profile errors on other parameters.
use crate::ast::program_source::{FunctionRef, ProgramSource};
use crate::ast::{Span, Type};
use crate::source_verify::type_table::TypeTable;
use std::collections::HashSet;

pub(super) fn unknown_component(
    program: &dyn ProgramSource,
    ty: &Type,
    span: Span,
    types: &TypeTable<'_>,
    parameters: &HashSet<&str>,
) -> bool {
    // A missing child cannot erase a profile failure of its known outer
    // carrier (for example a by-value record or function-value signature).
    // Only this slot's own unresolved root identity defers its shape check.
    let Type::Named { name, .. } = ty else {
        return false;
    };
    if parameters.contains(name.as_str()) || types.declaration(name).is_some() {
        return false;
    }
    let mut scratch = Vec::new();
    crate::source_verify::declared_type::check_declared_type(
        program,
        ty,
        span,
        types,
        parameters,
        &mut scratch,
    );
    scratch.iter().any(|diagnostic| {
        diagnostic.code == "SPX-T001"
            || diagnostic.code == "SPX-T220"
                && diagnostic
                    .message
                    .ends_with("` is not an in-scope type parameter")
    })
}

/// Borrowed origin issued only inside the public function-body producer.
/// Its private fields bind the exact immutable source and specialization;
/// equality of IDs or spans alone cannot construct a receiving origin.
pub(in crate::source_verify) struct SignatureOrigin<'p> {
    subject: crate::ast::program_source::Subject<'p>,
    template: &'p FunctionRef<'p>,
    current: &'p crate::ast::Function,
}

#[cfg(test)]
pub(super) fn origin<'p>(
    program: &'p dyn ProgramSource,
    template: &'p FunctionRef<'p>,
    current: &'p crate::ast::Function,
) -> Option<SignatureOrigin<'p>> {
    origin_ref(program, template, &FunctionRef::ordinary(current))
}

pub(super) fn origin_ref<'p>(
    program: &'p dyn ProgramSource,
    template: &'p FunctionRef<'p>,
    current: &FunctionRef<'p>,
) -> Option<SignatureOrigin<'p>> {
    // Ordinary, un-specialized bodies already use their own exact environment.
    if template.same_presentation(current) {
        return None;
    }
    if !template.same_alias_cell(current)
        || !program.function_refs().contains_presentation(template)
        || !(1..=2).contains(&template.payload().type_parameters.len())
        || template
            .payload()
            .type_parameters
            .iter()
            .any(|parameter| !crate::source_verify::diagnostics::source_identifier(&parameter.name))
        || template.payload().type_parameters.len() == 2
            && template.payload().type_parameters[0].name
                == template.payload().type_parameters[1].name
        || !current.payload().type_parameters.is_empty()
        || template.payload().stable_id != current.payload().stable_id
        || template.name() != current.name()
        || template.payload().span != current.payload().span
        || template.payload().name_span != current.payload().name_span
        || template.payload().params.len() != current.payload().params.len()
        || !template
            .payload()
            .params
            .iter()
            .zip(&current.payload().params)
            .all(|(raw, concrete)| {
                raw.name == concrete.name && raw.mode == concrete.mode && raw.span == concrete.span
            })
    {
        return None;
    }
    Some(SignatureOrigin {
        subject: program.subject(),
        template,
        current: current.payload(),
    })
}

impl<'p> SignatureOrigin<'p> {
    #[cfg(test)]
    pub(in crate::source_verify) fn return_refusal(
        &self,
        program: &dyn ProgramSource,
        current: &crate::ast::Function,
        types: &TypeTable<'_>,
        diagnostics: &[crate::diagnostic::Diagnostic],
    ) -> Option<usize> {
        self.return_refusal_ref(program, &FunctionRef::ordinary(current), types, diagnostics)
    }

    pub(in crate::source_verify) fn return_refusal_ref(
        &self,
        program: &dyn ProgramSource,
        current: &FunctionRef<'_>,
        types: &TypeTable<'_>,
        diagnostics: &[crate::diagnostic::Diagnostic],
    ) -> Option<usize> {
        let template = self.template_for_ref(program, current)?.payload();
        let Type::Named { name, .. } = &template.return_type else {
            return None;
        };
        if template
            .type_parameters
            .iter()
            .any(|parameter| parameter.name == *name)
            || types.declaration(name).is_some()
            || !diagnostics.iter().any(|diagnostic| {
                diagnostic.span == Some(template.span)
                    && matches!(diagnostic.code, "SPX-T001" | "SPX-T220")
            })
        {
            return None;
        }
        let parameters = template
            .type_parameters
            .iter()
            .map(|parameter| parameter.name.as_str())
            .collect();
        let mut scratch = Vec::new();
        crate::source_verify::declared_type::check_declared_type(
            program,
            &template.return_type,
            template.span,
            types,
            &parameters,
            &mut scratch,
        );
        scratch
            .iter()
            .filter(|refusal| {
                refusal.code == "SPX-T001"
                    || refusal.code == "SPX-T220"
                        && refusal
                            .message
                            .ends_with("` is not an in-scope type parameter")
            })
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
    }

    #[cfg(test)]
    pub(in crate::source_verify) fn template_for(
        &self,
        program: &dyn ProgramSource,
        current: &crate::ast::Function,
    ) -> Option<&'p crate::ast::Function> {
        self.template_for_ref(program, &FunctionRef::ordinary(current))
            .map(|function| function.payload())
    }

    pub(in crate::source_verify) fn template_for_ref(
        &self,
        program: &dyn ProgramSource,
        current: &FunctionRef<'_>,
    ) -> Option<&'p FunctionRef<'p>> {
        (self.subject.same(program.subject())
            && std::ptr::eq(current.payload(), self.current)
            && current.same_alias_cell(self.template))
        .then_some(self.template)
    }
}

#[cfg(test)]
mod tests;
