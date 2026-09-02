//! Owned-allocation capacity of a resolved program, counted per type,
//! pattern, expression, loan plan, and function.

use super::*;

fn hir_type_owned_capacity(ty: &ResolvedType) -> Option<usize> {
    match ty {
        ResolvedType::Unit
        | ResolvedType::I64
        | ResolvedType::I32
        | ResolvedType::Char
        | ResolvedType::U8
        | ResolvedType::Usize
        | ResolvedType::F32
        | ResolvedType::F64
        | ResolvedType::Bool
        | ResolvedType::String
        | ResolvedType::ArrayU8(_)
        | ResolvedType::Bytes
        | ResolvedType::Str
        | ResolvedType::SliceU8 => Some(0),
        ResolvedType::TypeParameter { owner, .. } => Some(owner.as_str().len()),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => arguments
            .iter()
            .try_fold(declaration.as_str().len(), |bytes, argument| {
                bytes.checked_add(hir_type_owned_capacity(argument)?)
            })?
            .checked_add(arguments.capacity() * std::mem::size_of::<ResolvedType>()),
    }
}

fn add_capacity(total: &mut usize, capacity: usize, element: usize) -> Result<(), Diagnostic> {
    *total = total
        .checked_add(
            capacity
                .checked_mul(element)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    Ok(())
}

fn hir_binding_owned_capacity(binding: &crate::hir::ResolvedBinding) -> Option<usize> {
    binding
        .id
        .as_str()
        .len()
        .checked_add(binding.name.capacity())?
        .checked_add(hir_type_owned_capacity(&binding.ty)?)
}

fn hir_match_pattern_owned_capacity(pattern: &crate::hir::ResolvedMatchPattern) -> Option<usize> {
    match pattern {
        crate::hir::ResolvedMatchPattern::Wildcard => Some(0),
        // Refutable Match v1: literal/or/binding capacity accounting.
        crate::hir::ResolvedMatchPattern::Literal(_) => Some(0),
        crate::hir::ResolvedMatchPattern::Binding(binding) => hir_binding_owned_capacity(binding),
        crate::hir::ResolvedMatchPattern::Or(alternatives) => {
            let mut bytes = alternatives
                .capacity()
                .checked_mul(std::mem::size_of::<crate::hir::ResolvedMatchPattern>())?;
            for alternative in alternatives {
                bytes = bytes.checked_add(hir_match_pattern_owned_capacity(alternative)?)?;
            }
            Some(bytes)
        }
        crate::hir::ResolvedMatchPattern::Variant {
            variant,
            case,
            fields,
        } => fields
            .iter()
            .try_fold(
                variant.as_str().len().checked_add(case.as_str().len())?,
                |bytes, field| {
                    bytes
                        .checked_add(field.field.as_str().len())?
                        .checked_add(hir_binding_owned_capacity(&field.binding)?)
                },
            )?
            .checked_add(
                fields.capacity() * std::mem::size_of::<crate::hir::ResolvedMatchPatternField>(),
            ),
        crate::hir::ResolvedMatchPattern::Record {
            record,
            instance,
            fields,
        } => fields
            .iter()
            .try_fold(
                record
                    .as_str()
                    .len()
                    .checked_add(hir_type_owned_capacity(instance)?)?,
                |bytes, field| {
                    bytes
                        .checked_add(field.field.as_str().len())?
                        .checked_add(hir_record_pattern_field_owned_capacity(&field.pattern)?)
                },
            )?
            .checked_add(
                fields.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedRecordMatchPatternField>(),
            ),
    }
}

fn hir_record_pattern_field_owned_capacity(
    pattern: &crate::hir::ResolvedRecordMatchFieldPattern,
) -> Option<usize> {
    match pattern {
        crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
            hir_binding_owned_capacity(binding)
        }
        crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => Some(0),
        crate::hir::ResolvedRecordMatchFieldPattern::Record {
            record,
            instance,
            fields,
        } => fields
            .iter()
            .try_fold(
                record
                    .as_str()
                    .len()
                    .checked_add(hir_type_owned_capacity(instance)?)?,
                |bytes, field| {
                    bytes
                        .checked_add(field.field.as_str().len())?
                        .checked_add(hir_record_pattern_field_owned_capacity(&field.pattern)?)
                },
            )?
            .checked_add(
                fields.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedRecordMatchPatternField>(),
            ),
    }
}

fn hir_expr_owned_capacity(expression: &ResolvedExpr) -> Result<usize, Diagnostic> {
    let mut total = 0_usize;
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        total = total
            .checked_add(std::mem::size_of::<ResolvedExpr>())
            .and_then(|bytes| bytes.checked_add(expression.id.as_str().len()))
            .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&expression.ty)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match &expression.kind {
            ResolvedExprKind::ArrayU8(values) => {
                total = total
                    .checked_add(values.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            ResolvedExprKind::RepeatArrayU8 { .. } => {}
            ResolvedExprKind::ByteRange {
                operation,
                source,
                start,
                end,
            } => {
                total = total
                    .checked_add(operation.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                pending.push(source);
                pending.push(start);
                pending.push(end);
            }
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            } => {
                total = total
                    .checked_add(callee.as_str().len())
                    .and_then(|bytes| {
                        bytes.checked_add(
                            type_arguments.capacity() * std::mem::size_of::<ResolvedType>(),
                        )
                    })
                    .and_then(|bytes| {
                        instance.as_ref().map_or(Some(bytes), |instance| {
                            bytes.checked_add(instance.as_str().len())
                        })
                    })
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                for ty in type_arguments {
                    let ty_bytes = hir_type_owned_capacity(ty)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    total = total
                        .checked_add(ty_bytes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                add_capacity(
                    &mut total,
                    args.capacity(),
                    std::mem::size_of::<ResolvedExpr>(),
                )?;
                pending.extend(args);
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                total = total
                    .checked_add(call.expression.as_str().len())
                    .and_then(|bytes| bytes.checked_add(call.import.as_str().len()))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    call.args.capacity(),
                    std::mem::size_of::<ResolvedExpr>(),
                )?;
                pending.extend(&call.args);
            }
            ResolvedExprKind::HostCommandCall(call) => {
                total = total
                    .checked_add(call.expression.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    call.args.capacity(),
                    std::mem::size_of::<ResolvedExpr>(),
                )?;
                pending.extend(&call.args);
            }
            ResolvedExprKind::Unary { value, .. } => pending.push(value),
            ResolvedExprKind::Try {
                operand,
                result,
                ok_case,
                ok_field,
                err_case,
                err_field,
                residual_type,
            } => {
                for id in [result, ok_case, ok_field, err_case, err_field] {
                    total = total
                        .checked_add(id.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                total = total
                    .checked_add(
                        hir_type_owned_capacity(residual_type)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
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
                for id in [option, some_case, some_field, none_case] {
                    total = total
                        .checked_add(id.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                total = total
                    .checked_add(
                        hir_type_owned_capacity(residual_type)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                pending.push(operand);
            }
            ResolvedExprKind::Project { base, field } => {
                total = total
                    .checked_add(field.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                pending.push(base);
            }
            ResolvedExprKind::Upcast { source } => pending.push(source),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(left);
                pending.push(right);
            }
            ResolvedExprKind::Block { statements, tail } => {
                add_capacity(
                    &mut total,
                    statements.capacity(),
                    std::mem::size_of::<ResolvedStatement>(),
                )?;
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. }
                        | ResolvedStatement::Assign { binding, value, .. } => {
                            total = total
                                .checked_add(binding.id.as_str().len())
                                .and_then(|bytes| bytes.checked_add(binding.name.capacity()))
                                .and_then(|bytes| {
                                    bytes.checked_add(hir_type_owned_capacity(&binding.ty)?)
                                })
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            pending.push(value);
                        }
                        ResolvedStatement::Unsafe { audit, body, .. } => {
                            total = total
                                .checked_add(audit.capacity())
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            pending.push(body);
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            pending.push(condition);
                            pending.push(body);
                        }
                    }
                }
                pending.push(tail);
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
            ResolvedExprKind::ConstructRecord { record, fields } => {
                total = total
                    .checked_add(record.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    fields.capacity(),
                    std::mem::size_of::<crate::hir::ResolvedFieldInitializer>(),
                )?;
                for field in fields {
                    total = total
                        .checked_add(field.field.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::ConstructVariant {
                variant,
                case,
                fields,
            } => {
                total = total
                    .checked_add(variant.as_str().len())
                    .and_then(|bytes| bytes.checked_add(case.as_str().len()))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    fields.capacity(),
                    std::mem::size_of::<crate::hir::ResolvedFieldInitializer>(),
                )?;
                for field in fields {
                    total = total
                        .checked_add(field.field.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                add_capacity(
                    &mut total,
                    arms.capacity(),
                    std::mem::size_of::<crate::hir::ResolvedMatchArm>(),
                )?;
                for arm in arms {
                    total = total
                        .checked_add(
                            hir_match_pattern_owned_capacity(&arm.pattern)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    if let Some(guard) = &arm.guard {
                        pending.push(guard);
                    }
                }
                pending.push(scrutinee);
                pending.extend(arms.iter().map(|arm| &arm.value));
            }
            ResolvedExprKind::UpdateRecord {
                base,
                record,
                fields,
            } => {
                total = total
                    .checked_add(record.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    fields.capacity(),
                    std::mem::size_of::<crate::hir::ResolvedFieldInitializer>(),
                )?;
                for field in fields {
                    total = total
                        .checked_add(field.field.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                pending.push(base);
                pending.extend(fields.iter().map(|field| &field.value));
            }
            ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } => {
                if let ResolvedExprKind::BorrowPlace { operation, .. } = &expression.kind {
                    total = total
                        .checked_add(operation.as_str().len())
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                total = total
                    .checked_add(place.root.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_capacity(
                    &mut total,
                    place.projections.capacity(),
                    std::mem::size_of::<crate::hir::PlaceProjection>(),
                )?;
                for projection in &place.projections {
                    total = total
                        .checked_add(match projection {
                            crate::hir::PlaceProjection::Field(field) => field.as_str().len(),
                            crate::hir::PlaceProjection::VariantField { case, field } => {
                                case.as_str().len().saturating_add(field.as_str().len())
                            }
                        })
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_) => {}
            ResolvedExprKind::String(contents) => {
                total = total
                    .checked_add(contents.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
        }
    }
    Ok(total)
}

fn hir_loan_program_point_owned_capacity(point: &semaprax::loan_plan::LoanProgramPoint) -> usize {
    point.expression.as_str().len()
}

fn hir_loan_place_owned_capacity(place: &crate::hir::Place) -> Option<usize> {
    let mut total = place.root.as_str().len().checked_add(
        place
            .projections
            .capacity()
            .checked_mul(std::mem::size_of::<crate::hir::PlaceProjection>())?,
    )?;
    for projection in &place.projections {
        total = total.checked_add(match projection {
            crate::hir::PlaceProjection::Field(field) => field.as_str().len(),
            crate::hir::PlaceProjection::VariantField { case, field } => {
                case.as_str().len().checked_add(field.as_str().len())?
            }
        })?;
    }
    Some(total)
}

pub(in crate::implementation) fn hir_loan_plan_owned_capacity(
    plan: &semaprax::loan_plan::LoanPlan,
) -> Option<usize> {
    // `schema` is a static string and therefore retains no owned allocation.
    // The inline `LoanPlan` itself is already part of `ResolvedFunction`.
    let mut total = plan
        .loans
        .capacity()
        .checked_mul(std::mem::size_of::<semaprax::loan_plan::Loan>())?
        .checked_add(
            plan.endpoints
                .capacity()
                .checked_mul(std::mem::size_of::<semaprax::loan_plan::LoanEndpoint>())?,
        )?
        .checked_add(
            plan.edges
                .capacity()
                .checked_mul(std::mem::size_of::<semaprax::loan_plan::LoanEdge>())?,
        )?;
    for loan in &plan.loans {
        total = total
            .checked_add(loan.site.as_str().len())?
            .checked_add(hir_loan_place_owned_capacity(&loan.origin)?)?
            .checked_add(hir_loan_program_point_owned_capacity(&loan.start))?
            .checked_add(
                loan.ends
                    .capacity()
                    .checked_mul(std::mem::size_of::<semaprax::loan_plan::LoanProgramPoint>())?,
            )?
            .checked_add(
                loan.end_edges
                    .capacity()
                    .checked_mul(std::mem::size_of::<u16>())?,
            )?;
        for end in &loan.ends {
            total = total.checked_add(hir_loan_program_point_owned_capacity(end))?;
        }
    }
    for endpoint in &plan.endpoints {
        total = total.checked_add(hir_loan_program_point_owned_capacity(&endpoint.point))?;
        for ids in [
            &endpoint.live_before,
            &endpoint.starts,
            &endpoint.kills,
            &endpoint.live_after,
        ] {
            total = total.checked_add(
                ids.capacity()
                    .checked_mul(std::mem::size_of::<semaprax::loan_plan::LoanId>())?,
            )?;
        }
    }
    for edge in &plan.edges {
        total = total.checked_add(
            edge.live
                .capacity()
                .checked_mul(std::mem::size_of::<semaprax::loan_plan::LoanId>())?,
        )?;
    }
    Some(total)
}

fn hir_function_owned_capacity(function: &ResolvedFunction) -> Result<usize, Diagnostic> {
    let mut total = std::mem::size_of::<ResolvedFunction>()
        .checked_add(function.id.as_str().len())
        .and_then(|bytes| bytes.checked_add(function.result_id.as_str().len()))
        .and_then(|bytes| bytes.checked_add(function.name.capacity()))
        .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&function.return_type)?))
        .and_then(|bytes| {
            bytes.checked_add(
                function.params.capacity() * std::mem::size_of::<crate::hir::ResolvedParam>(),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(function.effects.capacity() * std::mem::size_of::<String>())
        })
        .and_then(|bytes| {
            bytes.checked_add(function.requires.capacity() * std::mem::size_of::<ResolvedExpr>())
        })
        .and_then(|bytes| {
            bytes.checked_add(function.ensures.capacity() * std::mem::size_of::<ResolvedExpr>())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for parameter in &function.params {
        total = total
            .checked_add(parameter.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(parameter.name.capacity()))
            .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&parameter.ty)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    for effect in &function.effects {
        total = total
            .checked_add(effect.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        total = total
            .checked_add(hir_expr_owned_capacity(expression)?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    total = total
        .checked_add(
            crate::private_capacity_contract::cleanup_inventory_owned_capacity(&function.cleanup)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                crate::private_capacity_contract::cleanup_plan_owned_capacity(
                    &function.cleanup_plan,
                )?,
            )
        })
        .and_then(|bytes| bytes.checked_add(hir_loan_plan_owned_capacity(&function.loan_plan)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    Ok(total)
}

pub(in crate::implementation) fn hir_owned_capacity(
    resolved: &ResolvedProgram,
) -> Result<usize, Diagnostic> {
    // `declaration_index_upper` separately owns the opaque index's inline
    // header and heap payload. Avoid charging its inline bytes twice here.
    let mut total = (std::mem::size_of::<ResolvedProgram>()
        - std::mem::size_of::<crate::hir::DeclarationIndex>())
    .checked_add(resolved.module.capacity())
    .and_then(|bytes| bytes.checked_add(resolved.entrypoint.as_str().len()))
    .and_then(|bytes| {
        bytes.checked_add(resolved.permits.capacity() * std::mem::size_of::<String>())
    })
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for permit in &resolved.permits {
        total = total
            .checked_add(permit.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    total = total
        .checked_add(resolved.functions.capacity() * std::mem::size_of::<ResolvedFunction>())
        .and_then(|bytes| {
            bytes.checked_add(
                resolved.interfaces.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedInterface>(),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                resolved.types.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedTypeDeclaration>(),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                resolved.function_templates.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedFunctionTemplate>(),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                resolved.function_instances.capacity()
                    * std::mem::size_of::<crate::hir::ResolvedFunctionInstance>(),
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for interface in &resolved.interfaces {
        total = total
            .checked_add(interface.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(interface.name.capacity()))
            .and_then(|bytes| {
                bytes.checked_add(interface.permits.capacity() * std::mem::size_of::<String>())
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    interface.imports.capacity()
                        * std::mem::size_of::<crate::hir::ResolvedImport>(),
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        for permit in &interface.permits {
            total = total
                .checked_add(permit.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        for import in &interface.imports {
            total = total
                .checked_add(import.id.as_str().len())
                .and_then(|bytes| bytes.checked_add(import.interface.as_str().len()))
                .and_then(|bytes| bytes.checked_add(import.name.capacity()))
                .and_then(|bytes| bytes.checked_add(import.import_key.capacity()))
                .and_then(|bytes| {
                    bytes.checked_add(
                        import.parameters.capacity()
                            * std::mem::size_of::<crate::hir::ResolvedImportParameter>(),
                    )
                })
                .and_then(|bytes| {
                    bytes.checked_add(import.effects.capacity() * std::mem::size_of::<String>())
                })
                .and_then(|bytes| {
                    bytes.checked_add(
                        import.required_authority.capacity() * std::mem::size_of::<String>(),
                    )
                })
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            for parameter in &import.parameters {
                total = total
                    .checked_add(parameter.name.capacity())
                    .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&parameter.ty)?))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            for value in import.effects.iter().chain(&import.required_authority) {
                total = total
                    .checked_add(value.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            if let ResolvedImportFailure::Status { domain_id, .. } = &import.failure {
                total = total
                    .checked_add(domain_id.capacity())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
        }
    }
    for function in &resolved.functions {
        // The outer function vector already accounts for each inline struct;
        // add only its recursively owned payload.
        let whole_function = hir_function_owned_capacity(function)?;
        total = total
            .checked_add(whole_function.saturating_sub(std::mem::size_of::<ResolvedFunction>()))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    for declaration in &resolved.types {
        total = total
            .checked_add(declaration.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(declaration.name.capacity()))
            .and_then(|bytes| {
                bytes.checked_add(
                    declaration.type_parameters.capacity()
                        * std::mem::size_of::<crate::hir::ResolvedTypeParameterDeclaration>(),
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        for parameter in &declaration.type_parameters {
            total = total
                .checked_add(parameter.name.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        match &declaration.kind {
            crate::hir::ResolvedTypeDeclarationKind::Resource { drop } => {
                total = total
                    .checked_add(drop.id.as_str().len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if let crate::hir::ResolvedResourceDropKind::Imported { import, import_key } =
                    &drop.kind
                {
                    total = total
                        .checked_add(import.as_str().len())
                        .and_then(|bytes| bytes.checked_add(import_key.capacity()))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            crate::hir::ResolvedTypeDeclarationKind::Record { fields } => {
                total = total
                    .checked_add(
                        fields.capacity()
                            * std::mem::size_of::<crate::hir::ResolvedFieldDeclaration>(),
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                for field in fields {
                    total = total
                        .checked_add(field.id.as_str().len())
                        .and_then(|bytes| bytes.checked_add(field.name.capacity()))
                        .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&field.ty)?))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            crate::hir::ResolvedTypeDeclarationKind::Class { fields, methods } => {
                total = total
                    .checked_add(
                        fields.capacity()
                            * std::mem::size_of::<crate::hir::ResolvedFieldDeclaration>(),
                    )
                    .and_then(|bytes| {
                        bytes.checked_add(methods.capacity() * std::mem::size_of::<String>())
                    })
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                for field in fields {
                    total = total
                        .checked_add(field.id.as_str().len())
                        .and_then(|bytes| bytes.checked_add(field.name.capacity()))
                        .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&field.ty)?))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            crate::hir::ResolvedTypeDeclarationKind::Variant { cases } => {
                total = total
                    .checked_add(
                        cases.capacity()
                            * std::mem::size_of::<crate::hir::ResolvedVariantCaseDeclaration>(),
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                for case in cases {
                    total = total
                        .checked_add(case.id.as_str().len())
                        .and_then(|bytes| bytes.checked_add(case.name.capacity()))
                        .and_then(|bytes| {
                            bytes.checked_add(
                                case.fields.capacity()
                                    * std::mem::size_of::<crate::hir::ResolvedFieldDeclaration>(),
                            )
                        })
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    for field in &case.fields {
                        total = total
                            .checked_add(field.id.as_str().len())
                            .and_then(|bytes| bytes.checked_add(field.name.capacity()))
                            .and_then(|bytes| {
                                bytes.checked_add(hir_type_owned_capacity(&field.ty)?)
                            })
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                }
            }
        }
    }
    for template in &resolved.function_templates {
        total = total
            .checked_add(template.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(template.result_id.as_str().len()))
            .and_then(|bytes| bytes.checked_add(template.name.capacity()))
            .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&template.return_type)?))
            .and_then(|bytes| {
                bytes.checked_add(
                    template.type_parameters.capacity()
                        * std::mem::size_of::<crate::hir::ResolvedTypeParameterDeclaration>(),
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    template.params.capacity() * std::mem::size_of::<crate::hir::ResolvedParam>(),
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(template.effects.capacity() * std::mem::size_of::<String>())
            })
            .and_then(|bytes| {
                bytes
                    .checked_add(template.requires.capacity() * std::mem::size_of::<ResolvedExpr>())
            })
            .and_then(|bytes| {
                bytes.checked_add(template.ensures.capacity() * std::mem::size_of::<ResolvedExpr>())
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        for parameter in &template.type_parameters {
            total = total
                .checked_add(parameter.name.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        for parameter in &template.params {
            total = total
                .checked_add(parameter.id.as_str().len())
                .and_then(|bytes| bytes.checked_add(parameter.name.capacity()))
                .and_then(|bytes| bytes.checked_add(hir_type_owned_capacity(&parameter.ty)?))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        for effect in &template.effects {
            total = total
                .checked_add(effect.capacity())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        for expression in template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
        {
            total = total
                .checked_add(hir_expr_owned_capacity(expression)?)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
    }
    for instance in &resolved.function_instances {
        total = total
            .checked_add(instance.id.as_str().len())
            .and_then(|bytes| bytes.checked_add(instance.template.as_str().len()))
            .and_then(|bytes| {
                bytes.checked_add(
                    instance.type_arguments.capacity() * std::mem::size_of::<ResolvedType>(),
                )
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        for ty in &instance.type_arguments {
            total = total
                .checked_add(
                    hir_type_owned_capacity(ty)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        total = total
            .checked_add(hir_function_owned_capacity(&instance.function)?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    Ok(total)
}
