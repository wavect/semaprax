//! Independent replay of the generated C artifact. The frame set, scratch
//! census, and emitter here are deliberately separate from the generator.

use super::*;

// Intentionally separate from `CExpressionFrame`: exact replay must not share
// the generator's scheduling state or traversal implementation.
enum ReplayCExpressionFrame<'a> {
    Evaluate(&'a ResolvedExpr),
    FinishUnary(crate::ast::UnaryOp),
    FinishBinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    FinishBinary(crate::ast::BinaryOp, String),
    FinishLazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    FinishLazy(String),
    ContinueBlock(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    FinishBinding(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    FinishAssignment(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    FinishCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType),
    FinishThen(&'a ResolvedExpr, Option<String>),
    FinishElse(Option<String>),
    ContinueNative(&'a crate::hir::ResolvedNativeRustImportCall, usize, usize),
    ContinueCall(&'a str, &'a [ResolvedExpr], &'a ResolvedType, usize, usize),
}

pub(in crate::implementation) const REPLAY_C_EXPRESSION_FRAME_BYTES: usize =
    std::mem::size_of::<ReplayCExpressionFrame<'static>>();

#[cfg(any())]
fn replay_c_expression(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut Vec<String>,
) -> Result<String, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedExpr, usize),
        Unary(crate::ast::UnaryOp, usize),
        BinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        BinaryRight(crate::ast::BinaryOp, String, usize),
        LazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        LazyRight(crate::ast::BinaryOp, String, usize, usize),
        Block(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        BlockLet(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        BlockAssign(&'a [ResolvedStatement], usize, &'a ResolvedExpr, usize),
        IfCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType, usize),
        IfThen(String, &'a ResolvedExpr, Option<String>, usize, usize),
        IfElse(String, Option<String>, String, usize, usize, usize),
        NativeArgs(
            &'a crate::hir::ResolvedNativeRustImportCall,
            usize,
            Vec<String>,
            usize,
        ),
        CallArgs(
            &'a str,
            &'a [ResolvedExpr],
            &'a ResolvedType,
            usize,
            Vec<String>,
            usize,
        ),
    }
    const _: () = assert!(std::mem::size_of::<Frame<'static>>() == C_EXPRESSION_FRAME_BYTES);
    let next_temporary = |count: &mut usize| {
        let value = format!("tmp_{}", *count);
        *count += 1;
        value
    };
    let (node_count, depth) = c_expression_shape(expression)?;
    let line_capacity = node_count
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if lines.capacity() < line_capacity {
        lines
            .try_reserve_exact(line_capacity - lines.capacity())
            .map_err(|_| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let frame_capacity = node_count
        .checked_mul(2)
        .and_then(|slots| slots.checked_add(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(frame_capacity);
    let mut values = Vec::<String>::with_capacity(depth + 1);
    let mut contexts = Vec::<Vec<String>>::with_capacity(node_count + 1);
    contexts.push(Vec::with_capacity(line_capacity));
    frames.push(Frame::Enter(expression, 0));
    while let Some(frame) = frames.pop() {
        #[cfg(test)]
        {
            let frame_payload = |frame: &Frame<'_>| match frame {
                Frame::BinaryRight(_, value, _)
                | Frame::LazyRight(_, value, _, _)
                | Frame::IfThen(value, _, _, _, _)
                | Frame::IfElse(value, _, _, _, _, _) => value.capacity(),
                Frame::NativeArgs(_, _, values, _) | Frame::CallArgs(_, _, _, _, values, _) => {
                    values.capacity() * std::mem::size_of::<String>()
                        + values.iter().map(String::capacity).sum::<usize>()
                }
                _ => 0,
            };
            let frame_owned = frames.iter().map(&frame_payload).sum::<usize>();
            let owned = frames.capacity() * std::mem::size_of::<Frame<'_>>()
                + frame_owned
                + frame_payload(&frame)
                + values.capacity() * std::mem::size_of::<String>()
                + values.iter().map(String::capacity).sum::<usize>()
                + contexts.capacity() * std::mem::size_of::<Vec<String>>()
                + contexts
                    .iter()
                    .map(|context| {
                        context.capacity() * std::mem::size_of::<String>()
                            + context.iter().map(String::capacity).sum::<usize>()
                    })
                    .sum::<usize>()
                + lines.capacity() * std::mem::size_of::<String>()
                + lines.iter().map(String::capacity).sum::<usize>();
            note_post_hir_replay_capacity(owned);
        }
        match frame {
            Frame::Enter(expression, context) => match &expression.kind {
                ResolvedExprKind::Int(value) => values.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    values.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                    values.push(format!("v_{}", replay_symbol_hash(place.root.as_str())))
                }
                ResolvedExprKind::NativeRustImportCall(call) => frames.push(Frame::NativeArgs(
                    call,
                    0,
                    Vec::with_capacity(call.args.len()),
                    context,
                )),
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(Frame::Unary(*op, context));
                    frames.push(Frame::Enter(value, context));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(Frame::LazyLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(Frame::BinaryLeft(*op, right, context));
                    frames.push(Frame::Enter(left, context));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(Frame::Block(statements, 0, tail, context));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = replay_resolved_scalar(&expression.ty).ok_or_else(b111)?;
                    frames.push(Frame::IfCondition(then_branch, else_branch, ty, context));
                    frames.push(Frame::Enter(condition, context));
                }
                ResolvedExprKind::Call { callee, args, .. } => frames.push(Frame::CallArgs(
                    callee.as_str(),
                    args,
                    &expression.ty,
                    0,
                    Vec::with_capacity(args.len()),
                    context,
                )),
                _ => return Err(b107("scalar value signature required")),
            },
            Frame::Unary(op, context) => {
                let value = values.pop().ok_or_else(b111)?;
                if op == crate::ast::UnaryOp::Not {
                    values.push(format!("(!({value}))"));
                } else {
                    let name = next_temporary(temporary_count);
                    contexts[context].push(format!("if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});"));
                    values.push(name);
                }
            }
            Frame::BinaryLeft(op, right, context) => {
                let left = values.pop().ok_or_else(b111)?;
                frames.push(Frame::BinaryRight(op, left, context));
                frames.push(Frame::Enter(right, context));
            }
            Frame::BinaryRight(op, left, context) => {
                let right = values.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = next_temporary(temporary_count);
                    contexts[context].push(format!("int64_t {name};"));
                    contexts[context].push(match op {
                        crate::ast::BinaryOp::Add => format!("if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => format!("if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => format!("if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    });
                    values.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    values.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            Frame::LazyLeft(op, right, context) => {
                let left = values.pop().ok_or_else(b111)?;
                let name = next_temporary(temporary_count);
                contexts[context].push(format!("uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);"));
                let branch = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::LazyRight(op, name, context, branch));
                frames.push(Frame::Enter(right, branch));
            }
            Frame::LazyRight(op, name, context, branch) => {
                let right = values.pop().ok_or_else(b111)?;
                let branch_lines = take_c_lines(&mut contexts[branch]);
                contexts[branch] = Vec::new();
                let condition = if op == crate::ast::BinaryOp::And {
                    name.clone()
                } else {
                    format!("!{name}")
                };
                contexts[context].push(format!(
                    "if({condition}){{{branch_lines} {name}=({right})?UINT8_C(1):UINT8_C(0);}}"
                ));
                values.push(name);
            }
            Frame::Block(statements, index, tail, context) => match statements.get(index) {
                Some(ResolvedStatement::Let { value, .. }) => {
                    frames.push(Frame::BlockLet(statements, index, tail, context));
                    frames.push(Frame::Enter(value, context));
                }
                Some(ResolvedStatement::Assign { value, .. }) => {
                    frames.push(Frame::BlockAssign(statements, index, tail, context));
                    frames.push(Frame::Enter(value, context));
                }
                _ => frames.push(Frame::Enter(tail, context)),
            },
            Frame::BlockLet(statements, index, tail, context) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at a let");
                };
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "{} v_{} = {value};",
                        replay_c_scalar(ty),
                        replay_symbol_hash(binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::BlockAssign(statements, index, tail, context) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Assign { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at an assignment");
                };
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "v_{} = {value};",
                        replay_symbol_hash(binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::IfCondition(then_branch, else_branch, ty, context) => {
                let condition = values.pop().ok_or_else(b111)?;
                let name = (ty != ScalarType::Unit).then(|| next_temporary(temporary_count));
                if let Some(name) = &name {
                    contexts[context].push(format!("{} {name};", replay_c_scalar(ty)));
                }
                let then_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfThen(
                    condition,
                    else_branch,
                    name,
                    context,
                    then_context,
                ));
                frames.push(Frame::Enter(then_branch, then_context));
            }
            Frame::IfThen(condition, else_branch, name, context, then_context) => {
                let then_value = values.pop().ok_or_else(b111)?;
                let else_context = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::IfElse(
                    condition,
                    name,
                    then_value,
                    context,
                    then_context,
                    else_context,
                ));
                frames.push(Frame::Enter(else_branch, else_context));
            }
            Frame::IfElse(condition, name, then_value, context, then_context, else_context) => {
                let else_value = values.pop().ok_or_else(b111)?;
                let then_lines = take_c_lines(&mut contexts[then_context]);
                let else_lines = take_c_lines(&mut contexts[else_context]);
                contexts[then_context] = Vec::new();
                contexts[else_context] = Vec::new();
                if let Some(name) = name {
                    contexts[context].push(format!("if({condition}){{{then_lines}{name}={then_value};}}else{{{else_lines}{name}={else_value};}}"));
                    values.push(name);
                } else {
                    contexts[context].push(format!(
                        "if({condition}){{{then_lines}}}else{{{else_lines}}}"
                    ));
                    values.push("INT64_C(0)".to_owned());
                }
            }
            Frame::NativeArgs(call, index, mut args, context) => {
                if index < call.args.len() {
                    if index > 0 {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::NativeArgs(call, index + 1, args, context));
                    frames.push(Frame::Enter(&call.args[index], context));
                } else {
                    if !call.args.is_empty() {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = if import.result == ScalarType::Unit {
                        format!("tmp_{}", *temporary_count)
                    } else {
                        next_temporary(temporary_count)
                    };
                    if import.result != ScalarType::Unit {
                        contexts[context]
                            .push(format!("{} {name};", replay_c_scalar(import.result)));
                    }
                    contexts[context].push(format!("status = ctx->imports->{}(ctx->userdata{}{}{}); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.c_field, if args.is_empty() { "" } else { ", " }, args.join(", "), if import.result == ScalarType::Unit { String::new() } else { format!(", &{name}") }, import.rust_method));
                    if import.result == ScalarType::Bool {
                        contexts[context]
                            .push(format!("if ({name} > UINT8_C(1)) return spxnr_adapter(4);"));
                    }
                    values.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            Frame::CallArgs(callee, args_source, ty, index, mut args, context) => {
                if index < args_source.len() {
                    if index > 0 {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::CallArgs(
                        callee,
                        args_source,
                        ty,
                        index + 1,
                        args,
                        context,
                    ));
                    frames.push(Frame::Enter(&args_source[index], context));
                } else {
                    if !args_source.is_empty() {
                        args.push(values.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        contexts[context].push(format!(
                            "status=spxnr1_f_{}(ctx{}{});if(status!=0)return status;",
                            replay_symbol_hash(callee),
                            if args.is_empty() { "" } else { ", " },
                            args.join(",")
                        ));
                        values.push("INT64_C(0)".to_owned());
                    } else {
                        let name = next_temporary(temporary_count);
                        contexts[context].push(format!("{} {name};status=spxnr1_f_{}(ctx{}{},&{name});if(status!=0)return status;", replay_c_scalar(replay_resolved_scalar(ty).ok_or_else(b111)?), replay_symbol_hash(callee), if args.is_empty() { "" } else { ", " }, args.join(",")));
                        values.push(name);
                    }
                }
            }
        }
    }
    if values.len() != 1 {
        return Err(b111());
    }
    move_root_c_lines(lines, &mut contexts);
    values.pop().ok_or_else(b111)
}

fn replay_c_expression_shape(expression: &ResolvedExpr) -> Result<(usize, usize), Diagnostic> {
    let mut pending = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    pending[0] = Some((expression, 0usize, 1usize));
    let mut pending_len = 1usize;
    let mut nodes = 0usize;
    let mut maximum_depth = 1usize;
    while pending_len != 0 {
        let (node, child_index, node_depth) = pending[pending_len - 1].take().ok_or_else(b111)?;
        pending_len -= 1;
        if child_index == 0 {
            nodes = nodes
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            maximum_depth = maximum_depth.max(node_depth);
        }
        let mut child_cursor = child_index;
        if let Some((_, child)) = super::resolved_expression_child(node, &mut child_cursor) {
            if pending_len + 2 > pending.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            pending[pending_len] = Some((node, child_cursor, node_depth));
            pending[pending_len + 1] = Some((child, 0, node_depth + 1));
            pending_len += 2;
        }
    }
    Ok((nodes, maximum_depth))
}

fn replay_c_frame_payload(frame: &ReplayCExpressionFrame<'_>) -> usize {
    match frame {
        ReplayCExpressionFrame::FinishBinary(_, value)
        | ReplayCExpressionFrame::FinishLazy(value) => value.capacity(),
        ReplayCExpressionFrame::FinishThen(_, value)
        | ReplayCExpressionFrame::FinishElse(value) => value.as_ref().map_or(0, String::capacity),
        _ => 0,
    }
}

#[allow(clippy::ptr_arg)] // Exact Vec capacities are part of the scratch proof.
fn note_replay_c_expression_scratch(
    current: &ReplayCExpressionFrame<'_>,
    frames: &Vec<ReplayCExpressionFrame<'_>>,
    values: &Vec<String>,
    arguments: &Vec<String>,
    lines: &CExpressionLineArena,
) -> Result<(), Diagnostic> {
    #[cfg(not(test))]
    let _ = lines;
    let mut string_payload = replay_c_frame_payload(current);
    for frame in frames {
        string_payload = string_payload
            .checked_add(replay_c_frame_payload(frame))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    for value in values.iter().chain(arguments) {
        string_payload = string_payload
            .checked_add(value.capacity())
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    if string_payload > MAX_GENERATED_C_BYTES {
        return Err(b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES));
    }
    #[cfg(test)]
    note_post_hir_replay_capacity(
        frames
            .capacity()
            .saturating_mul(REPLAY_C_EXPRESSION_FRAME_BYTES)
            .saturating_add(
                values
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(
                arguments
                    .capacity()
                    .saturating_mul(std::mem::size_of::<String>()),
            )
            .saturating_add(lines.retained_bytes())
            .saturating_add(string_payload),
    );
    Ok(())
}

fn replay_write_c_arguments(
    lines: &mut CExpressionLineArena,
    arguments: &[String],
    separator: &str,
) -> Result<(), Diagnostic> {
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            lines
                .write_str(separator)
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
        }
        lines
            .write_str(argument)
            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
    }
    Ok(())
}

fn replay_c_expression_linear_independent(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    let (node_count, depth) = replay_c_expression_shape(expression)?;
    let capacity = depth
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(capacity);
    let mut values = Vec::<String>::with_capacity(capacity);
    let mut arguments = Vec::<String>::with_capacity(node_count);
    frames.push(ReplayCExpressionFrame::Evaluate(expression));
    while let Some(frame) = frames.pop() {
        note_replay_c_expression_scratch(&frame, &frames, &values, &arguments, lines)?;
        match frame {
            ReplayCExpressionFrame::Evaluate(expression) => match &expression.kind {
                ResolvedExprKind::Int(value) => values.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    values.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => {
                    values.push(format!("v_{}", replay_symbol_hash(place.root.as_str())))
                }
                ResolvedExprKind::NativeRustImportCall(call) => {
                    frames.push(ReplayCExpressionFrame::ContinueNative(
                        call,
                        0,
                        arguments.len(),
                    ));
                }
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(ReplayCExpressionFrame::FinishUnary(*op));
                    frames.push(ReplayCExpressionFrame::Evaluate(value));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(ReplayCExpressionFrame::FinishLazyLeft(*op, right));
                    frames.push(ReplayCExpressionFrame::Evaluate(left));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(ReplayCExpressionFrame::FinishBinaryLeft(*op, right));
                    frames.push(ReplayCExpressionFrame::Evaluate(left));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(ReplayCExpressionFrame::ContinueBlock(statements, 0, tail));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = replay_resolved_scalar(&expression.ty).ok_or_else(b111)?;
                    frames.push(ReplayCExpressionFrame::FinishCondition(
                        then_branch,
                        else_branch,
                        ty,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(condition));
                }
                ResolvedExprKind::Call { callee, args, .. } => {
                    frames.push(ReplayCExpressionFrame::ContinueCall(
                        callee.as_str(),
                        args,
                        &expression.ty,
                        0,
                        arguments.len(),
                    ));
                }
                _ => return Err(b107("scalar value signature required")),
            },
            ReplayCExpressionFrame::FinishUnary(op) => {
                let value = values.pop().ok_or_else(b111)?;
                if op == crate::ast::UnaryOp::Not {
                    values.push(format!("(!({value}))"));
                } else {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                }
            }
            ReplayCExpressionFrame::FinishBinaryLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                frames.push(ReplayCExpressionFrame::FinishBinary(op, left));
                frames.push(ReplayCExpressionFrame::Evaluate(right));
            }
            ReplayCExpressionFrame::FinishBinary(op, left) => {
                let right = values.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "int64_t {name};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    match op {
                        crate::ast::BinaryOp::Add => write!(lines, "if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => write!(lines, "if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => write!(lines, "if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => write!(lines, "if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    }
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    let operator = match op {
                        crate::ast::BinaryOp::Eq => "==",
                        crate::ast::BinaryOp::Ne => "!=",
                        crate::ast::BinaryOp::Lt => "<",
                        crate::ast::BinaryOp::Le => "<=",
                        crate::ast::BinaryOp::Gt => ">",
                        crate::ast::BinaryOp::Ge => ">=",
                        crate::ast::BinaryOp::And => "&&",
                        crate::ast::BinaryOp::Or => "||",
                        _ => unreachable!(),
                    };
                    values.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            ReplayCExpressionFrame::FinishLazyLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                let name = format!("tmp_{}", *temporary_count);
                *temporary_count += 1;
                write!(
                    lines,
                    "uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);if({}){{",
                    if op == crate::ast::BinaryOp::And {
                        name.clone()
                    } else {
                        format!("!{name}")
                    }
                )
                .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(ReplayCExpressionFrame::FinishLazy(name));
                frames.push(ReplayCExpressionFrame::Evaluate(right));
            }
            ReplayCExpressionFrame::FinishLazy(name) => {
                let right = values.pop().ok_or_else(b111)?;
                write!(lines, " {name}=({right})?UINT8_C(1):UINT8_C(0);}}")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                values.push(name);
            }
            ReplayCExpressionFrame::ContinueBlock(statements, index, tail) => {
                match statements.get(index) {
                    Some(ResolvedStatement::Let { value, .. }) => {
                        frames.push(ReplayCExpressionFrame::FinishBinding(
                            statements, index, tail,
                        ));
                        frames.push(ReplayCExpressionFrame::Evaluate(value));
                    }
                    Some(ResolvedStatement::Assign { value, .. }) => {
                        frames.push(ReplayCExpressionFrame::FinishAssignment(
                            statements, index, tail,
                        ));
                        frames.push(ReplayCExpressionFrame::Evaluate(value));
                    }
                    _ => frames.push(ReplayCExpressionFrame::Evaluate(tail)),
                }
            }
            ReplayCExpressionFrame::FinishBinding(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at a let");
                };
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "{} v_{} = {value};",
                        replay_c_scalar(ty),
                        replay_symbol_hash(binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(ReplayCExpressionFrame::ContinueBlock(
                    statements,
                    index + 1,
                    tail,
                ));
            }
            ReplayCExpressionFrame::FinishAssignment(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Assign { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at an assignment");
                };
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "v_{} = {value};",
                        replay_symbol_hash(binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(ReplayCExpressionFrame::ContinueBlock(
                    statements,
                    index + 1,
                    tail,
                ));
            }
            ReplayCExpressionFrame::FinishCondition(then_branch, else_branch, ty) => {
                let condition = values.pop().ok_or_else(b111)?;
                let name = if ty == ScalarType::Unit {
                    None
                } else {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "{} {name};", replay_c_scalar(ty))
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    Some(name)
                };
                write!(lines, "if({condition}){{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(ReplayCExpressionFrame::FinishThen(else_branch, name));
                frames.push(ReplayCExpressionFrame::Evaluate(then_branch));
            }
            ReplayCExpressionFrame::FinishThen(else_branch, name) => {
                let then_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = &name {
                    write!(lines, "{name}={then_value};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                lines
                    .write_str("}else{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(ReplayCExpressionFrame::FinishElse(name));
                frames.push(ReplayCExpressionFrame::Evaluate(else_branch));
            }
            ReplayCExpressionFrame::FinishElse(name) => {
                let else_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = name {
                    write!(lines, "{name}={else_value};}}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push(name);
                } else {
                    lines
                        .write_str("}")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    values.push("INT64_C(0)".to_owned());
                }
            }
            ReplayCExpressionFrame::ContinueNative(call, index, start) => {
                if index < call.args.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(ReplayCExpressionFrame::ContinueNative(
                        call,
                        index + 1,
                        start,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(&call.args[index]));
                } else {
                    if !call.args.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = format!("tmp_{}", *temporary_count);
                    if import.result != ScalarType::Unit {
                        *temporary_count += 1;
                        write!(lines, "{} {name};", replay_c_scalar(import.result))
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(
                        lines,
                        "status = ctx->imports->{}(ctx->userdata",
                        import.c_field
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if start < arguments.len() {
                        lines
                            .write_str(", ")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        replay_write_c_arguments(lines, &arguments[start..], ", ")?;
                    }
                    if import.result != ScalarType::Unit {
                        write!(lines, ", &{name}")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(lines, "); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.rust_method)
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if import.result == ScalarType::Bool {
                        write!(lines, "if ({name} > UINT8_C(1)) return spxnr_adapter(4);")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    arguments.truncate(start);
                    values.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            ReplayCExpressionFrame::ContinueCall(callee, source, ty, index, start) => {
                if index < source.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(ReplayCExpressionFrame::ContinueCall(
                        callee,
                        source,
                        ty,
                        index + 1,
                        start,
                    ));
                    frames.push(ReplayCExpressionFrame::Evaluate(&source[index]));
                } else {
                    if !source.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        write!(lines, "status=spxnr1_f_{}(ctx", replay_symbol_hash(callee))
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start < arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            replay_write_c_arguments(lines, &arguments[start..], ",")?;
                        }
                        lines
                            .write_str(");if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push("INT64_C(0)".to_owned());
                    } else {
                        let name = format!("tmp_{}", *temporary_count);
                        *temporary_count += 1;
                        write!(
                            lines,
                            "{} {name};status=spxnr1_f_{}(ctx",
                            replay_c_scalar(replay_resolved_scalar(ty).ok_or_else(b111)?),
                            replay_symbol_hash(callee)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start < arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            replay_write_c_arguments(lines, &arguments[start..], ",")?;
                        }
                        write!(lines, ",&{name});if(status!=0)return status;")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push(name);
                    }
                    arguments.truncate(start);
                }
            }
        }
    }
    let terminal = ReplayCExpressionFrame::Evaluate(expression);
    note_replay_c_expression_scratch(&terminal, &frames, &values, &arguments, lines)?;
    if values.len() != 1 || !arguments.is_empty() {
        return Err(b111());
    }
    values.pop().ok_or_else(b111)
}

fn replay_c_expression(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    replay_c_expression_linear_independent(expression, imports, temporary_count, lines)
}

// Kept out of every build: the iterative generator above is the sole replay
// evaluator. This source reference makes authored formatting changes easy to
// audit while preventing a recursive production route from reappearing.
#[cfg(any())]
fn replay_c_expression_recursive_reference(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut Vec<String>,
) -> Result<String, Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Int(value) => Ok(if *value == i64::MIN {
            "INT64_MIN".to_owned()
        } else {
            format!("INT64_C({value})")
        }),
        ResolvedExprKind::Bool(value) => Ok(if *value {
            "UINT8_C(1)".to_owned()
        } else {
            "UINT8_C(0)".to_owned()
        }),
        ResolvedExprKind::Place(place) if place.projections.is_empty() => {
            Ok(format!("v_{}", replay_symbol_hash(place.root.as_str())))
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let import = imports
                .iter()
                .find(|item| item.id == call.import.as_str())
                .ok_or_else(b111)?;
            let args = call
                .args
                .iter()
                .map(|arg| replay_c_expression(arg, imports, temporary_count, lines))
                .collect::<Result<Vec<_>, _>>()?;
            let name = format!("tmp_{}", *temporary_count);
            if import.result != ScalarType::Unit {
                lines.push(format!("{} {name};", replay_c_scalar(import.result)));
                *temporary_count += 1;
            }
            lines.push(format!(
                "status = ctx->imports->{}(ctx->userdata{}{}{}); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}",
                import.c_field,
                if args.is_empty() { "" } else { ", " },
                args.join(", "),
                if import.result == ScalarType::Unit {
                    String::new()
                } else {
                    format!(", &{name}")
                },
                import.rust_method,
            ));
            if import.result == ScalarType::Bool {
                lines.push(format!("if ({name} > UINT8_C(1)) return spxnr_adapter(4);"));
            }
            Ok(if import.result == ScalarType::Unit {
                "INT64_C(0)".to_owned()
            } else {
                name
            })
        }
        ResolvedExprKind::Unary { op, value } => {
            let value = replay_c_expression(value, imports, temporary_count, lines)?;
            match op {
                crate::ast::UnaryOp::Neg => {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    lines.push(format!("if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});"));
                    Ok(name)
                }
                crate::ast::UnaryOp::Not => Ok(format!("(!({value}))")),
            }
        }
        ResolvedExprKind::Binary {
            op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
            left,
            right,
        } => {
            let left = replay_c_expression(left, imports, temporary_count, lines)?;
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            lines.push(format!("uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);"));
            let mut branch_lines = Vec::new();
            let right = replay_c_expression(right, imports, temporary_count, &mut branch_lines)?;
            let condition = if matches!(
                expression.kind,
                ResolvedExprKind::Binary {
                    op: crate::ast::BinaryOp::And,
                    ..
                }
            ) {
                name.clone()
            } else {
                format!("!{name}")
            };
            lines.push(format!(
                "if({condition}){{{} {name}=({right})?UINT8_C(1):UINT8_C(0);}}",
                branch_lines.join("")
            ));
            Ok(name)
        }
        ResolvedExprKind::Binary { op, left, right } => {
            let left = replay_c_expression(left, imports, temporary_count, lines)?;
            let right = replay_c_expression(right, imports, temporary_count, lines)?;
            if matches!(
                op,
                crate::ast::BinaryOp::Add
                    | crate::ast::BinaryOp::Sub
                    | crate::ast::BinaryOp::Mul
                    | crate::ast::BinaryOp::Div
                    | crate::ast::BinaryOp::Rem
            ) {
                let name = format!("tmp_{}", *temporary_count);
                *temporary_count += 1;
                lines.push(format!("int64_t {name};"));
                lines.push(match op {
                    crate::ast::BinaryOp::Add => format!("if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                    crate::ast::BinaryOp::Sub => format!("if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                    crate::ast::BinaryOp::Mul => format!("if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                    crate::ast::BinaryOp::Div => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                    crate::ast::BinaryOp::Rem => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                    _ => unreachable!(),
                });
                return Ok(name);
            }
            let operator = match op {
                crate::ast::BinaryOp::Add => "+",
                crate::ast::BinaryOp::Sub => "-",
                crate::ast::BinaryOp::Mul => "*",
                crate::ast::BinaryOp::Div => "/",
                crate::ast::BinaryOp::Rem => "%",
                crate::ast::BinaryOp::Eq => "==",
                crate::ast::BinaryOp::Ne => "!=",
                crate::ast::BinaryOp::Lt => "<",
                crate::ast::BinaryOp::Le => "<=",
                crate::ast::BinaryOp::Gt => ">",
                crate::ast::BinaryOp::Ge => ">=",
                crate::ast::BinaryOp::And => "&&",
                crate::ast::BinaryOp::Or => "||",
            };
            Ok(format!("(({left}) {operator} ({right}))"))
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let ResolvedStatement::Let { binding, value, .. } = statement;
                let value = replay_c_expression(value, imports, temporary_count, lines)?;
                let ty = replay_resolved_scalar(&binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    lines.push(format!(
                        "{} v_{} = {value};",
                        replay_c_scalar(ty),
                        replay_symbol_hash(binding.id.as_str())
                    ));
                }
            }
            replay_c_expression(tail, imports, temporary_count, lines)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            let condition = replay_c_expression(condition, imports, temporary_count, lines)?;
            if replay_resolved_scalar(&expression.ty) == Some(ScalarType::Unit) {
                let mut then_lines = Vec::new();
                let _ =
                    replay_c_expression(then_branch, imports, temporary_count, &mut then_lines)?;
                let mut else_lines = Vec::new();
                let _ =
                    replay_c_expression(else_branch, imports, temporary_count, &mut else_lines)?;
                lines.push(format!(
                    "if({condition}){{{}}}else{{{}}}",
                    then_lines.join(""),
                    else_lines.join("")
                ));
                return Ok("INT64_C(0)".to_owned());
            }
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            lines.push(format!(
                "{} {name};",
                replay_c_scalar(replay_resolved_scalar(&expression.ty).ok_or_else(b111)?)
            ));
            let mut then_lines = Vec::new();
            let then_value =
                replay_c_expression(then_branch, imports, temporary_count, &mut then_lines)?;
            let mut else_lines = Vec::new();
            let else_value =
                replay_c_expression(else_branch, imports, temporary_count, &mut else_lines)?;
            lines.push(format!(
                "if({condition}){{{}{name}={then_value};}}else{{{}{name}={else_value};}}",
                then_lines.join(""),
                else_lines.join("")
            ));
            Ok(name)
        }
        ResolvedExprKind::Call { callee, args, .. } => {
            let args = args
                .iter()
                .map(|arg| replay_c_expression(arg, imports, temporary_count, lines))
                .collect::<Result<Vec<_>, _>>()?;
            if expression.ty == ResolvedType::Unit {
                lines.push(format!(
                    "status=spxnr1_f_{}(ctx{}{});if(status!=0)return status;",
                    replay_symbol_hash(callee.as_str()),
                    if args.is_empty() { "" } else { ", " },
                    args.join(",")
                ));
                return Ok("INT64_C(0)".to_owned());
            }
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            lines.push(format!(
                "{} {name};status=spxnr1_f_{}(ctx{}{},&{name});if(status!=0)return status;",
                replay_c_scalar(replay_resolved_scalar(&expression.ty).ok_or_else(b111)?),
                replay_symbol_hash(callee.as_str()),
                if args.is_empty() { "" } else { ", " },
                args.join(",")
            ));
            Ok(name)
        }
        ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::UpdateRecord { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Place(_) => Err(b107("scalar value signature required")),
    }
}

pub(super) fn replay_c_exact(
    source: &str,
    spec: &Spec,
    closure: &[&ResolvedFunction],
    exports: &[ExportFact],
    imports: &[ImportFact],
) -> Result<bool, Diagnostic> {
    let mut replay = ExactReplay::new(source);
    replay.text("#include \"semaprax_native_rust_interop.h\"\n#include <stdint.h>\n#include <stddef.h>\n#include <string.h>\n#include <limits.h>\nstatic const uint8_t spxnr_capabilities[32] = {");
    let digest = replay_capabilities_digest(&spec.capabilities);
    let hex = digest.strip_prefix("sha256:").ok_or_else(b111)?;
    if hex.len() != 64 {
        return Err(b111());
    }
    for index in (0..64).step_by(2) {
        if index != 0 {
            replay.text(",");
        }
        replay.text("0x");
        replay.text(&hex[index..index + 2]);
    }
    replay.text("};\nstatic spxnr_status_v1 spxnr_adapter(uint32_t code){return (((uint64_t)65535)<<48)|(((uint64_t)4)<<32)|code;}\nstatic spxnr_status_v1 spxnr_validate(const spxnr_context_v1 *ctx){if(!ctx||((uintptr_t)ctx%_Alignof(spxnr_context_v1))!=0)return spxnr_adapter(1);if(ctx->abi_version!=1||ctx->size!=sizeof(*ctx)||ctx->reserved!=0)return spxnr_adapter(1);if(!ctx->imports||((uintptr_t)ctx->imports%_Alignof(spxnr_imports_v1))!=0)return spxnr_adapter(2);if(ctx->imports->abi_version!=1||ctx->imports->size!=sizeof(*ctx->imports))return spxnr_adapter(2);if(memcmp(ctx->capabilities_digest,spxnr_capabilities,32)!=0)return spxnr_adapter(3);if(ctx->call_depth>=32)return spxnr_adapter(7);return 0;}\n");
    if !imports.is_empty() {
        replay.text("static int spxnr_status_canonical(spxnr_status_v1 status){if(status==0)return 1;uint32_t code=(uint32_t)status;uint8_t class_=(uint8_t)(status>>32);uint8_t retry=(uint8_t)((status>>40)&1);uint8_t reserved=(uint8_t)((status>>41)&0x7f);uint16_t domain=(uint16_t)(status>>48);if(code==0||reserved!=0||domain==0)return 0;if(domain==65533)return retry==0&&((class_==1&&code>=1&&code<=6)||(class_==2&&code>=1&&code<=2));");
    }
    let domains = imports
        .iter()
        .filter_map(|import| import.failure.as_ref())
        .collect::<BTreeSet<_>>();
    for (index, _) in domains.iter().enumerate() {
        replay.text("if(domain==");
        replay.number(index + 1);
        replay.text(")return class_==3;");
    }
    if !imports.is_empty() {
        replay.text("if(domain==65534)return class_==4&&retry==0&&code>=1&&code<=2;if(domain==65535)return class_==4&&retry==0&&code>=1&&code<=8;return 0;}\n");
    }
    let ordinals = domains
        .iter()
        .enumerate()
        .map(|(index, domain)| (domain.as_str(), index + 1))
        .collect::<BTreeMap<_, _>>();
    for import in imports {
        replay.text("static int spxnr_status_for_");
        replay.text(&import.rust_method);
        replay.text("(spxnr_status_v1 status){if(!spxnr_status_canonical(status))return 0;uint16_t domain=(uint16_t)(status>>48);return domain==65534||domain==65535");
        if let Some(ordinal) = import
            .failure
            .as_deref()
            .and_then(|domain| ordinals.get(domain).copied())
        {
            replay.text("||domain==");
            replay.number(ordinal);
        }
        replay.text(";}\nstatic spxnr_status_v1 spxnr_validate_");
        replay.text(&import.rust_method);
        replay.text("(const spxnr_context_v1 *ctx){return ctx->imports->");
        replay.text(&import.c_field);
        replay.text("?0:spxnr_adapter(2);}\n");
    }
    for function in closure {
        let parameters = replay_parameter_facts(function)?;
        let result = replay_resolved_scalar(&function.return_type).ok_or_else(b111)?;
        replay.text("static spxnr_status_v1 spxnr1_f_");
        replay.text(&replay_symbol_hash(function.id.as_str()));
        replay.text("(const spxnr_context_v1 *ctx");
        if !parameters.is_empty() {
            replay.text(", ");
            replay.text(&replay_c_parameters(&parameters));
        }
        if result != ScalarType::Unit {
            replay.text(", ");
            replay.text(replay_c_scalar(result));
            replay.text(" *result_out");
        }
        replay.text(");\n");
    }
    for function in closure {
        let parameters = replay_parameter_facts(function)?;
        let result = replay_resolved_scalar(&function.return_type).ok_or_else(b111)?;
        replay.text("static spxnr_status_v1 spxnr1_f_");
        replay.text(&replay_symbol_hash(function.id.as_str()));
        replay.text("(const spxnr_context_v1 *ctx");
        if !parameters.is_empty() {
            replay.text(", ");
            replay.text(&replay_c_parameters(&parameters));
        }
        if result != ScalarType::Unit {
            replay.text(", ");
            replay.text(replay_c_scalar(result));
            replay.text(" *result_out");
        }
        replay.text(" ){spxnr_status_v1 status=0;(void)ctx;");
        for index in 0..parameters.len() {
            replay.text("(void)arg_");
            replay.number(index);
            replay.text(";");
        }
        for (index, (parameter, resolved)) in parameters.iter().zip(&function.params).enumerate() {
            replay.text(replay_c_scalar(parameter.ty));
            replay.text(" v_");
            replay.text(&replay_symbol_hash(resolved.id.as_str()));
            replay.text("=arg_");
            replay.number(index);
            replay.text(";");
        }
        let mut temporary_count = 0;
        let mut lines = CExpressionLineArena::new();
        for requirement in &function.requires {
            lines.clear();
            let value =
                replay_c_expression(requirement, imports, &mut temporary_count, &mut lines)?;
            replay.text(lines.as_str()?);
            replay.text("if(!(");
            replay.text(&value);
            replay.text("))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(1);");
        }
        lines.clear();
        let value = replay_c_expression(&function.body, imports, &mut temporary_count, &mut lines)?;
        replay.text(lines.as_str()?);
        if result != ScalarType::Unit {
            replay.text(replay_c_scalar(result));
            replay.text(" v_");
            replay.text(&replay_symbol_hash(function.result_id.as_str()));
            replay.text("=");
            replay.text(&value);
            replay.text(";");
        }
        for guarantee in &function.ensures {
            lines.clear();
            let value = replay_c_expression(guarantee, imports, &mut temporary_count, &mut lines)?;
            replay.text(lines.as_str()?);
            replay.text("if(!(");
            replay.text(&value);
            replay.text("))return (((uint64_t)65533)<<48)|(((uint64_t)2)<<32)|UINT32_C(2);");
        }
        if result != ScalarType::Unit {
            replay.text("*result_out=v_");
            replay.text(&replay_symbol_hash(function.result_id.as_str()));
            replay.text(";");
        }
        replay.text("return status;}\n");
    }
    for export in exports {
        replay.text("spxnr_status_v1 ");
        replay.text(&export.c_symbol);
        replay.text("(const spxnr_context_v1 *ctx");
        if !export.parameters.is_empty() {
            replay.text(", ");
            replay.text(&replay_c_parameters(&export.parameters));
        }
        if export.result != ScalarType::Unit {
            replay.text(", ");
            replay.text(replay_c_scalar(export.result));
            replay.text(" *result_out");
        }
        replay.text(" ){spxnr_status_v1 status=spxnr_validate(ctx);if(status!=0)return status;");
        for import in imports {
            replay.text("status=spxnr_validate_");
            replay.text(&import.rust_method);
            replay.text("(ctx);if(status!=0)return status;");
        }
        if export.result != ScalarType::Unit {
            replay.text("if(!result_out||((uintptr_t)result_out%_Alignof(");
            replay.text(replay_c_scalar(export.result));
            replay.text("))!=0)return spxnr_adapter(5);");
        }
        for (index, parameter) in export.parameters.iter().enumerate() {
            if parameter.ty == ScalarType::Bool {
                replay.text("if(arg_");
                replay.number(index);
                replay.text(">1)return spxnr_adapter(4);");
            }
        }
        replay.text(
            "spxnr_context_v1 local=*ctx;local.call_depth=ctx->call_depth+1;status=spxnr1_f_",
        );
        replay.text(&replay_symbol_hash(&export.id));
        replay.text("(&local");
        for index in 0..export.parameters.len() {
            replay.text(if index == 0 { ", " } else { "," });
            replay.text("arg_");
            replay.number(index);
        }
        if export.result != ScalarType::Unit {
            replay.text(", result_out");
        }
        replay.text(");return status;}\n");
    }
    Ok(replay.finish())
}
