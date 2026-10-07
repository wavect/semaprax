//! Closed selection and iterative work admission before recursive lowering.

use super::{error, Export, PreparedSelection};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, OwnershipMode, ResolvedExprKind, ResolvedProgram,
    ResolvedStatement, ResolvedType,
};
use std::collections::BTreeSet;

pub(super) fn prepare(
    program: &ResolvedProgram,
    ids: &[String],
) -> Result<PreparedSelection, Diagnostic> {
    prepare_profile(program, ids, false, false, false, false)
}

pub(super) fn prepare_copy_variants(
    program: &ResolvedProgram,
    ids: &[String],
) -> Result<PreparedSelection, Diagnostic> {
    prepare_profile(program, ids, true, false, false, false)
}

pub(super) fn prepare_replacements(
    program: &ResolvedProgram,
    ids: &[String],
) -> Result<PreparedSelection, Diagnostic> {
    prepare_profile(program, ids, true, true, false, false)
}
pub(super) fn prepare_general_loop_matches(
    program: &ResolvedProgram,
    ids: &[String],
) -> Result<PreparedSelection, Diagnostic> {
    prepare_profile(program, ids, true, true, true, false)
}

pub(super) fn prepare_toolkit(
    program: &ResolvedProgram,
    ids: &[String],
) -> Result<PreparedSelection, Diagnostic> {
    prepare_profile(program, ids, true, true, true, true)
}

fn prepare_profile(
    program: &ResolvedProgram,
    ids: &[String],
    copy_variants: bool,
    replacements: bool,
    general_guards: bool,
    toolkit: bool,
) -> Result<PreparedSelection, Diagnostic> {
    if !(1..=32).contains(&ids.len()) {
        return Err(error("standalone String selection requires 1..=32 exports"));
    }
    let mut roots = BTreeSet::new();
    for id in ids {
        if !roots.insert(DeclarationId::new(id.clone())) {
            return Err(error("standalone String selection repeats an identity"));
        }
    }
    let mut exports = Vec::new();
    for id in &roots {
        let function = program
            .functions
            .iter()
            .find(|function| &function.id == id)
            .ok_or_else(|| error("standalone String export is absent"))?;
        if !program
            .declarations
            .declaration(id)
            .is_some_and(|declaration| declaration.identity_origin == IdentityOrigin::Explicit)
            || function.params.len() > 8
            || !public_scalar(&function.return_type)
            || function.params.iter().any(|parameter| {
                parameter.ownership != OwnershipMode::Value || !public_scalar(&parameter.ty)
            })
        {
            return Err(error("standalone String export requires an explicit identity and bounded value i64/bool signature"));
        }
        exports.push(Export {
            id: id.clone(),
            parameters: function
                .params
                .iter()
                .map(|parameter| parameter.ty.clone())
                .collect(),
            result: function.return_type.clone(),
        });
    }
    let calls = crate::call_index::PersistentCallIndex::build(program)?;
    let mut closure = BTreeSet::new();
    let mut pending = roots.into_iter().collect::<Vec<_>>();
    while let Some(id) = pending.pop() {
        if !closure.insert(id.clone()) {
            continue;
        }
        if closure.len() > 256 {
            return Err(error("standalone String closure exceeds 256 functions"));
        }
        let children = calls
            .calls_by_owner()
            .get(&id)
            .ok_or_else(|| error("standalone String callee is absent"))?;
        pending.extend(children.iter().filter(|id| !closure.contains(*id)).cloned());
    }
    let mut nodes = 0usize;
    let mut literals = BTreeSet::new();
    let mut literal_bytes = 0usize;
    for function in program
        .functions
        .iter()
        .filter(|function| closure.contains(&function.id))
    {
        if !replacements && crate::string_ops::replacement::requires(function) {
            return Err(error(
                "whole String replacement requires the explicit string-replacement-v1 profile",
            ));
        }
        if (!function.effects.is_empty()
            && !(toolkit
                && function
                    .effects
                    .iter()
                    .all(|effect| effect == crate::string_ops::FILE_READ_TEXT_EFFECT)))
            || !(signature_type(
                program,
                &function.return_type,
                copy_variants,
                toolkit,
                general_guards,
            ) || general_guards
                && crate::variant_guards::copy_variant(
                    &program.declarations,
                    &function.return_type,
                ))
            || function.params.iter().any(|parameter| {
                // By-value String source parameters are implicitly Own in
                // validated HIR; only the internal Copy scalars are Value.
                let ownership = if parameter.ty == ResolvedType::String {
                    OwnershipMode::Own
                } else if toolkit && parameter.ty == ResolvedType::Str {
                    OwnershipMode::Borrow
                } else {
                    OwnershipMode::Value
                };
                !(parameter.ownership == ownership
                    || toolkit
                        && hir::is_admitted_owned_string_variant(
                            &program.declarations,
                            &parameter.ty,
                        )
                        && matches!(
                            parameter.ownership,
                            OwnershipMode::Own | OwnershipMode::Borrow
                        ))
                    || !(signature_type(
                        program,
                        &parameter.ty,
                        copy_variants,
                        toolkit,
                        general_guards,
                    ) || general_guards
                        && crate::variant_guards::copy_variant(
                            &program.declarations,
                            &parameter.ty,
                        ))
            })
        {
            return Err(error(
                "standalone String internal signature is outside the closed profile",
            ));
        }
        let mut expressions = function
            .requires
            .iter()
            .chain(&function.ensures)
            .chain(std::iter::once(&function.body))
            .map(|expression| (expression, 1usize))
            .collect::<Vec<_>>();
        while let Some((expression, depth)) = expressions.pop() {
            nodes = nodes
                .checked_add(1)
                .filter(|nodes| *nodes <= 65_536)
                .ok_or_else(|| {
                    error("standalone String expression inventory exceeds 65536 nodes")
                })?;
            if depth > 256
                || !expression_type(
                    program,
                    &expression.ty,
                    copy_variants,
                    toolkit,
                    general_guards,
                )
            {
                return Err(error(
                    "standalone String expression depth or type is outside the profile",
                ));
            }
            if copy_variants
                && !general_guards
                && matches!(expression.kind, ResolvedExprKind::Match { .. })
                && crate::variant_guards::copy_variant(&program.declarations, &expression.ty)
            {
                return Err(error(
                    "Copy variant match results require the explicit general-loop-match-v1 profile",
                ));
            }
            if copy_variants
                && !general_guards
                && matches!(&expression.kind, ResolvedExprKind::Match {scrutinee,arms,..} if crate::variant_guards::copy_variant(&program.declarations,&scrutinee.ty) && arms.iter().any(|arm| arm.guard.as_ref().is_some_and(|guard| !crate::variant_guards::scalar_guard_shape(guard) || !matches!(arm.pattern,hir::ResolvedMatchPattern::Variant {..}))))
            {
                return Err(error("general Copy variant guards require the explicit general-loop-match-v1 profile"));
            }
            match &expression.kind {
                ResolvedExprKind::Float64(_) | ResolvedExprKind::Float32(_) | ResolvedExprKind::Int32(_) if toolkit || general_guards => {}
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Unary { .. }
                | ResolvedExprKind::Binary { .. }
                | ResolvedExprKind::If { .. } => {}
                ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::ArrayU8(_)
                | ResolvedExprKind::RepeatArrayU8 { .. }
                    if copy_variants => {}
                ResolvedExprKind::ConstructRecord { .. } if toolkit && hir::is_admitted_copy_aggregate_variant_field(&program.declarations, &expression.ty) => {}
                ResolvedExprKind::ConstructVariant { .. }
                    if copy_variants && (crate::variant_guards::copy_variant(&program.declarations, &expression.ty) || toolkit && hir::is_admitted_owned_string_variant(&program.declarations, &expression.ty)) => {}
                ResolvedExprKind::BorrowPlace { operation, place }
                    if toolkit && operation.as_str() == crate::byte_ops::STRING_AS_STR_ID && place.projections.is_empty() => {}
                ResolvedExprKind::BorrowPlace { operation, place }
                    if copy_variants && operation.as_str() == crate::byte_ops::ARRAY_AS_SLICE_ID && place.projections.is_empty() => {}
                ResolvedExprKind::String(value) => {
                    // The selected profile owns the fixed literal segment's
                    // admission. Match the emitter's exact UTF-8 deduplication
                    // without allocating another copy of the payload bytes.
                    if literals.insert(value.as_str()) {
                        literal_bytes = literal_bytes
                            .checked_add(value.len())
                            .filter(|bytes| *bytes <= 65_536)
                            .ok_or_else(|| {
                                error("standalone String literal pool exceeds 65536 bytes")
                            })?;
                    }
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() || toolkit && place.projections.iter().all(|projection| matches!(projection, hir::PlaceProjection::Field(_))) => {}
                ResolvedExprKind::Call {
                    callee,
                    instance,
                    type_arguments,
                    ..
                } if instance.is_none()
                    && type_arguments.is_empty()
                    && (closure.contains(callee)
                        || crate::string_ops::by_id(callee.as_str()).is_some()
                        || copy_variants && matches!(crate::byte_ops::by_id(callee.as_str()), Some(crate::byte_ops::ByteOp::Len | crate::byte_ops::ByteOp::Get))) => {}
                ResolvedExprKind::Block { statements, .. } => {
                    if !toolkit && statements.iter().any(|statement| matches!(statement,
                        ResolvedStatement::While { condition, .. } if condition_allocates_string(condition))) {
                        return Err(error("String condition temporaries require text-toolkit-v1"));
                    }
                    if statements.iter().any(|statement| {
                        matches!(
                            statement,
                            ResolvedStatement::Unsafe { .. }
                                | ResolvedStatement::Assign { field: Some(_), .. }
                        )
                    }) {
                        return Err(error(
                            "standalone String profile excludes unsafe or projected mutation",
                        ));
                    }
                }
                ResolvedExprKind::Match {
                    scrutinee,
                    arms,
                    mode,
                } if *mode == hir::ResolvedMatchMode::Value
                    && matches!(
                        scrutinee.ty,
                        ResolvedType::I64 | ResolvedType::Bool | ResolvedType::Char
                    )
                    && arms
                        .iter()
                        .all(|arm| arm.pattern_is_literal_or_irrefutable()) => {}
                ResolvedExprKind::Match { scrutinee, .. }
                    if toolkit && (hir::is_admitted_owned_string_variant(&program.declarations, &scrutinee.ty) || hir::is_admitted_copy_aggregate_variant_field(&program.declarations, &scrutinee.ty)) => {}
                ResolvedExprKind::Match { scrutinee, mode, .. }
                    if copy_variants && *mode == hir::ResolvedMatchMode::Value && crate::variant_guards::copy_variant(&program.declarations, &scrutinee.ty) => {}
                ResolvedExprKind::Match { scrutinee, .. }
                    if !matches!(
                        scrutinee.ty,
                        ResolvedType::I64 | ResolvedType::Bool | ResolvedType::Char
                    ) =>
                {
                    return Err(error(
                        "standalone String profile matches only `i64`, `bool`, or `char` scrutinees",
                    )
                    .with_help(
                        "run a variant or record `match` on the reference interpreter or native C11, \
                         or match an `i64` code in the exported Wasm function",
                    ))
                }
                _ => {
                    return Err(error(
                        "standalone String expression is outside the closed profile",
                    ))
                }
            }
            for child in crate::interpreter::trace_child_expressions(expression) {
                expressions.push((child, depth + 1));
            }
        }
    }
    Ok((exports, closure))
}

fn public_scalar(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::I64 | ResolvedType::Bool)
}
fn internal_type(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64 | ResolvedType::Bool | ResolvedType::Char | ResolvedType::String
    )
}

fn signature_type(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    copy_variants: bool,
    toolkit: bool,
    general_guards: bool,
) -> bool {
    internal_type(ty)
        || general_guards && hir::is_scalar_resolved_type(ty)
        || toolkit
            && (hir::is_scalar_resolved_type(ty)
                || hir::is_admitted_owned_string_variant(&program.declarations, ty)
                || crate::variant_guards::copy_variant(&program.declarations, ty)
                || hir::is_admitted_copy_aggregate_variant_field(&program.declarations, ty))
        || toolkit && matches!(ty, ResolvedType::F64 | ResolvedType::Str)
        || copy_variants && matches!(ty, ResolvedType::U8 | ResolvedType::Usize)
}

fn expression_type(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    copy_variants: bool,
    toolkit: bool,
    general_guards: bool,
) -> bool {
    signature_type(program, ty, copy_variants, toolkit, general_guards)
        || toolkit && hir::is_admitted_owned_string_variant(&program.declarations, ty)
        || copy_variants
            && (matches!(ty, ResolvedType::ArrayU8(_) | ResolvedType::SliceU8)
                || crate::variant_guards::copy_variant(&program.declarations, ty))
}

fn condition_allocates_string(condition: &hir::ResolvedExpr) -> bool {
    let mut pending = vec![condition];
    while let Some(expression) = pending.pop() {
        if expression.ty == ResolvedType::String
            && matches!(
                expression.kind,
                ResolvedExprKind::String(_) | ResolvedExprKind::Call { .. }
            )
        {
            return true;
        }
        hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}
