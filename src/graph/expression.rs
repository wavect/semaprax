//! Complete expression projection shared by additive graph schemas.
use super::*;

pub(super) fn expr_json(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
) -> Result<String, Diagnostic> {
    let header = format!(
        "\"id\":{},\"type_id\":{},\"ownership_mode\":{}",
        quote_json(expression.id.as_str()),
        quote_json(&expression.ty.identity_key()),
        quote_json(ownership_text(expression.ownership))
    );
    let output = match &expression.kind {
        ResolvedExprKind::FunctionReference {..} | ResolvedExprKind::Invoke {..} => function_values::expression_json(program,expression,&header)?,
        ResolvedExprKind::Int(value) => {
            format!(
                "{{{header},\"kind\":\"int\",\"value\":{}}}",
                quote_json(&value.to_string())
            )
        }
        ResolvedExprKind::Int32(value) => {
            format!("{{{header},\"kind\":\"int32\",\"value\":{value}}}")
        }
        ResolvedExprKind::Char(value) => format!(
            "{{{header},\"kind\":\"char\",\"value\":{value},\"display\":{}}}",
            quote_json(&crate::format::canonical_char(*value))
        ),
        ResolvedExprKind::Uint8(value) => {
            format!("{{{header},\"kind\":\"uint8\",\"value\":{value}}}")
        }
        ResolvedExprKind::Usize(value) => format!(
            "{{{header},\"kind\":\"usize\",\"value\":{}}}",
            quote_json(&value.to_string())
        ),
        ResolvedExprKind::ArrayU8(values) => format!(
            "{{{header},\"kind\":\"array_u8\",\"form\":\"explicit\",\"length\":{},\"values\":[{}]}}",
            values.len(),
            values.iter().map(u8::to_string).collect::<Vec<_>>().budgeted_join(",")
        ),
        ResolvedExprKind::RepeatArrayU8 { value, count } => format!(
            "{{{header},\"kind\":\"array_u8\",\"form\":\"repeat\",\"length\":{count},\"value\":{value}}}"
        ),
        ResolvedExprKind::Float32(bits) => format!(
            "{{{header},\"kind\":\"float32\",\"bits\":\"{bits:08x}\",\"value\":{}}}",
            quote_json(&crate::format::canonical_f32_bits(*bits))
        ),
        ResolvedExprKind::Float64(bits) => format!(
            "{{{header},\"kind\":\"float64\",\"bits\":\"{bits:016x}\",\"value\":{}}}",
            quote_json(&crate::format::canonical_f64_bits(*bits))
        ),
        ResolvedExprKind::Bool(value) => {
            format!("{{{header},\"kind\":\"bool\",\"value\":{value}}}")
        }
        ResolvedExprKind::String(value) => format!(
            "{{{header},\"kind\":\"string\",\"value\":{},\"display\":{}}}",
            quote_json(value),
            quote_json(&crate::format::canonical_string(value))
        ),
        ResolvedExprKind::Place(place) => format!(
            "{{{header},\"kind\":\"place\",\"place\":{}}}",
            place_json(place)
        ),
        ResolvedExprKind::BorrowPlace { operation, place } => format!(
            "{{{header},\"kind\":\"byte_view\",\"operation\":{},\"place\":{}}}",
            quote_json(operation.as_str()),
            place_json(place)
        ),
        ResolvedExprKind::ByteRange { operation, source, start, end } => format!(
            "{{{header},\"kind\":\"byte_range\",\"operation\":{},\"source\":{},\"start\":{},\"end\":{},\"status_domain\":{},\"status_codes\":{{\"start_after_end\":{},\"end_out_of_bounds\":{}}}}}",
            quote_json(operation.as_str()), expr_json(program, source)?,
            expr_json(program, start)?, expr_json(program, end)?,
            quote_json(crate::byte_ops::RANGE_STATUS_DOMAIN),
            crate::byte_ops::RANGE_START_AFTER_END_CODE,
            crate::byte_ops::RANGE_END_OUT_OF_BOUNDS_CODE,
        ),
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
        } => {
            let args = args
                .iter()
                .map(|argument| expr_json(program, argument))
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",");
            if let Some(instance) = instance {
                format!(
                    "{{{header},\"kind\":\"call_instance\",\"template\":{},\"instance\":{},\"type_arguments\":[{}],\"args\":[{}]}}",
                    quote_json(callee.as_str()),
                    quote_json(instance.as_str()),
                    type_arguments.iter().map(type_json).collect::<Vec<_>>().budgeted_join(","),
                    args
                )
            } else {
                format!(
                    "{{{header},\"kind\":\"call\",\"callee\":{},\"args\":[{}]}}",
                    quote_json(callee.as_str()),
                    args
                )
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let args = call
                .args
                .iter()
                .map(|argument| expr_json(program, argument))
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",");
            format!(
                "{{{header},\"kind\":\"native_rust_import_call\",\"import\":{},\"result\":{},\"args\":[{}]}}",
                quote_json(call.import.as_str()),
                quote_json(native_import::result_text(&call.result)),
                args
            )
        }
        ResolvedExprKind::HostCommandCall(call) => {
            let args = call
                .args
                .iter()
                .map(|argument| expr_json(program, argument))
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",");
            format!(
                "{{{header},\"kind\":\"host_command_call\",\"operation\":{},\"args\":[{}]}}",
                quote_json(crate::command_io_ops::id(call.operation)),
                args
            )
        }
        ResolvedExprKind::Unary { op, value } => format!(
            "{{{header},\"kind\":\"unary\",\"op\":{},\"value\":{}}}",
            quote_json(unary_text(*op)),
            expr_json(program, value)?
        ),
        ResolvedExprKind::Binary { op, left, right } => format!(
            "{{{header},\"kind\":\"binary\",\"op\":{},\"left\":{},\"right\":{}}}",
            quote_json(binary_text(*op)),
            expr_json(program, left)?,
            expr_json(program, right)?
        ),
        ResolvedExprKind::Block { statements, tail } => format!(
            "{{{header},\"kind\":\"block\",\"statements\":[{}],\"tail\":{}}}",
            statements
                .iter()
                .map(|statement| statement_json(program, statement))
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(","),
            expr_json(program, tail)?
        ),
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "{{{header},\"kind\":\"if\",\"condition\":{},\"then\":{},\"else\":{}}}",
            expr_json(program, condition)?,
            expr_json(program, then_branch)?,
            expr_json(program, else_branch)?
        ),
        ResolvedExprKind::ConstructRecord { record, fields } => {
            let instance = match &expression.ty {
                ResolvedType::Nominal { arguments, .. } if !arguments.is_empty() => {
                    format!(",\"record_type\":{}", type_json(&expression.ty))
                }
                _ => String::new(),
            };
            format!(
                "{{{header},\"kind\":\"construct_record\",\"record\":{}{instance},\"fields\":[{}]}}",
                quote_json(record.as_str()),
                fields
                    .iter()
                    .map(|initializer| {
                        Ok(format!(
                            "{{\"field\":{},\"value\":{}}}",
                            quote_json(initializer.field.as_str()),
                            expr_json(program, &initializer.value)?
                        ))
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?
                    .budgeted_join(",")
            )
        }
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => format!(
            "{{{header},\"kind\":\"construct_variant\",\"variant\":{},\"case\":{},\"fields\":[{}]}}",
            quote_json(variant.as_str()),
            quote_json(case.as_str()),
            fields
                .iter()
                .map(|initializer| {
                    Ok(format!(
                        "{{\"field\":{},\"value\":{}}}",
                        quote_json(initializer.field.as_str()),
                        expr_json(program, &initializer.value)?
                    ))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .budgeted_join(",")
        ),
        ResolvedExprKind::Match {
            mode,
            scrutinee,
            arms,
        } => {
            // Refutable Match v1: matches carrying guards or literal/or
            // patterns project `"exhaustive":false` plus additive per-arm
            // guard nodes; every pre-feature match keeps the exact
            // `"exhaustive":true` bytes.
            let exhaustive = !arms.iter().any(|arm| {
                arm.guard.is_some()
                    || matches!(
                        &arm.pattern,
                        crate::hir::ResolvedMatchPattern::Literal(_)
                            | crate::hir::ResolvedMatchPattern::Or(_)
                            | crate::hir::ResolvedMatchPattern::Binding(_)
                    )
            });
            format!(
                "{{{header},\"kind\":\"match\"{},\"exhaustive\":{exhaustive},\"scrutinee\":{},\"arms\":[{}]}}",
                explicit_match_mode_json(*mode),
                expr_json(program, scrutinee)?,
                arms.iter()
                    .enumerate()
                    .map(|(index, arm)| {
                        let arm_id = format!("{}:match-arm:{index}", expression.id.as_str());
                        let pattern_id = format!("{arm_id}:pattern");
                        let guard = match &arm.guard {
                            Some(guard) => {
                                let guard_id = format!("{arm_id}:guard");
                                format!(
                                    ",\"guard\":{{\"id\":{},\"kind\":\"guard\",\"condition\":{}}}",
                                    quote_json(&guard_id),
                                    expr_json(program, guard)?
                                )
                            }
                            None => String::new(),
                        };
                        Ok(format!(
                            "{{\"id\":{},\"kind\":\"match_arm\",\"pattern\":{},\"value\":{}{guard}}}",
                            quote_json(&arm_id),
                            graph_match_pattern_json(&arm.pattern, &pattern_id),
                            expr_json(program, &arm.value)?
                        ))
                    })
                    .collect::<Result<Vec<_>, Diagnostic>>()?
                    .budgeted_join(",")
            )
        }
        ResolvedExprKind::Try {
            operand,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            residual_type,
        } => format!(
            "{{{header},\"kind\":\"try_result\",\"evaluation\":\"once\",\"operand\":{},\"source_result_type_id\":{},\"source_result_type\":{},\"residual_result_type_id\":{},\"residual_result_type\":{},\"result\":{},\"ok_case\":{},\"ok_field\":{},\"err_case\":{},\"err_field\":{},\"err_exit\":\"normal_result\",\"epilogue\":\"shared_postconditions\"}}",
            expr_json(program, operand)?,
            quote_json(&operand.ty.identity_key()),
            type_json(&operand.ty),
            quote_json(&residual_type.identity_key()),
            type_json(residual_type),
            quote_json(result.as_str()),
            quote_json(ok_case.as_str()),
            quote_json(ok_field.as_str()),
            quote_json(err_case.as_str()),
            quote_json(err_field.as_str())
        ),
        ResolvedExprKind::TryOption {
            operand,
            option,
            some_case,
            some_field,
            none_case,
            residual_type,
        } => format!(
            "{{{header},\"kind\":\"try_option\",\"evaluation\":\"once\",\"operand\":{},\"source_option_type_id\":{},\"source_option_type\":{},\"residual_option_type_id\":{},\"residual_option_type\":{},\"option\":{},\"some_case\":{},\"some_field\":{},\"none_case\":{},\"none_exit\":\"normal_result\",\"epilogue\":\"shared_postconditions\"}}",
            expr_json(program, operand)?,
            quote_json(&operand.ty.identity_key()),
            type_json(&operand.ty),
            quote_json(&residual_type.identity_key()),
            type_json(residual_type),
            quote_json(option.as_str()),
            quote_json(some_case.as_str()),
            quote_json(some_field.as_str()),
            quote_json(none_case.as_str())
        ),
        ResolvedExprKind::UpdateRecord {
            base,
            record,
            fields,
        } => format!(
            "{{{header},\"kind\":\"update_record\",\"base\":{},\"record\":{},\"fields\":[{}]}}",
            expr_json(program, base)?,
            quote_json(record.as_str()),
            fields
                .iter()
                .map(|initializer| {
                    Ok(format!(
                        "{{\"field\":{},\"value\":{}}}",
                        quote_json(initializer.field.as_str()),
                        expr_json(program, &initializer.value)?
                    ))
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?
                .budgeted_join(",")
        ),
        ResolvedExprKind::Project { base, field } => format!(
            "{{{header},\"kind\":\"project\",\"base\":{},\"field\":{}}}",
            expr_json(program, base)?,
            quote_json(field.as_str())
        ),
        ResolvedExprKind::Upcast { source } => format!(
            "{{{header},\"kind\":\"upcast\",\"source\":{}}}",
            expr_json(program, source)?
        ),
    };
    Ok(output)
}

fn statement_json(
    program: &ResolvedProgram,
    statement: &ResolvedStatement,
) -> Result<String, Diagnostic> {
    match statement {
        ResolvedStatement::Let {
            binding,
            mutable,
            value,
            ..
        } => {
            // The mutable flag is additive and emitted only for `let mut`
            // bindings so pre-mutation graphs stay byte-identical.
            let mutable_field = if *mutable { ",\"mutable\":true" } else { "" };
            Ok(format!(
                "{{\"kind\":\"let\",\"binding\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}}{},\"value\":{}}}",
                quote_json(binding.id.as_str()),
                quote_json(&binding.name),
                quote_json(&binding.ty.identity_key()),
                quote_json(ownership_text(binding.ownership)),
                mutable_field,
                expr_json(program, value)?
            ))
        }
        ResolvedStatement::Assign {
            binding,
            field,
            value,
            ..
        } => {
            // The field attribute is additive and emitted only on
            // `<binding>.<field>` targets so pre-field-mutation graphs stay
            // byte-identical.
            let field_attribute = match field {
                Some(field) => format!(",\"field\":{}", quote_json(field.as_str())),
                None => String::new(),
            };
            Ok(format!(
                "{{\"kind\":\"assign\",\"target\":{{\"id\":{},\"name\":{},\"type_id\":{},\"ownership_mode\":{}}},\"value\":{}{field_attribute}}}",
                quote_json(binding.id.as_str()),
                quote_json(&binding.name),
                quote_json(&binding.ty.identity_key()),
                quote_json(ownership_text(binding.ownership)),
                expr_json(program, value)?
            ))
        }
        ResolvedStatement::Unsafe { audit, body, .. } => Ok(format!(
            "{{\"kind\":\"unsafe\",\"audit\":{},\"body\":{}}}",
            quote_json(audit),
            expr_json(program, body)?
        )),
        ResolvedStatement::While {
            condition, body, ..
        } => Ok(format!(
            "{{\"kind\":\"while\",\"condition\":{},\"body\":{}}}",
            expr_json(program, condition)?,
            expr_json(program, body)?
        )),
    }
}
