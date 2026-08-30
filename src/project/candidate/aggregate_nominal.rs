//! Stable nominal signature selectors; rebuilt checked HIR owns Copy admission.
use super::*;

pub(in crate::project::candidate) fn nominal_type_dependency_fingerprint(
    revision: &ProjectRevision,
    target: &str,
) -> Result<Option<Value>> {
    selector(target)?;
    if let Some(subject) = subject(revision, target)? {
        if subject.kind == "record" {
            return descriptor(&subject, None).map(Some);
        }
    }
    aggregate_match_dependency_fingerprint(revision, target)
}

pub(in crate::project::candidate) fn nominal_type_plan(
    revision: &ProjectRevision,
    program: &Program,
    target: &str,
    arguments: &Value,
) -> Result<Type> {
    let shape = nominal_type_dependency_fingerprint(revision, target)?
        .ok_or_else(|| grammar("nominal type requires an explicit record or variant owner, or the checked compiler prelude"))?;
    let (kind, selected) = if shape["kind"] == "record" {
        ("record", target)
    } else {
        (
            "variant",
            shape["cases"][0]["target"]
                .as_str()
                .ok_or_else(|| grammar("nominal variant has no authenticated case"))?,
        )
    };
    let plan = plan(revision, program, kind, selected, Some(arguments))?;
    Ok(Type::Named {
        name: plan.type_name,
        arguments: plan.type_arguments,
    })
}

/// Shared append/extraction callers cannot bypass nominal source identity
/// admission by handing the declaration helper an arbitrary named AST type.
pub(in crate::project::candidate) fn validate_nominal_ast(
    revision: &ProjectRevision,
    program: &Program,
    ty: &Type,
) -> Result<()> {
    let Type::Named { name, arguments } = ty else {
        return Ok(());
    };
    if arguments.len() > MAX_AGGREGATE_TYPE_ARGUMENTS {
        return Err(capacity(
            "nominal type argument inventory exceeds its bound",
        ));
    }
    let mut targets = BTreeSet::new();
    for declaration in &program.types {
        if declaration.name == *name {
            targets.insert(declaration.stable_id.as_str());
        }
    }
    for imported in &program.module_uses {
        if imported.kind == ModuleUseKind::Type && imported.alias == *name {
            targets.insert(imported.persistent_id.as_str());
        }
    }
    match name.as_str() {
        "Option" => {
            targets.insert(crate::prelude::OPTION_ID);
        }
        "Result" => {
            targets.insert(crate::prelude::RESULT_ID);
        }
        _ => {}
    }
    if targets.len() != 1 {
        return Err(grammar(
            "nominal AST type has no unique existing stable binding",
        ));
    }
    let arguments = arguments
        .iter()
        .map(|argument| match argument {
            Type::I64 => Ok(json!("i64")),
            Type::Bool => Ok(json!("bool")),
            _ => Err(grammar(
                "nominal type arguments admit only direct i64 or bool",
            )),
        })
        .collect::<Result<Vec<_>>>()?;
    let planned = nominal_type_plan(
        revision,
        program,
        targets.into_iter().next().expect("one binding checked"),
        &json!(arguments),
    )?;
    if &planned != ty {
        return Err(grammar(
            "nominal AST type disagrees with its authenticated binding",
        ));
    }
    Ok(())
}

pub(in crate::project::candidate) fn nominal_types(
    revision: &ProjectRevision,
    program: &Program,
) -> Result<Vec<Value>> {
    let mut targets = BTreeSet::new();
    for module in revision.semantic.image_modules() {
        for ty in module.types() {
            if matches!(
                &ty.kind,
                ResolvedTypeDeclarationKind::Record { .. }
                    | ResolvedTypeDeclarationKind::Variant { .. }
            ) && binding(program, ty.id.as_str(), module.module())?.is_some()
            {
                targets.insert(ty.id.as_str().to_owned());
                if targets.len() > MAX_ITEMS {
                    return Err(capacity("nominal type catalogue exceeds its item bound"));
                }
            }
        }
    }
    targets.extend([
        crate::prelude::OPTION_ID.to_owned(),
        crate::prelude::RESULT_ID.to_owned(),
    ]);
    let mut result = Vec::new();
    let mut bytes = 2usize;
    let mut items = 0usize;
    for target in targets {
        let Some(shape) = nominal_type_dependency_fingerprint(revision, &target)? else {
            continue;
        };
        let type_binding = if shape["identity_origin"] == "compiler_owned" {
            let selected = shape["cases"][0]["target"]
                .as_str()
                .ok_or_else(|| grammar("nominal prelude case is absent"))?;
            let subject = prelude_subject(revision, selected)?
                .ok_or_else(|| grammar("nominal prelude owner is absent"))?;
            visible_binding(program, &subject)?
        } else {
            binding(
                program,
                &target,
                shape["module"]
                    .as_str()
                    .ok_or_else(|| grammar("nominal source module is absent"))?,
            )?
        };
        let Some(type_binding) = type_binding else {
            continue;
        };
        let parameters = shape.get("type_parameters");
        items = items.saturating_add(1 + parameters.and_then(Value::as_array).map_or(0, Vec::len));
        if items > MAX_ITEMS {
            return Err(capacity(
                "nominal type catalogue exceeds its parameter bound",
            ));
        }
        let mut value = json!({"kind":"nominal","target":target,"binding":type_binding,
            "generic":shape["generic"],"declaration_kind":if shape["kind"]=="record" {"record"} else {"variant"},
            "path":shape["path"],"module":shape["module"],"evidence_owner":shape["evidence_owner"],
            "requires_full_candidate_validation":true,"copy_admission":"checked_candidate_signature"});
        if let Some(parameters) = parameters {
            value["type_parameters"] = parameters.clone();
        }
        if shape["identity_origin"] == "compiler_owned" {
            value["identity_origin"] = shape["identity_origin"].clone();
            value["compiler_prelude"] = shape["compiler_prelude"].clone();
        }
        let encoded = super::super::super::wire::render(value.clone(), MAX_CATALOG_BYTES)?;
        bytes = bytes.saturating_add(encoded.len());
        if bytes > MAX_CATALOG_BYTES {
            return Err(capacity("nominal type catalogue exceeds its byte bound"));
        }
        result.push(value);
    }
    Ok(result)
}
