use crate::cleanup::{FieldLiveness, FieldLivenessShape, LivenessFlagId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedFunction, ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind,
};

use super::{replay_error, type_needs_drop};
use crate::cleanup_plan::{CLEANUP_PLAN_SCHEMA_V7, CLEANUP_PLAN_SCHEMA_V8};

fn nested_schema(schema: &str) -> bool {
    matches!(schema, CLEANUP_PLAN_SCHEMA_V7 | CLEANUP_PLAN_SCHEMA_V8)
}

#[derive(Default)]
struct Budget {
    owned_leaves: usize,
    visited_fields: usize,
}

pub(super) fn expected_shape_for_type(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next_flag: &mut u32,
) -> Result<FieldLivenessShape, Diagnostic> {
    derive(program, function, ty, next_flag, 0, &mut Budget::default())
}

fn derive(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    next_flag: &mut u32,
    depth: usize,
    budget: &mut Budget,
) -> Result<FieldLivenessShape, Diagnostic> {
    if !type_needs_drop(program, function, ty)? {
        return Ok(FieldLivenessShape::NoDrop);
    }
    if matches!(ty, ResolvedType::Bytes) {
        if depth > 1 && !nested_schema(function.cleanup_plan.schema) {
            return Err(replay_error(
                function,
                "nested compiler-owned Bytes cleanup leaf is outside flat record v1",
            ));
        }
        charge_leaf(function, budget)?;
        let flag = next_flag_id(function, next_flag)?;
        return Ok(FieldLivenessShape::Leaf {
            flag,
            lifecycle: DeclarationId::new(crate::cleanup::BYTES_DROP_LIFECYCLE_ID),
        });
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Err(replay_error(
            function,
            "droppable supplemental slot is not nominal",
        ));
    };
    let item = program
        .types
        .iter()
        .find(|item| item.id == *declaration)
        .ok_or_else(|| replay_error(function, format!("unknown cleanup type `{declaration}`")))?;
    match &item.kind {
        ResolvedTypeDeclarationKind::Resource { drop } => {
            if !arguments.is_empty() {
                return Err(replay_error(
                    function,
                    "resource cleanup slot has generic arguments",
                ));
            }
            charge_leaf(function, budget)?;
            Ok(FieldLivenessShape::Leaf {
                flag: next_flag_id(function, next_flag)?,
                lifecycle: drop.id.clone(),
            })
        }
        ResolvedTypeDeclarationKind::Record { fields }
        | ResolvedTypeDeclarationKind::Class { fields, .. } => {
            if !arguments.is_empty() {
                return Err(replay_error(
                    function,
                    "record cleanup slot has generic arguments",
                ));
            }
            let child_depth = depth
                .checked_add(1)
                .ok_or_else(|| replay_error(function, "cleanup projection depth overflowed"))?;
            if nested_schema(function.cleanup_plan.schema)
                && child_depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
            {
                return Err(replay_error(
                    function,
                    format!(
                        "cleanup shape exceeds the {} record-depth limit",
                        crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                    ),
                ));
            }
            charge_fields(function, budget, fields.len())?;
            let mut expected = Vec::with_capacity(fields.len());
            for field in fields {
                expected.push(FieldLiveness {
                    field: field.id.clone(),
                    field_index: field.index,
                    shape: derive(program, function, &field.ty, next_flag, child_depth, budget)?,
                });
            }
            Ok(FieldLivenessShape::Record {
                declaration: declaration.clone(),
                fields: expected,
            })
        }
        ResolvedTypeDeclarationKind::Variant { cases } => {
            let count = cases
                .iter()
                .try_fold(0usize, |count, case| count.checked_add(case.fields.len()))
                .ok_or_else(|| replay_error(function, "cleanup visited-field count overflowed"))?;
            charge_fields(function, budget, count)?;
            let mut expected_cases = Vec::with_capacity(cases.len());
            for case in cases {
                let mut expected_fields = Vec::with_capacity(case.fields.len());
                for field in &case.fields {
                    let ty = crate::hir::substitute_type(&field.ty, declaration, arguments)?;
                    expected_fields.push(FieldLiveness {
                        field: field.id.clone(),
                        field_index: field.index,
                        shape: derive(program, function, &ty, next_flag, depth, budget)?,
                    });
                }
                expected_cases.push(crate::cleanup::VariantCaseLiveness {
                    case: case.id.clone(),
                    case_index: case.index,
                    fields: expected_fields,
                });
            }
            Ok(FieldLivenessShape::Variant {
                declaration: declaration.clone(),
                cases: expected_cases,
            })
        }
    }
}

fn next_flag_id(function: &ResolvedFunction, next: &mut u32) -> Result<LivenessFlagId, Diagnostic> {
    let flag = LivenessFlagId(*next);
    *next = next
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "too many cleanup flags"))?;
    Ok(flag)
}

fn charge_leaf(function: &ResolvedFunction, budget: &mut Budget) -> Result<(), Diagnostic> {
    budget.owned_leaves = budget
        .owned_leaves
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "cleanup owned-leaf count overflowed"))?;
    if nested_schema(function.cleanup_plan.schema)
        && budget.owned_leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES
    {
        return Err(replay_error(
            function,
            format!(
                "cleanup shape exceeds the {} owned-leaf limit",
                crate::cleanup::MAX_CLEANUP_OWNED_LEAVES
            ),
        ));
    }
    Ok(())
}

fn charge_fields(
    function: &ResolvedFunction,
    budget: &mut Budget,
    count: usize,
) -> Result<(), Diagnostic> {
    budget.visited_fields = budget
        .visited_fields
        .checked_add(count)
        .ok_or_else(|| replay_error(function, "cleanup visited-field count overflowed"))?;
    if nested_schema(function.cleanup_plan.schema)
        && budget.visited_fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS
    {
        return Err(replay_error(
            function,
            format!(
                "cleanup shape exceeds the {} visited-field limit",
                crate::cleanup::MAX_CLEANUP_VISITED_FIELDS
            ),
        ));
    }
    Ok(())
}
