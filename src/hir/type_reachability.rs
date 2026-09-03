//! Exact nominal-type closure for an already selected HIR function inventory.
//!
//! This is structural retention, not public-ABI admission.  It deliberately
//! walks contracts and every nested expression/pattern type so target and
//! cleanup consumers never receive a declaration set inferred from signatures
//! alone.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use super::*;

pub(crate) fn reachable_authored_types(
    functions: &[LinkedScalarFunction],
    interfaces: &[ResolvedInterface],
    available: &BTreeMap<DeclarationId, ResolvedTypeDeclaration>,
) -> Result<Vec<ResolvedTypeDeclaration>, Diagnostic> {
    let mut selected = BTreeSet::new();
    for linked in functions {
        collect_function(&linked.function, &mut selected);
    }
    for interface in interfaces {
        for import in &interface.imports {
            for parameter in &import.parameters {
                collect_type(&parameter.ty, &mut selected);
            }
        }
    }

    let mut pending = selected.iter().cloned().collect::<VecDeque<_>>();
    while let Some(id) = pending.pop_front() {
        if crate::prelude::is_compiler_owned_id(id.as_str()) {
            continue;
        }
        let declaration = available.get(&id).ok_or_else(|| {
            Diagnostic::io(
                "SPX-G173",
                format!("owned-data closure references unknown type `{id}`"),
            )
        })?;
        let fields = match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields } => fields.iter().collect::<Vec<_>>(),
            ResolvedTypeDeclarationKind::Variant { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            ResolvedTypeDeclarationKind::Class { .. }
            | ResolvedTypeDeclarationKind::Resource { .. } => {
                return Err(Diagnostic::io(
                    "SPX-G172",
                    format!(
                        "owned-data closure type `{id}` is outside the shared record/variant target profile"
                    ),
                ));
            }
        };
        for field in fields {
            let mut referenced = BTreeSet::new();
            collect_type(&field.ty, &mut referenced);
            for referenced_id in referenced {
                if selected.insert(referenced_id.clone()) {
                    pending.push_back(referenced_id);
                }
            }
        }
    }

    selected
        .into_iter()
        .filter(|id| !crate::prelude::is_compiler_owned_id(id.as_str()))
        .map(|id| {
            available.get(&id).cloned().ok_or_else(|| {
                Diagnostic::io(
                    "SPX-G173",
                    format!("owned-data type closure lost declaration `{id}`"),
                )
            })
        })
        .collect()
}

fn collect_function(function: &ResolvedFunction, declarations: &mut BTreeSet<DeclarationId>) {
    for parameter in &function.params {
        collect_type(&parameter.ty, declarations);
    }
    collect_type(&function.return_type, declarations);
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_expression(expression, declarations);
    }
}

fn collect_expression(expression: &ResolvedExpr, declarations: &mut BTreeSet<DeclarationId>) {
    collect_type(&expression.ty, declarations);
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            collect_expression(source, declarations);
            collect_expression(start, declarations);
            collect_expression(end, declarations);
        }
        ResolvedExprKind::Call {
            type_arguments,
            args,
            ..
        } => {
            for ty in type_arguments {
                collect_type(ty, declarations);
            }
            for argument in args {
                collect_expression(argument, declarations);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for argument in &call.args {
                collect_expression(argument, declarations);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for argument in &call.args {
                collect_expression(argument, declarations);
            }
        }
        ResolvedExprKind::Unary { value, .. } | ResolvedExprKind::Upcast { source: value } => {
            collect_expression(value, declarations);
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_expression(left, declarations);
            collect_expression(right, declarations);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                if let ResolvedStatement::Let { binding, .. }
                | ResolvedStatement::Assign { binding, .. } = statement
                {
                    collect_type(&binding.ty, declarations);
                }
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        collect_expression(child, declarations);
                    }
                }
            }
            collect_expression(tail, declarations);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_expression(condition, declarations);
            collect_expression(then_branch, declarations);
            collect_expression(else_branch, declarations);
        }
        ResolvedExprKind::ConstructRecord { record, fields }
        | ResolvedExprKind::UpdateRecord { record, fields, .. } => {
            declarations.insert(record.clone());
            for field in fields {
                collect_expression(&field.value, declarations);
            }
            if let ResolvedExprKind::UpdateRecord { base, .. } = &expression.kind {
                collect_expression(base, declarations);
            }
        }
        ResolvedExprKind::ConstructVariant {
            variant, fields, ..
        } => {
            declarations.insert(variant.clone());
            for field in fields {
                collect_expression(&field.value, declarations);
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            collect_expression(scrutinee, declarations);
            for arm in arms {
                collect_pattern(&arm.pattern, declarations);
                if let Some(guard) = &arm.guard {
                    collect_expression(guard, declarations);
                }
                collect_expression(&arm.value, declarations);
            }
        }
        ResolvedExprKind::Try {
            operand,
            result,
            residual_type,
            ..
        } => {
            declarations.insert(result.clone());
            collect_type(residual_type, declarations);
            collect_expression(operand, declarations);
        }
        ResolvedExprKind::TryOption {
            operand,
            option,
            residual_type,
            ..
        } => {
            declarations.insert(option.clone());
            collect_type(residual_type, declarations);
            collect_expression(operand, declarations);
        }
        ResolvedExprKind::Project { base, .. } => collect_expression(base, declarations),
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

fn collect_pattern(pattern: &ResolvedMatchPattern, declarations: &mut BTreeSet<DeclarationId>) {
    match pattern {
        ResolvedMatchPattern::Variant {
            variant, fields, ..
        } => {
            declarations.insert(variant.clone());
            for field in fields {
                collect_type(&field.binding.ty, declarations);
            }
        }
        ResolvedMatchPattern::Record {
            record,
            instance,
            fields,
        } => {
            declarations.insert(record.clone());
            collect_type(instance, declarations);
            collect_record_pattern(fields, declarations);
        }
        ResolvedMatchPattern::Binding(binding) => collect_type(&binding.ty, declarations),
        ResolvedMatchPattern::Or(items) => {
            for item in items {
                collect_pattern(item, declarations);
            }
        }
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => {}
    }
}

fn collect_record_pattern(
    fields: &[ResolvedRecordMatchPatternField],
    declarations: &mut BTreeSet<DeclarationId>,
) {
    for field in fields {
        match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => {
                collect_type(&binding.ty, declarations);
            }
            ResolvedRecordMatchFieldPattern::Record {
                record,
                instance,
                fields,
            } => {
                declarations.insert(record.clone());
                collect_type(instance, declarations);
                collect_record_pattern(fields, declarations);
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {}
        }
    }
}

fn collect_type(ty: &ResolvedType, declarations: &mut BTreeSet<DeclarationId>) {
    if let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    {
        declarations.insert(declaration.clone());
        for argument in arguments {
            collect_type(argument, declarations);
        }
    }
}

const MAX_NESTED_OWNED_RECORD_DEPTH: usize = 64;
const MAX_NESTED_OWNED_BYTE_LEAVES: usize = 256;
const MAX_NESTED_OWNED_RECORD_FIELDS: usize = 4_096;

struct NestedOwnedRecordFacts {
    /// Declaration-stable paths to every owned byte leaf, in declaration
    /// order. HIR admission therefore never depends on display names.
    byte_paths: Vec<Vec<PlaceProjection>>,
    visited_fields: usize,
}

enum NestedOwnedRecordAdmission {
    Admitted(NestedOwnedRecordFacts),
    NoOwnedBytes,
    OutsideProfile,
    Recursive,
    LimitExceeded,
}

pub(super) fn nested_record_copy_scalar_is_admitted(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
    )
}

/// Re-derive the nested record profile only from the declaration index. This
/// deliberately does not reuse source admission or cached `TypeFacts`.
fn classify_nested_owned_byte_record(
    declarations: &DeclarationIndex,
    root: &ResolvedType,
) -> NestedOwnedRecordAdmission {
    enum Frame<'a> {
        Type(&'a ResolvedType, usize),
        Fields(&'a [ResolvedFieldDeclaration], usize, usize),
        LeaveRecord(String),
        LeaveField,
    }

    let mut frames = vec![Frame::Type(root, 1)];
    let mut active = BTreeSet::new();
    let mut path = Vec::new();
    let mut byte_paths = Vec::new();
    let mut visited_fields = 0usize;

    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Type(ResolvedType::Bytes, _) => {
                byte_paths.push(path.clone());
                if byte_paths.len() > MAX_NESTED_OWNED_BYTE_LEAVES {
                    return NestedOwnedRecordAdmission::LimitExceeded;
                }
            }
            Frame::Type(ty, _) if nested_record_copy_scalar_is_admitted(ty) => {}
            Frame::Type(
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                },
                depth,
            ) => {
                if depth > MAX_NESTED_OWNED_RECORD_DEPTH {
                    return NestedOwnedRecordAdmission::LimitExceeded;
                }
                if !arguments.is_empty()
                    || declarations
                        .type_parameters(declaration)
                        .is_none_or(|parameters| !parameters.is_empty())
                    || declarations
                        .declaration(declaration)
                        .is_none_or(|item| item.kind != DeclarationKind::Record)
                {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                }
                let Some(fields) = declarations.record_fields(declaration) else {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                };
                let identity = declaration.as_str().to_owned();
                if !active.insert(identity.clone()) {
                    return NestedOwnedRecordAdmission::Recursive;
                }
                frames.push(Frame::LeaveRecord(identity));
                frames.push(Frame::Fields(fields, 0, depth));
            }
            Frame::Type(
                ResolvedType::Unit
                | ResolvedType::ArrayU8(_)
                | ResolvedType::String
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::TypeParameter { .. },
                _,
            ) => return NestedOwnedRecordAdmission::OutsideProfile,
            Frame::Type(
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
                _,
            ) => unreachable!("admitted scalar handled above"),
            Frame::Fields(fields, index, depth) => {
                let Some(field) = fields.get(index) else {
                    continue;
                };
                visited_fields += 1;
                if visited_fields > MAX_NESTED_OWNED_RECORD_FIELDS {
                    return NestedOwnedRecordAdmission::LimitExceeded;
                }
                frames.push(Frame::Fields(fields, index + 1, depth));
                path.push(PlaceProjection::Field(field.id.clone()));
                frames.push(Frame::LeaveField);
                frames.push(Frame::Type(&field.ty, depth + 1));
            }
            Frame::LeaveRecord(identity) => {
                active.remove(&identity);
            }
            Frame::LeaveField => {
                path.pop();
            }
        }
    }

    if byte_paths.is_empty() {
        NestedOwnedRecordAdmission::NoOwnedBytes
    } else {
        NestedOwnedRecordAdmission::Admitted(NestedOwnedRecordFacts {
            byte_paths,
            visited_fields,
        })
    }
}

pub(super) fn is_admitted_nested_owned_byte_record(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    match classify_nested_owned_byte_record(declarations, ty) {
        NestedOwnedRecordAdmission::Admitted(facts) => {
            !facts.byte_paths.is_empty() && facts.visited_fields <= MAX_NESTED_OWNED_RECORD_FIELDS
        }
        NestedOwnedRecordAdmission::NoOwnedBytes
        | NestedOwnedRecordAdmission::OutsideProfile
        | NestedOwnedRecordAdmission::Recursive
        | NestedOwnedRecordAdmission::LimitExceeded => false,
    }
}

pub(super) fn is_flat_owned_byte_record(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    arguments.is_empty()
        && declarations
            .record_fields(declaration)
            .is_some_and(|fields| {
                fields.iter().any(|field| field.ty == ResolvedType::Bytes)
                    && fields.iter().all(|field| {
                        field.ty == ResolvedType::Bytes
                            || nested_record_copy_scalar_is_admitted(&field.ty)
                    })
            })
}

pub(super) fn is_nested_nonflat_owned_byte_record(
    declarations: &DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    is_admitted_nested_owned_byte_record(declarations, ty)
        && !is_flat_owned_byte_record(declarations, ty)
}
