//! Validation and read-only inspection over resolved HIR.

use super::*;

/// Validate an identity-resolved program before a semantic consumer uses it.
///
/// Resolved HIR is intentionally public for agent and compiler integrations,
/// so callers can inspect or transform HIR produced by [`resolve`]. Every
/// backend calls this function and therefore fails closed when a transformation
/// breaks identities, lexical scope, or current type rules. A versioned wire
/// schema for constructing HIR outside the compiler is future work.
pub fn validate(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    validate_core(program)?;
    validate_attached_identity_references(program)?;
    crate::cleanup::validate_program(program)?;
    crate::cleanup_plan::validate_program(program)?;
    Ok(())
}

pub(super) fn validate_nul_free_identities(program: &ResolvedProgram) -> Result<(), Diagnostic> {
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
    for function in &program.functions {
        audit_cleanup_inventory(&function.cleanup)?;
        audit_cleanup_plan(&function.cleanup_plan)?;
    }
    Ok(())
}

fn audit_resolved_type(root: &ResolvedType) -> Result<(), Diagnostic> {
    let mut pending = vec![root];
    while let Some(ty) = pending.pop() {
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
            | ResolvedType::Bool
            | ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::Str
            | ResolvedType::SliceU8 => {}
            ResolvedType::TypeParameter { owner, .. } => {
                reject_nul_identity("resolved type-parameter owner", owner.as_str())?;
            }
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                reject_nul_identity("resolved nominal type", declaration.as_str())?;
                pending.extend(arguments);
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
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        reject_nul_identity("resolved expression", expression.id.as_str())?;
        audit_resolved_type(&expression.ty)?;
        match &expression.kind {
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
            | ResolvedExprKind::String(_) => {}
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
                pending.push(end);
                pending.push(start);
                pending.push(source);
            }
            ResolvedExprKind::Call { callee, args, .. } => {
                reject_nul_identity("resolved call target", callee.as_str())?;
                pending.extend(args);
            }
            ResolvedExprKind::Upcast { source } => pending.push(source),
            ResolvedExprKind::NativeRustImportCall(call) => {
                reject_nul_identity("resolved native Rust import target", call.import.as_str())?;
                if call.expression != expression.id {
                    return Err(hir_error(
                        "resolved native Rust import call identity is inconsistent",
                    ));
                }
                pending.extend(&call.args);
            }
            ResolvedExprKind::HostCommandCall(call) => {
                if call.expression != expression.id {
                    return Err(hir_error(
                        "resolved host-command call identity is inconsistent",
                    ));
                }
                pending.extend(&call.args);
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            ResolvedExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements.iter().rev() {
                    if let ResolvedStatement::Let { binding, .. }
                    | ResolvedStatement::Assign { binding, .. } = statement
                    {
                        reject_nul_identity("resolved value", binding.id.as_str())?;
                        audit_resolved_type(&binding.ty)?;
                    }
                    for index in (0..statement.child_count()).rev() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch);
                pending.push(then_branch);
                pending.push(condition);
            }
            ResolvedExprKind::ConstructRecord { record, fields } => {
                reject_nul_identity("resolved record constructor", record.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved record initializer field", field.field.as_str())?;
                    pending.push(&field.value);
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
                    pending.push(&field.value);
                }
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
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
                        pending.push(guard);
                    }
                    pending.push(&arm.value);
                }
                pending.push(scrutinee);
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
                pending.push(operand);
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
                pending.push(operand);
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                reject_nul_identity("resolved record update", record.as_str())?;
                for field in fields.iter().rev() {
                    reject_nul_identity("resolved record replacement field", field.field.as_str())?;
                    pending.push(&field.value);
                }
                pending.push(base);
            }
            ResolvedExprKind::Project { base, field } => {
                reject_nul_identity("resolved projected field", field.as_str())?;
                pending.push(base);
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
    let mut pending = vec![root];
    while let Some(shape) = pending.pop() {
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
                    pending.push(&field.shape);
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
                crate::cleanup_plan::CleanupTransition::Initialize { at, destination } => {
                    reject_nul_identity("cleanup-plan initialize expression", at.as_str())?;
                    audit_plan_place(destination)?;
                }
                crate::cleanup_plan::CleanupTransition::Transfer {
                    at,
                    source,
                    destination,
                } => {
                    reject_nul_identity("cleanup-plan transfer expression", at.as_str())?;
                    audit_plan_place(source)?;
                    audit_plan_place(destination)?;
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
                                ("Result", result),
                                ("Ok case", ok_case),
                                ("Ok field", ok_field),
                                ("Err case", err_case),
                                ("Err field", err_field),
                            ] {
                                reject_nul_identity(
                                    &format!("cleanup-plan staged `?` {kind}"),
                                    declaration.as_str(),
                                )?;
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
                                ("Option", option),
                                ("Some case", some_case),
                                ("Some field", some_field),
                                ("None case", none_case),
                            ] {
                                reject_nul_identity(
                                    &format!("cleanup-plan staged Option `?` {kind}"),
                                    declaration.as_str(),
                                )?;
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

pub(super) fn reject_nul_identity(subject: &str, value: &str) -> Result<(), Diagnostic> {
    if value.contains('\0') {
        Err(hir_error(format!("{subject} identity contains NUL")))
    } else {
        Ok(())
    }
}

pub(super) fn path_is_prefix<T: PartialEq>(prefix: &[T], path: &[T]) -> bool {
    prefix.len() <= path.len() && prefix.iter().zip(path).all(|(left, right)| left == right)
}

pub(super) fn resolved_lifecycle_effects(
    program: &ResolvedProgram,
    ty: &ResolvedType,
) -> Result<BTreeSet<String>, Diagnostic> {
    fn collect(
        program: &ResolvedProgram,
        ty: &ResolvedType,
        visiting: &mut BTreeSet<DeclarationId>,
        effects: &mut BTreeSet<String>,
    ) -> Result<(), Diagnostic> {
        let Some(id) = ty.nominal_id() else {
            return Ok(());
        };
        if !visiting.insert(id.clone()) {
            return Ok(());
        }
        let declaration = program
            .types
            .iter()
            .find(|item| item.id == *id)
            .ok_or_else(|| hir_error(format!("type `{id}` has no lifecycle declaration")))?;
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Resource { drop } => {
                if let ResolvedResourceDropKind::Imported { import, .. } = &drop.kind {
                    let resolved = program
                        .interfaces
                        .iter()
                        .flat_map(|interface| &interface.imports)
                        .find(|item| item.id == *import)
                        .ok_or_else(|| {
                            hir_error(format!(
                                "resource `{id}` references missing import `{import}`"
                            ))
                        })?;
                    effects.extend(resolved.effects.iter().cloned());
                }
            }
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    collect(program, &field.ty, visiting, effects)?;
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    for field in &case.fields {
                        collect(program, &field.ty, visiting, effects)?;
                    }
                }
            }
        }
        visiting.remove(id);
        Ok(())
    }

    let mut effects = BTreeSet::new();
    collect(program, ty, &mut BTreeSet::new(), &mut effects)?;
    Ok(effects)
}

pub(super) fn visit_resolved_calls(
    expression: &ResolvedExpr,
    visit: &mut impl FnMut(&DeclarationId, Option<&FunctionInstanceId>, &[ResolvedType]),
) {
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            visit_resolved_calls(source, visit);
            visit_resolved_calls(start, visit);
            visit_resolved_calls(end, visit);
        }
        ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            args,
        } => {
            visit(callee, instance.as_ref(), type_arguments);
            for arg in args {
                visit_resolved_calls(arg, visit);
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for arg in &call.args {
                visit_resolved_calls(arg, visit);
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for arg in &call.args {
                visit_resolved_calls(arg, visit);
            }
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value } => visit_resolved_calls(value, visit),
        ResolvedExprKind::Binary { left, right, .. } => {
            visit_resolved_calls(left, visit);
            visit_resolved_calls(right, visit);
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        visit_resolved_calls(child, visit);
                    }
                }
            }
            visit_resolved_calls(tail, visit);
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            visit_resolved_calls(condition, visit);
            visit_resolved_calls(then_branch, visit);
            visit_resolved_calls(else_branch, visit);
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            visit_resolved_calls(scrutinee, visit);
            for arm in arms {
                visit_resolved_calls(&arm.value, visit);
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            visit_resolved_calls(base, visit);
            for field in fields {
                visit_resolved_calls(&field.value, visit);
            }
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

pub(super) fn workspace_call_edges(
    program: &ResolvedProgram,
) -> BTreeSet<(DeclarationId, DeclarationId)> {
    let mut edges = BTreeSet::new();
    for function in &program.functions {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            visit_resolved_calls(expression, &mut |callee, _, _| {
                edges.insert((function.id.clone(), callee.clone()));
            });
        }
    }
    edges
}

#[allow(dead_code, reason = "private Workspace Semantic Graph Phase-A seam")]
pub(crate) fn workspace_expression_identity(owner: &DeclarationId, path: &str) -> String {
    ExpressionId::new(&FunctionExecutionId::Monomorphic(owner.clone()), path)
        .as_str()
        .to_owned()
}

#[allow(dead_code, reason = "private Workspace Semantic Graph Phase-A seam")]
pub(crate) fn workspace_call_sites(
    program: &ResolvedProgram,
) -> Vec<(DeclarationId, String, DeclarationId)> {
    fn walk(
        owner: &DeclarationId,
        expression: &ResolvedExpr,
        sites: &mut Vec<(DeclarationId, String, DeclarationId)>,
    ) {
        match &expression.kind {
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                walk(owner, source, sites);
                walk(owner, start, sites);
                walk(owner, end, sites);
            }
            ResolvedExprKind::Call { callee, args, .. } => {
                sites.push((
                    owner.clone(),
                    expression.id.as_str().to_owned(),
                    callee.clone(),
                ));
                for argument in args {
                    walk(owner, argument, sites);
                }
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                for argument in &call.args {
                    walk(owner, argument, sites);
                }
            }
            ResolvedExprKind::HostCommandCall(call) => {
                for argument in &call.args {
                    walk(owner, argument, sites);
                }
            }
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => walk(owner, value, sites),
            ResolvedExprKind::Binary { left, right, .. } => {
                walk(owner, left, sites);
                walk(owner, right, sites);
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    for index in 0..statement.child_count() {
                        if let Some(child) = statement.child(index) {
                            walk(owner, child, sites);
                        }
                    }
                }
                walk(owner, tail, sites);
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                walk(owner, condition, sites);
                walk(owner, then_branch, sites);
                walk(owner, else_branch, sites);
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                for field in fields {
                    walk(owner, &field.value, sites);
                }
            }
            ResolvedExprKind::Match { scrutinee, arms } => {
                walk(owner, scrutinee, sites);
                for arm in arms {
                    walk(owner, &arm.value, sites);
                }
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                walk(owner, base, sites);
                for field in fields {
                    walk(owner, &field.value, sites);
                }
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

    let mut sites = Vec::new();
    for function in &program.functions {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            walk(&function.id, expression, &mut sites);
        }
    }
    for template in &program.function_templates {
        for expression in template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
        {
            walk(&template.id, expression, &mut sites);
        }
    }
    sites
}
