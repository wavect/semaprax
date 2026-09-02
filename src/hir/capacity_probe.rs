//! Test-only owned-capacity probes.
//!
//! Measures the bounded heap a resolver or validator phase retains so
//! the private capacity contract stays executable.

use std::collections::BTreeMap;

use super::expr_nodes::{
    ResolvedExpr, ResolvedExprKind, ResolvedFieldInitializer, ResolvedMatchArm,
    ResolvedMatchPattern, ResolvedMatchPatternField, ResolvedRecordMatchFieldPattern,
    ResolvedRecordMatchPatternField, ResolvedStatement,
};
use super::ids::ValueId;
use super::nodes::{
    ResolvedBinding, ResolvedFieldDeclaration, ResolvedType, ResolvedVariantCaseDeclaration,
};
use super::{
    Availability, Binding, Place, PlaceProjection, ValidationBinding,
    ITERATIVE_PHASE_CAPACITY_HIGH_WATER, TYPE_FACTS_OUTER_BASELINE,
};

#[cfg(test)]
pub(crate) fn reset_iterative_phase_capacity_high_water() {
    ITERATIVE_PHASE_CAPACITY_HIGH_WATER.with(|water| water.set([0; 3]));
}

#[cfg(test)]
pub(crate) fn iterative_phase_capacity_high_water() -> [usize; 3] {
    ITERATIVE_PHASE_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn note_iterative_phase_capacity(index: usize, bytes: usize) {
    ITERATIVE_PHASE_CAPACITY_HIGH_WATER.with(|water| {
        let mut values = water.get();
        values[index] = values[index].max(bytes);
        water.set(values);
    });
}

#[cfg(test)]
pub(super) fn type_facts_outer_baseline() -> usize {
    TYPE_FACTS_OUTER_BASELINE.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn validation_scope_owned_capacity(
    scope: &BTreeMap<ValueId, ValidationBinding>,
) -> usize {
    let node_bytes = scope.len().saturating_mul(
        std::mem::size_of::<(ValueId, ValidationBinding)>()
            + std::mem::size_of::<BTreeMap<ValueId, ValidationBinding>>(),
    );
    node_bytes
        + scope.iter().fold(0usize, |bytes, (id, binding)| {
            let moved = binding
                .moved_places
                .iter()
                .fold(0usize, |bytes, (place, _)| {
                    bytes
                        + std::mem::size_of::<(Vec<PlaceProjection>, Availability)>()
                        + place.capacity() * std::mem::size_of::<PlaceProjection>()
                        + place
                            .iter()
                            .map(place_projection_owned_capacity)
                            .sum::<usize>()
                });
            let partial = binding
                .definitely_partial
                .iter()
                .fold(0usize, |bytes, place| {
                    bytes
                        + std::mem::size_of::<Vec<PlaceProjection>>()
                        + place.capacity() * std::mem::size_of::<PlaceProjection>()
                        + place
                            .iter()
                            .map(place_projection_owned_capacity)
                            .sum::<usize>()
                });
            bytes + id.as_str().len() + resolved_type_owned_capacity(&binding.ty) + moved + partial
        })
}

#[cfg(test)]
pub(super) fn place_projection_owned_capacity(projection: &PlaceProjection) -> usize {
    match projection {
        PlaceProjection::Field(field) => field.as_str().len(),
        PlaceProjection::VariantField { case, field } => {
            case.as_str().len().saturating_add(field.as_str().len())
        }
    }
}

#[cfg(test)]
pub(super) fn resolved_type_owned_capacity(ty: &ResolvedType) -> usize {
    match ty {
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool => 0,
        ResolvedType::String | ResolvedType::Bytes | ResolvedType::Str | ResolvedType::SliceU8 => 0,
        ResolvedType::TypeParameter { owner, .. } => owner.as_str().len(),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => declaration
            .as_str()
            .len()
            .saturating_add(arguments.capacity() * std::mem::size_of::<ResolvedType>())
            .saturating_add(
                arguments
                    .iter()
                    .map(resolved_type_owned_capacity)
                    .sum::<usize>(),
            ),
    }
}

#[cfg(test)]
pub(super) fn resolved_place_owned_capacity(place: &Place) -> usize {
    place.root.as_str().len()
        + place.projections.capacity() * std::mem::size_of::<PlaceProjection>()
        + place
            .projections
            .iter()
            .map(place_projection_owned_capacity)
            .sum::<usize>()
}

#[cfg(test)]
pub(super) fn resolved_expr_owned_capacity(expression: &ResolvedExpr) -> usize {
    let mut bytes = expression
        .id
        .as_str()
        .len()
        .saturating_add(resolved_type_owned_capacity(&expression.ty));
    let child = |value: &ResolvedExpr| {
        std::mem::size_of::<ResolvedExpr>().saturating_add(resolved_expr_owned_capacity(value))
    };
    match &expression.kind {
        ResolvedExprKind::Place(place) => bytes += resolved_place_owned_capacity(place),
        ResolvedExprKind::BorrowPlace { operation, place } => {
            bytes += operation.as_str().len() + resolved_place_owned_capacity(place);
        }
        ResolvedExprKind::ByteRange {
            operation,
            source,
            start,
            end,
        } => {
            bytes += operation.as_str().len() + child(source) + child(start) + child(end);
        }
        ResolvedExprKind::ArrayU8(values) => bytes += values.capacity(),
        ResolvedExprKind::Unary { value: operand, .. } => bytes += child(operand),
        ResolvedExprKind::Upcast { source } => bytes += child(source),
        ResolvedExprKind::Project { base, field } => {
            bytes += child(base) + field.as_str().len();
        }
        ResolvedExprKind::Binary { left, right, .. } => bytes += child(left) + child(right),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => bytes += child(condition) + child(then_branch) + child(else_branch),
        ResolvedExprKind::Call {
            callee,
            args,
            type_arguments,
            instance,
        } => {
            bytes += callee.as_str().len();
            bytes += instance.as_ref().map_or(0, |id| id.as_str().len());
            bytes += args.capacity() * std::mem::size_of::<ResolvedExpr>();
            bytes += args.iter().map(resolved_expr_owned_capacity).sum::<usize>();
            bytes += type_arguments.capacity() * std::mem::size_of::<ResolvedType>();
            bytes += type_arguments
                .iter()
                .map(resolved_type_owned_capacity)
                .sum::<usize>();
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            bytes += call.expression.as_str().len() + call.import.as_str().len();
            bytes += call.args.capacity() * std::mem::size_of::<ResolvedExpr>();
            bytes += call
                .args
                .iter()
                .map(resolved_expr_owned_capacity)
                .sum::<usize>();
        }
        ResolvedExprKind::HostCommandCall(call) => {
            bytes += call.expression.as_str().len();
            bytes += call.args.capacity() * std::mem::size_of::<ResolvedExpr>();
            bytes += call
                .args
                .iter()
                .map(resolved_expr_owned_capacity)
                .sum::<usize>();
        }
        ResolvedExprKind::Try {
            operand,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            residual_type,
        } => {
            bytes += child(operand)
                + result.as_str().len()
                + ok_case.as_str().len()
                + ok_field.as_str().len()
                + err_case.as_str().len()
                + err_field.as_str().len()
                + resolved_type_owned_capacity(residual_type);
        }
        ResolvedExprKind::TryOption {
            operand,
            option,
            some_case,
            some_field,
            none_case,
            residual_type,
        } => {
            bytes += child(operand)
                + option.as_str().len()
                + some_case.as_str().len()
                + some_field.as_str().len()
                + none_case.as_str().len()
                + resolved_type_owned_capacity(residual_type);
        }
        ResolvedExprKind::Block { statements, tail } => {
            bytes += statements.capacity() * std::mem::size_of::<ResolvedStatement>();
            for statement in statements {
                if let ResolvedStatement::Let { binding, value, .. } = statement {
                    bytes += binding.id.as_str().len()
                        + binding.name.capacity()
                        + resolved_type_owned_capacity(&binding.ty)
                        + resolved_expr_owned_capacity(value);
                } else {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            bytes += resolved_expr_owned_capacity(child);
                        }
                    }
                }
            }
            bytes += child(tail);
        }
        ResolvedExprKind::ConstructRecord { record, fields } => {
            bytes += record.as_str().len();
            bytes += fields.capacity() * std::mem::size_of::<ResolvedFieldInitializer>();
            bytes += fields
                .iter()
                .map(|field| {
                    field.field.as_str().len() + resolved_expr_owned_capacity(&field.value)
                })
                .sum::<usize>();
        }
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => {
            bytes += variant.as_str().len() + case.as_str().len();
            bytes += fields.capacity() * std::mem::size_of::<ResolvedFieldInitializer>();
            bytes += fields
                .iter()
                .map(|field| {
                    field.field.as_str().len() + resolved_expr_owned_capacity(&field.value)
                })
                .sum::<usize>();
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            bytes += child(scrutinee);
            bytes += arms.capacity() * std::mem::size_of::<ResolvedMatchArm>();
            bytes += arms
                .iter()
                .map(|arm| {
                    resolved_match_pattern_owned_capacity(&arm.pattern)
                        + arm.guard.as_ref().map_or(0, |guard| child(guard))
                        + resolved_expr_owned_capacity(&arm.value)
                })
                .sum::<usize>();
        }
        ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } => {
            bytes += child(base) + record.as_str().len();
            bytes += fields.capacity() * std::mem::size_of::<ResolvedFieldInitializer>();
            bytes += fields
                .iter()
                .map(|field| {
                    field.field.as_str().len() + resolved_expr_owned_capacity(&field.value)
                })
                .sum::<usize>();
        }
        ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_) => {}
    }
    bytes
}

#[cfg(test)]
pub(super) fn resolved_binding_owned_capacity(binding: &ResolvedBinding) -> usize {
    binding.id.as_str().len() + binding.name.capacity() + resolved_type_owned_capacity(&binding.ty)
}

#[cfg(test)]
pub(super) fn resolved_record_pattern_field_owned_capacity(
    field: &ResolvedRecordMatchPatternField,
) -> usize {
    field.field.as_str().len()
        + match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => {
                resolved_binding_owned_capacity(binding)
            }
            ResolvedRecordMatchFieldPattern::Wildcard => 0,
            ResolvedRecordMatchFieldPattern::Record {
                record,
                instance,
                fields,
            } => {
                record.as_str().len()
                    + resolved_type_owned_capacity(instance)
                    + fields.capacity() * std::mem::size_of::<ResolvedRecordMatchPatternField>()
                    + fields
                        .iter()
                        .map(resolved_record_pattern_field_owned_capacity)
                        .sum::<usize>()
            }
        }
}

#[cfg(test)]
pub(super) fn resolved_match_pattern_owned_capacity(pattern: &ResolvedMatchPattern) -> usize {
    match pattern {
        ResolvedMatchPattern::Wildcard => 0,
        ResolvedMatchPattern::Literal(_) => 0,
        ResolvedMatchPattern::Binding(binding) => resolved_binding_owned_capacity(binding),
        ResolvedMatchPattern::Or(alternatives) => {
            alternatives.capacity() * std::mem::size_of::<ResolvedMatchPattern>()
                + alternatives
                    .iter()
                    .map(resolved_match_pattern_owned_capacity)
                    .sum::<usize>()
        }
        ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => {
            variant.as_str().len()
                + case.as_str().len()
                + fields.capacity() * std::mem::size_of::<ResolvedMatchPatternField>()
                + fields
                    .iter()
                    .map(|field| {
                        field.field.as_str().len() + resolved_binding_owned_capacity(&field.binding)
                    })
                    .sum::<usize>()
        }
        ResolvedMatchPattern::Record {
            record,
            instance,
            fields,
        } => {
            record.as_str().len()
                + resolved_type_owned_capacity(instance)
                + fields.capacity() * std::mem::size_of::<ResolvedRecordMatchPatternField>()
                + fields
                    .iter()
                    .map(resolved_record_pattern_field_owned_capacity)
                    .sum::<usize>()
        }
    }
}

#[cfg(test)]
pub(super) fn resolved_statement_owned_capacity(statement: &ResolvedStatement) -> usize {
    match statement {
        ResolvedStatement::Let { .. } | ResolvedStatement::Assign { .. } => {
            resolved_binding_owned_capacity(statement.binding())
                + resolved_expr_owned_capacity(statement.value())
        }
        // Unsafe boundaries carry only the verbatim audit summary plus their
        // ordinary block body.
        ResolvedStatement::Unsafe { audit, body, .. } => {
            audit.capacity() + resolved_expr_owned_capacity(body)
        }
        // While loops carry their condition plus their ordinary block body.
        ResolvedStatement::While {
            condition, body, ..
        } => resolved_expr_owned_capacity(condition) + resolved_expr_owned_capacity(body),
    }
}

#[cfg(test)]
pub(super) fn resolved_field_initializer_owned_capacity(field: &ResolvedFieldInitializer) -> usize {
    field.field.as_str().len() + resolved_expr_owned_capacity(&field.value)
}

#[cfg(test)]
pub(super) fn resolved_match_arm_owned_capacity(arm: &ResolvedMatchArm) -> usize {
    resolved_match_pattern_owned_capacity(&arm.pattern)
        + arm
            .guard
            .as_ref()
            .map_or(0, |guard| resolved_expr_owned_capacity(guard))
        + resolved_expr_owned_capacity(&arm.value)
}

#[cfg(test)]
pub(super) fn resolved_field_declaration_owned_capacity(field: &ResolvedFieldDeclaration) -> usize {
    field.id.as_str().len() + field.name.capacity() + resolved_type_owned_capacity(&field.ty)
}

#[cfg(test)]
pub(super) fn resolved_variant_case_owned_capacity(case: &ResolvedVariantCaseDeclaration) -> usize {
    case.id.as_str().len()
        + case.name.capacity()
        + case.fields.capacity() * std::mem::size_of::<ResolvedFieldDeclaration>()
        + case
            .fields
            .iter()
            .map(resolved_field_declaration_owned_capacity)
            .sum::<usize>()
}

#[cfg(test)]
pub(super) fn resolver_scope_owned_capacity(scope: &BTreeMap<String, Binding>) -> usize {
    scope
        .len()
        .saturating_mul(
            std::mem::size_of::<(String, Binding)>()
                + std::mem::size_of::<BTreeMap<String, Binding>>(),
        )
        .saturating_add(
            scope
                .iter()
                .map(|(name, binding)| {
                    name.capacity()
                        + binding.id.as_str().len()
                        + resolved_type_owned_capacity(&binding.ty)
                })
                .sum::<usize>(),
        )
}
