use crate::ast::{Expr, FieldDeclaration, Type};
use crate::diagnostic::Diagnostic;
use crate::source_verify::diagnostics::error;
use crate::source_verify::scope::VerifierFrame;
use crate::source_verify::IterativeVerifier;
use std::collections::HashSet;
impl<'a, 'p> IterativeVerifier<'a, 'p> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::source_verify) fn recovery_old_frame_prepare_record_field(
        &mut self,
        expression: &'p Expr,
        type_name: &'p str,
        type_arguments: &'p [Type],
        fields: &'p [crate::ast::FieldInitializer],
        declared_fields: Option<&'p [FieldDeclaration]>,
        scope: usize,
        index: usize,
        mut supplied: HashSet<&'p str>,
    ) -> Result<(), Diagnostic> {
        let field = &fields[index];
        let declared = self
            .types
            .declared_field(type_name, &field.name)
            .or_else(|| {
                declared_fields.and_then(|declared| {
                    declared
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                })
            });
        if !supplied.insert(field.name.as_str()) || declared.is_none() {
            self.diagnostics.push(error(
                self.program,
                "SPX-T212",
                format!(
                    "unknown or duplicate field `{}` in `{type_name}` construction",
                    field.name
                ),
                field.span,
            ));
        }
        self.frames.push(VerifierFrame::ResumeRecordField {
            expression,
            type_name,
            type_arguments,
            fields,
            declared_fields,
            scope,
            index,
            supplied,
        });
        self.frames.push(VerifierFrame::Enter {
            expression: &field.value,
            scope,
        });
        Ok(())
    }
}
