//! Pre-effect authentication for nested owned-record immutable updates.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};

const DEPTH_LIMIT: usize = 64;
const LEAF_LIMIT: usize = 256;
const FIELD_LIMIT: usize = 4_096;

pub(in crate::cleanup_plan::build) fn authenticate(
    builder: &mut super::super::PlanBuilder<'_>,
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
        return Err(super::super::plan_error(
            "nested record update changes its result shape",
        ));
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &base.ty
    else {
        return Err(super::super::plan_error(
            "record update base is not nominal",
        ));
    };
    if declaration != record {
        return Err(super::super::plan_error(
            "record update base and declaration disagree",
        ));
    }
    if !is_nested_owned_bytes(builder.program, &base.ty)? {
        return Ok(false);
    }
    let declared = builder
        .program
        .declarations
        .record_fields(record)
        .ok_or_else(|| super::super::plan_error("nested record update has no field inventory"))?;
    let mut seen = BTreeSet::new();
    for initializer in fields {
        if !seen.insert(&initializer.field) {
            return Err(super::super::plan_error(
                "nested record update repeats a replacement field",
            ));
        }
        let field = declared
            .iter()
            .find(|field| field.id == initializer.field)
            .ok_or_else(|| super::super::plan_error("nested record update has a foreign field"))?;
        let ty = crate::hir::substitute_type(&field.ty, record, arguments)?;
        let needs_drop = builder
            .program
            .declarations
            .type_facts(&ty)
            .ok_or_else(|| super::super::plan_error("nested update field has no type facts"))?
            .needs_drop;
        let ownership = if needs_drop {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        };
        if initializer.value.ty != ty || initializer.value.ownership != ownership {
            return Err(super::super::plan_error(
                "nested update replacement type or ownership is not canonical",
            ));
        }
    }
    super::super::schema::promote_v9(&mut builder.schema);
    Ok(true)
}

fn is_nested_owned_bytes(
    program: &crate::hir::ResolvedProgram,
    root: &ResolvedType,
) -> Result<bool, Diagnostic> {
    let mut pending = vec![(root.clone(), 0usize, BTreeSet::new())];
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut nested = false;
    while let Some((ty, depth, ancestors)) = pending.pop() {
        if depth > DEPTH_LIMIT {
            return Err(super::super::plan_error(
                "nested record update exceeds its depth bound",
            ));
        }
        if ty == ResolvedType::Bytes {
            leaves += 1;
            nested |= depth > 1;
            if leaves > LEAF_LIMIT {
                return Err(super::super::plan_error(
                    "nested record update exceeds its owned-leaf bound",
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
        // A generic instance or a cycle is outside the monomorphic acyclic
        // profile, so the base is not a nested owned-Bytes record at all.
        // Report that instead of failing the plan; the legacy flat path owns
        // these shapes, exactly as the source-side classifier treats
        // OutsideProfile and Recursive as not admitted.
        if !arguments.is_empty() || ancestors.contains(&declaration) {
            return Ok(false);
        }
        let Some(declared) = program.declarations.record_fields(&declaration) else {
            continue;
        };
        fields = fields
            .checked_add(declared.len())
            .ok_or_else(|| super::super::plan_error("nested update field work overflowed"))?;
        if fields > FIELD_LIMIT {
            return Err(super::super::plan_error(
                "nested record update exceeds its field-work bound",
            ));
        }
        let mut next_ancestors = ancestors;
        next_ancestors.insert(declaration.clone());
        for field in declared.iter().rev() {
            pending.push((
                crate::hir::substitute_type(&field.ty, &declaration, &arguments)?,
                depth + 1,
                next_ancestors.clone(),
            ));
        }
    }
    Ok(nested)
}
