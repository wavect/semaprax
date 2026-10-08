use super::*;

pub(super) fn resolved_owned_utf8_signature_is_admitted(
    function: &ResolvedFunction,
    declarations: &hir::DeclarationIndex,
) -> bool {
    function.effects.is_empty()
        && function.requires.is_empty()
        && function.ensures.is_empty()
        && resolved_owned_utf8_body_is_import_and_generic_free(&function.body)
        && function.params.iter().all(|parameter| {
            resolved_data_parameter_is_admitted(&parameter.ty, parameter.ownership, declarations)
                || (parameter.ty == ResolvedType::String
                    && parameter.ownership == hir::OwnershipMode::Own)
        })
        && (resolved_data_result_is_admitted(&function.return_type, declarations)
            || function.return_type == ResolvedType::String)
}

fn resolved_owned_utf8_body_is_import_and_generic_free(root: &ResolvedExpr) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::NativeRustImportCall(_) | ResolvedExprKind::HostCommandCall(_) => {
                return false;
            }
            ResolvedExprKind::Call {
                type_arguments,
                instance,
                ..
            } if !type_arguments.is_empty() || instance.is_some() => return false,
            _ => pending.extend(child_expressions(expression)),
        }
    }
    true
}

pub(super) fn owned_utf8_api_result_matches(
    ty: &ResolvedType,
    expected: crate::project::PublicApiResultType,
) -> bool {
    match expected {
        crate::project::PublicApiResultType::I64 => ty == &ResolvedType::I64,
        crate::project::PublicApiResultType::Bool => ty == &ResolvedType::Bool,
        crate::project::PublicApiResultType::Usize => ty == &ResolvedType::Usize,
        crate::project::PublicApiResultType::OwnedBytes => ty == &ResolvedType::Bytes,
        crate::project::PublicApiResultType::OptionOwnedBytes => matches!(
            ty,
            ResolvedType::Nominal { declaration, arguments }
                if declaration.as_str() == crate::prelude::OPTION_ID
                    && arguments.as_slice() == [ResolvedType::Bytes]
        ),
        crate::project::PublicApiResultType::ResultOwnedBytesI64 => matches!(
            ty,
            ResolvedType::Nominal { declaration, arguments }
                if declaration.as_str() == crate::prelude::RESULT_ID
                    && arguments.as_slice() == [ResolvedType::Bytes, ResolvedType::I64]
        ),
        crate::project::PublicApiResultType::OwnedUtf8 => ty == &ResolvedType::String,
    }
}

pub(super) fn public_api_argument_matches(
    parameter: &hir::ResolvedParam,
    argument: &PublicApiArgument<'_>,
) -> bool {
    matches!(
        (&parameter.ty, parameter.ownership, argument),
        (
            ResolvedType::I64,
            hir::OwnershipMode::Value,
            PublicApiArgument::I64(_)
        ) | (
            ResolvedType::U8,
            hir::OwnershipMode::Value,
            PublicApiArgument::U8(_)
        ) | (
            ResolvedType::Usize,
            hir::OwnershipMode::Value,
            PublicApiArgument::Usize(_)
        ) | (
            ResolvedType::Bool,
            hir::OwnershipMode::Value,
            PublicApiArgument::Bool(_)
        ) | (
            ResolvedType::Str,
            hir::OwnershipMode::Borrow,
            PublicApiArgument::BorrowStr(_)
        ) | (
            ResolvedType::SliceU8,
            hir::OwnershipMode::Borrow,
            PublicApiArgument::BorrowSliceU8(_)
        )
    )
}

pub(super) fn public_api_parameter_type_matches(
    parameter: &hir::ResolvedParam,
    expected: crate::project::PublicApiParameterType,
) -> bool {
    matches!(
        (&parameter.ty, parameter.ownership, expected),
        (
            ResolvedType::I64,
            hir::OwnershipMode::Value,
            crate::project::PublicApiParameterType::I64
        ) | (
            ResolvedType::Bool,
            hir::OwnershipMode::Value,
            crate::project::PublicApiParameterType::Bool
        ) | (
            ResolvedType::Str,
            hir::OwnershipMode::Borrow,
            crate::project::PublicApiParameterType::BorrowStr
        ) | (
            ResolvedType::SliceU8,
            hir::OwnershipMode::Borrow,
            crate::project::PublicApiParameterType::BorrowSliceU8
        )
    )
}

pub(super) fn validate_public_api_borrowed_input_bound(
    arguments: &[PublicApiArgument<'_>],
    label: &str,
) -> Result<(), Vec<Diagnostic>> {
    let mut borrowed_bytes = 0usize;
    for argument in arguments {
        let length = match argument {
            PublicApiArgument::BorrowStr(value) => value.len(),
            PublicApiArgument::BorrowSliceU8(value) => value.len(),
            PublicApiArgument::I64(_)
            | PublicApiArgument::U8(_)
            | PublicApiArgument::Usize(_)
            | PublicApiArgument::Bool(_) => 0,
        };
        borrowed_bytes = borrowed_bytes.checked_add(length).ok_or_else(|| {
            vec![argument_error(format!(
                "{label} cumulative borrowed input byte count overflowed"
            ))]
        })?;
        if borrowed_bytes > crate::project::MAX_PUBLIC_API_BORROWED_INPUT_BYTES {
            return Err(vec![argument_error(format!(
                "{label} cumulative borrowed input exceeds {} bytes",
                crate::project::MAX_PUBLIC_API_BORROWED_INPUT_BYTES
            ))]);
        }
    }
    Ok(())
}

pub(super) fn validate_flat_owned_record_result_shape(
    program: &hir::ResolvedProgram,
    entry: &ResolvedFunction,
    expected: &crate::project::FlatOwnedRecordExport,
) -> Result<(), Vec<Diagnostic>> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &entry.return_type
    else {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            "flat owned-record result is not nominal".to_owned(),
        )]);
    };
    if declaration != expected.record_id() || !arguments.is_empty() {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            "flat owned-record result identity or monomorphization disagrees with its descriptor"
                .to_owned(),
        )]);
    }
    let record = program
        .types
        .iter()
        .find(|candidate| &candidate.id == declaration)
        .ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_RESULT_TYPE,
                "flat owned-record result declaration is absent".to_owned(),
            )]
        })?;
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            "flat owned-record result identity is not a record".to_owned(),
        )]);
    };
    if !record.type_parameters.is_empty()
        || fields.len() != expected.fields().len()
        || fields.is_empty()
        || fields.len() > crate::project::MAX_FLAT_RECORD_FIELDS
        || program
            .declarations
            .declaration(&record.id)
            .is_none_or(|fact| fact.identity_origin != hir::IdentityOrigin::Explicit)
    {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            "flat owned-record result declaration disagrees with its closed descriptor".to_owned(),
        )]);
    }
    let mut owned_bytes = 0usize;
    for (index, (field, expected_field)) in fields.iter().zip(expected.fields()).enumerate() {
        let expected_type = match expected_field.ty() {
            crate::project::FlatOwnedRecordFieldType::I64 => ResolvedType::I64,
            crate::project::FlatOwnedRecordFieldType::Bool => ResolvedType::Bool,
            crate::project::FlatOwnedRecordFieldType::Usize => ResolvedType::Usize,
            crate::project::FlatOwnedRecordFieldType::OwnedBytes => {
                owned_bytes += 1;
                ResolvedType::Bytes
            }
        };
        if field.index as usize != index
            || expected_field.ordinal() as usize != index
            || field.id != *expected_field.stable_id()
            || field.name != expected_field.source_name()
            || field.ty != expected_type
            || program
                .declarations
                .declaration(&field.id)
                .is_none_or(|fact| fact.identity_origin != hir::IdentityOrigin::Explicit)
        {
            return Err(vec![selection_error(
                REASON_UNSUPPORTED_RESULT_TYPE,
                format!("flat owned-record field {index} disagrees with its exact descriptor"),
            )]);
        }
    }
    if owned_bytes != 1 {
        return Err(vec![selection_error(
            REASON_UNSUPPORTED_RESULT_TYPE,
            "flat owned-record result requires exactly one direct Bytes field".to_owned(),
        )]);
    }
    Ok(())
}

pub(super) fn public_api_result_is_admitted(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64 | ResolvedType::Bool | ResolvedType::Usize
    ) || owned_data_result_is_admitted(ty)
}

pub(super) fn require_acyclic_public_api_closure(
    entry_id: &str,
    admitted: &BTreeMap<&str, &ResolvedFunction>,
) -> Result<(), Vec<Diagnostic>> {
    fn visit<'a>(
        id: &'a str,
        admitted: &BTreeMap<&'a str, &'a ResolvedFunction>,
        states: &mut BTreeMap<&'a str, u8>,
    ) -> Result<(), Vec<Diagnostic>> {
        match states.get(id) {
            Some(1) => {
                return Err(vec![selection_error(
                    REASON_UNSUPPORTED_CALLEE,
                    "public API selected closure must be acyclic".to_owned(),
                )])
            }
            Some(2) => return Ok(()),
            _ => {}
        }
        states.insert(id, 1);
        let function = admitted.get(id).ok_or_else(|| {
            vec![selection_error(
                REASON_UNSUPPORTED_CALLEE,
                format!("public API closure function `{id}` is not admitted"),
            )]
        })?;
        for expression in function
            .requires
            .iter()
            .chain(&function.ensures)
            .chain(std::iter::once(&function.body))
        {
            let mut pending = vec![expression];
            while let Some(expression) = pending.pop() {
                if let ResolvedExprKind::Call {
                    callee,
                    instance: None,
                    ..
                } = &expression.kind
                {
                    if admitted.contains_key(callee.as_str()) {
                        visit(callee.as_str(), admitted, states)?;
                    }
                }
                pending.extend(child_expressions(expression));
            }
        }
        states.insert(id, 2);
        Ok(())
    }

    visit(entry_id, admitted, &mut BTreeMap::new())
}

pub(super) fn resolved_signature_is_admitted(
    function: &ResolvedFunction,
    declarations: &hir::DeclarationIndex,
) -> bool {
    function.effects.is_empty() && resolved_data_signature_is_admitted(function, declarations)
}

pub(super) fn resolved_data_signature_is_admitted(
    function: &ResolvedFunction,
    declarations: &hir::DeclarationIndex,
) -> bool {
    function.params.iter().all(|parameter| {
        resolved_data_parameter_is_admitted(&parameter.ty, parameter.ownership, declarations)
    }) && (resolved_data_result_is_admitted(&function.return_type, declarations)
        || nested_owned::owned_input_copy_result_is_admitted(function, declarations))
}

pub(super) fn resolved_data_parameter_is_admitted(
    ty: &ResolvedType,
    ownership: hir::OwnershipMode,
    declarations: &hir::DeclarationIndex,
) -> bool {
    match (ty, ownership) {
        (ty, hir::OwnershipMode::Value)
            if is_admitted_resolved_scalar(ty)
                || crate::list_ops::is_list(ty)
                || hir::function_value::is_signature(ty)
                || matches!(ty, ResolvedType::ArrayU8(_)) =>
        {
            true
        }
        (ty @ ResolvedType::Nominal { declaration, .. }, hir::OwnershipMode::Value)
            if declarations.declaration(declaration).is_some_and(|item| {
                matches!(
                    item.kind,
                    hir::DeclarationKind::Class | hir::DeclarationKind::Record
                )
            }) && record_construction_is_admitted(declarations, ty) =>
        {
            true
        }
        // A Copy variant (payload-free or Copy payloads) passes by value.
        (ty @ ResolvedType::Nominal { .. }, hir::OwnershipMode::Value)
            if is_admitted_copy_aggregate_variant(declarations, ty)
                || super::variant_admission::is_admitted_copy_scalar_variant(declarations, ty) =>
        {
            true
        }
        (
            ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
            | ResolvedType::MutFunctionI64,
            hir::OwnershipMode::Own,
        )
        | (ResolvedType::Bytes, hir::OwnershipMode::Own)
        | (ResolvedType::Bytes, hir::OwnershipMode::Borrow)
        | (ResolvedType::Str, hir::OwnershipMode::Borrow)
        | (ResolvedType::SliceU8, hir::OwnershipMode::Borrow)
        | (ResolvedType::ArrayU8(_), hir::OwnershipMode::Borrow) => true,
        (ty, hir::OwnershipMode::Own | hir::OwnershipMode::Borrow)
            if crate::map_ops::is_collection(ty)
                || owned_vec::is_collection_type(ty)
                || is_admitted_owned_byte_record(declarations, ty)
                || is_admitted_owned_variant(declarations, ty) =>
        {
            true
        }
        _ => false,
    }
}
pub(super) fn resolved_data_result_is_admitted(
    ty: &ResolvedType,
    declarations: &hir::DeclarationIndex,
) -> bool {
    is_admitted_resolved_scalar(ty)
        || crate::list_ops::is_list(ty)
        || hir::function_value::is_signature(ty)
        || matches!(
            ty,
            ResolvedType::ArrayU8(_)
                | ResolvedType::Bytes
                | ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64
        )
        || crate::map_ops::is_collection(ty)
        || owned_vec::is_collection_type(ty)
        || is_admitted_owned_byte_record(declarations, ty)
        || is_admitted_owned_variant(declarations, ty)
        || super::variant_admission::is_admitted_copy_scalar_variant(declarations, ty)
        || matches!(ty, ResolvedType::Nominal { declaration, .. }
            if declarations.declaration(declaration).is_some_and(|item| {
                matches!(item.kind, hir::DeclarationKind::Class | hir::DeclarationKind::Record)
            }) && record_construction_is_admitted(declarations, ty))
}
