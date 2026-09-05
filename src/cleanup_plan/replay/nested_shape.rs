use crate::cleanup::{FieldLiveness, FieldLivenessShape, LivenessFlagId};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationId, ResolvedFunction, ResolvedProgram, ResolvedType, ResolvedTypeDeclarationKind,
};

use super::{replay_error, type_needs_drop};
use crate::cleanup_plan::{CLEANUP_PLAN_SCHEMA_V7, CLEANUP_PLAN_SCHEMA_V8, CLEANUP_PLAN_SCHEMA_V9};

fn nested_schema(schema: &str) -> bool {
    matches!(
        schema,
        CLEANUP_PLAN_SCHEMA_V7 | CLEANUP_PLAN_SCHEMA_V8 | CLEANUP_PLAN_SCHEMA_V9
    )
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

/// Depth-first shape derivation over an explicit work stack.
///
/// A record chain is as deep as the program admits, and the legacy flat
/// schemas carry no depth bound, so recursing per level overflows the default
/// stack. The build side already lowers this shape iteratively; mirror it here
/// so replay agrees. Side effects keep their recursive order exactly: a
/// parent charges its fields before any child is entered, children are entered
/// left to right, and liveness flags are therefore still allocated in
/// depth-first order.
fn derive(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    root: &ResolvedType,
    next_flag: &mut u32,
    depth: usize,
    budget: &mut Budget,
) -> Result<FieldLivenessShape, Diagnostic> {
    struct CaseMeta {
        case: DeclarationId,
        case_index: u32,
        fields: Vec<(DeclarationId, u32)>,
    }
    enum Frame {
        Enter(ResolvedType, usize),
        FinishRecord(DeclarationId, Vec<(DeclarationId, u32)>),
        FinishVariant(DeclarationId, Vec<CaseMeta>),
    }

    let mut frames = vec![Frame::Enter(root.clone(), depth)];
    let mut shapes = Vec::<FieldLivenessShape>::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(ty, depth) => {
                if !type_needs_drop(program, function, &ty)? {
                    shapes.push(FieldLivenessShape::NoDrop);
                    continue;
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
                    shapes.push(FieldLivenessShape::Leaf {
                        flag,
                        lifecycle: DeclarationId::new(crate::cleanup::BYTES_DROP_LIFECYCLE_ID),
                    });
                    continue;
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &ty
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
                    .ok_or_else(|| {
                        replay_error(function, format!("unknown cleanup type `{declaration}`"))
                    })?;
                match &item.kind {
                    ResolvedTypeDeclarationKind::Resource { drop } => {
                        if !arguments.is_empty() {
                            return Err(replay_error(
                                function,
                                "resource cleanup slot has generic arguments",
                            ));
                        }
                        charge_leaf(function, budget)?;
                        shapes.push(FieldLivenessShape::Leaf {
                            flag: next_flag_id(function, next_flag)?,
                            lifecycle: drop.id.clone(),
                        });
                    }
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                        if !arguments.is_empty()
                            && !matches!(&item.kind, ResolvedTypeDeclarationKind::Record { .. })
                        {
                            return Err(replay_error(
                                function,
                                "record cleanup slot has generic arguments",
                            ));
                        }
                        let child_depth = depth.checked_add(1).ok_or_else(|| {
                            replay_error(function, "cleanup projection depth overflowed")
                        })?;
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
                        let meta = fields
                            .iter()
                            .map(|field| (field.id.clone(), field.index))
                            .collect::<Vec<_>>();
                        frames
                            .try_reserve(fields.len().saturating_add(1))
                            .map_err(|_| {
                                replay_error(
                                    function,
                                    "cleanup shape capacity exceeds address space",
                                )
                            })?;
                        frames.push(Frame::FinishRecord(declaration.clone(), meta));
                        for field in fields.iter().rev() {
                            frames.push(Frame::Enter(
                                crate::hir::substitute_type(&field.ty, declaration, arguments)?,
                                child_depth,
                            ));
                        }
                    }
                    ResolvedTypeDeclarationKind::Variant { cases } => {
                        let count = cases
                            .iter()
                            .try_fold(0usize, |count, case| count.checked_add(case.fields.len()))
                            .ok_or_else(|| {
                                replay_error(function, "cleanup visited-field count overflowed")
                            })?;
                        charge_fields(function, budget, count)?;
                        let mut meta = Vec::with_capacity(cases.len());
                        let mut entries = Vec::with_capacity(count);
                        for case in cases {
                            let mut case_fields = Vec::with_capacity(case.fields.len());
                            for field in &case.fields {
                                let ty =
                                    crate::hir::substitute_type(&field.ty, declaration, arguments)?;
                                case_fields.push((field.id.clone(), field.index));
                                entries.push(ty);
                            }
                            meta.push(CaseMeta {
                                case: case.id.clone(),
                                case_index: case.index,
                                fields: case_fields,
                            });
                        }
                        frames
                            .try_reserve(entries.len().saturating_add(1))
                            .map_err(|_| {
                                replay_error(
                                    function,
                                    "cleanup shape capacity exceeds address space",
                                )
                            })?;
                        frames.push(Frame::FinishVariant(declaration.clone(), meta));
                        for ty in entries.into_iter().rev() {
                            frames.push(Frame::Enter(ty, depth));
                        }
                    }
                }
            }
            Frame::FinishRecord(declaration, meta) => {
                let start = shapes.len().checked_sub(meta.len()).ok_or_else(|| {
                    replay_error(function, "cleanup record field shape is absent")
                })?;
                let children = shapes.split_off(start);
                let expected = meta
                    .into_iter()
                    .zip(children)
                    .map(|((field, field_index), shape)| FieldLiveness {
                        field,
                        field_index,
                        shape,
                    })
                    .collect::<Vec<_>>();
                shapes.push(FieldLivenessShape::Record {
                    declaration,
                    fields: expected,
                });
            }
            Frame::FinishVariant(declaration, meta) => {
                let total = meta.iter().map(|case| case.fields.len()).sum::<usize>();
                let start = shapes.len().checked_sub(total).ok_or_else(|| {
                    replay_error(function, "cleanup variant field shape is absent")
                })?;
                let mut children = shapes.split_off(start).into_iter();
                let mut expected_cases = Vec::with_capacity(meta.len());
                for case in meta {
                    let mut expected_fields = Vec::with_capacity(case.fields.len());
                    for (field, field_index) in case.fields {
                        let shape = children.next().ok_or_else(|| {
                            replay_error(function, "cleanup variant field shape is absent")
                        })?;
                        expected_fields.push(FieldLiveness {
                            field,
                            field_index,
                            shape,
                        });
                    }
                    expected_cases.push(crate::cleanup::VariantCaseLiveness {
                        case: case.case,
                        case_index: case.case_index,
                        fields: expected_fields,
                    });
                }
                shapes.push(FieldLivenessShape::Variant {
                    declaration,
                    cases: expected_cases,
                });
            }
        }
    }
    let shape = shapes
        .pop()
        .ok_or_else(|| replay_error(function, "cleanup shape is absent"))?;
    if !shapes.is_empty() {
        return Err(replay_error(function, "cleanup shape did not settle"));
    }
    Ok(shape)
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
