//! Validation-only rules for bounded generic templates.

use super::*;

impl HirValidator<'_> {
    pub(super) fn validate_template_expressions(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
    ) -> Result<(), Diagnostic> {
        let mut values = BTreeMap::new();
        for parameter in &template.params {
            self.insert_value(&parameter.id)?;
            values.insert(parameter.id.clone(), parameter.ty.clone());
        }
        for (index, expression) in template.requires.iter().enumerate() {
            let mut contract_values = values.clone();
            self.validate_template_expr(
                template,
                execution,
                expression,
                &mut contract_values,
                &format!("requires.{index}"),
            )?;
        }
        self.validate_template_expr_with_context(
            template,
            execution,
            &template.body,
            &mut values,
            "body",
            (false, true),
        )?;
        self.insert_value(&template.result_id)?;
        values.insert(template.result_id.clone(), template.return_type.clone());
        for (index, expression) in template.ensures.iter().enumerate() {
            let mut contract_values = values.clone();
            self.validate_template_expr(
                template,
                execution,
                expression,
                &mut contract_values,
                &format!("ensures.{index}"),
            )?;
        }
        Ok(())
    }
}

pub(super) fn has_owned_record_composition(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> bool {
    let owned = template
        .params
        .iter()
        .filter(|parameter| parameter.ownership == OwnershipMode::Own)
        .collect::<Vec<_>>();
    !owned.is_empty()
        && template_has_owned_record_slot(program, template)
        && owned.iter().all(|parameter| {
            super::super::type_reachability::is_nested_owned_byte_record_template(
                &program.declarations,
                &parameter.ty,
                &template.id,
                template.type_parameters.len(),
            )
        })
}

pub(super) fn is_owned_record_expression(expression: &ResolvedExpr) -> bool {
    matches!(
        expression.kind,
        ResolvedExprKind::ConstructRecord { .. }
            | ResolvedExprKind::Match { .. }
            | ResolvedExprKind::UpdateRecord { .. }
            | ResolvedExprKind::Project { .. }
    )
}

pub(super) fn expression_ownership_is_valid(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    expression: &ResolvedExpr,
) -> bool {
    expression.ownership == template_ownership(program, template, &expression.ty)
        || (expression.ownership != OwnershipMode::Value
            && program
                .declarations
                .type_facts(&expression.ty)
                .is_some_and(|facts| facts.needs_drop))
}

pub(super) fn body_type_requires_validation(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    ty: &ResolvedType,
) -> bool {
    ty != &ResolvedType::Char
        && ty != &ResolvedType::Str
        && !(ty == &ResolvedType::Bytes && has_owned_record_composition(program, template))
}

fn admits_owned_record_match_result(
    program: &ResolvedProgram,
    execution: &FunctionExecutionId,
    expression: &ResolvedExpr,
    arm: &ResolvedMatchArm,
) -> bool {
    let FunctionExecutionId::Generic(instance_id) = execution else {
        return false;
    };
    let Some((template, instance_return)) = owned_record_template_execution(program, instance_id)
    else {
        return false;
    };
    let ResolvedExprKind::Match {
        mode,
        scrutinee,
        arms,
    } = &expression.kind
    else {
        return false;
    };
    let ResolvedMatchPattern::Record {
        record,
        instance: pattern_instance,
        fields: _,
    } = &arm.pattern
    else {
        return false;
    };
    let common = arms.len() == 1
        && has_owned_record_composition(program, template)
        && pattern_instance == &scrutinee.ty
        && matches!(&scrutinee.ty, ResolvedType::Nominal { declaration, .. }
            if declaration == record)
        && super::super::type_reachability::is_admitted_nested_owned_byte_record(
            &program.declarations,
            &scrutinee.ty,
        );
    let owned_result = *mode == ResolvedMatchMode::Own
        && (instance_return == expression.ty
            || super::super::type_reachability::is_admitted_nested_owned_byte_record(
                &program.declarations,
                &expression.ty,
            ))
        && expression.ty == arm.value.ty
        && expression.ownership == OwnershipMode::Own
        && scrutinee.ownership == OwnershipMode::Own
        && arm.value.ownership == OwnershipMode::Own;
    let borrowed_copy_field = *mode == ResolvedMatchMode::Borrow
        && expression.ty == arm.value.ty
        && expression.ownership == OwnershipMode::Value
        && arm.value.ownership == OwnershipMode::Value
        && matches!(
            scrutinee.ownership,
            OwnershipMode::Own | OwnershipMode::Borrow
        )
        && super::super::type_reachability::nested_record_copy_scalar_is_admitted(&expression.ty);
    common && (owned_result || borrowed_copy_field)
}

fn owned_record_template_execution<'a>(
    program: &'a ResolvedProgram,
    instance_id: &FunctionInstanceId,
) -> Option<(&'a ResolvedFunctionTemplate, ResolvedType)> {
    if let Some(instance) = program
        .function_instances
        .iter()
        .find(|candidate| candidate.id == *instance_id)
    {
        let template = program
            .function_templates
            .iter()
            .find(|candidate| candidate.id == instance.template)?;
        return Some((template, instance.function.return_type.clone()));
    }
    let mut matched = None;
    for template in &program.function_templates {
        if !(1..=2).contains(&template.type_parameters.len())
            || !has_owned_record_composition(program, template)
        {
            continue;
        }
        for arguments in super::super::monomorphize::resolved_owned_record_substitutions(
            template.type_parameters.len(),
        ) {
            if FunctionInstanceId::derive(&template.id, &arguments) != *instance_id {
                continue;
            }
            let return_type =
                substitute_type(&template.return_type, &template.id, &arguments).ok()?;
            if matched.is_some() {
                return None;
            }
            matched = Some((template, return_type));
        }
    }
    matched
}

fn validate_record_match_result(
    program: &ResolvedProgram,
    execution: &FunctionExecutionId,
    expression: &ResolvedExpr,
    arm: &ResolvedMatchArm,
) -> Result<(), Diagnostic> {
    if matches!(arm.value.ty, ResolvedType::I64 | ResolvedType::Bool)
        || admits_owned_record_match_result(program, execution, expression, arm)
    {
        Ok(())
    } else {
        Err(hir_error(
            "resolved record match arm must produce i64 or bool",
        ))
    }
}

impl HirValidator<'_> {
    pub(super) fn validate_generic_record_match_result(
        &self,
        execution: &FunctionExecutionId,
        expression: &ResolvedExpr,
        arm: &ResolvedMatchArm,
    ) -> Result<(), Diagnostic> {
        validate_record_match_result(self.program, execution, expression, arm)
    }
}

pub(super) fn projected_place_type(
    program: &ResolvedProgram,
    place: &Place,
    values: &BTreeMap<ValueId, ResolvedType>,
) -> Result<ResolvedType, Diagnostic> {
    let mut ty = values
        .get(&place.root)
        .cloned()
        .ok_or_else(|| hir_error("generic template place root is out of scope"))?;
    for projection in &place.projections {
        let PlaceProjection::Field(field) = projection else {
            return Err(hir_error(
                "generic template place uses a non-record projection",
            ));
        };
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &ty
        else {
            return Err(hir_error("generic template projects a non-record place"));
        };
        let field_ty = program
            .declarations
            .record_fields(declaration)
            .and_then(|fields| fields.iter().find(|candidate| candidate.id == *field))
            .map(|candidate| &candidate.ty)
            .ok_or_else(|| hir_error("generic template projects an unknown record field"))?;
        ty = substitute_type(field_ty, declaration, arguments)?;
    }
    Ok(ty)
}

pub(super) fn validate_template_place(
    program: &ResolvedProgram,
    _template: &ResolvedFunctionTemplate,
    expression: &ResolvedExpr,
    place: &Place,
    values: &BTreeMap<ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    if projected_place_type(program, place, values)? != expression.ty {
        return Err(hir_error("generic projected place type differs"));
    }
    Ok(())
}

pub(super) fn authenticate_vec_wrapper(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> Result<Option<crate::vec_ops::VecOp>, Diagnostic> {
    let candidate = crate::vec_ops::wrapper_by_id(template.id.as_str()).is_some()
        || (program.module == crate::vec_ops::MODULE
            && crate::vec_ops::ALL
                .into_iter()
                .any(|op| crate::vec_ops::wrapper_name(op) == template.name));
    match (
        candidate,
        crate::vec_ops::hir_wrapper_in_program(program, template),
    ) {
        (false, _) => Ok(None),
        (true, Some(op)) => Ok(Some(op)),
        (true, None) => Err(hir_error(format!(
            "generic template `{}` is not an authenticated std.collections vector wrapper",
            template.id
        ))),
    }
}

pub(super) fn authenticate_box_wrapper(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> Result<Option<crate::box_ops::BoxOp>, Diagnostic> {
    let candidate = crate::box_ops::wrapper_by_id(template.id.as_str()).is_some()
        || (program.module == crate::box_ops::MODULE
            && crate::box_ops::ALL
                .into_iter()
                .any(|op| crate::box_ops::wrapper_name(op) == template.name));
    match (
        candidate,
        crate::box_ops::hir_wrapper_in_program(program, template),
    ) {
        (false, _) => Ok(None),
        (true, Some(op)) => Ok(Some(op)),
        (true, None) => Err(hir_error(format!(
            "generic template `{}` is not an authenticated std.mem box wrapper",
            template.id
        ))),
    }
}

pub(super) fn vec_wrapper_substitutions() -> Vec<Vec<ResolvedType>> {
    [
        ResolvedType::I64,
        ResolvedType::I32,
        ResolvedType::U8,
        ResolvedType::Usize,
        ResolvedType::Char,
        ResolvedType::F32,
        ResolvedType::F64,
        ResolvedType::Bool,
    ]
    .into_iter()
    .map(|ty| vec![ty])
    .collect()
}

pub(super) fn validate_type(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    ty: &ResolvedType,
) -> Result<(), Diagnostic> {
    let admitted = (super::super::generic_collection::profile(template)
        && (super::super::generic_collection::slot(
            ty,
            &template.id,
            template.type_parameters.len(),
        ) || super::super::generic_collection::scalar(ty)))
        || (super::super::generic_result::profile(template)
            && (super::super::generic_result::slot(
                ty,
                &template.id,
                template.type_parameters.len(),
            ) || *ty == ResolvedType::Bytes))
        || matches!(
            ty,
            ResolvedType::I64 | ResolvedType::Bool | ResolvedType::String
        )
        || matches!(ty, ResolvedType::TypeParameter { owner, index }
            if owner == &template.id && usize::try_from(*index).ok()
                .is_some_and(|index| index < template.type_parameters.len()))
        || super::super::type_reachability::is_nested_owned_byte_record_template(
            &program.declarations,
            ty,
            &template.id,
            template.type_parameters.len(),
        )
        || crate::vec_ops::template_type_is_admitted(template, ty)
        || crate::box_ops::template_type_is_admitted(template, ty);
    admitted.then_some(()).ok_or_else(|| {
        hir_error(format!(
            "generic template `{}` has an invalid direct-scalar signature slot",
            template.id
        ))
    })
}

pub(super) fn is_vec_wrapper_call(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    callee: &DeclarationId,
    type_arguments: &[ResolvedType],
    instance: &Option<FunctionInstanceId>,
) -> bool {
    instance.is_none()
        && (super::super::generic_collection::profile(template)
            && (crate::vec_ops::by_id(callee.as_str()).is_some()
                || crate::box_ops::by_id(callee.as_str()).is_some())
            || crate::vec_ops::hir_wrapper_in_program(program, template)
                == crate::vec_ops::by_id(callee.as_str()))
        && matches!(type_arguments,
            [ResolvedType::TypeParameter { owner, index: 0 }] if owner == &template.id)
}

pub(super) fn is_forwarded_call(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    callee: &DeclarationId,
    type_arguments: &[ResolvedType],
    instance: &Option<FunctionInstanceId>,
) -> bool {
    program
        .function_templates
        .iter()
        .find(|target| target.id == *callee)
        .is_some_and(|target| {
            target.type_parameters.len() == type_arguments.len()
                && crate::hir::generic_mapping::arguments(
                    &template.id,
                    template.type_parameters.len(),
                    type_arguments,
                )
                && instance.as_ref().is_some_and(|instance| {
                    FunctionInstanceId::derive(callee, type_arguments) == *instance
                })
        })
}

pub(super) fn validate_call_graph(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    let ids = program
        .function_templates
        .iter()
        .map(|template| template.id.clone())
        .collect::<BTreeSet<_>>();
    let mut edges = BTreeMap::<DeclarationId, BTreeSet<DeclarationId>>::new();
    let mut incoming = ids
        .iter()
        .cloned()
        .map(|id| (id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for template in &program.function_templates {
        for expression in template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
        {
            visit_resolved_calls(expression, &mut |callee, _, _| {
                if ids.contains(callee)
                    && edges
                        .entry(template.id.clone())
                        .or_default()
                        .insert(callee.clone())
                {
                    *incoming.get_mut(callee).expect("template was indexed") += 1;
                }
            });
        }
    }
    let mut pending = incoming
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<std::collections::VecDeque<_>>();
    let mut visited = 0usize;
    while let Some(id) = pending.pop_front() {
        visited += 1;
        for callee in edges.get(&id).into_iter().flatten() {
            let count = incoming.get_mut(callee).expect("template was indexed");
            *count -= 1;
            if *count == 0 {
                pending.push_back(callee.clone());
            }
        }
    }
    (visited == ids.len())
        .then_some(())
        .ok_or_else(|| hir_error("generic function template call graph contains a direct cycle"))
}

pub(super) fn reachable_instances(
    program: &ResolvedProgram,
) -> Result<Vec<(FunctionInstanceId, DeclarationId, Vec<ResolvedType>)>, Diagnostic> {
    const MAX_FUNCTION_INSTANCES: usize = 256;
    let mut seen = BTreeSet::new();
    let mut reachable = Vec::new();
    let mut pending = std::collections::VecDeque::new();
    for function in &program.functions {
        collect_calls(function, &mut pending);
    }
    while let Some((instance, template, arguments)) = pending.pop_front() {
        if FunctionInstanceId::derive(&template, &arguments) != instance {
            return Err(hir_error(
                "reachable generic function instance identity is inconsistent",
            ));
        }
        if !seen.insert(instance.clone()) {
            continue;
        }
        if reachable.len() == MAX_FUNCTION_INSTANCES {
            return Err(hir_error(format!(
                "generic function instance closure exceeds {MAX_FUNCTION_INSTANCES} entries"
            )));
        }
        reachable.push((instance.clone(), template, arguments));
        if let Some(materialized) = program
            .function_instances
            .iter()
            .find(|candidate| candidate.id == instance)
        {
            collect_calls(&materialized.function, &mut pending);
        }
    }
    Ok(reachable)
}

fn collect_calls(
    function: &ResolvedFunction,
    pending: &mut std::collections::VecDeque<(
        FunctionInstanceId,
        DeclarationId,
        Vec<ResolvedType>,
    )>,
) {
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        visit_resolved_calls(expression, &mut |callee, instance, arguments| {
            if let Some(instance) = instance {
                pending.push_back((instance.clone(), callee.clone(), arguments.to_vec()));
            }
        });
    }
}

pub(super) fn proof_return_type(
    program: &ResolvedProgram,
    callee: &DeclarationId,
    instance: Option<&FunctionInstanceId>,
    arguments: &[ResolvedType],
) -> Result<ResolvedType, Diagnostic> {
    let template = program
        .function_templates
        .iter()
        .find(|template| template.id == *callee)
        .ok_or_else(|| hir_error(format!("function `{callee}` is not indexed")))?;
    if instance.is_none_or(|instance| FunctionInstanceId::derive(callee, arguments) != *instance) {
        return Err(hir_error("proof generic call identity is inconsistent"));
    }
    substitute_type(&template.return_type, &template.id, arguments)
}

pub(super) fn proof_signature(
    program: &ResolvedProgram,
    callee: &DeclarationId,
    instance: Option<&FunctionInstanceId>,
    arguments: &[ResolvedType],
) -> Result<(Vec<ResolvedParam>, ResolvedType, Vec<String>), Diagnostic> {
    let template = program
        .function_templates
        .iter()
        .find(|template| template.id == *callee)
        .ok_or_else(|| hir_error(format!("resolved callee `{callee}` is not indexed")))?;
    if instance.is_none_or(|instance| FunctionInstanceId::derive(callee, arguments) != *instance) {
        return Err(hir_error("proof generic call identity is inconsistent"));
    }
    let params = template
        .params
        .iter()
        .map(|parameter| {
            Ok(ResolvedParam {
                id: parameter.id.clone(),
                name: parameter.name.clone(),
                ownership: parameter.ownership,
                ty: substitute_type(&parameter.ty, &template.id, arguments)?,
                span: parameter.span,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok((
        params,
        substitute_type(&template.return_type, &template.id, arguments)?,
        template.effects.clone(),
    ))
}

pub(super) fn substitutions(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    transparent_owned_wrapper: bool,
) -> Vec<Vec<ResolvedType>> {
    if super::super::generic_collection::profile(template) {
        vec_wrapper_substitutions()
    } else if super::super::generic_result::profile(template) {
        super::super::generic_result::substitutions(&template.return_type)
    } else if transparent_owned_wrapper {
        vec_wrapper_substitutions()
    } else if template_has_owned_record_slot(program, template) {
        resolved_owned_record_substitutions(template.type_parameters.len())
    } else {
        resolved_scalar_substitutions(template.type_parameters.len())
    }
}

impl HirValidator<'_> {
    pub(super) fn validate_template_result_expression(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &ResolvedExpr,
        values: &mut BTreeMap<ValueId, ResolvedType>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if !super::super::generic_result::profile(template) || !path.starts_with("body") {
            return Err(hir_error(
                "generic Result expression requires its exact owning relay body",
            ));
        }
        let ResolvedType::Nominal { arguments, .. } = &template.return_type else {
            unreachable!()
        };
        let success = &arguments[0];
        let success_ownership = template_ownership(self.program, template, success);
        match &expression.kind {
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                let [field] = fields.as_slice() else {
                    return Err(hir_error(
                        "generic Result reconstruction requires one Ok payload",
                    ));
                };
                if variant.as_str() != crate::prelude::RESULT_ID
                    || case.as_str() != crate::prelude::RESULT_OK_ID
                    || field.field.as_str() != crate::prelude::RESULT_OK_VALUE_ID
                    || expression.ty != template.return_type
                    || expression.ownership != OwnershipMode::Own
                    || &field.value.ty != success
                    || field.value.ownership != success_ownership
                {
                    return Err(hir_error(
                        "generic Result reconstruction differs from its owning signature",
                    ));
                }
                self.validate_template_expr(
                    template,
                    execution,
                    &field.value,
                    values,
                    &format!("{path}.field.0.value"),
                )
            }
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                if result.as_str() != crate::prelude::RESULT_ID
                    || ok_case.as_str() != crate::prelude::RESULT_OK_ID
                    || ok_field.as_str() != crate::prelude::RESULT_OK_VALUE_ID
                    || err_case.as_str() != crate::prelude::RESULT_ERR_ID
                    || err_field.as_str() != crate::prelude::RESULT_ERR_ERROR_ID
                    || *residual_type != template.return_type
                    || operand.ty != *residual_type
                    || operand.ownership != OwnershipMode::Own
                    || &expression.ty != success
                    || expression.ownership != success_ownership
                {
                    return Err(hir_error(
                        "generic Result propagation differs from its exact owning residual",
                    ));
                }
                self.validate_template_expr(
                    template,
                    execution,
                    operand,
                    values,
                    &format!("{path}.operand"),
                )
            }
            _ => Err(hir_error(
                "generic Result helper received unsupported expression",
            )),
        }
    }
}
