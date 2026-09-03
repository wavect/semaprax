//! Source-scoped computed signature defaults; no independent type admission.

use super::*;

pub(super) fn requested_type(
    revision: Option<&ProjectRevision>,
    program: &Program,
    request: &Value,
) -> Result<Type> {
    if let Some(name) = request.as_str() {
        return scalar_type(name);
    }
    object(request, &["kind", "target", "type_arguments"])?;
    if text(request, "kind")? != "nominal" {
        return Err(grammar(
            "computed signature type object requires nominal kind",
        ));
    }
    let revision = revision.ok_or_else(|| {
        grammar("nominal signature arguments require a retained checked Project revision")
    })?;
    super::super::nominal_type_plan(
        revision,
        program,
        text(request, "target")?,
        member(request, "type_arguments")?,
    )
}

pub(super) fn nominal_type_nodes(ty: &Type) -> usize {
    match ty {
        Type::Named { arguments, .. } => 1 + arguments.len(),
        _ => 0,
    }
}

/// The new provider signature must prove Copy even if no caller materialized
/// the default. Exact nominal IDs and arguments are checked independently of
/// provider/caller display aliases after the full canonical source rebuild.
pub(in crate::project::candidate) fn validate_computed_signature(
    revision: &ProjectRevision,
    intent: &Value,
) -> Result<()> {
    let Some(parameters) = intent.get("parameters").and_then(Value::as_array) else {
        return Ok(());
    };
    if parameters.len() > MAX_PARAMETERS {
        return Err(capacity(
            "rebuilt computed signature exceeds its parameter bound",
        ));
    }
    let target = text(intent, "target")?;
    let mut selected = None;
    for module in revision.semantic.image_modules() {
        for function in module
            .functions()
            .iter()
            .filter(|function| function.id.as_str() == target)
        {
            if selected.replace((module, function)).is_some() {
                return Err(grammar("rebuilt computed signature identity is ambiguous"));
            }
        }
    }
    let (module, function) =
        selected.ok_or_else(|| grammar("rebuilt computed signature function is absent"))?;
    if function.params.len() != parameters.len() {
        return Err(grammar(
            "rebuilt computed signature parameter inventory disagrees",
        ));
    }
    for (mapping, parameter) in parameters.iter().zip(&function.params) {
        if mapping.get("borrow_slice_from_owner").is_some()
            || mapping.get("borrow_str_from_owner").is_some()
        {
            let (field, expected) = if mapping.get("borrow_slice_from_owner").is_some() {
                ("borrow_slice_from_owner", ResolvedType::SliceU8)
            } else {
                ("borrow_str_from_owner", ResolvedType::Str)
            };
            object(mapping, &["name", field])?;
            if parameter.name != text(mapping, "name")?
                || parameter.ownership != OwnershipMode::Borrow
                || parameter.ty != expected
            {
                return Err(super::owner_view::invalid(
                    "rebuilt owner-to-view parameter disagrees with its exact borrowed view",
                ));
            }
            if parameters.iter().any(|other| {
                other.get("from").and_then(Value::as_str)
                    == mapping.get(field).and_then(Value::as_str)
            }) {
                return Err(super::owner_view::invalid(
                    "rebuilt owner-to-view source was also retained",
                ));
            }
            continue;
        }
        if mapping.get("borrow_from").is_some() {
            object(mapping, &["name", "borrow_from"])?;
            if parameter.name != text(mapping, "name")?
                || parameter.ownership != OwnershipMode::Borrow
                || !matches!(parameter.ty, ResolvedType::Str | ResolvedType::SliceU8)
            {
                return Err(grammar(
                    "rebuilt borrowed parameter disagrees with its exact view request",
                ));
            }
            let source_name = text(mapping, "borrow_from")?;
            let mut source = None;
            for (source_mapping, source_parameter) in parameters.iter().zip(&function.params) {
                if source_mapping.get("from").and_then(Value::as_str) == Some(source_name)
                    && source.replace(source_parameter).is_some()
                {
                    return Err(grammar(
                        "rebuilt borrowed parameter source is retained ambiguously",
                    ));
                }
            }
            let source = source.ok_or_else(|| {
                grammar("rebuilt borrowed parameter source was not retained exactly once")
            })?;
            if source.ownership != OwnershipMode::Borrow
                || source.ty != parameter.ty
                || !matches!(source.ty, ResolvedType::Str | ResolvedType::SliceU8)
            {
                return Err(grammar(
                    "rebuilt borrowed parameter does not preserve its authenticated source view",
                ));
            }
            continue;
        }
        if mapping.get("argument_expression").is_none()
            || !mapping.get("type").is_some_and(Value::is_object)
        {
            continue;
        }
        let request = member(mapping, "type")?;
        object(request, &["kind", "target", "type_arguments"])?;
        if text(request, "kind")? != "nominal"
            || parameter.name != text(mapping, "name")?
            || parameter.ownership != OwnershipMode::Value
        {
            return Err(grammar(
                "rebuilt nominal parameter binding disagrees with its request",
            ));
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &parameter.ty
        else {
            return Err(grammar(
                "rebuilt nominal parameter is not a checked nominal type",
            ));
        };
        let requested_arguments = array(request, "type_arguments")?;
        if requested_arguments.len() > super::super::MAX_AGGREGATE_TYPE_ARGUMENTS {
            return Err(capacity(
                "rebuilt nominal type arguments exceed their bound",
            ));
        }
        if declaration.as_str() != text(request, "target")?
            || arguments.len() != requested_arguments.len()
            || !arguments
                .iter()
                .zip(requested_arguments)
                .all(|(actual, requested)| {
                    matches!(
                        (actual, requested.as_str()),
                        (ResolvedType::I64, Some("i64")) | (ResolvedType::Bool, Some("bool"))
                    )
                })
        {
            return Err(grammar(
                "rebuilt nominal parameter has a different stable type identity",
            ));
        }
        let (kind, facts) = module.signature_type_facts(&parameter.ty).ok_or_else(|| {
            grammar("rebuilt nominal parameter has no retained checked type facts")
        })?;
        if !matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
            || !facts.copy
            || !facts.sized
            || facts.needs_drop
            || facts.contains_resource
        {
            return Err(grammar("computed nominal parameters require checked sized Copy records or variants without owned cleanup or resources"));
        }
    }
    Ok(())
}

pub(super) fn charge(nodes: &mut usize, additional: usize) -> Result<()> {
    *nodes = nodes
        .checked_add(additional)
        .ok_or_else(|| capacity("computed signature argument inventory overflow"))?;
    if *nodes > MAX_WALK_NODES {
        return Err(capacity(
            "computed signature argument inventory exceeds its bound",
        ));
    }
    Ok(())
}

fn carrier(body: Expr, original: &[Param]) -> Function {
    Function {
        stable_id: String::new(),
        explicit_id: false,
        name: String::new(),
        name_span: Span::default(),
        type_parameters: Vec::new(),
        params: original.to_vec(),
        return_type: Type::I64,
        effects: Vec::new(),
        requires: Vec::new(),
        ensures: Vec::new(),
        body,
        span: Span::default(),
    }
}

/// Construct against actual module bindings and reserve every local in the
/// lowered template before selecting argument staging names. The carrier is
/// solely input to the existing bounded AST visitors, never source admission.
pub(super) fn prepare(
    revision: Option<&ProjectRevision>,
    program: &Program,
    original: &[Param],
    nominal_scope: &super::super::NominalScope,
    template: &Value,
    occupied: &mut BTreeSet<String>,
    total_nodes: &mut usize,
) -> Result<(Expr, usize)> {
    let revision = revision.ok_or_else(|| {
        grammar("computed signature arguments require a retained checked Project revision")
    })?;
    let scope = original.iter().map(|param| param.name.clone()).collect();
    charge(total_nodes, nominal_scope.len())?;
    let body = super::super::construct_expression_with_scope(
        revision,
        program,
        &scope,
        nominal_scope.clone(),
        template,
    )?;
    let mut function = carrier(body, original);
    let mut expressions = 0usize;
    let mut bindings = 0usize;
    let mut patterns = 0usize;
    super::super::walk_function(&mut function, &mut expressions, &mut |expression| {
        match &expression.kind {
            ExprKind::Var(name) | ExprKind::Call { name, .. } => {
                occupied.insert(name.clone());
            }
            ExprKind::Block { statements, .. } => {
                charge(&mut bindings, statements.len())?;
                for statement in statements {
                    if let Statement::Let { name, .. } | Statement::Assign { name, .. } = statement
                    {
                        occupied.insert(name.clone());
                    }
                }
            }
            ExprKind::Match { arms, .. } => {
                for arm in arms {
                    reserve_pattern(&arm.pattern, occupied, 0, &mut patterns)?;
                    if let MatchPattern::Variant { fields, .. } = &arm.pattern {
                        charge(&mut bindings, fields.len())?;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    })?;
    let count = expressions
        .checked_add(bindings)
        .and_then(|count| count.checked_add(patterns))
        .ok_or_else(|| capacity("computed signature template inventory overflow"))?;
    charge(total_nodes, count)?;
    Ok((function.body, count))
}

pub(super) fn substitute(
    body: Expr,
    original: &[Param],
    stages: &[String],
    occupied: &mut BTreeSet<String>,
) -> Result<Expr> {
    let renames = original
        .iter()
        .zip(stages)
        .map(|(param, stage)| (param.name.clone(), stage.clone()))
        .collect();
    let destinations = stages.iter().cloned().collect();
    let mut function = carrier(body, original);
    rename::apply(&mut function, original, &renames, &destinations, occupied)?;
    Ok(function.body)
}
