//! Structural symbolic records; complete materialized ownership remains mandatory.
use super::*;

fn fields(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<Vec<(DeclarationId, ResolvedType)>, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(hir_error("generic record expression has a non-record type"));
    };
    program
        .declarations
        .record_fields(declaration)
        .ok_or_else(|| hir_error("generic record declaration is absent"))?
        .iter()
        .map(|field| {
            Ok((
                field.id.clone(),
                substitute_type(&field.ty, declaration, arguments)?,
            ))
        })
        .collect()
}

impl HirValidator<'_> {
    pub(super) fn validate_composed_template_record(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &ResolvedExpr,
        values: &mut BTreeMap<ValueId, ResolvedType>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        match &expression.kind {
            ResolvedExprKind::ConstructRecord {
                record,
                fields: initialized,
            } => {
                if !matches!(&expression.ty,ResolvedType::Nominal {declaration,..} if declaration==record)
                {
                    return Err(hir_error(
                        "generic constructor identity differs from its exact type",
                    ));
                }
                let declared = fields(self.program, &expression.ty)?;
                if declared.len() != initialized.len() {
                    return Err(hir_error("generic constructor fields are incomplete"));
                }
                for (index, (field, (id, ty))) in initialized.iter().zip(declared).enumerate() {
                    if field.field != id || field.value.ty != ty {
                        return Err(hir_error(
                            "generic constructor field identity or type differs",
                        ));
                    }
                    self.validate_template_expr(
                        template,
                        execution,
                        &field.value,
                        values,
                        &format!("{path}.field.{index}.value"),
                    )?;
                }
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields: initialized,
            } => {
                if base.ty != expression.ty
                    || !matches!(&expression.ty,ResolvedType::Nominal {declaration,..} if declaration==record)
                {
                    return Err(hir_error(
                        "generic update must preserve the exact record type",
                    ));
                }
                self.validate_template_expr(
                    template,
                    execution,
                    base,
                    values,
                    &format!("{path}.base"),
                )?;
                let declared = fields(self.program, &expression.ty)?;
                let mut previous = None;
                for (index, field) in initialized.iter().enumerate() {
                    let Some((position, (_, ty))) = declared
                        .iter()
                        .enumerate()
                        .find(|(_, (id, _))| *id == field.field)
                    else {
                        return Err(hir_error("generic update field is unknown"));
                    };
                    if previous.is_some_and(|p| p >= position) || field.value.ty != *ty {
                        return Err(hir_error("generic update fields are not canonically typed"));
                    }
                    previous = Some(position);
                    self.validate_template_expr(
                        template,
                        execution,
                        &field.value,
                        values,
                        &format!("{path}.field.{index}.value"),
                    )?;
                }
            }
            ResolvedExprKind::Project { base, field } => {
                let declared = fields(self.program, &base.ty)?;
                if !declared
                    .iter()
                    .any(|(id, ty)| id == field && *ty == expression.ty)
                {
                    return Err(hir_error("generic projection has the wrong declared type"));
                }
                self.validate_template_expr(
                    template,
                    execution,
                    base,
                    values,
                    &format!("{path}.base"),
                )?;
            }
            ResolvedExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                self.validate_template_expr(
                    template,
                    execution,
                    scrutinee,
                    values,
                    &format!("{path}.scrutinee"),
                )?;
                if arms.len() != 1 || arms[0].guard.is_some() {
                    return Err(hir_error(
                        "generic record match must have one complete unguarded arm",
                    ));
                }
                for (index, arm) in arms.iter().enumerate() {
                    let ResolvedMatchPattern::Record {
                        record,
                        instance,
                        fields,
                    } = &arm.pattern
                    else {
                        return Err(hir_error("generic record match has a non-record pattern"));
                    };
                    let mut arm_values = values.clone();
                    self.validate_composed_record_pattern(
                        template,
                        execution,
                        &scrutinee.ty,
                        record,
                        instance,
                        fields,
                        *mode,
                        &mut arm_values,
                        &format!("{path}.arm.{index}.record"),
                    )?;
                    if arm.value.ty != expression.ty || arm.value.ownership != expression.ownership
                    {
                        return Err(hir_error("generic record arm has a mismatched result"));
                    }
                    self.validate_template_expr(
                        template,
                        execution,
                        &arm.value,
                        &mut arm_values,
                        &format!("{path}.arm.{index}.value"),
                    )?;
                }
            }
            _ => return Err(hir_error("generic record expression kind differs")),
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn validate_composed_record_pattern(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expected: &ResolvedType,
        record: &DeclarationId,
        instance: &ResolvedType,
        pattern: &[ResolvedRecordMatchPatternField],
        mode: ResolvedMatchMode,
        values: &mut BTreeMap<ValueId, ResolvedType>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if expected != instance
            || !matches!(instance,ResolvedType::Nominal{declaration,..} if declaration==record)
        {
            return Err(hir_error(
                "generic recursive pattern has a mismatched instance",
            ));
        }
        let declared = fields(self.program, instance)?;
        if pattern.len() != declared.len() {
            return Err(hir_error("generic recursive pattern is incomplete"));
        }
        for (index, (field, (id, ty))) in pattern.iter().zip(declared).enumerate() {
            if field.field != id {
                return Err(hir_error("generic recursive pattern field order differs"));
            }
            let field_path = format!("{path}.field.{index}");
            let owner = template_ownership(self.program, template, &ty);
            match &field.pattern {
                ResolvedRecordMatchFieldPattern::Binding(binding) => {
                    let ownership = if owner == OwnershipMode::Own {
                        match mode {
                            ResolvedMatchMode::Own => OwnershipMode::Own,
                            ResolvedMatchMode::Borrow => OwnershipMode::Borrow,
                            ResolvedMatchMode::Value => OwnershipMode::Value,
                        }
                    } else {
                        OwnershipMode::Value
                    };
                    if binding.id != ValueId::local(execution, &format!("{field_path}.binding"))
                        || binding.ty != ty
                        || binding.ownership != ownership
                    {
                        return Err(hir_error(
                            "generic recursive binding identity or ownership differs",
                        ));
                    }
                    self.insert_value(&binding.id)?;
                    values.insert(binding.id.clone(), ty);
                }
                ResolvedRecordMatchFieldPattern::Wildcard => {}
                ResolvedRecordMatchFieldPattern::Record {
                    record,
                    instance,
                    fields,
                } => {
                    self.validate_composed_record_pattern(
                        template,
                        execution,
                        &ty,
                        record,
                        instance,
                        fields,
                        mode,
                        values,
                        &format!("{field_path}.record"),
                    )?;
                }
            }
        }
        Ok(())
    }
}
