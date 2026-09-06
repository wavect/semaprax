//! Validation-only rules for bounded generic templates.

use super::*;

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
    let admitted = matches!(
        ty,
        ResolvedType::I64 | ResolvedType::Bool | ResolvedType::String
    ) || matches!(ty, ResolvedType::TypeParameter { owner, index }
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
                && template.type_parameters.len() == type_arguments.len()
                && type_arguments.iter().enumerate().all(|(index, argument)| {
                    matches!(argument, ResolvedType::TypeParameter { owner, index: actual }
                        if owner == &template.id && usize::try_from(*actual).ok() == Some(index))
                })
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
