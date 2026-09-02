//! Resolved-type identity metrics, the exact identity writer, and the
//! scratch upper bounds those writers require.

use super::*;

#[derive(Clone, Copy, Debug)]
pub(in crate::implementation) struct TypeIdentityMetrics {
    nodes: usize,
    all_key_bytes: usize,
    root_bytes: usize,
    maximum_encoded_bytes: usize,
}

enum TypeIdentityFrame<'a> {
    Enter(&'a ResolvedType),
    Finish(&'a DeclarationId, usize, usize, usize),
}

#[derive(Clone, Copy)]
enum TypeIdentityMetricFrame<'a> {
    Enter(&'a ResolvedType, usize),
    Finish(&'a DeclarationId, usize),
}

fn decimal_bytes(mut value: usize) -> usize {
    let mut bytes = 1usize;
    while value >= 10 {
        value /= 10;
        bytes += 1;
    }
    bytes
}

pub(in crate::implementation) fn type_identity_metrics(
    ty: &ResolvedType,
    initial_depth: usize,
) -> Result<TypeIdentityMetrics, Diagnostic> {
    let leaf = |root_bytes| TypeIdentityMetrics {
        nodes: 1,
        all_key_bytes: root_bytes,
        root_bytes,
        maximum_encoded_bytes: 0,
    };
    let mut frames = [None; FINGERPRINT_ACTION_SLOTS];
    let mut frame_len = 1usize;
    frames[0] = Some(TypeIdentityMetricFrame::Enter(ty, initial_depth));
    let mut results = [None; FINGERPRINT_ACTION_SLOTS];
    let mut result_len = 0usize;
    let mut work = 0usize;
    while frame_len > 0 {
        frame_len -= 1;
        let frame = frames[frame_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match frame {
            TypeIdentityMetricFrame::Enter(ty, depth) => {
                if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                work = work
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if work > FINGERPRINT_ACTION_SLOTS {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                let metric = match ty {
                    ResolvedType::Unit => Some(leaf("unit".len())),
                    ResolvedType::I64 => Some(leaf("i64".len())),
                    ResolvedType::I32 => Some(leaf("i32".len())),
                    ResolvedType::Char => Some(leaf("char".len())),
                    ResolvedType::U8 => Some(leaf("u8".len())),
                    ResolvedType::Usize => Some(leaf("usize".len())),
                    ResolvedType::ArrayU8(length) => Some(leaf(
                        "array:u8:"
                            .len()
                            .checked_add(decimal_bytes(*length as usize))
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )),
                    ResolvedType::F32 => Some(leaf("f32".len())),
                    ResolvedType::F64 => Some(leaf("f64".len())),
                    ResolvedType::Bool => Some(leaf("bool".len())),
                    ResolvedType::String => Some(leaf("string".len())),
                    ResolvedType::Bytes => Some(leaf("bytes".len())),
                    ResolvedType::Str => Some(leaf("str".len())),
                    ResolvedType::SliceU8 => Some(leaf("slice-u8".len())),
                    ResolvedType::TypeParameter { owner, index } => {
                        let owner_bytes = owner.as_str().len();
                        let root_bytes = "parameter:"
                            .len()
                            .checked_add(decimal_bytes(owner_bytes))
                            .and_then(|bytes| bytes.checked_add(1))
                            .and_then(|bytes| bytes.checked_add(owner_bytes))
                            .and_then(|bytes| bytes.checked_add(1))
                            .and_then(|bytes| bytes.checked_add(decimal_bytes(*index as usize)))
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        Some(leaf(root_bytes))
                    }
                    ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } => {
                        if frame_len
                            .checked_add(arguments.len())
                            .and_then(|len| len.checked_add(1))
                            .is_none_or(|len| len > frames.len())
                        {
                            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                        }
                        frames[frame_len] = Some(TypeIdentityMetricFrame::Finish(
                            declaration,
                            arguments.len(),
                        ));
                        frame_len += 1;
                        for argument in arguments.iter().rev() {
                            frames[frame_len] =
                                Some(TypeIdentityMetricFrame::Enter(argument, depth + 1));
                            frame_len += 1;
                        }
                        None
                    }
                };
                if let Some(metric) = metric {
                    let slot = results
                        .get_mut(result_len)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    *slot = Some(metric);
                    result_len += 1;
                }
            }
            TypeIdentityMetricFrame::Finish(declaration, count) => {
                let split = result_len
                    .checked_sub(count)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let mut nodes = 1usize;
                let mut all_key_bytes = 0usize;
                let mut encoded_bytes = 0usize;
                let mut maximum_encoded_bytes = 0usize;
                for slot in &mut results[split..result_len] {
                    let child = slot
                        .take()
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    nodes = nodes
                        .checked_add(child.nodes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    all_key_bytes = all_key_bytes
                        .checked_add(child.all_key_bytes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    encoded_bytes = encoded_bytes
                        .checked_add(decimal_bytes(child.root_bytes))
                        .and_then(|bytes| bytes.checked_add(1))
                        .and_then(|bytes| bytes.checked_add(child.root_bytes))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    maximum_encoded_bytes = maximum_encoded_bytes.max(child.maximum_encoded_bytes);
                }
                let declaration_bytes = declaration.as_str().len();
                let root_bytes = "nominal:"
                    .len()
                    .checked_add(decimal_bytes(declaration_bytes))
                    .and_then(|bytes| bytes.checked_add(1))
                    .and_then(|bytes| bytes.checked_add(declaration_bytes))
                    .and_then(|bytes| bytes.checked_add(1))
                    .and_then(|bytes| bytes.checked_add(decimal_bytes(count)))
                    .and_then(|bytes| bytes.checked_add(1))
                    .and_then(|bytes| bytes.checked_add(encoded_bytes))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                result_len = split;
                results[result_len] = Some(TypeIdentityMetrics {
                    nodes,
                    all_key_bytes: all_key_bytes
                        .checked_add(root_bytes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    root_bytes,
                    maximum_encoded_bytes: maximum_encoded_bytes.max(encoded_bytes),
                });
                result_len += 1;
            }
        }
    }
    if result_len != 1 {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    results[0]
        .take()
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

pub(in crate::implementation) fn type_identity_scratch_upper(
    ty: &ResolvedType,
) -> Result<usize, Diagnostic> {
    let metrics = type_identity_metrics(ty, 1)?;
    metrics
        .nodes
        .checked_mul(std::mem::size_of::<TypeIdentityFrame<'_>>())
        .and_then(|bytes| {
            bytes.checked_add(metrics.nodes.checked_mul(std::mem::size_of::<String>())?)
        })
        .and_then(|bytes| bytes.checked_add(metrics.all_key_bytes))
        .and_then(|bytes| bytes.checked_add(metrics.maximum_encoded_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

pub(in crate::implementation) fn fingerprint_type_identity(
    ty: &ResolvedType,
    _capacity_baseline: usize,
    _outer_scratch: usize,
) -> Result<String, Diagnostic> {
    let metrics = type_identity_metrics(ty, 1)?;
    let mut frames = Vec::with_capacity(metrics.nodes);
    let mut keys = Vec::<String>::with_capacity(metrics.nodes);
    frames.push(TypeIdentityFrame::Enter(ty));
    while let Some(frame) = frames.pop() {
        match frame {
            TypeIdentityFrame::Enter(ty) => match ty {
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
                | ResolvedType::Bytes
                | ResolvedType::Str
                | ResolvedType::SliceU8 => {
                    let text = match ty {
                        ResolvedType::Unit => "unit",
                        ResolvedType::I64 => "i64",
                        ResolvedType::I32 => "i32",
                        ResolvedType::Char => "char",
                        ResolvedType::U8 => "u8",
                        ResolvedType::Usize => "usize",
                        ResolvedType::F32 => "f32",
                        ResolvedType::F64 => "f64",
                        ResolvedType::Bool => "bool",
                        ResolvedType::String => "string",
                        ResolvedType::Bytes => "bytes",
                        ResolvedType::Str => "str",
                        ResolvedType::SliceU8 => "slice-u8",
                        _ => unreachable!(),
                    };
                    let mut key = String::with_capacity(text.len());
                    key.push_str(text);
                    keys.push(key);
                }
                ResolvedType::ArrayU8(length) => {
                    let key_bytes = "array:u8:"
                        .len()
                        .checked_add(decimal_bytes(*length as usize))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    let mut key = String::with_capacity(key_bytes);
                    write!(key, "array:u8:{length}")
                        .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    keys.push(key);
                }
                ResolvedType::TypeParameter { owner, index } => {
                    let key_bytes = type_identity_metrics(ty, 1)?.root_bytes;
                    let mut key = String::with_capacity(key_bytes);
                    write!(key, "parameter:{}:{}:{index}", owner.as_str().len(), owner)
                        .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    keys.push(key);
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    let node = type_identity_metrics(ty, 1)?;
                    let encoded_bytes = arguments
                        .iter()
                        .try_fold(0usize, |bytes, argument| {
                            let child_bytes = type_identity_metrics(argument, 1).ok()?.root_bytes;
                            bytes
                                .checked_add(decimal_bytes(child_bytes))?
                                .checked_add(1)?
                                .checked_add(child_bytes)
                        })
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    frames.push(TypeIdentityFrame::Finish(
                        declaration,
                        arguments.len(),
                        encoded_bytes,
                        node.root_bytes,
                    ));
                    frames.extend(arguments.iter().rev().map(TypeIdentityFrame::Enter));
                }
            },
            TypeIdentityFrame::Finish(declaration, count, encoded_bytes, result_bytes) => {
                let split = keys
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let mut encoded = String::with_capacity(encoded_bytes);
                for key in &keys[split..] {
                    write!(encoded, "{}:{key}", key.len())
                        .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                let mut result = String::with_capacity(result_bytes);
                write!(
                    result,
                    "nominal:{}:{}:{}:{}",
                    declaration.as_str().len(),
                    declaration,
                    count,
                    encoded
                )
                .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                #[cfg(test)]
                note_post_hir_facts_live(
                    _capacity_baseline,
                    _outer_scratch
                        .saturating_add(
                            frames
                                .capacity()
                                .saturating_mul(std::mem::size_of::<TypeIdentityFrame<'_>>()),
                        )
                        .saturating_add(
                            keys.capacity()
                                .saturating_mul(std::mem::size_of::<String>()),
                        )
                        .saturating_add(keys.iter().map(String::capacity).sum::<usize>())
                        .saturating_add(encoded.capacity())
                        .saturating_add(result.capacity()),
                );
                keys.truncate(split);
                keys.push(result);
            }
        }
        #[cfg(test)]
        note_post_hir_facts_live(
            _capacity_baseline,
            _outer_scratch
                .saturating_add(
                    frames
                        .capacity()
                        .saturating_mul(std::mem::size_of::<TypeIdentityFrame<'_>>()),
                )
                .saturating_add(
                    keys.capacity()
                        .saturating_mul(std::mem::size_of::<String>()),
                )
                .saturating_add(keys.iter().map(String::capacity).sum::<usize>()),
        );
    }
    if keys.len() != 1 {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    Ok(keys.pop().expect("one checked type identity"))
}

fn fingerprint_binding_type_scratch(
    binding: &crate::hir::ResolvedBinding,
) -> Result<usize, Diagnostic> {
    type_identity_scratch_upper(&binding.ty)
}

fn fingerprint_record_pattern_types_scratch(
    fields: &[crate::hir::ResolvedRecordMatchPatternField],
) -> Result<usize, Diagnostic> {
    fields.iter().try_fold(0usize, |maximum, field| {
        let current = match &field.pattern {
            crate::hir::ResolvedRecordMatchFieldPattern::Binding(binding) => {
                fingerprint_binding_type_scratch(binding)?
            }
            crate::hir::ResolvedRecordMatchFieldPattern::Wildcard => 0,
            crate::hir::ResolvedRecordMatchFieldPattern::Record {
                instance, fields, ..
            } => type_identity_scratch_upper(instance)?
                .max(fingerprint_record_pattern_types_scratch(fields)?),
        };
        Ok(maximum.max(current))
    })
}

fn fingerprint_pattern_types_scratch(
    pattern: &crate::hir::ResolvedMatchPattern,
) -> Result<usize, Diagnostic> {
    match pattern {
        crate::hir::ResolvedMatchPattern::Wildcard
        | crate::hir::ResolvedMatchPattern::Literal(_) => Ok(0),
        crate::hir::ResolvedMatchPattern::Binding(binding) => {
            fingerprint_binding_type_scratch(binding)
        }
        crate::hir::ResolvedMatchPattern::Or(alternatives) => alternatives
            .iter()
            .try_fold(0usize, |maximum, alternative| {
                Ok(maximum.max(fingerprint_pattern_types_scratch(alternative)?))
            }),
        crate::hir::ResolvedMatchPattern::Variant { fields, .. } => {
            fields.iter().try_fold(0usize, |maximum, field| {
                Ok(maximum.max(fingerprint_binding_type_scratch(&field.binding)?))
            })
        }
        crate::hir::ResolvedMatchPattern::Record {
            instance, fields, ..
        } => Ok(type_identity_scratch_upper(instance)?
            .max(fingerprint_record_pattern_types_scratch(fields)?)),
    }
}

pub(in crate::implementation) fn fingerprint_expression_types_scratch(
    expression: &ResolvedExpr,
    depth: usize,
) -> Result<usize, Diagnostic> {
    #[derive(Clone, Copy)]
    enum Frame<'a> {
        Expr(&'a ResolvedExpr, usize),
        Exprs(&'a [ResolvedExpr], usize, usize),
        Statements(&'a [ResolvedStatement], usize, usize),
        Fields(&'a [crate::hir::ResolvedFieldInitializer], usize, usize),
        Arms(&'a [crate::hir::ResolvedMatchArm], usize, usize),
    }
    fn push<'a>(
        stack: &mut [Option<Frame<'a>>],
        stack_len: &mut usize,
        frame: Frame<'a>,
    ) -> Result<(), Diagnostic> {
        let slot = stack.get_mut(*stack_len).ok_or_else(|| {
            b109(
                "max_semantic_expression_depth",
                MAX_SEMANTIC_EXPRESSION_DEPTH,
            )
        })?;
        *slot = Some(frame);
        *stack_len += 1;
        Ok(())
    }

    let mut stack = [None; FINGERPRINT_ACTION_SLOTS];
    let mut stack_len = 0usize;
    push(&mut stack, &mut stack_len, Frame::Expr(expression, depth))?;
    let mut maximum = 0usize;
    while stack_len > 0 {
        stack_len -= 1;
        let frame = stack[stack_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match frame {
            Frame::Expr(expression, depth) => {
                if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_depth = depth.checked_add(1).ok_or_else(|| {
                    b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    )
                })?;
                maximum = maximum.max(type_identity_scratch_upper(&expression.ty)?);
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
                    | ResolvedExprKind::String(_)
                    | ResolvedExprKind::Place(_)
                    | ResolvedExprKind::BorrowPlace { .. } => {}
                    ResolvedExprKind::ByteRange {
                        source, start, end, ..
                    } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(end, child_depth))?;
                        push(&mut stack, &mut stack_len, Frame::Expr(start, child_depth))?;
                        push(&mut stack, &mut stack_len, Frame::Expr(source, child_depth))?;
                    }
                    ResolvedExprKind::Call {
                        type_arguments,
                        args,
                        ..
                    } => {
                        for ty in type_arguments {
                            maximum = maximum.max(type_identity_scratch_upper(ty)?);
                        }
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Exprs(args, 0, child_depth),
                        )?;
                    }
                    ResolvedExprKind::NativeRustImportCall(call) => push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Exprs(&call.args, 0, child_depth),
                    )?,
                    ResolvedExprKind::HostCommandCall(call) => push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Exprs(&call.args, 0, child_depth),
                    )?,
                    ResolvedExprKind::Unary { value, .. } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(value, child_depth))?
                    }
                    ResolvedExprKind::Binary { left, right, .. } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(right, child_depth))?;
                        push(&mut stack, &mut stack_len, Frame::Expr(left, child_depth))?;
                    }
                    ResolvedExprKind::Block { statements, tail } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(tail, child_depth))?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Statements(statements, 0, child_depth),
                        )?;
                    }
                    ResolvedExprKind::If {
                        condition,
                        then_branch,
                        else_branch,
                    } => {
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(else_branch, child_depth),
                        )?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(then_branch, child_depth),
                        )?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(condition, child_depth),
                        )?;
                    }
                    ResolvedExprKind::ConstructRecord { fields, .. }
                    | ResolvedExprKind::ConstructVariant { fields, .. } => push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Fields(fields, 0, child_depth),
                    )?,
                    ResolvedExprKind::Match {
                        scrutinee, arms, ..
                    } => {
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Arms(arms, 0, child_depth),
                        )?;
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(scrutinee, child_depth),
                        )?;
                    }
                    ResolvedExprKind::Try {
                        operand,
                        residual_type,
                        ..
                    }
                    | ResolvedExprKind::TryOption {
                        operand,
                        residual_type,
                        ..
                    } => {
                        maximum = maximum.max(type_identity_scratch_upper(residual_type)?);
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Expr(operand, child_depth),
                        )?;
                    }
                    ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                        push(
                            &mut stack,
                            &mut stack_len,
                            Frame::Fields(fields, 0, child_depth),
                        )?;
                        push(&mut stack, &mut stack_len, Frame::Expr(base, child_depth))?;
                    }
                    ResolvedExprKind::Project { base, .. } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(base, child_depth))?
                    }
                    ResolvedExprKind::Upcast { source } => {
                        push(&mut stack, &mut stack_len, Frame::Expr(source, child_depth))?
                    }
                }
            }
            Frame::Exprs(expressions, index, depth) => {
                if let Some(expression) = expressions.get(index) {
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Exprs(expressions, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(expression, depth))?;
                }
            }
            Frame::Statements(statements, index, depth) => {
                if let Some(statement) = statements.get(index) {
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Statements(statements, index + 1, depth),
                    )?;
                    match statement {
                        ResolvedStatement::Let { binding, value, .. }
                        | ResolvedStatement::Assign { binding, value, .. } => {
                            maximum = maximum.max(fingerprint_binding_type_scratch(binding)?);
                            push(&mut stack, &mut stack_len, Frame::Expr(value, depth))?;
                        }
                        ResolvedStatement::Unsafe { body, .. } => {
                            push(&mut stack, &mut stack_len, Frame::Expr(body, depth))?;
                        }
                        ResolvedStatement::While {
                            condition, body, ..
                        } => {
                            push(&mut stack, &mut stack_len, Frame::Expr(body, depth))?;
                            push(&mut stack, &mut stack_len, Frame::Expr(condition, depth))?;
                        }
                    }
                }
            }
            Frame::Fields(fields, index, depth) => {
                if let Some(field) = fields.get(index) {
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Fields(fields, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(&field.value, depth))?;
                }
            }
            Frame::Arms(arms, index, depth) => {
                if let Some(arm) = arms.get(index) {
                    maximum = maximum.max(fingerprint_pattern_types_scratch(&arm.pattern)?);
                    push(
                        &mut stack,
                        &mut stack_len,
                        Frame::Arms(arms, index + 1, depth),
                    )?;
                    push(&mut stack, &mut stack_len, Frame::Expr(&arm.value, depth))?;
                    if let Some(guard) = &arm.guard {
                        push(&mut stack, &mut stack_len, Frame::Expr(guard, depth))?;
                    }
                }
            }
        }
    }
    Ok(maximum)
}

pub(in crate::implementation) fn fingerprint_type_scratch_upper(
    closure: &[&ResolvedFunction],
) -> Result<usize, Diagnostic> {
    closure.iter().try_fold(0usize, |mut maximum, function| {
        maximum = maximum.max(type_identity_scratch_upper(&function.return_type)?);
        for parameter in &function.params {
            maximum = maximum.max(type_identity_scratch_upper(&parameter.ty)?);
        }
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            maximum = maximum.max(fingerprint_expression_types_scratch(expression, 1)?);
        }
        Ok(maximum)
    })
}
