//! Which resolved types the native lane lowers as plan-owned carriers.

use super::*;
use crate::hir::{ResolvedProgram, ResolvedType};

/// The compiler-owned bounded `Vec<T>` carriers the native lane lowers.
///
/// `crate::cleanup::is_owned_bounded_vec_type` answers the target-neutral
/// question from the resolved type alone. The SPX-AI-019 owned-record element
/// profile additionally needs `DeclarationIndex` facts to re-derive its
/// element admission, so it stays a separate question and every native site
/// that maps a carrier onto a machine representation asks this one instead.
/// Widening the shared predicate would silently change the Wasm lane, which
/// still refuses this profile.
pub(in crate::codegen) fn is_native_owned_vec_type(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> bool {
    crate::cleanup::is_owned_bounded_vec_type(ty)
        || crate::hir::owned_record_collection::is_owned_record_vec_type(&program.declarations, ty)
}

/// `true` when the canonical cleanup plan owns `ty` directly, as one leaf,
/// rather than through projected record or variant fields.
pub(super) fn is_direct_plan_owned(program: &ResolvedProgram, ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::Bytes
            | ResolvedType::String
            | ResolvedType::StringMap
            | ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
    ) || is_native_owned_vec_type(program, ty)
        || crate::stdin_stream_ops::is_reader(ty)
        || crate::cleanup::is_owned_bounded_box_type(ty)
        || crate::iterator_ops::is_iter(ty)
}

pub(super) fn c_value_type(
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    ty: &ResolvedType,
) -> Result<String, Diagnostic> {
    if crate::stdin_stream_ops::is_reader(ty) {
        Ok("uintptr_t".to_owned())
    } else if ty.is_once_function() || ty.is_mut_function() {
        Ok(once::c_type(ty).to_owned())
    } else if crate::list_ops::is_list(ty) {
        Ok("spx_list_v1".to_owned())
    } else if matches!(ty, ResolvedType::Function { .. }) {
        function_value::c_type(program, ty)
    } else if let Some(iterator) = native_iter::c_type(ty) {
        Ok(iterator.to_owned())
    } else if is_native_owned_vec_type(program, ty) {
        Ok("spx_vec_v1".to_owned())
    } else if crate::cleanup::is_owned_bounded_box_type(ty) {
        Ok("spx_box_v1".to_owned())
    } else if matches!(ty, ResolvedType::ArrayU8(0)) {
        // ISO C11 has no zero-sized value type. Ordinary internal calls use
        // one byte as a non-semantic ABI carrier while all actual array
        // storage and element access remain erased.
        Ok("uint8_t".to_owned())
    } else if let ResolvedType::ArrayU8(length) = ty {
        Ok(format!("struct spx_array_u8_{length}"))
    } else if record_declaration_id(program, ty)?.is_some() {
        Ok(format!("struct {}", c_record_symbol(ty)))
    } else if variant_declaration_id(program, ty)?.is_some() {
        Ok(format!("struct {}", c_variant_symbol(ty)))
    } else {
        resource_abi.c_type(program, ty).map(str::to_owned)
    }
}

pub(super) fn record_declaration_id<'a>(
    program: &ResolvedProgram,
    ty: &'a ResolvedType,
) -> Result<Option<&'a DeclarationId>, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    if crate::stdin_stream_ops::is_reader(ty)
        || crate::list_ops::is_list(ty)
        || crate::iterator_ops::is_iter(ty)
        || is_native_owned_vec_type(program, ty)
        || crate::cleanup::is_owned_bounded_box_type(ty)
    {
        return Ok(None);
    }
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| backend_error(format!("unknown native type `{declaration}`")))?;
    if !matches!(
        item.kind,
        ResolvedTypeDeclarationKind::Record { .. } | ResolvedTypeDeclarationKind::Class { .. }
    ) {
        return Ok(None);
    }
    if arguments.len() != item.type_parameters.len()
        || (!arguments.is_empty()
            && (!matches!(item.kind, ResolvedTypeDeclarationKind::Record { .. })
                || !generic_record::is_admitted(program, ty)?))
    {
        return Err(backend_error(format!(
            "native record representation requires admitted exact concrete arguments for `{}`",
            ty.identity_key()
        )));
    }
    Ok(Some(declaration))
}

pub(super) fn variant_declaration_id<'a>(
    program: &ResolvedProgram,
    ty: &'a ResolvedType,
) -> Result<Option<&'a DeclarationId>, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| backend_error(format!("unknown native type `{declaration}`")))?;
    if !matches!(item.kind, ResolvedTypeDeclarationKind::Variant { .. }) {
        return Ok(None);
    }
    if arguments.len() != item.type_parameters.len()
        || (!crate::iterator_ops::step_shape(&program.declarations, ty)
            && !crate::hir::admitted_owned_byte_prelude_instance(declaration, arguments)
            && !crate::hir::is_admitted_concrete_owned_byte_variant(&program.declarations, ty)
            && arguments.iter().any(|argument| {
                !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                    && !(declaration.as_str() == crate::prelude::OPTION_ID
                        && *argument == ResolvedType::U8)
            }))
    {
        return Err(backend_error(format!(
            "native variant representation requires admitted exact concrete arguments for `{}`",
            ty.identity_key()
        )));
    }
    Ok(Some(declaration))
}
