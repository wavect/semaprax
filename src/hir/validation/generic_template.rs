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

pub(super) fn has_exact_owned_record_relay(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> bool {
    let owned = template
        .params
        .iter()
        .filter(|parameter| parameter.ownership == OwnershipMode::Own)
        .collect::<Vec<_>>();
    matches!(owned.as_slice(), [parameter]
    if parameter.ty == template.return_type
        && template_has_owned_record_slot(program, template))
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
        && !(ty == &ResolvedType::Bytes && has_exact_owned_record_relay(program, template))
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
        fields,
    } = &arm.pattern
    else {
        return false;
    };
    let common = arms.len() == 1
        && has_exact_owned_record_relay(program, template)
        && pattern_instance == &scrutinee.ty
        && matches!(&scrutinee.ty, ResolvedType::Nominal { declaration, .. }
            if declaration == record)
        && super::super::type_reachability::is_flat_owned_byte_record(
            &program.declarations,
            &scrutinee.ty,
        );
    let owned_result = *mode == ResolvedMatchMode::Own
        && instance_return == expression.ty
        && expression.ty == scrutinee.ty
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
        && super::super::type_reachability::nested_record_copy_scalar_is_admitted(&expression.ty)
        && fields.iter().any(|field| {
            let ResolvedRecordMatchFieldPattern::Binding(binding) = &field.pattern else {
                return false;
            };
            let ResolvedExprKind::Place(place) = &arm.value.kind else {
                return false;
            };
            let Some(field_ty) = program
                .declarations
                .record_fields(record)
                .and_then(|items| items.iter().find(|item| item.id == field.field))
                .map(|item| &item.ty)
            else {
                return false;
            };
            matches!((field_ty, pattern_instance),
                (ResolvedType::TypeParameter { owner, index },
                 ResolvedType::Nominal { arguments, .. })
                    if owner == record
                        && arguments.get(*index as usize) == Some(&expression.ty))
                && binding.ty == expression.ty
                && place.root == binding.id
                && place.projections.is_empty()
        });
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
            || !has_exact_owned_record_relay(program, template)
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

fn exact_flat_relay(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
) -> Result<(ValueId, ResolvedType, DeclarationId, Vec<ResolvedType>), Diagnostic> {
    let owned = template
        .params
        .iter()
        .filter(|parameter| parameter.ownership == OwnershipMode::Own)
        .collect::<Vec<_>>();
    let [parameter] = owned.as_slice() else {
        return Err(hir_error(
            "generic aggregate expression requires exactly one owned relay parameter",
        ));
    };
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &parameter.ty
    else {
        return Err(hir_error("generic aggregate relay must be a record"));
    };
    if parameter.ty != template.return_type
        || !super::super::type_reachability::is_flat_owned_byte_record_template(
            &program.declarations,
            &parameter.ty,
            &template.id,
            template.type_parameters.len(),
        )
    {
        return Err(hir_error(
            "generic aggregate expression requires an exact flat owned-record relay",
        ));
    }
    Ok((
        parameter.id.clone(),
        parameter.ty.clone(),
        declaration.clone(),
        arguments.clone(),
    ))
}

fn relay_fields(
    program: &ResolvedProgram,
    record: &DeclarationId,
    arguments: &[ResolvedType],
) -> Result<Vec<(DeclarationId, ResolvedType)>, Diagnostic> {
    program
        .declarations
        .record_fields(record)
        .ok_or_else(|| hir_error("generic aggregate relay record is missing"))?
        .iter()
        .map(|field| {
            Ok((
                field.id.clone(),
                substitute_type(&field.ty, record, arguments)?,
            ))
        })
        .collect()
}

fn field_ownership(ty: &ResolvedType) -> OwnershipMode {
    if *ty == ResolvedType::Bytes {
        OwnershipMode::Own
    } else {
        OwnershipMode::Value
    }
}

fn is_frozen_direct_scalar(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_) => true,
        ResolvedExprKind::Place(place) => place.projections.is_empty(),
        ResolvedExprKind::Call { args, .. } => args.iter().all(is_frozen_direct_scalar),
        ResolvedExprKind::Unary { value, .. } => is_frozen_direct_scalar(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            is_frozen_direct_scalar(left) && is_frozen_direct_scalar(right)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().all(|statement| match statement {
                ResolvedStatement::Let { value, .. } => is_frozen_direct_scalar(value),
                ResolvedStatement::Unsafe { body, .. } => is_frozen_direct_scalar(body),
                ResolvedStatement::Assign { .. } | ResolvedStatement::While { .. } => false,
            }) && is_frozen_direct_scalar(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            is_frozen_direct_scalar(condition)
                && is_frozen_direct_scalar(then_branch)
                && is_frozen_direct_scalar(else_branch)
        }
        _ => false,
    }
}

fn references_any_value(expression: &ResolvedExpr, values: &BTreeSet<ValueId>) -> bool {
    match &expression.kind {
        ResolvedExprKind::Place(place) => values.contains(&place.root),
        ResolvedExprKind::Call { args, .. } => args
            .iter()
            .any(|argument| references_any_value(argument, values)),
        ResolvedExprKind::Unary { value, .. } => references_any_value(value, values),
        ResolvedExprKind::Binary { left, right, .. } => {
            references_any_value(left, values) || references_any_value(right, values)
        }
        ResolvedExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| {
                (0..statement.child_count()).any(|index| {
                    statement
                        .child(index)
                        .is_some_and(|child| references_any_value(child, values))
                })
            }) || references_any_value(tail, values)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            references_any_value(condition, values)
                || references_any_value(then_branch, values)
                || references_any_value(else_branch, values)
        }
        _ => false,
    }
}

fn is_owner_or_top_level_update(
    template: &ResolvedFunctionTemplate,
    owner: &ValueId,
    candidate: &ValueId,
) -> bool {
    if candidate == owner {
        return true;
    }
    let ResolvedExprKind::Block { statements, .. } = &template.body.kind else {
        return false;
    };
    statements.iter().any(|statement| {
        matches!(statement,
            ResolvedStatement::Let { binding, value, .. }
                if &binding.id == candidate
                    && matches!(&value.kind, ResolvedExprKind::UpdateRecord { .. }))
    })
}

pub(super) fn validate_template_place(
    program: &ResolvedProgram,
    template: &ResolvedFunctionTemplate,
    expression: &ResolvedExpr,
    place: &Place,
    values: &BTreeMap<ValueId, ResolvedType>,
) -> Result<(), Diagnostic> {
    if place.projections.is_empty() {
        return Ok(());
    }
    let (owner, relay, record, arguments) = exact_flat_relay(program, template)?;
    let [PlaceProjection::Field(field)] = place.projections.as_slice() else {
        return Err(hir_error(
            "generic owned-record projection must be one direct field",
        ));
    };
    if place.root != owner || values.get(&place.root) != Some(&relay) {
        return Err(hir_error(
            "generic owned-record projection must start at the exact relay type",
        ));
    }
    let fields = relay_fields(program, &record, &arguments)?;
    let Some((_, field_ty)) = fields.iter().find(|(candidate, _)| candidate == field) else {
        return Err(hir_error(
            "generic owned-record projection field is unknown",
        ));
    };
    if *field_ty == ResolvedType::Bytes
        || expression.ty != *field_ty
        || expression.ownership != OwnershipMode::Value
    {
        return Err(hir_error(
            "generic owned-record projection must select one direct Copy field",
        ));
    }
    Ok(())
}

impl HirValidator<'_> {
    pub(super) fn validate_template_owned_record_expression(
        &mut self,
        template: &ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &ResolvedExpr,
        values: &mut BTreeMap<ValueId, ResolvedType>,
        path: &str,
        allow_record_reconstruction: bool,
    ) -> Result<(), Diagnostic> {
        let (owner, relay, relay_record, relay_arguments) =
            exact_flat_relay(self.program, template)?;
        let declared_fields = relay_fields(self.program, &relay_record, &relay_arguments)?;
        match &expression.kind {
            ResolvedExprKind::ConstructRecord { record, fields } => {
                if !allow_record_reconstruction
                    || *record != relay_record
                    || expression.ty != relay
                    || expression.ownership != OwnershipMode::Own
                    || fields.len() != declared_fields.len()
                {
                    return Err(hir_error(
                        "generic record constructor must be an exact owned-match reconstruction",
                    ));
                }
                for (index, field) in fields.iter().enumerate() {
                    let (declared, field_ty) = &declared_fields[index];
                    if field.field != *declared
                        || field.value.ty != *field_ty
                        || field.value.ownership != field_ownership(field_ty)
                        || (*field_ty != ResolvedType::Bytes
                            && !is_frozen_direct_scalar(&field.value))
                    {
                        return Err(hir_error(
                            "generic record reconstruction fields are not canonical",
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
                fields,
            } => {
                let ResolvedExprKind::Place(base_place) = &base.kind else {
                    return Err(hir_error(
                        "generic record update base must be the direct owner parameter",
                    ));
                };
                if *record != relay_record
                    || expression.ty != relay
                    || expression.ownership != OwnershipMode::Own
                    || base.ty != relay
                    || base.ownership != OwnershipMode::Own
                    || base_place.root != owner
                    || !base_place.projections.is_empty()
                    || fields.is_empty()
                {
                    return Err(hir_error(
                        "generic record update must preserve the exact owned relay",
                    ));
                }
                self.validate_template_expr(
                    template,
                    execution,
                    base,
                    values,
                    &format!("{path}.base"),
                )?;
                let mut previous = None;
                for (index, field) in fields.iter().enumerate() {
                    let Some((position, (_, field_ty))) = declared_fields
                        .iter()
                        .enumerate()
                        .find(|(_, (candidate, _))| candidate == &field.field)
                    else {
                        return Err(hir_error("generic record update field is unknown"));
                    };
                    if previous.is_some_and(|prior| prior >= position)
                        || *field_ty == ResolvedType::Bytes
                        || field.value.ty != *field_ty
                        || field.value.ownership != OwnershipMode::Value
                        || !is_frozen_direct_scalar(&field.value)
                    {
                        return Err(hir_error(
                            "generic record update must replace ordered Copy fields",
                        ));
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
                let ResolvedExprKind::Place(base_place) = &base.kind else {
                    return Err(hir_error(
                        "generic record projection base must be the direct owner parameter",
                    ));
                };
                let Some((_, field_ty)) = declared_fields
                    .iter()
                    .find(|(candidate, _)| candidate == field)
                else {
                    return Err(hir_error("generic record projection field is unknown"));
                };
                if base.ty != relay
                    || base.ownership != OwnershipMode::Own
                    || base_place.root != owner
                    || !base_place.projections.is_empty()
                    || *field_ty == ResolvedType::Bytes
                    || expression.ty != *field_ty
                    || expression.ownership != OwnershipMode::Value
                {
                    return Err(hir_error(
                        "generic record projection must select one direct Copy field",
                    ));
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
                let ResolvedExprKind::Place(scrutinee_place) = &scrutinee.kind else {
                    return Err(hir_error(
                        "generic owned-record match scrutinee must be a direct relay place",
                    ));
                };
                if arms.len() != 1
                    || arms[0].guard.is_some()
                    || scrutinee.ty != relay
                    || !scrutinee_place.projections.is_empty()
                    || (*mode == ResolvedMatchMode::Borrow && scrutinee_place.root != owner)
                    || (*mode == ResolvedMatchMode::Own
                        && !is_owner_or_top_level_update(template, &owner, &scrutinee_place.root))
                {
                    return Err(hir_error(
                        "generic owned-record match must be one unguarded exact-relay arm",
                    ));
                }
                self.validate_template_expr(
                    template,
                    execution,
                    scrutinee,
                    values,
                    &format!("{path}.scrutinee"),
                )?;
                for (index, arm) in arms.iter().enumerate() {
                    let mut arm_values = values.clone();
                    let ResolvedMatchPattern::Record {
                        record,
                        instance,
                        fields,
                    } = &arm.pattern
                    else {
                        return Err(hir_error(
                            "generic template match pattern is outside the owned-record slice",
                        ));
                    };
                    if *record != relay_record
                        || *instance != relay
                        || fields.len() != declared_fields.len()
                    {
                        return Err(hir_error(
                            "generic template match pattern does not authenticate the relay record",
                        ));
                    }
                    let mut borrow_binding = None;
                    let mut owned_byte_bindings = BTreeMap::new();
                    for (field_index, field) in fields.iter().enumerate() {
                        let (declared, field_ty) = &declared_fields[field_index];
                        if field.field != *declared {
                            return Err(hir_error(
                                "generic template match fields are incomplete or out of order",
                            ));
                        }
                        if let ResolvedRecordMatchFieldPattern::Binding(binding) = &field.pattern {
                            let ownership = match (mode, field_ty) {
                                (ResolvedMatchMode::Own, ResolvedType::Bytes) => OwnershipMode::Own,
                                (ResolvedMatchMode::Borrow, ResolvedType::Bytes) => {
                                    OwnershipMode::Borrow
                                }
                                _ => OwnershipMode::Value,
                            };
                            if binding.id
                                != ValueId::local(
                                    execution,
                                    &format!(
                                        "{path}.arm.{index}.record.field.{field_index}.binding"
                                    ),
                                )
                                || binding.ty != *field_ty
                                || binding.ownership != ownership
                            {
                                return Err(hir_error(
                                    "generic template record binding is not canonical",
                                ));
                            }
                            self.insert_value(&binding.id)?;
                            arm_values.insert(binding.id.clone(), binding.ty.clone());
                            if *mode == ResolvedMatchMode::Borrow {
                                if *field_ty == ResolvedType::Bytes || borrow_binding.is_some() {
                                    return Err(hir_error(
                                        "generic borrow match must bind exactly one Copy field",
                                    ));
                                }
                                borrow_binding = Some(binding.id.clone());
                            } else if *mode == ResolvedMatchMode::Own
                                && *field_ty == ResolvedType::Bytes
                            {
                                owned_byte_bindings.insert(field.field.clone(), binding.id.clone());
                            }
                        } else if !matches!(
                            &field.pattern,
                            ResolvedRecordMatchFieldPattern::Wildcard
                        ) {
                            return Err(hir_error(
                                "generic owned-record match cannot contain nested patterns",
                            ));
                        }
                    }
                    match mode {
                        ResolvedMatchMode::Borrow => {
                            if !matches!(
                                scrutinee.ownership,
                                OwnershipMode::Own | OwnershipMode::Borrow
                            ) || expression.ty == ResolvedType::Bytes
                            {
                                return Err(hir_error(
                                    "generic borrow match must borrow the relay and return Copy data",
                                ));
                            }
                            let Some(binding) = borrow_binding else {
                                return Err(hir_error(
                                    "generic borrow match must return one bound Copy field",
                                ));
                            };
                            let ResolvedExprKind::Place(place) = &arm.value.kind else {
                                return Err(hir_error(
                                    "generic borrow match result must be the bound Copy field",
                                ));
                            };
                            if place.root != binding
                                || !place.projections.is_empty()
                                || arm.value.ty != expression.ty
                                || arm.value.ownership != OwnershipMode::Value
                                || expression.ownership != OwnershipMode::Value
                            {
                                return Err(hir_error(
                                    "generic borrow match result must be the bound Copy field",
                                ));
                            }
                        }
                        ResolvedMatchMode::Own => {
                            if scrutinee.ownership != OwnershipMode::Own
                                || expression.ty != relay
                                || expression.ownership != OwnershipMode::Own
                                || arm.value.ty != relay
                                || arm.value.ownership != OwnershipMode::Own
                            {
                                return Err(hir_error(
                                    "generic owned match must reconstruct the exact relay",
                                ));
                            }
                            let ResolvedExprKind::ConstructRecord { fields, .. } = &arm.value.kind
                            else {
                                return Err(hir_error(
                                    "generic owned match arm must directly reconstruct the relay",
                                ));
                            };
                            let forbidden = owned_byte_bindings
                                .values()
                                .cloned()
                                .collect::<BTreeSet<_>>();
                            for (field, (_, field_ty)) in fields.iter().zip(&declared_fields) {
                                if *field_ty != ResolvedType::Bytes
                                    && references_any_value(&field.value, &forbidden)
                                {
                                    return Err(hir_error(
                                        "generic Copy reconstruction cannot read an owned field binding",
                                    ));
                                }
                            }
                            for field in fields {
                                if let Some(binding) = owned_byte_bindings.get(&field.field) {
                                    let ResolvedExprKind::Place(place) = &field.value.kind else {
                                        return Err(hir_error(
                                            "generic owned match must transfer each byte field",
                                        ));
                                    };
                                    if &place.root != binding || !place.projections.is_empty() {
                                        return Err(hir_error(
                                            "generic owned match must transfer each byte field",
                                        ));
                                    }
                                }
                            }
                            if owned_byte_bindings.len()
                                != declared_fields
                                    .iter()
                                    .filter(|(_, ty)| *ty == ResolvedType::Bytes)
                                    .count()
                            {
                                return Err(hir_error(
                                    "generic owned match must bind every byte field",
                                ));
                            }
                        }
                        ResolvedMatchMode::Value => {
                            return Err(hir_error(
                                "generic owned-record value matches are outside the bounded slice",
                            ));
                        }
                    }
                    self.validate_template_expr_with_context(
                        template,
                        execution,
                        &arm.value,
                        &mut arm_values,
                        &format!("{path}.arm.{index}.value"),
                        (*mode == ResolvedMatchMode::Own, false),
                    )?;
                }
            }
            _ => {
                return Err(hir_error(
                    "generic owned-record expression kind is inconsistent",
                ));
            }
        }
        Ok(())
    }
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
    let admitted = (super::super::generic_result::profile(template)
        && (super::super::generic_result::slot(ty, &template.id, template.type_parameters.len())
            || *ty == ResolvedType::Bytes))
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
        && crate::vec_ops::hir_wrapper_in_program(program, template)
            == crate::vec_ops::by_id(callee.as_str())
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
    if super::super::generic_result::profile(template) {
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
