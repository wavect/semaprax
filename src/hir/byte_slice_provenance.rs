//! Symbolic byte-slice provenance derivation.
//!
//! Resolves every byte-slice value back to its canonical authenticated
//! root before any consumer may trust a view.

use std::collections::{BTreeMap, BTreeSet};

use crate::diagnostic::Diagnostic;

use super::expr_nodes::{ResolvedExpr, ResolvedExprKind, ResolvedStatement};
use super::ids::ValueId;
use super::nodes::{
    ByteSliceExtent, ByteSliceProvenance, ByteSliceRangeStep, ByteSliceRootKind, OwnershipMode,
    ResolvedBinding, ResolvedFunction, ResolvedHostCommandCall, ResolvedHostCommandOperation,
    ResolvedType,
};
use super::{hir_error, DeclarationIndex, PlaceProjection};

fn projected_field_type(
    declarations: &DeclarationIndex,
    root: &ResolvedType,
    projections: &[PlaceProjection],
) -> Option<ResolvedType> {
    let mut ty = root.clone();
    for projection in projections {
        let PlaceProjection::Field(field) = projection else {
            return None;
        };
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &ty
        else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }
        ty = declarations
            .record_fields(declaration)?
            .iter()
            .find(|candidate| candidate.id == *field)?
            .ty
            .clone();
    }
    Some(ty)
}

pub(super) fn derive_byte_slice_provenance(
    functions: &[ResolvedFunction],
    declarations: &DeclarationIndex,
) -> Result<BTreeMap<ValueId, ByteSliceProvenance>, Diagnostic> {
    let mut facts = BTreeMap::new();
    let mut root_types = BTreeMap::<ValueId, ResolvedType>::new();
    let command_argument_root =
        ValueId::intrinsic_parameter(crate::command_io_ops::ARG_UTF8_ID, usize::MAX);
    let mut command_argument_views = BTreeSet::<ValueId>::new();
    let mut aliases = Vec::<(&ResolvedBinding, bool, &ResolvedExpr)>::new();
    for function in functions {
        for parameter in &function.params {
            root_types.insert(parameter.id.clone(), parameter.ty.clone());
            if parameter.ty == ResolvedType::SliceU8 {
                facts.insert(
                    parameter.id.clone(),
                    ByteSliceProvenance {
                        root: parameter.id.clone(),
                        projections: Vec::new(),
                        projected_type: ResolvedType::SliceU8,
                        root_kind: ByteSliceRootKind::FunctionParameter,
                        root_length: ByteSliceExtent::ParameterLength,
                        offset: ByteSliceExtent::Constant(0),
                        length: ByteSliceExtent::ParameterLength,
                        producer: None,
                        ranges: Vec::new(),
                    },
                );
            }
        }
        let mut pending = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expression) = pending.pop() {
            match &expression.kind {
                ResolvedExprKind::Call { args, .. } => pending.extend(args),
                ResolvedExprKind::ByteRange {
                    source, start, end, ..
                } => {
                    pending.push(source);
                    pending.push(start);
                    pending.push(end);
                }
                ResolvedExprKind::NativeRustImportCall(call) => pending.extend(&call.args),
                ResolvedExprKind::HostCommandCall(call) => pending.extend(&call.args),
                ResolvedExprKind::Unary { value, .. }
                | ResolvedExprKind::Try { operand: value, .. }
                | ResolvedExprKind::TryOption { operand: value, .. }
                | ResolvedExprKind::Project { base: value, .. }
                | ResolvedExprKind::Upcast { source: value } => pending.push(value),
                ResolvedExprKind::Binary { left, right, .. } => {
                    pending.push(left);
                    pending.push(right);
                }
                ResolvedExprKind::Block { statements, tail } => {
                    pending.push(tail);
                    for statement in statements {
                        if let ResolvedStatement::Let {
                            binding,
                            mutable,
                            value,
                            ..
                        } = statement
                        {
                            root_types.insert(binding.id.clone(), binding.ty.clone());
                            if matches!(
                                &value.kind,
                                ResolvedExprKind::HostCommandCall(ResolvedHostCommandCall {
                                    operation: ResolvedHostCommandOperation::ArgUtf8,
                                    ..
                                })
                            ) {
                                command_argument_views.insert(binding.id.clone());
                            }
                            if binding.ty == ResolvedType::SliceU8 {
                                aliases.push((binding, *mutable, value));
                            }
                        }
                        for index in 0..statement.child_count() {
                            pending.push(
                                statement
                                    .child(index)
                                    .expect("statement child count is canonical"),
                            );
                        }
                    }
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    pending.push(condition);
                    pending.push(then_branch);
                    pending.push(else_branch);
                }
                ResolvedExprKind::ConstructRecord { fields, .. }
                | ResolvedExprKind::ConstructVariant { fields, .. } => {
                    pending.extend(fields.iter().map(|field| &field.value));
                }
                ResolvedExprKind::Match {
                    scrutinee, arms, ..
                } => {
                    pending.push(scrutinee);
                    for arm in arms {
                        if let Some(guard) = &arm.guard {
                            pending.push(guard);
                        }
                        pending.push(&arm.value);
                    }
                }
                ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                    pending.push(base);
                    pending.extend(fields.iter().map(|field| &field.value));
                }
                ResolvedExprKind::Int(_)
                | ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::ArrayU8(_)
                | ResolvedExprKind::RepeatArrayU8 { .. }
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::Bool(_)
                | ResolvedExprKind::String(_)
                | ResolvedExprKind::Place(_)
                | ResolvedExprKind::BorrowPlace { .. } => {}
            }
        }
    }
    let mut unresolved = aliases;
    loop {
        let before = unresolved.len();
        unresolved.retain(|(binding, mutable, value)| {
            if let ResolvedExprKind::BorrowPlace { operation, place } = &value.kind {
                if *mutable {
                    return true;
                }
                let Some(root_ty) = root_types.get(&place.root) else {
                    return true;
                };
                let (root_kind, root_length, projected_type) = match root_ty {
                    ResolvedType::Bytes => (
                        ByteSliceRootKind::OwnedBytes,
                        ByteSliceExtent::ValueLength,
                        ResolvedType::Bytes,
                    ),
                    ResolvedType::ArrayU8(length) => (
                        ByteSliceRootKind::FixedArray,
                        ByteSliceExtent::Constant(u64::from(*length)),
                        root_ty.clone(),
                    ),
                    ResolvedType::Str => {
                        if command_argument_views.contains(&place.root) {
                            (
                                ByteSliceRootKind::CommandArguments,
                                ByteSliceExtent::ValueLength,
                                ResolvedType::Str,
                            )
                        } else {
                            (
                                ByteSliceRootKind::BorrowedStr,
                                ByteSliceExtent::ValueLength,
                                ResolvedType::Str,
                            )
                        }
                    }
                    ResolvedType::Nominal { arguments, .. }
                        if arguments.is_empty() && !place.projections.is_empty() =>
                    {
                        if projected_field_type(declarations, root_ty, &place.projections)
                            != Some(ResolvedType::Bytes)
                        {
                            return true;
                        }
                        (
                            ByteSliceRootKind::OwnedBytes,
                            ByteSliceExtent::ValueLength,
                            ResolvedType::Bytes,
                        )
                    }
                    _ => return true,
                };
                if place.projections.is_empty() != !matches!(root_ty, ResolvedType::Nominal { .. })
                {
                    return true;
                }
                let expected_operation = match &projected_type {
                    ResolvedType::Bytes => crate::byte_ops::BYTES_AS_SLICE_ID,
                    ResolvedType::ArrayU8(_) => crate::byte_ops::ARRAY_AS_SLICE_ID,
                    ResolvedType::Str => crate::byte_ops::STR_AS_BYTES_ID,
                    _ => return true,
                };
                if operation.as_str() != expected_operation {
                    return true;
                }
                facts.insert(
                    binding.id.clone(),
                    ByteSliceProvenance {
                        root: if root_kind == ByteSliceRootKind::CommandArguments {
                            command_argument_root.clone()
                        } else {
                            place.root.clone()
                        },
                        projections: place.projections.clone(),
                        projected_type,
                        root_kind,
                        root_length,
                        offset: ByteSliceExtent::Constant(0),
                        length: root_length,
                        producer: Some(value.id.clone()),
                        ranges: Vec::new(),
                    },
                );
                return false;
            }
            if let ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } = &value.kind
            {
                if *mutable
                    || operation.as_str() != crate::byte_ops::RANGE_ID
                    || value.ty != ResolvedType::SliceU8
                    || value.ownership != OwnershipMode::Borrow
                    || start.ty != ResolvedType::Usize
                    || end.ty != ResolvedType::Usize
                    || start.ownership != OwnershipMode::Value
                    || end.ownership != OwnershipMode::Value
                {
                    return true;
                }
                let ResolvedExprKind::Place(place) = &source.kind else {
                    return true;
                };
                if !place.projections.is_empty()
                    || source.ty != ResolvedType::SliceU8
                    || source.ownership != OwnershipMode::Borrow
                {
                    return true;
                }
                let Some(mut provenance) = facts.get(&place.root).cloned() else {
                    return true;
                };
                if provenance.ranges.len() >= crate::byte_ops::MAX_RANGE_DEPTH {
                    return true;
                }
                provenance.producer = Some(value.id.clone());
                provenance.ranges.push(ByteSliceRangeStep {
                    source: place.root.clone(),
                    producer: value.id.clone(),
                    start: start.id.clone(),
                    end: end.id.clone(),
                });
                facts.insert(binding.id.clone(), provenance);
                return false;
            }
            let ResolvedExprKind::Place(place) = &value.kind else {
                return true;
            };
            if *mutable || !place.projections.is_empty() {
                return true;
            }
            let Some(source) = facts.get(&place.root).cloned() else {
                return true;
            };
            facts.insert(binding.id.clone(), source);
            false
        });
        if unresolved.is_empty() {
            return Ok(facts);
        }
        if unresolved.len() == before {
            return Err(hir_error(
                "byte-slice alias lacks a canonical symbolic parameter root",
            ));
        }
    }
}
