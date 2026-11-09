//! Complete identity audit order with private active-owner paid backing.
use super::*;
mod pending;
mod text;
use pending::Pending;
use text::{reject_nul_identity, reserve};

pub(super) fn validate_nul_free_identities(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    reserve(0)?;
    reject_nul_identity("resolved entry point", program.entrypoint.as_str())?;

    for (key, declaration) in &program.declarations.declarations {
        reject_nul_identity("declaration index key", key.as_str())?;
        reject_nul_identity(
            declaration_identity_subject(declaration.kind),
            declaration.id.as_str(),
        )?;
        if let Some(owner) = &declaration.owner {
            reject_nul_identity("resolved declaration owner", owner.as_str())?;
        }
    }
    for id in program.declarations.types_by_name.values() {
        reject_nul_identity("resolved type lookup", id.as_str())?;
    }
    for id in program.declarations.functions_by_name.values() {
        reject_nul_identity("resolved function lookup", id.as_str())?;
    }
    for ((owner, _), field) in &program.declarations.fields_by_owner_name {
        reject_nul_identity("resolved field owner lookup", owner.as_str())?;
        reject_nul_identity("resolved field lookup", field.as_str())?;
    }
    for ((owner, _), case) in &program.declarations.cases_by_owner_name {
        reject_nul_identity("resolved variant owner lookup", owner.as_str())?;
        reject_nul_identity("resolved variant case lookup", case.as_str())?;
    }
    for (owner, fields) in &program.declarations.record_fields {
        reject_nul_identity("resolved record-field owner", owner.as_str())?;
        for field in fields {
            reject_nul_identity("resolved field", field.id.as_str())?;
            audit_resolved_type(&field.ty)?;
        }
    }
    for (owner, cases) in &program.declarations.variant_cases {
        reject_nul_identity("resolved variant-case owner", owner.as_str())?;
        for case in cases {
            reject_nul_identity("resolved variant case", case.id.as_str())?;
            for field in &case.fields {
                reject_nul_identity("resolved case field", field.id.as_str())?;
                audit_resolved_type(&field.ty)?;
            }
        }
    }
    for (case, fields) in &program.declarations.case_fields {
        reject_nul_identity("resolved case-field owner", case.as_str())?;
        for field in fields {
            reject_nul_identity("resolved case field", field.id.as_str())?;
            audit_resolved_type(&field.ty)?;
        }
    }
    for (key, import) in &program.declarations.imports_by_key {
        reject_nul_identity("resolved logical import key", key)?;
        reject_nul_identity("resolved import lookup", import.as_str())?;
    }

    for declaration in &program.types {
        let subject = match declaration.kind {
            ResolvedTypeDeclarationKind::Resource { .. } => "resolved resource",
            ResolvedTypeDeclarationKind::Record { .. }
            | ResolvedTypeDeclarationKind::Class { .. } => "resolved record",
            ResolvedTypeDeclarationKind::Variant { .. } => "resolved variant",
        };
        reject_nul_identity(subject, declaration.id.as_str())?;
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                reject_nul_identity("resolved resource lifecycle", drop.id.as_str())?;
                if let ResolvedResourceDropKind::Imported { import, import_key } = &drop.kind {
                    reject_nul_identity("resolved lifecycle import", import.as_str())?;
                    reject_nul_identity("resolved lifecycle logical import key", import_key)?;
                }
            }
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    reject_nul_identity("resolved field", field.id.as_str())?;
                    audit_resolved_type(&field.ty)?;
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    reject_nul_identity("resolved variant case", case.id.as_str())?;
                    for field in &case.fields {
                        reject_nul_identity("resolved case field", field.id.as_str())?;
                        audit_resolved_type(&field.ty)?;
                    }
                }
            }
        }
    }
    for interface in &program.interfaces {
        reject_nul_identity("resolved interface", interface.id.as_str())?;
        for import in &interface.imports {
            reject_nul_identity("resolved import", import.id.as_str())?;
            reject_nul_identity("resolved import owner", import.interface.as_str())?;
            reject_nul_identity("resolved logical import key", &import.import_key)?;
            for parameter in &import.parameters {
                audit_resolved_type(&parameter.ty)?;
            }
        }
    }
    for function in &program.functions {
        reject_nul_identity("resolved function", function.id.as_str())?;
        for parameter in &function.params {
            reject_nul_identity("resolved value", parameter.id.as_str())?;
            audit_resolved_type(&parameter.ty)?;
        }
        reject_nul_identity("resolved value", function.result_id.as_str())?;
        audit_resolved_type(&function.return_type)?;
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            audit_resolved_expression(expression)?;
        }
    }
    Ok(())
}

/// Reject target-neutral attached metadata containing identities that cannot
/// cross C-string-backed backend and trace boundaries losslessly.
///
/// This is intentionally narrower than semantic inventory/plan validation so
/// independent replayers can call it without trusting either canonical builder.
pub(crate) fn validate_attached_identity_references(
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    reserve(0)?;
    for function in &program.functions {
        audit_cleanup_inventory(&function.cleanup)?;
        audit_cleanup_plan(&function.cleanup_plan)?;
    }
    Ok(())
}

fn audit_resolved_type(root: &ResolvedType) -> Result<(), Diagnostic> {
    // These exact leaves contain neither declaration identities nor child
    // types. The original walk allocated one worklist only to pop the root.
    match root {
        ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair
        | ResolvedType::MutFunctionI64
        | ResolvedType::Never
        | ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::ArrayU8(_)
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::String
        | ResolvedType::Bytes
        | ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::StringMap => return Ok(()),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if arguments.is_empty() => {
            return reject_nul_identity("resolved nominal type", declaration.as_str());
        }
        ResolvedType::TypeParameter { owner, .. } => {
            return reject_nul_identity("resolved type-parameter owner", owner.as_str());
        }
        ResolvedType::Function { .. } | ResolvedType::Nominal { .. } => {}
    }
    let mut pending = Pending::new(root)?;
    while let Some(ty) = pending.pop()? {
        match ty {
            ResolvedType::Function { parameters, result } => {
                pending.extend(parameters)?;
                pending.push(result)?;
            }
            ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
            | ResolvedType::MutFunctionI64
            | ResolvedType::Never
            | ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::ArrayU8(_)
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8
            | ResolvedType::StringMap => {}
            ResolvedType::TypeParameter { owner, .. } => {
                reject_nul_identity("resolved type-parameter owner", owner.as_str())?;
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                reject_nul_identity("resolved nominal type", declaration.as_str())?;
                pending.extend(arguments)?;
            }
        }
    }
    Ok(())
}

fn audit_resolved_record_match_pattern(
    record: &DeclarationId,
    instance: &ResolvedType,
    fields: &[ResolvedRecordMatchPatternField],
) -> Result<(), Diagnostic> {
    reject_nul_identity("resolved record match", record.as_str())?;
    audit_resolved_type(instance)?;
    for field in fields {
        reject_nul_identity("resolved record match field", field.field.as_str())?;
        match &field.pattern {
            ResolvedRecordMatchFieldPattern::Binding(binding) => {
                reject_nul_identity("resolved record match binding", binding.id.as_str())?;
                audit_resolved_type(&binding.ty)?;
            }
            ResolvedRecordMatchFieldPattern::Wildcard => {}
            ResolvedRecordMatchFieldPattern::Record {
                record,
                instance,
                fields,
            } => audit_resolved_record_match_pattern(record, instance, fields)?,
        }
    }
    Ok(())
}

fn audit_resolved_expression(root: &ResolvedExpr) -> Result<(), Diagnostic> {
    let mut pending = Pending::new(root)?;
    while let Some(expression) = pending.pop()? {
        reject_nul_identity("resolved expression", expression.id.as_str())?;
        audit_resolved_type(&expression.ty)?;
        match &expression.kind {
            ResolvedExprKind::Closure {
                parameters,
                captures,
                body,
            } => {
                for parameter in parameters {
                    reject_nul_identity("closure parameter", parameter.id.as_str())?;
                    audit_resolved_type(&parameter.ty)?;
                }
                for capture in captures {
                    reject_nul_identity("closure capture", capture.binding.id.as_str())?;
                    audit_resolved_type(&capture.binding.ty)?;
                    pending.push(&capture.value)?;
                }
                pending.push(body)?;
            }
            ResolvedExprKind::FunctionReference { target } => {
                reject_nul_identity("function reference", target.as_str())?
            }
            ResolvedExprKind::Invoke { callable, args } => {
                pending.push(callable)?;
                pending.extend(args)?;
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
            | ResolvedExprKind::BorrowedUtf8Literal(_) => {}
            ResolvedExprKind::Place(place) => audit_hir_place(place)?,
            ResolvedExprKind::BorrowPlace { operation, place } => {
                reject_nul_identity("resolved byte-view operation", operation.as_str())?;
                audit_hir_place(place)?;
            }
            ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } => {
                reject_nul_identity("resolved byte-range operation", operation.as_str())?;
                pending.push(end)?;
                pending.push(start)?;
                pending.push(source)?;
            }
            ResolvedExprKind::Call { callee, args, .. } => {
                reject_nul_identity("resolved call target", callee.as_str())?;
                pending.extend(args)?;
            }
            ResolvedExprKind::LiteralFormat { args, .. } => pending.extend(args)?,
            ResolvedExprKind::VecFieldRead {
                element,
                field,
                args,
                ..
            } => {
                audit_resolved_type(element)?;
                reject_nul_identity("scoped vector field", field.as_str())?;
                pending.extend(args)?;
            }
            ResolvedExprKind::RowLoan { loan, args } => {
                audit_resolved_type(&loan.element)?;
                reject_nul_identity("row loan traversal", loan.traversal.as_str())?;
                reject_nul_identity("row loan bound", loan.bound.as_str())?;
                reject_nul_identity("row loan callee", loan.callee.as_str())?;
                reject_nul_identity("row loan parameter", loan.parameter.as_str())?;
                pending.extend(args)?;
            }
            ResolvedExprKind::Upcast { source } | ResolvedExprKind::Yield { request: source } => {
                pending.push(source)?
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                reject_nul_identity("resolved native Rust import target", call.import.as_str())?;
                if call.expression != expression.id {
                    return Err(hir_error_static(
                        "resolved native Rust import call identity is inconsistent",
                    ));
                }
                pending.extend(&call.args)?;
            }
            ResolvedExprKind::HostCommandCall(call) => {
                if call.expression != expression.id {
                    return Err(hir_error_static(
                        "resolved host-command call identity is inconsistent",
                    ));
                }
                pending.extend(&call.args)?;
            }
            ResolvedExprKind::LexicalExit { value, .. } => pending.extend(value.as_deref())?,
            ResolvedExprKind::Unary { value, .. } => pending.push(value)?,
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(right)?;
                pending.push(left)?;
            }
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                pending.push(tail)?;
                for statement in statements.iter().rev() {
                    if let ResolvedStatement::Let { binding, .. }
                    | ResolvedStatement::Assign { binding, .. } = statement
                    {
                        reject_nul_identity("resolved value", binding.id.as_str())?;
                        audit_resolved_type(&binding.ty)?;
                    }
                    for index in (0..statement.child_count()).rev() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child)?;
                        }
                    }
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch)?;
                pending.push(then_branch)?;
                pending.push(condition)?;
            }
            ResolvedExprKind::ConstructRecord { record, fields } => {
                reject_nul_identity("resolved record constructor", record.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved record initializer field", field.field.as_str())?;
                    pending.push(&field.value)?;
                }
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                reject_nul_identity("resolved variant constructor", variant.as_str())?;
                reject_nul_identity("resolved variant case", case.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved case initializer field", field.field.as_str())?;
                    pending.push(&field.value)?;
                }
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                for arm in arms.iter().rev() {
                    match &arm.pattern {
                        ResolvedMatchPattern::Wildcard => {}
                        ResolvedMatchPattern::Variant {
                            variant,
                            case,
                            fields,
                        } => {
                            reject_nul_identity("resolved match variant", variant.as_str())?;
                            reject_nul_identity("resolved match case", case.as_str())?;
                            for field in fields {
                                reject_nul_identity("resolved match field", field.field.as_str())?;
                                reject_nul_identity(
                                    "resolved match binding",
                                    field.binding.id.as_str(),
                                )?;
                                audit_resolved_type(&field.binding.ty)?;
                            }
                        }
                        ResolvedMatchPattern::Record {
                            record,
                            instance,
                            fields,
                        } => audit_resolved_record_match_pattern(record, instance, fields)?,
                        // Refutable Match v1: binding arms carry a real value
                        // identity; literals and or-patterns carry none.
                        ResolvedMatchPattern::Binding(binding) => {
                            reject_nul_identity("resolved match binding", binding.id.as_str())?;
                            audit_resolved_type(&binding.ty)?;
                        }
                        ResolvedMatchPattern::Literal(_) | ResolvedMatchPattern::Or(_) => {}
                    }
                    if let Some(guard) = &arm.guard {
                        pending.push(guard)?;
                    }
                    pending.push(&arm.value)?;
                }
                pending.push(scrutinee)?;
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
                reject_nul_identity("resolved `?` Result", result.as_str())?;
                reject_nul_identity("resolved `?` Ok case", ok_case.as_str())?;
                reject_nul_identity("resolved `?` Ok field", ok_field.as_str())?;
                reject_nul_identity("resolved `?` Err case", err_case.as_str())?;
                reject_nul_identity("resolved `?` Err field", err_field.as_str())?;
                audit_resolved_type(residual_type)?;
                pending.push(operand)?;
            }
            ResolvedExprKind::TryOption {
                operand,
                option,
                some_case,
                some_field,
                none_case,
                residual_type,
            } => {
                reject_nul_identity("resolved Option `?` Option", option.as_str())?;
                reject_nul_identity("resolved Option `?` Some case", some_case.as_str())?;
                reject_nul_identity("resolved Option `?` Some field", some_field.as_str())?;
                reject_nul_identity("resolved Option `?` None case", none_case.as_str())?;
                audit_resolved_type(residual_type)?;
                pending.push(operand)?;
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                reject_nul_identity("resolved record update", record.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved record replacement field", field.field.as_str())?;
                    pending.push(&field.value)?;
                }
                pending.push(base)?;
            }
            ResolvedExprKind::Project { base, field } => {
                reject_nul_identity("resolved projected field", field.as_str())?;
                pending.push(base)?;
            }
        }
    }
    Ok(())
}

fn audit_hir_place(place: &Place) -> Result<(), Diagnostic> {
    reject_nul_identity("resolved place root", place.root.as_str())?;
    for projection in &place.projections {
        match projection {
            PlaceProjection::Field(field) => {
                reject_nul_identity("resolved place field", field.as_str())?;
            }
            PlaceProjection::VariantField { case, field } => {
                reject_nul_identity("resolved place variant case", case.as_str())?;
                reject_nul_identity("resolved place variant field", field.as_str())?;
            }
        }
    }
    Ok(())
}

fn audit_field_liveness_shape(root: &crate::cleanup::FieldLivenessShape) -> Result<(), Diagnostic> {
    // These terminal shapes have precisely the same verdict as popping only
    // the root in the full walker. No inventory normalization is implied.
    match root {
        crate::cleanup::FieldLivenessShape::NoDrop => return Ok(()),
        crate::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } => {
            return reject_nul_identity("cleanup lifecycle", lifecycle.as_str());
        }
        _ => {}
    }
    let mut pending = Pending::new(root)?;
    while let Some(shape) = pending.pop()? {
        match shape {
            crate::cleanup::FieldLivenessShape::NoDrop => {}
            crate::cleanup::FieldLivenessShape::Leaf { lifecycle, .. } => {
                reject_nul_identity("cleanup lifecycle", lifecycle.as_str())?;
            }
            crate::cleanup::FieldLivenessShape::Record {
                declaration,
                fields,
            } => {
                reject_nul_identity("cleanup record", declaration.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("cleanup field", field.field.as_str())?;
                    pending.push(&field.shape)?;
                }
            }
            crate::cleanup::FieldLivenessShape::Variant { declaration, cases } => {
                reject_nul_identity("cleanup variant", declaration.as_str())?;
                for (case_index, case) in cases.iter().enumerate().rev() {
                    reject_nul_identity("cleanup variant case", case.case.as_str())?;
                    if usize::try_from(case.case_index) != Ok(case_index) {
                        return Err(hir_error_static(
                            "cleanup variant case inventory has a non-canonical index",
                        ));
                    }
                    for field in case.fields.iter().rev() {
                        reject_nul_identity("cleanup variant field", field.field.as_str())?;
                        pending.push(&field.shape)?;
                    }
                }
            }
        }
    }
    Ok(())
}

fn audit_inventory_place(place: &crate::cleanup::CleanupPlace) -> Result<(), Diagnostic> {
    for projection in &place.projections {
        reject_nul_identity("cleanup inventory projection", projection.as_str())?;
    }
    Ok(())
}

fn audit_cleanup_inventory(inventory: &CleanupInventory) -> Result<(), Diagnostic> {
    for slot in &inventory.slots {
        match &slot.origin {
            crate::cleanup::CleanupStorageOrigin::Parameter { value, .. }
            | crate::cleanup::CleanupStorageOrigin::Binding { value }
            | crate::cleanup::CleanupStorageOrigin::ProvisionalResult { value } => {
                reject_nul_identity("cleanup inventory value", value.as_str())?;
            }
            crate::cleanup::CleanupStorageOrigin::Temporary { expression } => {
                reject_nul_identity("cleanup inventory expression", expression.as_str())?;
            }
        }
        audit_resolved_type(&slot.ty)?;
        audit_field_liveness_shape(&slot.shape)?;
    }
    for flag in &inventory.flags {
        audit_inventory_place(&flag.place)?;
        reject_nul_identity("cleanup inventory lifecycle", flag.lifecycle.as_str())?;
    }
    Ok(())
}

fn audit_plan_storage(storage: &crate::cleanup_plan::StorageId) -> Result<(), Diagnostic> {
    match storage {
        crate::cleanup_plan::StorageId::Value(value) => {
            reject_nul_identity("cleanup-plan value storage", value.as_str())?;
        }
        crate::cleanup_plan::StorageId::Temporary(expression) => {
            reject_nul_identity("cleanup-plan temporary storage", expression.as_str())?;
        }
        crate::cleanup_plan::StorageId::CallArgument {
            call,
            value_expression,
            ..
        } => {
            reject_nul_identity("cleanup-plan call-argument call", call.as_str())?;
            reject_nul_identity(
                "cleanup-plan call-argument value",
                value_expression.as_str(),
            )?;
        }
        crate::cleanup_plan::StorageId::ProvisionalResult => {}
    }
    Ok(())
}

fn audit_plan_place(place: &crate::cleanup_plan::CleanupPlace) -> Result<(), Diagnostic> {
    audit_plan_storage(&place.storage)?;
    for projection in &place.projections {
        reject_nul_identity("cleanup-plan projection", projection.as_str())?;
    }
    Ok(())
}

fn audit_status_source(source: &crate::cleanup_plan::StatusSourceId) -> Result<(), Diagnostic> {
    reject_nul_identity("cleanup-plan status expression", source.expression.as_str())
}

fn audit_result_source(
    source: &crate::cleanup_plan::CleanupResultSource,
) -> Result<(), Diagnostic> {
    match source {
        crate::cleanup_plan::CleanupResultSource::Scalar { expression } => {
            reject_nul_identity("cleanup-plan scalar result", expression.as_str())?;
        }
        crate::cleanup_plan::CleanupResultSource::Owned { storage } => {
            audit_plan_place(storage)?;
        }
    }
    Ok(())
}

fn audit_cleanup_plan(plan: &CleanupPlan) -> Result<(), Diagnostic> {
    for place in &plan.entry_state.live_owned_parameters {
        audit_plan_place(place)?;
    }
    for slot in &plan.slots {
        audit_plan_storage(&slot.storage)?;
        audit_resolved_type(&slot.ty)?;
        audit_field_liveness_shape(&slot.field_liveness_shape)?;
    }
    for source in &plan.status_sources {
        audit_status_source(&source.id)?;
        if let crate::cleanup_plan::StatusProducer::PropagatedCall { callee } = &source.producer {
            reject_nul_identity("cleanup-plan propagated callee", callee.as_str())?;
        }
    }
    for block in &plan.blocks {
        for transition in &block.transitions {
            match transition {
                crate::cleanup_plan::CleanupTransition::Initialize { at, destination }
                | crate::cleanup_plan::CleanupTransition::ReserveRenewal {
                    at,
                    binding: destination,
                } => {
                    reject_nul_identity("cleanup-plan initialize expression", at.as_str())?;
                    audit_plan_place(destination)?;
                }
                crate::cleanup_plan::CleanupTransition::InitializeVariant {
                    at,
                    destination,
                    variant,
                } => {
                    reject_nul_identity("cleanup-plan variant initialize expression", at.as_str())?;
                    audit_plan_place(destination)?;
                    reject_nul_identity("cleanup-plan initialized variant", variant.as_str())?;
                }
                crate::cleanup_plan::CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                }
                | crate::cleanup_plan::CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } => {
                    reject_nul_identity("cleanup-plan transfer expression", at.as_str())?;
                    audit_plan_place(source)?;
                    audit_plan_place(destination)?;
                }
                crate::cleanup_plan::CleanupTransition::TransferVariant {
                    at,
                    source,
                    destination,
                    variant,
                } => {
                    reject_nul_identity("cleanup-plan variant transfer expression", at.as_str())?;
                    audit_plan_place(source)?;
                    audit_plan_place(destination)?;
                    reject_nul_identity("cleanup-plan transferred variant", variant.as_str())?;
                }
                crate::cleanup_plan::CleanupTransition::AuthenticateVariantCase {
                    at,
                    source,
                    variant,
                    case,
                } => {
                    reject_nul_identity(
                        "cleanup-plan variant authentication expression",
                        at.as_str(),
                    )?;
                    audit_plan_place(source)?;
                    reject_nul_identity("cleanup-plan authenticated variant", variant.as_str())?;
                    reject_nul_identity("cleanup-plan authenticated case", case.as_str())?;
                }
                crate::cleanup_plan::CleanupTransition::CallCommit { call, arguments } => {
                    reject_nul_identity("cleanup-plan committed call", call.as_str())?;
                    for argument in arguments {
                        audit_plan_place(&argument.source)?;
                    }
                }
                crate::cleanup_plan::CleanupTransition::SelectFailure { source } => {
                    audit_status_source(source)?;
                }
                crate::cleanup_plan::CleanupTransition::StageCopyResult { source } => {
                    match source {
                        crate::cleanup_plan::StagedCopyResultSource::Body {
                            expression,
                            instance,
                        } => {
                            reject_nul_identity(
                                "cleanup-plan staged body expression",
                                expression.as_str(),
                            )?;
                            audit_resolved_type(instance)?;
                        }
                        crate::cleanup_plan::StagedCopyResultSource::TryResidual {
                            expression,
                            operand,
                            source_instance,
                            target_instance,
                            result,
                            ok_case,
                            ok_field,
                            err_case,
                            err_field,
                        } => {
                            reject_nul_identity(
                                "cleanup-plan staged `?` expression",
                                expression.as_str(),
                            )?;
                            reject_nul_identity(
                                "cleanup-plan staged `?` operand",
                                operand.as_str(),
                            )?;
                            audit_resolved_type(source_instance)?;
                            audit_resolved_type(target_instance)?;
                            for (kind, declaration) in [
                                ("cleanup-plan staged `?` Result", result),
                                ("cleanup-plan staged `?` Ok case", ok_case),
                                ("cleanup-plan staged `?` Ok field", ok_field),
                                ("cleanup-plan staged `?` Err case", err_case),
                                ("cleanup-plan staged `?` Err field", err_field),
                            ] {
                                reject_nul_identity(kind, declaration.as_str())?;
                            }
                        }
                        crate::cleanup_plan::StagedCopyResultSource::TryOptionNone {
                            expression,
                            operand,
                            source_instance,
                            target_instance,
                            option,
                            some_case,
                            some_field,
                            none_case,
                        } => {
                            reject_nul_identity(
                                "cleanup-plan staged Option `?` expression",
                                expression.as_str(),
                            )?;
                            reject_nul_identity(
                                "cleanup-plan staged Option `?` operand",
                                operand.as_str(),
                            )?;
                            audit_resolved_type(source_instance)?;
                            audit_resolved_type(target_instance)?;
                            for (kind, declaration) in [
                                ("cleanup-plan staged Option `?` Option", option),
                                ("cleanup-plan staged Option `?` Some case", some_case),
                                ("cleanup-plan staged Option `?` Some field", some_field),
                                ("cleanup-plan staged Option `?` None case", none_case),
                            ] {
                                reject_nul_identity(kind, declaration.as_str())?;
                            }
                        }
                    }
                }
            }
        }
    }
    for edge in &plan.edges {
        match &edge.condition {
            crate::cleanup_plan::EdgeCondition::Always => {}
            crate::cleanup_plan::EdgeCondition::BooleanResult(expression, _) => {
                reject_nul_identity("cleanup-plan boolean expression", expression.as_str())?;
            }
            crate::cleanup_plan::EdgeCondition::VariantCase {
                scrutinee, case, ..
            } => {
                reject_nul_identity("cleanup-plan match scrutinee", scrutinee.as_str())?;
                reject_nul_identity("cleanup-plan variant case", case.as_str())?;
            }
            crate::cleanup_plan::EdgeCondition::ArmSelected { scrutinee, .. } => {
                reject_nul_identity("cleanup-plan scalar-match scrutinee", scrutinee.as_str())?;
            }
            crate::cleanup_plan::EdgeCondition::StatusZero(source)
            | crate::cleanup_plan::EdgeCondition::StatusNonzero(source) => {
                audit_status_source(source)?;
            }
        }
    }
    for region in &plan.regions {
        for storage in &region.slots {
            audit_plan_storage(storage)?;
        }
    }
    for exit in &plan.exits {
        for finalizer in &exit.finalize_in_order {
            audit_plan_place(&finalizer.source)?;
            reject_nul_identity(
                "cleanup-plan finalizer lifecycle",
                finalizer.lifecycle_id.as_str(),
            )?;
        }
        match &exit.continuation {
            crate::cleanup_plan::ExitContinuation::LoopLatch { target }
            | crate::cleanup_plan::ExitContinuation::LoopBreak { target, .. } => {
                audit_control_target(target)?;
            }
            crate::cleanup_plan::ExitContinuation::FunctionReturn { target, source, .. } => {
                audit_control_target(target)?;
                audit_result_source(source)?;
            }
            crate::cleanup_plan::ExitContinuation::Continue(_)
            | crate::cleanup_plan::ExitContinuation::ReturnUnit => {}
            crate::cleanup_plan::ExitContinuation::CommitResult { source } => {
                audit_result_source(source)?;
            }
            crate::cleanup_plan::ExitContinuation::ReturnFailure { source } => {
                audit_status_source(source)?;
            }
        }
    }
    Ok(())
}

fn declaration_identity_subject(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::Resource => "resolved resource declaration",
        DeclarationKind::ResourceDrop => "resolved resource lifecycle declaration",
        DeclarationKind::Record => "resolved record declaration",
        DeclarationKind::Class => "resolved class declaration",
        DeclarationKind::Field => "resolved field declaration",
        DeclarationKind::Variant => "resolved variant declaration",
        DeclarationKind::VariantCase => "resolved variant case declaration",
        DeclarationKind::CaseField => "resolved case field declaration",
        DeclarationKind::Interface => "resolved interface declaration",
        DeclarationKind::Import => "resolved import declaration",
        DeclarationKind::Function => "resolved function declaration",
    }
}

fn audit_control_target(target: &super::ControlTarget) -> Result<(), Diagnostic> {
    match target {
        super::ControlTarget::Function(owner) => {
            reject_nul_identity("lexical target execution", owner.diagnostic_text())
        }
        super::ControlTarget::Loop { owner, site } => {
            reject_nul_identity("lexical target execution", owner.diagnostic_text())?;
            reject_nul_identity("lexical target loop", site.as_str())
        }
    }
}

#[cfg(test)]
mod tests;
