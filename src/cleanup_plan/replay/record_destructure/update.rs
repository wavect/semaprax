//! Independent schema/authentication replay for nested record updates.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedType};

const DEPTH_LIMIT: usize = 64;
const LEAF_LIMIT: usize = 256;
const FIELD_LIMIT: usize = 4_096;

pub(in crate::cleanup_plan::replay) fn function_contains(
    program: &crate::hir::ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<bool, Diagnostic> {
    let mut pending = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .collect::<Vec<_>>();
    let mut nested = false;
    while let Some(expression) = pending.pop() {
        nested |= authenticate(program, function, expression)?;
        let mut index = 0usize;
        while let Some(child) = super::super::replay_expression_child(expression, index) {
            pending.push(child);
            index += 1;
        }
    }
    Ok(nested)
}

fn authenticate(
    program: &crate::hir::ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<bool, Diagnostic> {
    let ResolvedExprKind::UpdateRecord {
        base,
        record,
        fields,
    } = &expression.kind
    else {
        return Ok(false);
    };
    if expression.ty != base.ty {
        return Err(super::super::replay_error(
            function,
            "nested record update replay changes its result shape",
        ));
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &base.ty
    else {
        return Err(super::super::replay_error(
            function,
            "record update replay base is not nominal",
        ));
    };
    if declaration != record || !is_nested_owned_bytes(program, function, &base.ty)? {
        return Ok(false);
    }
    let declared = program
        .declarations
        .record_fields(record)
        .ok_or_else(|| super::super::replay_error(function, "nested update has no inventory"))?;
    let mut seen = BTreeSet::new();
    for initializer in fields {
        if !seen.insert(&initializer.field) {
            return Err(super::super::replay_error(
                function,
                "nested update replay repeats a replacement field",
            ));
        }
        let field = declared
            .iter()
            .find(|field| field.id == initializer.field)
            .ok_or_else(|| {
                super::super::replay_error(function, "nested update replay has a foreign field")
            })?;
        let ty = crate::hir::substitute_type(&field.ty, record, arguments)?;
        let ownership = if super::super::type_needs_drop(program, function, &ty)? {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        };
        if initializer.value.ty != ty || initializer.value.ownership != ownership {
            return Err(super::super::replay_error(
                function,
                "nested update replay replacement type or ownership is not canonical",
            ));
        }
    }
    Ok(true)
}

fn is_nested_owned_bytes(
    program: &crate::hir::ResolvedProgram,
    function: &ResolvedFunction,
    root: &ResolvedType,
) -> Result<bool, Diagnostic> {
    let mut pending = vec![(root.clone(), 0usize, BTreeSet::new())];
    let (mut fields, mut leaves, mut nested) = (0usize, 0usize, false);
    while let Some((ty, depth, ancestors)) = pending.pop() {
        if depth > DEPTH_LIMIT {
            return Err(super::super::replay_error(
                function,
                "nested update replay exceeds its depth bound",
            ));
        }
        if ty == ResolvedType::Bytes {
            leaves += 1;
            nested |= depth > 1;
            if leaves > LEAF_LIMIT {
                return Err(super::super::replay_error(
                    function,
                    "nested update replay exceeds its owned-leaf bound",
                ));
            }
            continue;
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            continue;
        };
        if !arguments.is_empty() || ancestors.contains(&declaration) {
            return Err(super::super::replay_error(
                function,
                "nested update replay has generic or cyclic shape",
            ));
        }
        let Some(declared) = program.declarations.record_fields(&declaration) else {
            continue;
        };
        fields = fields
            .checked_add(declared.len())
            .ok_or_else(|| super::super::replay_error(function, "nested update work overflowed"))?;
        if fields > FIELD_LIMIT {
            return Err(super::super::replay_error(
                function,
                "nested update replay exceeds its field-work bound",
            ));
        }
        let mut next = ancestors;
        next.insert(declaration.clone());
        for field in declared.iter().rev() {
            pending.push((
                crate::hir::substitute_type(&field.ty, &declaration, &arguments)?,
                depth + 1,
                next.clone(),
            ));
        }
    }
    Ok(nested)
}
