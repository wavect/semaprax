//! Iterative C expression generation: the generator frame set, its line
//! arena, the scratch census, and the linear emitter.

use super::*;

#[derive(Clone, Copy)]
enum CExpressionMode {
    Generate,
    Replay,
}

enum CExpressionFrame<'a> {
    Enter(&'a ResolvedExpr),
    Unary(crate::ast::UnaryOp),
    BinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    BinaryRight(crate::ast::BinaryOp, String),
    LazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr),
    LazyRight(String),
    Block(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    BlockLet(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    BlockAssign(&'a [ResolvedStatement], usize, &'a ResolvedExpr),
    IfCondition(&'a ResolvedExpr, &'a ResolvedExpr, ScalarType),
    IfThen(&'a ResolvedExpr, Option<String>),
    IfElse(Option<String>),
    NativeArgs(&'a crate::hir::ResolvedNativeRustImportCall, usize, usize),
    CallArgs(&'a str, &'a [ResolvedExpr], &'a ResolvedType, usize, usize),
}

// Pinned by module-local assertions beside the private iterative enums.
pub(in crate::implementation) const C_EXPRESSION_FRAME_BYTES: usize =
    std::mem::size_of::<CExpressionFrame<'static>>();

/// One fixed backing allocation owns every generated statement byte for one C
/// expression. The final C artifact has a separate reservation; this arena is
/// transient scratch and cannot grow geometrically past the admitted artifact
/// ceiling before the final-size gate observes it.
pub(super) struct CExpressionLineArena {
    bytes: Box<[u8]>,
    len: usize,
}

impl CExpressionLineArena {
    pub(in crate::implementation) fn new() -> Self {
        Self {
            bytes: vec![0; MAX_GENERATED_C_BYTES].into_boxed_slice(),
            len: 0,
        }
    }

    pub(in crate::implementation) fn as_str(&self) -> Result<&str, Diagnostic> {
        std::str::from_utf8(&self.bytes[..self.len]).map_err(|_| b111())
    }

    pub(in crate::implementation) fn clear(&mut self) {
        self.len = 0;
    }

    #[cfg(test)]
    pub(in crate::implementation) fn retained_bytes(&self) -> usize {
        self.bytes.len()
    }
}

impl std::fmt::Write for CExpressionLineArena {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        let end = self.len.checked_add(value.len()).ok_or(std::fmt::Error)?;
        let destination = self.bytes.get_mut(self.len..end).ok_or(std::fmt::Error)?;
        destination.copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

fn c_expression_hash(mode: CExpressionMode, value: &str) -> String {
    match mode {
        CExpressionMode::Generate => full_hash(value),
        CExpressionMode::Replay => replay_symbol_hash(value),
    }
}

fn c_expression_scalar(mode: CExpressionMode, value: ScalarType) -> &'static str {
    match mode {
        CExpressionMode::Generate => c_type(value),
        CExpressionMode::Replay => replay_c_scalar(value),
    }
}

fn c_expression_resolved_scalar(mode: CExpressionMode, value: &ResolvedType) -> Option<ScalarType> {
    match mode {
        CExpressionMode::Generate => scalar_type(value),
        CExpressionMode::Replay => replay_resolved_scalar(value),
    }
}

#[cfg(any())]
pub(super) fn take_c_lines(lines: &mut Vec<String>) -> String {
    let bytes = lines.iter().map(String::len).sum();
    let mut joined = String::with_capacity(bytes);
    for line in lines.drain(..) {
        joined.push_str(&line);
    }
    joined
}

#[cfg(any())]
fn append_c_lines(output: &mut String, lines: &mut Vec<String>) {
    for line in lines.drain(..) {
        output.push_str(&line);
    }
}

#[cfg(any())]
pub(super) fn move_root_c_lines(lines: &mut Vec<String>, contexts: &mut [Vec<String>]) {
    let mut root = std::mem::take(&mut contexts[0]);
    if lines.is_empty() {
        std::mem::swap(lines, &mut root);
    } else {
        lines.append(&mut root);
    }
}

pub(in crate::implementation) fn c_expression_shape(
    expression: &ResolvedExpr,
) -> Result<(usize, usize), Diagnostic> {
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    stack[0] = Some((expression, 0usize, 1usize));
    let mut stack_len = 1usize;
    let mut nodes = 0usize;
    let mut depth = 1usize;
    while stack_len > 0 {
        let (node, next_child, node_depth) = stack[stack_len - 1].take().ok_or_else(b111)?;
        stack_len -= 1;
        if next_child == 0 {
            nodes = nodes
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            depth = depth.max(node_depth);
        }
        let mut child_cursor = next_child;
        if let Some((_, child)) = super::resolved_expression_child(node, &mut child_cursor) {
            if stack_len + 2 > stack.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            stack[stack_len] = Some((node, child_cursor, node_depth));
            stack[stack_len + 1] = Some((child, 0, node_depth + 1));
            stack_len += 2;
        }
    }
    Ok((nodes, depth))
}

fn c_expression_frame_payload(frame: &CExpressionFrame<'_>) -> usize {
    match frame {
        CExpressionFrame::BinaryRight(_, value) | CExpressionFrame::LazyRight(value) => {
            value.capacity()
        }
        CExpressionFrame::IfThen(_, value) | CExpressionFrame::IfElse(value) => {
            value.as_ref().map_or(0, String::capacity)
        }
        _ => 0,
    }
}

fn c_expression_live_string_payload(
    current: &CExpressionFrame<'_>,
    frames: &[CExpressionFrame<'_>],
    values: &[String],
    arguments: &[String],
) -> Option<usize> {
    frames
        .iter()
        .try_fold(c_expression_frame_payload(current), |bytes, frame| {
            bytes.checked_add(c_expression_frame_payload(frame))
        })?
        .checked_add(
            values
                .iter()
                .try_fold(0usize, |bytes, value| bytes.checked_add(value.capacity()))?,
        )?
        .checked_add(
            arguments
                .iter()
                .try_fold(0usize, |bytes, value| bytes.checked_add(value.capacity()))?,
        )
}

#[allow(clippy::ptr_arg)] // Exact Vec capacities are part of the scratch proof.
fn note_c_expression_scratch(
    mode: CExpressionMode,
    current: &CExpressionFrame<'_>,
    frames: &Vec<CExpressionFrame<'_>>,
    values: &Vec<String>,
    arguments: &Vec<String>,
    lines: &CExpressionLineArena,
) -> Result<(), Diagnostic> {
    #[cfg(not(test))]
    let _ = mode;
    #[cfg(not(test))]
    let _ = lines;
    let string_payload = c_expression_live_string_payload(current, frames, values, arguments)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    if string_payload > MAX_GENERATED_C_BYTES {
        return Err(b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES));
    }
    #[cfg(test)]
    {
        let working = frames
            .capacity()
            .saturating_mul(C_EXPRESSION_FRAME_BYTES)
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
            .saturating_add(string_payload);
        match mode {
            CExpressionMode::Generate => note_post_hir_render_capacity(working),
            CExpressionMode::Replay => note_post_hir_replay_capacity(working),
        }
    }
    Ok(())
}

fn write_c_expression_arguments(
    lines: &mut CExpressionLineArena,
    arguments: &[String],
    separator: &str,
) -> Result<(), Diagnostic> {
    for (index, argument) in arguments.iter().enumerate() {
        if index != 0 {
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

fn c_expression_linear(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    let mode = CExpressionMode::Generate;
    let (node_count, depth) = c_expression_shape(expression)?;
    let frame_capacity = depth
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut frames = Vec::with_capacity(frame_capacity);
    let mut values = Vec::<String>::with_capacity(frame_capacity);
    let mut arguments = Vec::<String>::with_capacity(node_count);
    frames.push(CExpressionFrame::Enter(expression));
    while let Some(frame) = frames.pop() {
        note_c_expression_scratch(mode, &frame, &frames, &values, &arguments, lines)?;
        match frame {
            CExpressionFrame::Enter(expression) => match &expression.kind {
                ResolvedExprKind::Int32(_)
                | ResolvedExprKind::Char(_)
                | ResolvedExprKind::Uint8(_)
                | ResolvedExprKind::Usize(_)
                | ResolvedExprKind::ArrayU8(_)
                | ResolvedExprKind::RepeatArrayU8 { .. }
                | ResolvedExprKind::Float32(_)
                | ResolvedExprKind::Float64(_)
                | ResolvedExprKind::String(_)
                | ResolvedExprKind::BorrowPlace { .. }
                | ResolvedExprKind::ByteRange { .. } => {
                    // Non-i64 scalar signatures are outside the scalar
                    // native boundary; admission rejects them first.
                    return Err(b107("scalar value signature required"));
                }
                ResolvedExprKind::Int(value) => values.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    values.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => values.push(
                    format!("v_{}", c_expression_hash(mode, place.root.as_str())),
                ),
                ResolvedExprKind::NativeRustImportCall(call) => {
                    frames.push(CExpressionFrame::NativeArgs(call, 0, arguments.len()));
                }
                ResolvedExprKind::HostCommandCall(_) => {
                    // Command I/O is not part of the public Native Rust SDK
                    // boundary. Closure admission rejects it before emission.
                    return Err(b107("scalar value signature required"));
                }
                ResolvedExprKind::Unary { op, value } => {
                    frames.push(CExpressionFrame::Unary(*op));
                    frames.push(CExpressionFrame::Enter(value));
                }
                ResolvedExprKind::Binary { op, left, right }
                    if matches!(op, crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or) =>
                {
                    frames.push(CExpressionFrame::LazyLeft(*op, right));
                    frames.push(CExpressionFrame::Enter(left));
                }
                ResolvedExprKind::Binary { op, left, right } => {
                    frames.push(CExpressionFrame::BinaryLeft(*op, right));
                    frames.push(CExpressionFrame::Enter(left));
                }
                ResolvedExprKind::Block { statements, tail } => {
                    frames.push(CExpressionFrame::Block(statements, 0, tail));
                }
                ResolvedExprKind::If {
                    condition,
                    then_branch,
                    else_branch,
                } => {
                    let ty = c_expression_resolved_scalar(mode, &expression.ty).ok_or_else(b111)?;
                    frames.push(CExpressionFrame::IfCondition(then_branch, else_branch, ty));
                    frames.push(CExpressionFrame::Enter(condition));
                }
                ResolvedExprKind::Call { callee, args, .. } => {
                    frames.push(CExpressionFrame::CallArgs(
                        callee.as_str(),
                        args,
                        &expression.ty,
                        0,
                        arguments.len(),
                    ));
                }
                ResolvedExprKind::ConstructRecord { .. }
                | ResolvedExprKind::ConstructVariant { .. }
                | ResolvedExprKind::Match { .. }
                | ResolvedExprKind::Try { .. }
                | ResolvedExprKind::TryOption { .. }
                | ResolvedExprKind::UpdateRecord { .. }
                | ResolvedExprKind::Project { .. }
                | ResolvedExprKind::Upcast { .. }
                | ResolvedExprKind::Place(_) => {
                    return Err(b107("scalar value signature required"));
                }
            },
            CExpressionFrame::Unary(op) => {
                let value = values.pop().ok_or_else(b111)?;
                match op {
                    crate::ast::UnaryOp::Neg => {
                        let name = format!("tmp_{}", *temporary_count);
                        *temporary_count += 1;
                        write!(lines, "if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        values.push(name);
                    }
                    crate::ast::UnaryOp::Not => values.push(format!("(!({value}))")),
                }
            }
            CExpressionFrame::BinaryLeft(op, right) => {
                let left = values.pop().ok_or_else(b111)?;
                frames.push(CExpressionFrame::BinaryRight(op, left));
                frames.push(CExpressionFrame::Enter(right));
            }
            CExpressionFrame::BinaryRight(op, left) => {
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
            CExpressionFrame::LazyLeft(op, right) => {
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
                frames.push(CExpressionFrame::LazyRight(name));
                frames.push(CExpressionFrame::Enter(right));
            }
            CExpressionFrame::LazyRight(name) => {
                let right = values.pop().ok_or_else(b111)?;
                write!(lines, " {name}=({right})?UINT8_C(1):UINT8_C(0);}}")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                values.push(name);
            }
            CExpressionFrame::Block(statements, index, tail) => match statements.get(index) {
                Some(ResolvedStatement::Let { value, .. }) => {
                    frames.push(CExpressionFrame::BlockLet(statements, index, tail));
                    frames.push(CExpressionFrame::Enter(value));
                }
                Some(statement @ ResolvedStatement::Assign { value, .. }) => {
                    frames.push(CExpressionFrame::BlockAssign(statements, index, tail));
                    frames.push(CExpressionFrame::Enter(value));
                    let _ = statement;
                }
                _ => frames.push(CExpressionFrame::Enter(tail)),
            },
            CExpressionFrame::BlockLet(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at a let");
                };
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "{} v_{} = {value};",
                        c_expression_scalar(mode, ty),
                        c_expression_hash(mode, binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(CExpressionFrame::Block(statements, index + 1, tail));
            }
            CExpressionFrame::BlockAssign(statements, index, tail) => {
                let value = values.pop().ok_or_else(b111)?;
                let ResolvedStatement::Assign { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at an assignment");
                };
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    write!(
                        lines,
                        "v_{} = {value};",
                        c_expression_hash(mode, binding.id.as_str())
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                frames.push(CExpressionFrame::Block(statements, index + 1, tail));
            }
            CExpressionFrame::IfCondition(then_branch, else_branch, ty) => {
                let condition = values.pop().ok_or_else(b111)?;
                let name = if ty == ScalarType::Unit {
                    None
                } else {
                    let name = format!("tmp_{}", *temporary_count);
                    *temporary_count += 1;
                    write!(lines, "{} {name};", c_expression_scalar(mode, ty))
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    Some(name)
                };
                write!(lines, "if({condition}){{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(CExpressionFrame::IfThen(else_branch, name));
                frames.push(CExpressionFrame::Enter(then_branch));
            }
            CExpressionFrame::IfThen(else_branch, name) => {
                let then_value = values.pop().ok_or_else(b111)?;
                if let Some(name) = &name {
                    write!(lines, "{name}={then_value};")
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                }
                lines
                    .write_str("}else{")
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                frames.push(CExpressionFrame::IfElse(name));
                frames.push(CExpressionFrame::Enter(else_branch));
            }
            CExpressionFrame::IfElse(name) => {
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
            CExpressionFrame::NativeArgs(call, index, start) => {
                if index < call.args.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(CExpressionFrame::NativeArgs(call, index + 1, start));
                    frames.push(CExpressionFrame::Enter(&call.args[index]));
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
                        write!(
                            lines,
                            "{} {name};",
                            c_expression_scalar(mode, import.result)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    }
                    write!(
                        lines,
                        "status = ctx->imports->{}(ctx->userdata",
                        import.c_field
                    )
                    .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                    if start != arguments.len() {
                        lines
                            .write_str(", ")
                            .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        write_c_expression_arguments(lines, &arguments[start..], ", ")?;
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
            CExpressionFrame::CallArgs(callee, source, ty, index, start) => {
                if index < source.len() {
                    if index > 0 {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    frames.push(CExpressionFrame::CallArgs(
                        callee,
                        source,
                        ty,
                        index + 1,
                        start,
                    ));
                    frames.push(CExpressionFrame::Enter(&source[index]));
                } else {
                    if !source.is_empty() {
                        arguments.push(values.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        write!(
                            lines,
                            "status=spxnr1_f_{}(ctx",
                            c_expression_hash(mode, callee)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start != arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            write_c_expression_arguments(lines, &arguments[start..], ",")?;
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
                            c_expression_scalar(
                                mode,
                                c_expression_resolved_scalar(mode, ty).ok_or_else(b111)?
                            ),
                            c_expression_hash(mode, callee)
                        )
                        .map_err(|_| b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES))?;
                        if start != arguments.len() {
                            lines.write_str(",").map_err(|_| {
                                b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES)
                            })?;
                            write_c_expression_arguments(lines, &arguments[start..], ",")?;
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
    let terminal = CExpressionFrame::Enter(expression);
    note_c_expression_scratch(mode, &terminal, &frames, &values, &arguments, lines)?;
    if values.len() != 1 || !arguments.is_empty() {
        return Err(b111());
    }
    let result = values.pop().ok_or_else(b111)?;
    if result.capacity() > MAX_GENERATED_C_BYTES {
        return Err(b109("max_generated_c_bytes", MAX_GENERATED_C_BYTES));
    }
    Ok(result)
}

#[cfg(any())]
fn c_context_line_slots(expression: &ResolvedExpr) -> Result<usize, Diagnostic> {
    // A line is owned by exactly one active context. Branch results are
    // collapsed to one String before being appended to their parent, and the
    // drained child Vec is released immediately. Child contexts therefore do
    // not reserve their whole subtree: across all live contexts their logical
    // line count is at most 3N. Vec geometric growth is below twice logical
    // length, so 6N String slots bounds all context backings simultaneously.
    c_expression_shape(expression)?
        .0
        .checked_mul(6)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

#[cfg(any())]
fn c_expr_iterative(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    mut temporary_names: Option<&mut Vec<String>>,
    lines: &mut Vec<String>,
    mode: CExpressionMode,
) -> Result<String, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedExpr, usize),
        Unary(crate::ast::UnaryOp, usize),
        BinaryLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        BinaryRight(crate::ast::BinaryOp, String, usize),
        LazyLeft(crate::ast::BinaryOp, &'a ResolvedExpr, usize),
        LazyRight(crate::ast::BinaryOp, String, String, usize, usize),
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

    let allocate_temporary =
        |temporary_count: &mut usize, temporary_names: &mut Option<&mut Vec<String>>| {
            let name = format!("tmp_{}", *temporary_count);
            *temporary_count += 1;
            if let Some(names) = temporary_names.as_deref_mut() {
                names.push(name.clone());
            }
            name
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
    frames.push(Frame::Enter(expression, 0));
    let mut results = Vec::<String>::with_capacity(depth + 1);
    let mut contexts = Vec::with_capacity(node_count + 1);
    contexts.push(Vec::<String>::with_capacity(node_count.saturating_mul(3)));
    while let Some(frame) = frames.pop() {
        #[cfg(test)]
        {
            let frame_owned = frames
                .iter()
                .map(|frame| match frame {
                    Frame::BinaryRight(_, value, _)
                    | Frame::LazyRight(_, value, _, _, _)
                    | Frame::IfThen(value, _, _, _, _)
                    | Frame::IfElse(value, _, _, _, _, _) => value.capacity(),
                    Frame::NativeArgs(_, _, values, _) | Frame::CallArgs(_, _, _, _, values, _) => {
                        values.capacity() * std::mem::size_of::<String>()
                            + values.iter().map(String::capacity).sum::<usize>()
                    }
                    _ => 0,
                })
                .sum::<usize>();
            let result_owned = results.capacity() * std::mem::size_of::<String>()
                + results.iter().map(String::capacity).sum::<usize>();
            let context_owned = contexts.capacity() * std::mem::size_of::<Vec<String>>()
                + contexts
                    .iter()
                    .map(|context| {
                        context.capacity() * std::mem::size_of::<String>()
                            + context.iter().map(String::capacity).sum::<usize>()
                    })
                    .sum::<usize>();
            let caller_lines = lines.capacity() * std::mem::size_of::<String>()
                + lines.iter().map(String::capacity).sum::<usize>();
            let persistent_temporaries = temporary_names.as_deref().map_or(0, |names| {
                names.capacity() * std::mem::size_of::<String>()
                    + names.iter().map(String::capacity).sum::<usize>()
            });
            let working = frames.capacity() * std::mem::size_of::<Frame<'_>>()
                + frame_owned
                + result_owned
                + context_owned
                + caller_lines
                + persistent_temporaries;
            match mode {
                CExpressionMode::Generate => note_post_hir_render_capacity(working),
                CExpressionMode::Replay => note_post_hir_replay_capacity(working),
            }
        }
        match frame {
            Frame::Enter(expression, context) => match &expression.kind {
                ResolvedExprKind::Int(value) => results.push(if *value == i64::MIN {
                    "INT64_MIN".to_owned()
                } else {
                    format!("INT64_C({value})")
                }),
                ResolvedExprKind::Bool(value) => {
                    results.push(if *value { "UINT8_C(1)" } else { "UINT8_C(0)" }.to_owned())
                }
                ResolvedExprKind::Place(place) if place.projections.is_empty() => results.push(
                    format!("v_{}", c_expression_hash(mode, place.root.as_str())),
                ),
                ResolvedExprKind::NativeRustImportCall(call) => {
                    frames.push(Frame::NativeArgs(
                        call,
                        0,
                        Vec::with_capacity(call.args.len()),
                        context,
                    ));
                }
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
                    let ty = c_expression_resolved_scalar(mode, &expression.ty).ok_or_else(b111)?;
                    frames.push(Frame::IfCondition(then_branch, else_branch, ty, context));
                    frames.push(Frame::Enter(condition, context));
                }
                ResolvedExprKind::Call { callee, args, .. } => {
                    frames.push(Frame::CallArgs(
                        callee.as_str(),
                        args,
                        &expression.ty,
                        0,
                        Vec::with_capacity(args.len()),
                        context,
                    ));
                }
                ResolvedExprKind::ConstructRecord { .. }
                | ResolvedExprKind::ConstructVariant { .. }
                | ResolvedExprKind::Match { .. }
                | ResolvedExprKind::Try { .. }
                | ResolvedExprKind::TryOption { .. }
                | ResolvedExprKind::UpdateRecord { .. }
                | ResolvedExprKind::Project { .. }
                | ResolvedExprKind::Place(_) => {
                    return Err(b107("scalar value signature required"));
                }
            },
            Frame::Unary(op, context) => {
                let value = results.pop().ok_or_else(b111)?;
                match op {
                    crate::ast::UnaryOp::Neg => {
                        let name = allocate_temporary(temporary_count, &mut temporary_names);
                        contexts[context].push(format!("if(({value})==INT64_MIN)return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(1);int64_t {name}=-({value});"));
                        results.push(name);
                    }
                    crate::ast::UnaryOp::Not => results.push(format!("(!({value}))")),
                }
            }
            Frame::BinaryLeft(op, right, context) => {
                let left = results.pop().ok_or_else(b111)?;
                frames.push(Frame::BinaryRight(op, left, context));
                frames.push(Frame::Enter(right, context));
            }
            Frame::BinaryRight(op, left, context) => {
                let right = results.pop().ok_or_else(b111)?;
                if matches!(
                    op,
                    crate::ast::BinaryOp::Add
                        | crate::ast::BinaryOp::Sub
                        | crate::ast::BinaryOp::Mul
                        | crate::ast::BinaryOp::Div
                        | crate::ast::BinaryOp::Rem
                ) {
                    let name = allocate_temporary(temporary_count, &mut temporary_names);
                    contexts[context].push(format!("int64_t {name};"));
                    contexts[context].push(match op {
                        crate::ast::BinaryOp::Add => format!("if(__builtin_add_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(2);"),
                        crate::ast::BinaryOp::Sub => format!("if(__builtin_sub_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(3);"),
                        crate::ast::BinaryOp::Mul => format!("if(__builtin_mul_overflow({left},{right},&{name}))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(4);"),
                        crate::ast::BinaryOp::Div => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(5);{name}=({left})/({right});"),
                        crate::ast::BinaryOp::Rem => format!("if(({right})==0||(({left})==INT64_MIN&&({right})==-1))return (((uint64_t)65533)<<48)|(((uint64_t)1)<<32)|UINT32_C(6);{name}=({left})%({right});"),
                        _ => unreachable!(),
                    });
                    results.push(name);
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
                    results.push(format!("(({left}) {operator} ({right}))"));
                }
            }
            Frame::LazyLeft(op, right, context) => {
                let left = results.pop().ok_or_else(b111)?;
                let name = allocate_temporary(temporary_count, &mut temporary_names);
                contexts[context].push(format!("uint8_t {name}=({left})?UINT8_C(1):UINT8_C(0);"));
                let branch = contexts.len();
                contexts.push(Vec::new());
                frames.push(Frame::LazyRight(op, name, left, context, branch));
                frames.push(Frame::Enter(right, branch));
            }
            Frame::LazyRight(op, name, _left, context, branch) => {
                let right = results.pop().ok_or_else(b111)?;
                let condition = if op == crate::ast::BinaryOp::And {
                    name.clone()
                } else {
                    format!("!{name}")
                };
                let branch_lines = take_c_lines(&mut contexts[branch]);
                contexts[branch] = Vec::new();
                contexts[context].push(format!(
                    "if({condition}){{{branch_lines} {name}=({right})?UINT8_C(1):UINT8_C(0);}}"
                ));
                results.push(name);
            }
            Frame::Block(statements, index, tail, context) => {
                if index == statements.len() {
                    frames.push(Frame::Enter(tail, context));
                } else {
                    match &statements[index] {
                        ResolvedStatement::Let { value, .. } => {
                            frames.push(Frame::BlockLet(statements, index, tail, context));
                            frames.push(Frame::Enter(value, context));
                        }
                        ResolvedStatement::Assign { value, .. } => {
                            frames.push(Frame::BlockAssign(statements, index, tail, context));
                            frames.push(Frame::Enter(value, context));
                        }
                    }
                }
            }
            Frame::BlockLet(statements, index, tail, context) => {
                let value = results.pop().ok_or_else(b111)?;
                let ResolvedStatement::Let { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at a let");
                };
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "{} v_{} = {value};",
                        c_expression_scalar(mode, ty),
                        c_expression_hash(mode, binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::BlockAssign(statements, index, tail, context) => {
                let value = results.pop().ok_or_else(b111)?;
                let ResolvedStatement::Assign { binding, .. } = &statements[index] else {
                    unreachable!("statement frame resumed at an assignment");
                };
                let ty = c_expression_resolved_scalar(mode, &binding.ty).ok_or_else(b111)?;
                if ty != ScalarType::Unit {
                    contexts[context].push(format!(
                        "v_{} = {value};",
                        c_expression_hash(mode, binding.id.as_str())
                    ));
                }
                frames.push(Frame::Block(statements, index + 1, tail, context));
            }
            Frame::IfCondition(then_branch, else_branch, ty, context) => {
                let condition = results.pop().ok_or_else(b111)?;
                let name = if ty == ScalarType::Unit {
                    None
                } else {
                    let name = allocate_temporary(temporary_count, &mut temporary_names);
                    contexts[context].push(format!("{} {name};", c_expression_scalar(mode, ty)));
                    Some(name)
                };
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
                let then_value = results.pop().ok_or_else(b111)?;
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
                let else_value = results.pop().ok_or_else(b111)?;
                let then_lines = take_c_lines(&mut contexts[then_context]);
                let else_lines = take_c_lines(&mut contexts[else_context]);
                contexts[then_context] = Vec::new();
                contexts[else_context] = Vec::new();
                if let Some(name) = name {
                    contexts[context].push(format!("if({condition}){{{then_lines}{name}={then_value};}}else{{{else_lines}{name}={else_value};}}"));
                    results.push(name);
                } else {
                    contexts[context].push(format!(
                        "if({condition}){{{then_lines}}}else{{{else_lines}}}"
                    ));
                    results.push("INT64_C(0)".to_owned());
                }
            }
            Frame::NativeArgs(call, index, mut args, context) => {
                if index < call.args.len() {
                    if index > 0 {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::NativeArgs(call, index + 1, args, context));
                    frames.push(Frame::Enter(&call.args[index], context));
                } else {
                    if !call.args.is_empty() {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    let import = imports
                        .iter()
                        .find(|item| item.id == call.import.as_str())
                        .ok_or_else(b111)?;
                    let name = if import.result == ScalarType::Unit {
                        format!("tmp_{}", *temporary_count)
                    } else {
                        allocate_temporary(temporary_count, &mut temporary_names)
                    };
                    if import.result != ScalarType::Unit {
                        contexts[context].push(format!(
                            "{} {name};",
                            c_expression_scalar(mode, import.result)
                        ));
                    }
                    contexts[context].push(format!("status = ctx->imports->{}(ctx->userdata{}{}{}); if (status != 0) {{ if (!spxnr_status_for_{}(status)) return spxnr_adapter(8); return status; }}", import.c_field, if args.is_empty() { "" } else { ", " }, args.join(", "), if import.result == ScalarType::Unit { String::new() } else { format!(", &{name}") }, import.rust_method));
                    if import.result == ScalarType::Bool {
                        contexts[context]
                            .push(format!("if ({name} > UINT8_C(1)) return spxnr_adapter(4);"));
                    }
                    results.push(if import.result == ScalarType::Unit {
                        "INT64_C(0)".to_owned()
                    } else {
                        name
                    });
                }
            }
            Frame::CallArgs(callee, call_args, ty, index, mut args, context) => {
                if index < call_args.len() {
                    if index > 0 {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    frames.push(Frame::CallArgs(
                        callee,
                        call_args,
                        ty,
                        index + 1,
                        args,
                        context,
                    ));
                    frames.push(Frame::Enter(&call_args[index], context));
                } else {
                    if !call_args.is_empty() {
                        args.push(results.pop().ok_or_else(b111)?);
                    }
                    if *ty == ResolvedType::Unit {
                        contexts[context].push(format!(
                            "status=spxnr1_f_{}(ctx{}{});if(status!=0)return status;",
                            c_expression_hash(mode, callee),
                            if args.is_empty() { "" } else { ", " },
                            args.join(",")
                        ));
                        results.push("INT64_C(0)".to_owned());
                    } else {
                        let name = allocate_temporary(temporary_count, &mut temporary_names);
                        let scalar = c_expression_resolved_scalar(mode, ty).ok_or_else(b111)?;
                        contexts[context].push(format!("{} {name};status=spxnr1_f_{}(ctx{}{},&{name});if(status!=0)return status;", c_expression_scalar(mode, scalar), c_expression_hash(mode, callee), if args.is_empty() { "" } else { ", " }, args.join(",")));
                        results.push(name);
                    }
                }
            }
        }
    }
    if results.len() != 1 {
        return Err(b111());
    }
    move_root_c_lines(lines, &mut contexts);
    results.pop().ok_or_else(b111)
}

#[cfg(any())]
pub(super) fn c_expr(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporaries: &mut Vec<String>,
    lines: &mut Vec<String>,
) -> Result<String, Diagnostic> {
    let mut count = temporaries.len();
    c_expr_iterative(
        expression,
        imports,
        &mut count,
        Some(temporaries),
        lines,
        CExpressionMode::Generate,
    )
}

pub(super) fn c_expr(
    expression: &ResolvedExpr,
    imports: &[ImportFact],
    temporary_count: &mut usize,
    lines: &mut CExpressionLineArena,
) -> Result<String, Diagnostic> {
    c_expression_linear(expression, imports, temporary_count, lines)
}
