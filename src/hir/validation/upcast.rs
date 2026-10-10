//! Independent class-prefix and owned-tail checks.
use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_upcast(
        &self,
        expression: &ResolvedExpr,
        source: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let (
            ResolvedType::Nominal {
                declaration: child_id,
                arguments: child_arguments,
            },
            ResolvedType::Nominal {
                declaration: parent_id,
                arguments: parent_arguments,
            },
        ) = (&source.ty, &expression.ty)
        else {
            return Err(hir_error(
                "resolved upcast operands are not nominal classes",
            ));
        };
        if !child_arguments.is_empty() || !parent_arguments.is_empty() {
            return Err(hir_error("resolved upcast has generic class arguments"));
        }
        if !self.program.declarations.class_extends(child_id, parent_id) {
            return Err(hir_error(format!(
                "resolved upcast `{child_id}` does not inherit from `{parent_id}`"
            )));
        }
        let child_fields = self
            .program
            .declarations
            .record_fields(child_id)
            .ok_or_else(|| hir_error(format!("class `{child_id}` has no fields")))?;
        let parent_fields = self
            .program
            .declarations
            .record_fields(parent_id)
            .ok_or_else(|| hir_error(format!("class `{parent_id}` has no fields")))?;
        if child_fields.len() < parent_fields.len()
            || child_fields[..parent_fields.len()]
                .iter()
                .zip(parent_fields.iter())
                .any(|(child_field, parent_field)| child_field.id != parent_field.id)
        {
            return Err(hir_error(format!(
                "resolved upcast `{child_id}` prefix disagrees with ancestor `{parent_id}`"
            )));
        }
        for field in &child_fields[parent_fields.len()..] {
            let drops = self
                .program
                .declarations
                .type_facts(&field.ty)
                .is_some_and(|facts| facts.needs_drop);
            if drops {
                return Err(hir_error(format!(
                    "resolved upcast from `{child_id}` would discard owned field `{}`",
                    field.name
                )));
            }
        }
        let _ = source;
        Ok(())
    }
}
