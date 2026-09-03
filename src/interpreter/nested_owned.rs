//! Runtime-only support for the verifier-authenticated acyclic nested record profile.

use std::collections::BTreeSet;
use std::sync::Arc;

use super::{Environment, Flow, OwnedRecordValue, Value};
use crate::hir::{self, ResolvedType, ValueId};

pub(super) fn is_admitted_owned_byte_record(
    declarations: &hir::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    classify_record(declarations, ty).is_some_and(|profile| profile.has_bytes)
}

/// Authenticate every stable field identity in a projected place. The first
/// field identifies the root nominal; every subsequent field must belong to
/// the exact nominal selected by its predecessor.
pub(super) fn admitted_owned_record_field(
    declarations: &hir::DeclarationIndex,
    root: &ResolvedType,
    place: &hir::Place,
    leaf: &ResolvedType,
) -> bool {
    if place.projections.is_empty() || !is_admitted_owned_byte_record(declarations, root) {
        return false;
    }
    let mut ty = root.clone();
    for projection in &place.projections {
        let hir::PlaceProjection::Field(field) = projection else {
            return false;
        };
        let ResolvedType::Nominal { declaration, .. } = &ty else {
            return false;
        };
        let Some(next) = declarations
            .record_fields(declaration)
            .and_then(|fields| fields.iter().find(|candidate| candidate.id == *field))
            .map(|field| field.ty.clone())
        else {
            return false;
        };
        ty = next;
    }
    ty == *leaf
}

pub(super) fn record_pattern_is_admitted(
    declarations: &hir::DeclarationIndex,
    mode: hir::ResolvedMatchMode,
    ty: &ResolvedType,
    pattern: &hir::ResolvedMatchPattern,
) -> bool {
    if !classify_record(declarations, ty)
        .is_some_and(|profile| profile.has_bytes && !profile.has_nested_record)
    {
        return false;
    }
    pattern_is_exact(declarations, mode, ty, pattern, &mut BTreeSet::new())
}

pub(super) fn take_owned_place(environment: &mut Environment, place: &hir::Place) -> Option<Value> {
    let value = environment
        .iter_mut()
        .rev()
        .find(|(key, _)| key == &place.root)
        .map(|(_, value)| value)?;
    take_at(value, &place.projections)
}

pub(super) fn bind_owned_pattern(
    record: Arc<OwnedRecordValue>,
    fields: &[hir::ResolvedRecordMatchPatternField],
    bindings: &mut Vec<(ValueId, Value)>,
) -> Result<(), Flow> {
    let mut record = Arc::try_unwrap(record)
        .map_err(|_| Flow::Guard("owned-byte record still has a live alias at transfer"))?;
    if fields.len() != record.fields.len() {
        return Err(Flow::Guard(
            "owned-byte record pattern disagrees with its runtime carrier",
        ));
    }
    for field in fields {
        let value = record.fields.remove(&field.field).ok_or(Flow::Guard(
            "owned-byte record pattern references an absent field",
        ))?;
        match &field.pattern {
            hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                bindings.push((binding.id.clone(), value));
            }
            hir::ResolvedRecordMatchFieldPattern::Wildcard if !value_needs_drop(&value) => {}
            hir::ResolvedRecordMatchFieldPattern::Wildcard => {
                return Err(Flow::Guard("owned byte subtree reached a wildcard discard"));
            }
            hir::ResolvedRecordMatchFieldPattern::Record { .. } => {
                return Err(Flow::Guard("nested owned record patterns remain closed"));
            }
        }
    }
    if !record.fields.is_empty() {
        return Err(Flow::Guard(
            "owned-byte record transfer left unauthenticated fields",
        ));
    }
    Ok(())
}

pub(super) fn bind_borrowed_pattern(
    record: &Arc<OwnedRecordValue>,
    fields: &[hir::ResolvedRecordMatchPatternField],
    bindings: &mut Vec<(ValueId, Value)>,
) -> Result<(), Flow> {
    if fields.len() != record.fields.len() {
        return Err(Flow::Guard(
            "borrowed record pattern disagrees with its runtime carrier",
        ));
    }
    for field in fields {
        let value = record.fields.get(&field.field).ok_or(Flow::Guard(
            "borrowed record pattern references an absent field",
        ))?;
        match &field.pattern {
            hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                bindings.push((binding.id.clone(), borrow_alias(value)?));
            }
            hir::ResolvedRecordMatchFieldPattern::Wildcard => {}
            hir::ResolvedRecordMatchFieldPattern::Record { .. } => {
                return Err(Flow::Guard("nested borrowed record patterns remain closed"));
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct RecordProfile {
    has_bytes: bool,
    has_nested_record: bool,
}

fn classify_record(
    declarations: &hir::DeclarationIndex,
    root: &ResolvedType,
) -> Option<RecordProfile> {
    enum Frame {
        Enter(ResolvedType, usize),
        Leave(hir::DeclarationId),
    }
    let mut pending = vec![Frame::Enter(root.clone(), 1)];
    let mut active = BTreeSet::new();
    let mut visited_fields = 0usize;
    let mut owned_leaves = 0usize;
    let mut profile = RecordProfile {
        has_bytes: false,
        has_nested_record: false,
    };
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ResolvedType::Bytes, _) => {
                profile.has_bytes = true;
                owned_leaves = owned_leaves.checked_add(1)?;
                if owned_leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
                    return None;
                }
            }
            Frame::Enter(ty, _) if super::is_admitted_resolved_scalar(&ty) => {}
            Frame::Enter(ty @ ResolvedType::Nominal { .. }, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return None;
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = &ty
                else {
                    unreachable!()
                };
                if !arguments.is_empty()
                    || declarations
                        .type_parameters(declaration)
                        .is_none_or(|parameters| !parameters.is_empty())
                    || declarations.declaration(declaration)?.kind != hir::DeclarationKind::Record
                    || !active.insert(declaration.clone())
                {
                    return None;
                }
                profile.has_nested_record |= depth > 1;
                let fields = declarations.record_fields(declaration)?;
                visited_fields = visited_fields.checked_add(fields.len())?;
                if visited_fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return None;
                }
                pending.push(Frame::Leave(declaration.clone()));
                for field in fields.iter().rev() {
                    pending.push(Frame::Enter(field.ty.clone(), depth + 1));
                }
            }
            Frame::Enter(_, _) => return None,
            Frame::Leave(declaration) => {
                active.remove(&declaration);
            }
        }
    }
    Some(profile)
}

fn pattern_is_exact(
    declarations: &hir::DeclarationIndex,
    mode: hir::ResolvedMatchMode,
    ty: &ResolvedType,
    pattern: &hir::ResolvedMatchPattern,
    active: &mut BTreeSet<hir::DeclarationId>,
) -> bool {
    let ResolvedType::Nominal { declaration, .. } = ty else {
        return false;
    };
    let hir::ResolvedMatchPattern::Record {
        record,
        instance,
        fields,
    } = pattern
    else {
        return false;
    };
    let Some(declared_fields) = declarations.record_fields(declaration) else {
        return false;
    };
    if record != declaration
        || instance != ty
        || fields.len() != declared_fields.len()
        || !active.insert(declaration.clone())
    {
        return false;
    }
    let mut seen = BTreeSet::new();
    let valid = fields.iter().all(|field| {
        let Some(declared) = declared_fields
            .iter()
            .find(|candidate| candidate.id == field.field)
        else {
            return false;
        };
        if !seen.insert(field.field.clone()) {
            return false;
        }
        match &field.pattern {
            hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                let ownership = if declared.ty == ResolvedType::Bytes
                    || classify_record(declarations, &declared.ty)
                        .is_some_and(|profile| profile.has_bytes)
                {
                    match mode {
                        hir::ResolvedMatchMode::Own => hir::OwnershipMode::Own,
                        hir::ResolvedMatchMode::Borrow => hir::OwnershipMode::Borrow,
                        hir::ResolvedMatchMode::Value => return false,
                    }
                } else {
                    hir::OwnershipMode::Value
                };
                binding.ty == declared.ty && binding.ownership == ownership
            }
            hir::ResolvedRecordMatchFieldPattern::Wildcard => {
                mode == hir::ResolvedMatchMode::Borrow
                    || (declared.ty != ResolvedType::Bytes
                        && !classify_record(declarations, &declared.ty)
                            .is_some_and(|profile| profile.has_bytes))
            }
            hir::ResolvedRecordMatchFieldPattern::Record { .. } => false,
        }
    });
    active.remove(declaration);
    valid
}

fn take_at(value: &mut Value, projections: &[hir::PlaceProjection]) -> Option<Value> {
    let mut current = value;
    for projection in projections {
        let hir::PlaceProjection::Field(field) = projection else {
            return None;
        };
        let Value::Record(record) = current else {
            return None;
        };
        current = Arc::get_mut(record)?.fields.get_mut(field)?;
    }
    (!matches!(current, Value::Moved)).then(|| std::mem::replace(current, Value::Moved))
}

fn value_needs_drop(value: &Value) -> bool {
    let mut pending = vec![value];
    while let Some(value) = pending.pop() {
        match value {
            Value::Bytes(_) => return true,
            Value::Record(record) => pending.extend(record.fields.values()),
            _ => {}
        }
    }
    false
}

fn borrow_alias(value: &Value) -> Result<Value, Flow> {
    Ok(match value {
        Value::Int(value) => Value::Int(*value),
        Value::Int32(value) => Value::Int32(*value),
        Value::Uint8(value) => Value::Uint8(*value),
        Value::Usize(value) => Value::Usize(*value),
        Value::Char(value) => Value::Char(*value),
        Value::Float32(value) => Value::Float32(*value),
        Value::Float64(value) => Value::Float64(*value),
        Value::Bool(value) => Value::Bool(*value),
        Value::Bytes(value) => Value::Bytes(value.clone()),
        Value::Record(value) => Value::Record(Arc::clone(value)),
        _ => {
            return Err(Flow::Guard(
                "borrowed nested record contains a closed value kind",
            ))
        }
    })
}
