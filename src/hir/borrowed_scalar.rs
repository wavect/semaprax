//! Closed non-owning signatures and rooted reads for retained helper replay.
//! This is a shape check; current defining source and cleanup proofs remain required.
pub(crate) mod option_u8;
use super::{
    OwnershipMode as O, ResolvedExpr, ResolvedExprKind as K, ResolvedFunction, ResolvedType as T,
};
pub(crate) fn parameter(ty: &T, mode: O) -> bool {
    (mode == O::Value && super::is_scalar_resolved_type(ty))
        || (mode == O::Borrow && matches!(ty, T::Str | T::SliceU8))
}
pub(crate) fn signature(function: &ResolvedFunction) -> bool {
    super::is_scalar_resolved_type(&function.return_type)
        && function
            .params
            .iter()
            .all(|p| parameter(&p.ty, p.ownership))
}
pub(crate) fn builtin(id: &str) -> bool {
    id == crate::string_ops::I64_FROM_U8_ID
        || id == crate::byte_ops::GET_ID
        || id == crate::byte_ops::LEN_ID
        || id == crate::byte_ops::PREFIX_CLASS_ID
        || matches!(
            crate::str_ops::by_id(id),
            Some(
                crate::str_ops::StrOp::LenBytes
                    | crate::str_ops::StrOp::IsEmpty
                    | crate::str_ops::StrOp::StartsWith
                    | crate::str_ops::StrOp::Contains
            )
        )
}
fn root<E>(
    function: &ResolvedFunction,
    place: &super::Place,
    ty: &T,
    charge: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    if !place.projections.is_empty() {
        return Ok(false);
    }
    let mut found = false;
    for parameter in &function.params {
        // The exact current parameter identity is checked; no alias or owned root
        // receives authority from a nominally borrowed expression type.
        charge(parameter.id.as_str().len())?;
        charge(place.root.as_str().len())?;
        charge(1)?;
        if parameter.id == place.root {
            if found || parameter.ownership != O::Borrow || &parameter.ty != ty {
                return Ok(false);
            }
            found = true;
        }
    }
    Ok(found)
}
pub(crate) fn expression<E>(
    function: &ResolvedFunction,
    expr: &ResolvedExpr,
    charge: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    let scalar = super::is_scalar_resolved_type(&expr.ty) && expr.ownership == O::Value;
    let copy = (scalar || option_u8::is_type(&expr.ty)) && expr.ownership == O::Value;
    match &expr.kind {
        K::Place(place) if matches!(expr.ty, T::Str | T::SliceU8) => {
            Ok(expr.ownership == O::Borrow && root(function, place, &expr.ty, charge)?)
        }
        K::BorrowPlace { operation, place } => Ok(expr.ownership == O::Borrow
            && expr.ty == T::SliceU8
            && operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID
            && root(function, place, &T::Str, charge)?),
        K::Call {
            callee,
            instance,
            type_arguments,
            args,
        } if builtin(callee.as_str()) => {
            charge(args.len())?;
            charge(1)?;
            if !copy || instance.is_some() || !type_arguments.is_empty() {
                return Ok(false);
            }
            if callee.as_str() == crate::string_ops::I64_FROM_U8_ID {
                return Ok(expr.ty == T::I64
                    && args.len() == 1
                    && args[0].ty == T::U8
                    && args[0].ownership == O::Value);
            }
            if callee.as_str() == crate::byte_ops::GET_ID {
                return Ok(option_u8::is_type(&expr.ty)
                    && args.len() == 2
                    && args[0].ty == T::SliceU8
                    && args[0].ownership == O::Borrow
                    && args[1].ty == T::Usize
                    && args[1].ownership == O::Value);
            }
            if callee.as_str() == crate::byte_ops::LEN_ID {
                return Ok(expr.ty == T::Usize
                    && args.len() == 1
                    && args[0].ty == T::SliceU8
                    && args[0].ownership == O::Borrow);
            }
            if callee.as_str() == crate::byte_ops::PREFIX_CLASS_ID {
                return Ok(expr.ty == T::Usize
                    && args.len() == 7
                    && args[0].ty == T::SliceU8
                    && args[0].ownership == O::Borrow
                    && args[1..3]
                        .iter()
                        .all(|arg| arg.ty == T::Usize && arg.ownership == O::Value)
                    && args[3..]
                        .iter()
                        .all(|arg| arg.ty == T::U8 && arg.ownership == O::Value));
            }
            let Some(operation) = crate::str_ops::by_id(callee.as_str()) else {
                return Ok(false);
            };
            Ok(expr.ty == operation.return_type()
                && args.len() == operation.arity()
                && args
                    .iter()
                    .all(|arg| arg.ty == T::Str && arg.ownership == O::Borrow))
        }
        K::Call { callee, .. }
            if crate::byte_ops::by_id(callee.as_str()).is_some()
                || crate::str_ops::by_id(callee.as_str()).is_some()
                || crate::string_ops::by_id(callee.as_str()).is_some() =>
        {
            Ok(false)
        }
        K::Match {
            mode,
            scrutinee,
            arms,
        } if option_u8::is_type(&scrutinee.ty) => {
            charge(arms.len())?;
            Ok(scalar
                && *mode == super::ResolvedMatchMode::Value
                && arms.len() == 2
                && arms
                    .iter()
                    .all(|arm| arm.guard.is_none() && option_u8::pattern(&arm.pattern))
                && matches!((&arms[0].pattern, &arms[1].pattern),
                    (super::ResolvedMatchPattern::Variant {case:a,..}, super::ResolvedMatchPattern::Variant {case:b,..}) if a!=b))
        }
        K::Place(place) if option_u8::is_type(&expr.ty) => Ok(copy && place.projections.is_empty()),
        _ => Ok(scalar),
    }
}
