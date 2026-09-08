//! Exact case/field/binding validation for authored generic variant templates.
use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_template_authored_variant(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &ResolvedExpr,
        values: &mut BTreeMap<ValueId, ResolvedType>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if !super::super::generic_variant::profile(self.program, template) {
            return Err(hir_error(
                "authored variant expression requires an exact generic profile",
            ));
        }
        match &expression.kind {
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &expression.ty
                else {
                    return Err(hir_error("authored constructor type is not nominal"));
                };
                if declaration != variant
                    || !super::super::generic_variant::slot(
                        &self.program.declarations,
                        &expression.ty,
                        &template.id,
                        template.type_parameters.len(),
                    )
                    || expression.ownership != OwnershipMode::Own
                {
                    return Err(hir_error(
                        "authored constructor disagrees with its generic carrier",
                    ));
                }
                if !self
                    .program
                    .declarations
                    .variant_cases(variant)
                    .is_some_and(|cases| cases.iter().any(|item| item.id == *case))
                {
                    return Err(hir_error(
                        "authored constructor case belongs to another variant",
                    ));
                }
                let declared = self
                    .program
                    .declarations
                    .case_fields(case)
                    .ok_or_else(|| hir_error("authored constructor has unknown case"))?
                    .to_vec();
                if declared.len() != fields.len() {
                    return Err(hir_error(
                        "authored constructor field inventory is incomplete",
                    ));
                }
                for (index, (field, expected)) in fields.iter().zip(declared).enumerate() {
                    let ty = substitute_type(&expected.ty, variant, arguments)?;
                    if field.field != expected.id
                        || field.value.ty != ty
                        || field.value.ownership != template_ownership(self.program, template, &ty)
                    {
                        return Err(hir_error(
                            "authored constructor field identity, type, or ownership disagrees",
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
            ResolvedExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                if !matches!(mode, ResolvedMatchMode::Own | ResolvedMatchMode::Borrow)
                    || !super::super::generic_variant::slot(
                        &self.program.declarations,
                        &scrutinee.ty,
                        &template.id,
                        template.type_parameters.len(),
                    )
                {
                    return Err(hir_error(
                        "authored generic match has invalid mode or scrutinee",
                    ));
                }
                if *mode == ResolvedMatchMode::Borrow
                    && !matches!(&scrutinee.kind,ResolvedExprKind::Place(place) if place.projections.is_empty())
                {
                    return Err(hir_error(
                        "authored borrow match requires one unprojected place",
                    ));
                }
                self.validate_template_expr(
                    template,
                    execution,
                    scrutinee,
                    values,
                    &format!("{path}.scrutinee"),
                )?;
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &scrutinee.ty
                else {
                    unreachable!()
                };
                let cases = self
                    .program
                    .declarations
                    .variant_cases(declaration)
                    .ok_or_else(|| hir_error("authored match has unknown variant"))?
                    .to_vec();
                let mut covered = BTreeSet::new();
                for (index, arm) in arms.iter().enumerate() {
                    let ResolvedMatchPattern::Variant {
                        variant,
                        case,
                        fields,
                    } = &arm.pattern
                    else {
                        return Err(hir_error(
                            "authored generic match requires explicit variant cases",
                        ));
                    };
                    if variant != declaration
                        || !cases.iter().any(|item| item.id == *case)
                        || !covered.insert(case.clone())
                        || arm.guard.is_some()
                    {
                        return Err(hir_error(
                            "authored generic case identity, uniqueness, or guard is invalid",
                        ));
                    }
                    let declared = self
                        .program
                        .declarations
                        .case_fields(case)
                        .ok_or_else(|| hir_error("authored match has unknown case"))?
                        .to_vec();
                    if declared.len() != fields.len() {
                        return Err(hir_error("authored match field inventory is incomplete"));
                    }
                    let mut arm_values = values.clone();
                    let mut seen = BTreeSet::new();
                    for (field_index, field) in fields.iter().enumerate() {
                        let expected = declared
                            .iter()
                            .find(|item| item.id == field.field)
                            .ok_or_else(|| hir_error("authored match has a foreign field"))?;
                        let ty = substitute_type(&expected.ty, declaration, arguments)?;
                        let ownership = if ty == ResolvedType::Bytes {
                            if *mode == ResolvedMatchMode::Own {
                                OwnershipMode::Own
                            } else {
                                OwnershipMode::Borrow
                            }
                        } else {
                            OwnershipMode::Value
                        };
                        if !seen.insert(field.field.clone())
                            || field.binding.id
                                != ValueId::local(
                                    execution,
                                    &format!("{path}.arm.{index}.binding.{field_index}"),
                                )
                            || field.binding.ty != ty
                            || field.binding.ownership != ownership
                        {
                            return Err(hir_error(
                                "authored match binding identity, type, or ownership is invalid",
                            ));
                        }
                        self.insert_value(&field.binding.id)?;
                        arm_values.insert(field.binding.id.clone(), ty);
                    }
                    if arm.value.ty != expression.ty
                        || arm.value.ownership != expression.ownership
                        || (*mode == ResolvedMatchMode::Borrow
                            && expression.ownership != OwnershipMode::Value)
                    {
                        return Err(hir_error(
                            "authored match result type or ownership disagrees",
                        ));
                    }
                    self.validate_template_expr(
                        template,
                        execution,
                        &arm.value,
                        &mut arm_values,
                        &format!("{path}.arm.{index}.value"),
                    )?;
                }
                if covered.len() != cases.len() {
                    return Err(hir_error("authored generic match is not exhaustive"));
                }
            }
            _ => return Err(hir_error("not an authored variant template expression")),
        }
        Ok(())
    }
}

pub(super) fn handles(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    expression: &ResolvedExpr,
) -> bool {
    matches!(
        expression.kind,
        ResolvedExprKind::Match { .. } | ResolvedExprKind::ConstructVariant { .. }
    ) && super::super::generic_variant::profile(program, template)
}
