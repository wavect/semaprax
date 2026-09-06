//! Symbolic generic forwarding, authenticated by structural HIR expression paths.
use super::*;
use serde_json::{json, Value};

fn children(expression: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    let mut children = Vec::new();
    hir::push_resolved_expression_children_in_authored_order(expression, &mut children);
    children.reverse();
    children
}

fn roots(template: &hir::ResolvedFunctionTemplate) -> Vec<(String, &ResolvedExpr)> {
    template
        .requires
        .iter()
        .enumerate()
        .map(|(i, e)| (format!("requires/{i}"), e))
        .chain(std::iter::once(("body".to_owned(), &template.body)))
        .chain(
            template
                .ensures
                .iter()
                .enumerate()
                .map(|(i, e)| (format!("ensures/{i}"), e)),
        )
        .collect()
}

pub(crate) fn requires_v35(templates: &[hir::ResolvedFunctionTemplate]) -> bool {
    templates.iter().any(|template| {
        let mut pending = template.requires.iter().chain(std::iter::once(&template.body)).chain(&template.ensures).collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            if let ResolvedExprKind::Call { callee, type_arguments, .. } = &expression.kind {
                if templates.iter().any(|target| target.id == *callee)
                    && (type_arguments.len() != template.type_parameters.len()
                        || type_arguments.iter().enumerate().any(|(index,ty)| !matches!(ty,
                            ResolvedType::TypeParameter { owner, index: parameter } if owner == &template.id && *parameter as usize == index))) {
                    return true;
                }
            }
            pending.extend(children(expression));
        }
        false
    })
}

fn mapping(
    template: &hir::ResolvedFunctionTemplate,
    callee: &DeclarationId,
    arguments: &[ResolvedType],
) -> Result<Vec<Value>, Diagnostic> {
    arguments
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            let source = match ty {
                ResolvedType::TypeParameter { owner, index }
                    if owner == &template.id
                        && (*index as usize) < template.type_parameters.len() =>
                {
                    json!({"kind":"caller_parameter","owner":owner.as_str(),"index":index})
                }
                ResolvedType::TypeParameter { .. } => {
                    return Err(error(
                        "forwarding references a foreign or missing caller parameter",
                    ))
                }
                ty => json!({"kind":"concrete_type","type_identity":ty.identity_key()}),
            };
            Ok(json!({"callee_owner":callee.as_str(),"callee_index":index,"source":source}))
        })
        .collect()
}

pub(super) fn template_facts(
    program: &ResolvedProgram,
    selected: &BTreeSet<DeclarationId>,
) -> Result<Vec<Value>, Diagnostic> {
    let mut facts = Vec::new();
    for template in &program.function_templates {
        if !selected.contains(&template.id) {
            continue;
        }
        let mut pending = roots(template);
        while let Some((path, expression)) = pending.pop() {
            if let ResolvedExprKind::Call {
                callee,
                type_arguments,
                ..
            } = &expression.kind
            {
                if program
                    .function_templates
                    .iter()
                    .any(|target| target.id == *callee)
                {
                    facts.push(json!({"template":template.id.as_str(),"structural_path":path,
                        "callee_template":callee.as_str(),"forwarded_argument_mapping":mapping(template,callee,type_arguments)?}));
                }
            }
            for (index, child) in children(expression).into_iter().enumerate().rev() {
                pending.push((format!("{path}/{index}"), child));
            }
        }
    }
    facts.sort_by_key(|fact| {
        (
            fact["template"].as_str().unwrap().to_owned(),
            fact["structural_path"].as_str().unwrap().to_owned(),
        )
    });
    Ok(facts)
}

pub(super) fn instance_mappings(
    program: &ResolvedProgram,
    template: &hir::ResolvedFunctionTemplate,
    instance: &hir::ResolvedFunctionInstance,
) -> Result<BTreeMap<String, Vec<Value>>, Diagnostic> {
    let function = &instance.function;
    let concrete_roots = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    let symbolic_roots = roots(template);
    if symbolic_roots.len() != concrete_roots.len() {
        return Err(error("forwarding root association differs"));
    }
    let mut pending = symbolic_roots
        .into_iter()
        .zip(concrete_roots)
        .map(|((path, s), c)| (path, s, c))
        .collect::<Vec<_>>();
    let mut mappings = BTreeMap::new();
    while let Some((path, symbolic, concrete)) = pending.pop() {
        if std::mem::discriminant(&symbolic.kind) != std::mem::discriminant(&concrete.kind) {
            return Err(error("forwarding expression shape differs"));
        }
        if let ResolvedExprKind::Call {
            callee,
            type_arguments,
            ..
        } = &symbolic.kind
        {
            if program
                .function_templates
                .iter()
                .any(|target| target.id == *callee)
            {
                let ResolvedExprKind::Call {
                    callee: actual,
                    type_arguments: actual_args,
                    instance: Some(_),
                    ..
                } = &concrete.kind
                else {
                    return Err(error("forwarding concrete call is missing"));
                };
                let expected = type_arguments
                    .iter()
                    .map(|ty| match ty {
                        ResolvedType::TypeParameter { owner, index } if owner == &template.id => {
                            instance
                                .type_arguments
                                .get(*index as usize)
                                .cloned()
                                .ok_or_else(|| error("forwarding parameter is missing"))
                        }
                        ty => Ok(ty.clone()),
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if actual != callee || actual_args != &expected {
                    return Err(error("forwarding symbolic substitution differs"));
                }
                if mappings
                    .insert(
                        concrete.id.as_str().to_owned(),
                        mapping(template, callee, type_arguments)?,
                    )
                    .is_some()
                {
                    return Err(error("forwarding structural association is ambiguous"));
                }
            }
        }
        let symbolic_children = children(symbolic);
        let concrete_children = children(concrete);
        if symbolic_children.len() != concrete_children.len() {
            return Err(error("forwarding child association differs"));
        }
        for (index, (s, c)) in symbolic_children
            .into_iter()
            .zip(concrete_children)
            .enumerate()
            .rev()
        {
            pending.push((format!("{path}/{index}"), s, c));
        }
    }
    Ok(mappings)
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-G411", message)
}
