//! Internal ordinary calls over explicitly imported resource-free byte records.
//! Public Project descriptors and scalar linker signatures remain independent.
use super::*;

pub(super) fn admitted(
    caller: &Program,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    let Some(function) = target.function else {
        return false;
    };
    if !function.type_parameters.is_empty() {
        return false;
    }
    let record = |ty: &Type| record_slot(target.module, ty, caller, authored, programs);
    let uses_record = record(&function.return_type)
        || function
            .params
            .iter()
            .any(|parameter| record(&parameter.ty));
    let copy_result = function.params.iter().any(|parameter| {
        parameter.mode == ParamMode::Own && (parameter.ty == Type::Bytes || record(&parameter.ty))
    }) && record_kind(
        target.module,
        &function.return_type,
        caller,
        authored,
        programs,
    ) == Some(false);
    uses_record
        && (copy_result
            || scalar(&function.return_type)
            || function.return_type == Type::Bytes
            || record(&function.return_type))
        && function.params.iter().all(|parameter| {
            (parameter.mode == ParamMode::Value && scalar(&parameter.ty))
                || (parameter.mode == ParamMode::Own && parameter.ty == Type::Bytes)
                || (parameter.mode == ParamMode::Borrow && parameter.ty == Type::Str)
                || (matches!(parameter.mode, ParamMode::Own | ParamMode::Borrow)
                    && record(&parameter.ty))
        })
}
fn scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::Char
            | Type::F32
            | Type::F64
            | Type::Bool
    )
}
fn record_slot(
    module: &str,
    ty: &Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> bool {
    record_kind(module, ty, caller, authored, programs) == Some(true)
}
fn record_kind(
    module: &str,
    ty: &Type,
    caller: &Program,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Option<bool> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let id = resolve_type_id(module, name, programs)?;
    let target = authored.get(id.as_str())?;
    let declaration = target.ty?;
    let owns_bytes = record_shape(
        target.module,
        declaration,
        authored,
        programs,
        &mut BTreeSet::new(),
        &mut BTreeMap::new(),
    )?;
    signature_type_is_admitted(module, ty, caller, authored, programs, &mut BTreeSet::new())
        .then_some(owns_bytes)
}
/// Re-derive the complete explicit record closure. No resources, views, generic
/// substitutions, variants, classes, or callable storage enter this lane.
fn record_shape(
    module: &str,
    declaration: &TypeDeclaration,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
    visiting: &mut BTreeSet<String>,
    memo: &mut BTreeMap<String, bool>,
) -> Option<bool> {
    if let Some(result) = memo.get(&declaration.stable_id) {
        return Some(*result);
    }
    if visiting.len() >= MAX_CHECKED_VALUE_DEPTH {
        return None;
    }
    if !declaration.explicit_id
        || !declaration.type_parameters.is_empty()
        || !visiting.insert(declaration.stable_id.clone())
    {
        return None;
    }
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    let mut owns_bytes = false;
    for field in fields {
        if field.ty == Type::Bytes {
            owns_bytes = true;
        } else if !scalar(&field.ty) {
            let Type::Named { name, arguments } = &field.ty else {
                return None;
            };
            if !arguments.is_empty() {
                return None;
            }
            let id = resolve_type_id(module, name, programs)?;
            let target = authored.get(id.as_str())?;
            owns_bytes |= record_shape(
                target.module,
                target.ty?,
                authored,
                programs,
                visiting,
                memo,
            )?;
        }
    }
    visiting.remove(&declaration.stable_id);
    memo.insert(declaration.stable_id.clone(), owns_bytes);
    Some(owns_bytes)
}

pub(super) fn validate_imported_function(
    caller: &Program,
    module_use: &ModuleUse,
    target: &AuthoredDeclaration<'_>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    programs: &[Program],
) -> Result<(), Vec<Diagnostic>> {
    let function = target.function.expect("function target carries a function");
    let transparent_vec_wrapper = programs
        .iter()
        .find(|program| program.module == target.module)
        .is_some_and(|program| crate::vec_ops::source_wrapper(program, function).is_some());
    let transparent_box_wrapper = programs
        .iter()
        .find(|program| program.module == target.module)
        .is_some_and(|program| crate::box_ops::source_wrapper(program, function).is_some());
    if transparent_vec_wrapper
        || transparent_box_wrapper
        || admitted(caller, target, authored, programs)
    {
        return Ok(());
    }
    let byte_parameter = package::admitted_byte_parameter;
    let has_byte_parameter = function.params.iter().any(byte_parameter);
    let scalar_return = matches!(
        function.return_type,
        Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Bool
    );
    if !function.type_parameters.is_empty()
        || function.params.iter().any(|param| {
            param.mode != ParamMode::Value
                && !byte_parameter(param)
                && !(param.mode == ParamMode::Borrow && param.ty == Type::Str)
        })
        || (has_byte_parameter && !scalar_return)
    {
        return Err(vec![use_error(
            caller,
            module_use,
            package::import_profile_refusal(),
        )]);
    }
    for param in &function.params {
        if byte_parameter(param) {
            continue;
        }
        if !signature_type_is_admitted(
            target.module,
            &param.ty,
            caller,
            authored,
            programs,
            &mut BTreeSet::new(),
        ) {
            return Err(vec![use_error(
                caller,
                module_use,
                "function signature leaves the admitted scalar/Copy workspace domain",
            )
            .with_help(PROJECT_SIGNATURE_HELP)]);
        }
    }
    let ty = &function.return_type;
    {
        if !signature_type_is_admitted(
            target.module,
            ty,
            caller,
            authored,
            programs,
            &mut BTreeSet::new(),
        ) {
            return Err(vec![use_error(
                caller,
                module_use,
                "function signature leaves the admitted scalar/Copy workspace domain",
            )
            .with_help(PROJECT_SIGNATURE_HELP)]);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
