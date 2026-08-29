//! Pure source/HIR census and conservative builder-capacity proofs.
//!
//! This module performs bounded traversal and arithmetic only. It has no
//! filesystem, process, platform, publication, or settlement authority.

use super::*;

const HIR_RESOLVER_FRAME_BYTES: usize = 552;
const HIR_VALIDATOR_FRAME_BYTES: usize = 288;
const SOURCE_VERIFIER_FRAME_BYTES: usize = 320;
const SOURCE_VARIANT_MATCH_STATE_BYTES: usize = 312;
const CLEANUP_INVENTORY_SHAPE_FRAME_BYTES: usize = 40;
const CLEANUP_INVENTORY_EXPR_FRAME_BYTES: usize = 24;
const CLEANUP_LOWER_FRAME_BYTES: usize = 344;
const CLEANUP_EVAL_RESULT_BYTES: usize = 128;
const CALL_INDEX_FRAME_BYTES: usize = 16;

fn source_functions(program: &Program) -> impl Iterator<Item = &crate::ast::Function> {
    program
        .functions
        .iter()
        .chain(
            program
                .types
                .iter()
                .flat_map(|declaration| match &declaration.kind {
                    crate::ast::TypeDeclarationKind::Class { methods, .. } => methods.as_slice(),
                    _ => &[],
                }),
        )
}

pub(super) fn validate_native_rust_source_expression_budget(
    program: &Program,
) -> Result<(), Diagnostic> {
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for function in source_functions(program) {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut stack_len = 1;
            stack[0] = Some((root, 1_usize, 0_usize));
            while stack_len != 0 {
                stack_len -= 1;
                let (expression, depth, next_child) = stack[stack_len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if next_child == 0 {
                    debit(std::mem::size_of::<&crate::ast::Expr>())?;
                    if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                }
                let mut child_cursor = next_child;
                if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
                    if stack_len + 2 > stack.len() {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                    stack[stack_len] = Some((expression, depth, child_cursor));
                    stack[stack_len + 1] = Some((child, depth + 1, 0));
                    stack_len += 2;
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
pub(super) struct AstCapacityStats {
    pub(super) nodes: usize,
    pub(super) cumulative_depth: usize,
    pub(super) generic_calls: usize,
    pub(super) max_depth: usize,
    pub(super) max_match_arms: usize,
    pub(super) max_indexed_children: usize,
    pub(super) depth_arm_product_sum: usize,
    pub(super) depth_width_product_sum: usize,
    pub(super) local_bindings: usize,
    pub(super) pattern_bindings: usize,
    pub(super) binding_name_bytes: usize,
    pub(super) binding_depth_sum: usize,
    pub(super) max_index_digits: usize,
}

const AST_COMPLEX_CURSOR: usize = 1usize << (usize::BITS - 1);
const AST_CURSOR_INDEX_MASK: usize = AST_COMPLEX_CURSOR - 1;

fn ast_previous_child_path_index(cursor: usize) -> Option<usize> {
    (cursor & AST_CURSOR_INDEX_MASK).checked_sub(1)
}

fn ast_block_statement_index(
    statements: &[crate::ast::Statement],
    child_path_index: usize,
) -> usize {
    if statements
        .iter()
        .any(|statement| matches!(statement, crate::ast::Statement::While { .. }))
    {
        child_path_index / 2
    } else {
        child_path_index
    }
}

fn ast_block_statement_result_index(
    statements: &[crate::ast::Statement],
    statement_index: usize,
) -> Option<usize> {
    statements
        .get(..statement_index)?
        .iter()
        .try_fold(0usize, |result_index, statement| {
            result_index.checked_add(statement.child_count())
        })
}

fn ast_match_arm_index(arms: &[crate::ast::MatchArm], child_path_index: usize) -> Option<usize> {
    let arm_path_index = child_path_index.checked_sub(1)?;
    Some(if arms.iter().any(|arm| arm.guard.is_some()) {
        arm_path_index / 2
    } else {
        arm_path_index
    })
}

fn ast_match_arm_value_result_index(
    arms: &[crate::ast::MatchArm],
    arm_index: usize,
) -> Option<usize> {
    let preceding_arm_results = arms
        .get(..arm_index)?
        .iter()
        .try_fold(0usize, |results, arm| {
            results.checked_add(1 + usize::from(arm.guard.is_some()))
        })?;
    1usize
        .checked_add(preceding_arm_results)?
        .checked_add(usize::from(arms.get(arm_index)?.guard.is_some()))
}

pub(super) fn ast_child<'a>(
    expression: &'a crate::ast::Expr,
    cursor: &mut usize,
) -> Option<(usize, &'a crate::ast::Expr)> {
    let complex = *cursor & AST_COMPLEX_CURSOR != 0;
    let mut index = *cursor & AST_CURSOR_INDEX_MASK;
    let mut advance = |next: usize, path_index: usize, child| {
        *cursor = usize::from(complex)
            .checked_mul(AST_COMPLEX_CURSOR)?
            .checked_add(next)?;
        Some((path_index, child))
    };
    match &expression.kind {
        crate::ast::ExprKind::Call { args, .. } => {
            advance(index.checked_add(1)?, index, args.get(index)?)
        }
        crate::ast::ExprKind::MethodCall { receiver, args, .. } => {
            let child = if index == 0 {
                receiver.as_ref()
            } else {
                args.get(index - 1)?
            };
            advance(index.checked_add(1)?, index, child)
        }
        crate::ast::ExprKind::SuperMethod { args, .. } => {
            advance(index.checked_add(1)?, index, args.get(index)?)
        }
        crate::ast::ExprKind::Unary { value, .. }
        | crate::ast::ExprKind::Try { operand: value }
        | crate::ast::ExprKind::Project { base: value, .. } => {
            (index == 0).then(|| advance(1, 0, value.as_ref()))?
        }
        crate::ast::ExprKind::Binary { left, right, .. } => {
            let child = [left.as_ref(), right.as_ref()].get(index).copied()?;
            advance(index.checked_add(1)?, index, child)
        }
        crate::ast::ExprKind::Block { statements, tail } => {
            let has_while = complex
                || (index == 0
                    && statements
                        .iter()
                        .any(|statement| matches!(statement, crate::ast::Statement::While { .. })));
            if has_while {
                if !complex {
                    *cursor = AST_COMPLEX_CURSOR;
                    index = 0;
                }
                loop {
                    let slot_limit = statements.len().checked_mul(2)?;
                    if index < slot_limit {
                        let statement_index = index / 2;
                        let statement_child = index % 2;
                        let path_index = index;
                        index = index.checked_add(1)?;
                        *cursor = AST_COMPLEX_CURSOR.checked_add(index)?;
                        if let Some(child) = statements[statement_index].child(statement_child) {
                            return Some((path_index, child));
                        }
                        continue;
                    }
                    if index == slot_limit {
                        *cursor = AST_COMPLEX_CURSOR.checked_add(index.checked_add(1)?)?;
                        return Some((index, tail));
                    }
                    return None;
                }
            }
            let child = if index < statements.len() {
                statements[index].child(0)?
            } else if index == statements.len() {
                tail
            } else {
                return None;
            };
            advance(index.checked_add(1)?, index, child)
        }
        crate::ast::ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => [
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ]
        .get(index)
        .copied()
        .and_then(|child| advance(index.checked_add(1)?, index, child)),
        crate::ast::ExprKind::ConstructRecord { fields, .. }
        | crate::ast::ExprKind::ConstructVariant { fields, .. } => {
            let child = &fields.get(index)?.value;
            advance(index.checked_add(1)?, index, child)
        }
        crate::ast::ExprKind::Match {
            scrutinee, arms, ..
        } => {
            let has_guard = complex || (index == 0 && arms.iter().any(|arm| arm.guard.is_some()));
            if has_guard {
                if !complex {
                    *cursor = AST_COMPLEX_CURSOR;
                    index = 0;
                }
                loop {
                    if index == 0 {
                        *cursor = AST_COMPLEX_CURSOR + 1;
                        return Some((0, scrutinee));
                    }
                    let arm_slot = index - 1;
                    let arm_index = arm_slot / 2;
                    let arm_child = arm_slot % 2;
                    let arm = arms.get(arm_index)?;
                    let path_index = index;
                    index = index.checked_add(1)?;
                    *cursor = AST_COMPLEX_CURSOR.checked_add(index)?;
                    if arm_child == 0 {
                        if let Some(guard) = &arm.guard {
                            return Some((path_index, guard));
                        }
                    } else {
                        return Some((path_index, &arm.value));
                    }
                }
            }
            let child = if index == 0 {
                scrutinee.as_ref()
            } else {
                &arms.get(index - 1)?.value
            };
            advance(index.checked_add(1)?, index, child)
        }
        crate::ast::ExprKind::UpdateRecord { base, fields } => {
            let child = if index == 0 {
                base.as_ref()
            } else {
                &fields.get(index - 1)?.value
            };
            advance(index.checked_add(1)?, index, child)
        }
        crate::ast::ExprKind::Int(_)
        | crate::ast::ExprKind::Int32(_)
        | crate::ast::ExprKind::Char(_)
        | crate::ast::ExprKind::Uint8(_)
        | crate::ast::ExprKind::Usize(_)
        | crate::ast::ExprKind::ArrayU8(_)
        | crate::ast::ExprKind::RepeatArrayU8 { .. }
        | crate::ast::ExprKind::Float32(_)
        | crate::ast::ExprKind::Float64(_)
        | crate::ast::ExprKind::Bool(_)
        | crate::ast::ExprKind::String(_)
        | crate::ast::ExprKind::Var(_) => None,
    }
}

fn ast_child_identity_path_increment(
    expression: &crate::ast::Expr,
    child_index: usize,
    program: &Program,
) -> usize {
    match &expression.kind {
        crate::ast::ExprKind::Call { name, .. } => {
            let prefix = if program
                .interfaces
                .iter()
                .any(|interface| interface.imports.iter().any(|import| import.name == *name))
            {
                ".native-rust-arg."
            } else {
                ".arg."
            };
            prefix.len() + decimal_digits(child_index)
        }
        crate::ast::ExprKind::MethodCall { .. } | crate::ast::ExprKind::SuperMethod { .. } => {
            ".arg.".len() + decimal_digits(child_index)
        }
        crate::ast::ExprKind::Unary { .. } => ".value".len(),
        crate::ast::ExprKind::Binary { .. } => {
            if child_index == 0 { ".left" } else { ".right" }.len()
        }
        crate::ast::ExprKind::Block { statements, .. } => {
            let complex = statements
                .iter()
                .any(|statement| matches!(statement, crate::ast::Statement::While { .. }));
            let statement_index = if complex {
                child_index / 2
            } else {
                child_index
            };
            if statement_index < statements.len() {
                let suffix = match (
                    &statements[statement_index],
                    complex && child_index % 2 == 1,
                ) {
                    (crate::ast::Statement::While { .. }, false) => ".condition",
                    (crate::ast::Statement::While { .. }, true)
                    | (crate::ast::Statement::Unsafe { .. }, _) => ".body",
                    _ => ".value",
                };
                ".s".len() + decimal_digits(statement_index) + suffix.len()
            } else {
                ".tail".len()
            }
        }
        crate::ast::ExprKind::If { .. } => [".condition", ".then", ".else"]
            .get(child_index)
            .map_or(0, |segment| segment.len()),
        crate::ast::ExprKind::ConstructRecord { .. }
        | crate::ast::ExprKind::ConstructVariant { .. } => {
            ".field.".len() + decimal_digits(child_index) + ".value".len()
        }
        crate::ast::ExprKind::Match { arms, .. } => {
            if child_index == 0 {
                ".scrutinee".len()
            } else if arms.iter().any(|arm| arm.guard.is_some()) {
                let arm_index = (child_index - 1) / 2;
                let suffix = if (child_index - 1) & 1 == 0 {
                    ".guard"
                } else {
                    ".value"
                };
                ".arm.".len() + decimal_digits(arm_index) + suffix.len()
            } else {
                ".arm.".len() + decimal_digits(child_index - 1) + ".value".len()
            }
        }
        crate::ast::ExprKind::UpdateRecord { .. } => {
            if child_index == 0 {
                ".base".len()
            } else {
                ".field.".len() + decimal_digits(child_index - 1) + ".value".len()
            }
        }
        crate::ast::ExprKind::Try { .. } => ".operand".len(),
        crate::ast::ExprKind::Project { .. } => ".base".len(),
        crate::ast::ExprKind::Int(_)
        | crate::ast::ExprKind::Int32(_)
        | crate::ast::ExprKind::Char(_)
        | crate::ast::ExprKind::Uint8(_)
        | crate::ast::ExprKind::Usize(_)
        | crate::ast::ExprKind::ArrayU8(_)
        | crate::ast::ExprKind::RepeatArrayU8 { .. }
        | crate::ast::ExprKind::Float32(_)
        | crate::ast::ExprKind::Float64(_)
        | crate::ast::ExprKind::Bool(_)
        | crate::ast::ExprKind::String(_)
        | crate::ast::ExprKind::Var(_) => 0,
    }
}

fn ast_root_identity_path_len(function: &crate::ast::Function, root_index: usize) -> usize {
    match root_index.cmp(&function.requires.len()) {
        std::cmp::Ordering::Less => "requires.".len() + decimal_digits(root_index),
        std::cmp::Ordering::Equal => "body".len(),
        std::cmp::Ordering::Greater => {
            "ensures.".len() + decimal_digits(root_index - function.requires.len() - 1)
        }
    }
}

fn ast_type_identity_key_len(program: &Program, root: &crate::ast::Type) -> Option<usize> {
    #[derive(Clone, Copy)]
    enum Frame<'a> {
        Enter(&'a crate::ast::Type),
        Finish(&'a crate::ast::TypeDeclaration, usize),
    }

    let mut frames = [None; MAX_FORMAT_NESTING * 2];
    let mut results = [0usize; MAX_FORMAT_NESTING];
    frames[0] = Some(Frame::Enter(root));
    let mut frame_len = 1usize;
    let mut result_len = 0usize;
    while frame_len != 0 {
        frame_len -= 1;
        match frames[frame_len].take()? {
            Frame::Enter(crate::ast::Type::I64) => {
                results[result_len] = "i64".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::I32) => {
                results[result_len] = "i32".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::Char) => {
                results[result_len] = "char".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::U8) => {
                results[result_len] = "u8".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::Usize) => {
                results[result_len] = "usize".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::ArrayU8(length)) => {
                results[result_len] = "array:u8:"
                    .len()
                    .checked_add(decimal_digits(*length as usize))?;
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::F32) => {
                results[result_len] = "f32".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::F64) => {
                results[result_len] = "f64".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::Bool) => {
                results[result_len] = "bool".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::String) => {
                results[result_len] = "string".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::Bytes) => {
                results[result_len] = "bytes".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::Str) => {
                results[result_len] = "str".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::SliceU8) => {
                results[result_len] = "slice-u8".len();
                result_len = result_len.checked_add(1)?;
            }
            Frame::Enter(crate::ast::Type::Named { name, arguments }) => {
                let declaration = program
                    .types
                    .iter()
                    .find(|declaration| declaration.name == *name)?;
                if frame_len.checked_add(arguments.len())?.checked_add(1)? > frames.len() {
                    return None;
                }
                frames[frame_len] = Some(Frame::Finish(declaration, arguments.len()));
                frame_len += 1;
                for argument in arguments.iter().rev() {
                    frames[frame_len] = Some(Frame::Enter(argument));
                    frame_len += 1;
                }
            }
            Frame::Finish(declaration, argument_count) => {
                let start = result_len.checked_sub(argument_count)?;
                let encoded_arguments =
                    results[start..result_len]
                        .iter()
                        .try_fold(0usize, |bytes, key_len| {
                            bytes
                                .checked_add(decimal_digits(*key_len))?
                                .checked_add(1)?
                                .checked_add(*key_len)
                        })?;
                result_len = start;
                let declaration_len = declaration.stable_id.len();
                let key_len = "nominal:"
                    .len()
                    .checked_add(decimal_digits(declaration_len))?
                    .checked_add(1)?
                    .checked_add(declaration_len)?
                    .checked_add(1)?
                    .checked_add(decimal_digits(argument_count))?
                    .checked_add(1)?
                    .checked_add(encoded_arguments)?;
                results[result_len] = key_len;
                result_len = result_len.checked_add(1)?;
            }
        }
    }
    (result_len == 1).then_some(results[0])
}

fn function_instance_identity_len(
    program: &Program,
    function: &crate::ast::Function,
    type_arguments: &[crate::ast::Type],
) -> Option<usize> {
    if type_arguments.len() != function.type_parameters.len() {
        return None;
    }
    let encoded_arguments = type_arguments.iter().try_fold(0usize, |bytes, ty| {
        let key_len = ast_type_identity_key_len(program, ty)?;
        bytes
            .checked_add(decimal_digits(key_len))?
            .checked_add(1)?
            .checked_add(key_len)
    })?;
    "semaprax.function-instance.v1:"
        .len()
        .checked_add(decimal_digits(function.stable_id.len()))?
        .checked_add(1)?
        .checked_add(function.stable_id.len())?
        .checked_add(1)?
        .checked_add(decimal_digits(type_arguments.len()))?
        .checked_add(1)?
        .checked_add(encoded_arguments)
}

pub(super) fn generic_function_instance_identity_upper(
    program: &Program,
    function: &crate::ast::Function,
) -> Option<usize> {
    if function.type_parameters.is_empty() {
        return Some(0);
    }
    let mut maximum = 0usize;
    let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for caller in program
        .functions
        .iter()
        .filter(|caller| caller.type_parameters.is_empty())
    {
        for root in caller
            .requires
            .iter()
            .chain(std::iter::once(&caller.body))
            .chain(&caller.ensures)
        {
            let mut len = 1usize;
            traversal[0] = Some((root, 0usize, 0usize));
            while len != 0 {
                len -= 1;
                let (expression, next_child, _) = traversal[len].take()?;
                if next_child == 0 {
                    if let crate::ast::ExprKind::Call {
                        name,
                        type_arguments,
                        ..
                    } = &expression.kind
                    {
                        if *name == function.name {
                            if let Some(identity_len) =
                                function_instance_identity_len(program, function, type_arguments)
                            {
                                maximum = maximum.max(identity_len);
                            }
                        }
                    }
                }
                let mut child_cursor = next_child;
                if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
                    if len + 2 > traversal.len() {
                        return None;
                    }
                    traversal[len] = Some((expression, child_cursor, 0));
                    traversal[len + 1] = Some((child, 0, 0));
                    len += 2;
                }
            }
        }
    }
    Some(maximum)
}

fn scoped_identity_upper(
    function: &crate::ast::Function,
    generic_instance_identity_len: usize,
    kind_len: usize,
    path_len: usize,
) -> Option<usize> {
    let monomorphic = "declaration:"
        .len()
        .checked_add(decimal_digits(function.stable_id.len()))?
        .checked_add(1)?
        .checked_add(function.stable_id.len())?
        .checked_add(1)?
        .checked_add(kind_len)?
        .checked_add(1)?
        .checked_add(decimal_digits(path_len))?
        .checked_add(1)?
        .checked_add(path_len)?;
    if function.type_parameters.is_empty() {
        return Some(monomorphic);
    }
    if generic_instance_identity_len == 0 {
        return Some(monomorphic);
    }
    let owner_len = "semaprax.function-execution.v1:generic:"
        .len()
        .checked_add(decimal_digits(generic_instance_identity_len))?
        .checked_add(1)?
        .checked_add(generic_instance_identity_len)?;
    let generic = "function-execution:"
        .len()
        .checked_add(decimal_digits(owner_len))?
        .checked_add(1)?
        .checked_add(owner_len)?
        .checked_add(1)?
        .checked_add(kind_len)?
        .checked_add(1)?
        .checked_add(decimal_digits(path_len))?
        .checked_add(1)?
        .checked_add(path_len)?;
    Some(monomorphic.max(generic))
}

fn scoped_value_identity_upper(
    function: &crate::ast::Function,
    generic_instance_identity_len: usize,
    path_len: usize,
) -> Option<usize> {
    scoped_identity_upper(
        function,
        generic_instance_identity_len,
        "value:result".len().max("value:local".len()),
        path_len,
    )
}

fn scoped_expression_identity_upper(
    function: &crate::ast::Function,
    generic_instance_identity_len: usize,
    path_len: usize,
) -> Option<usize> {
    scoped_identity_upper(
        function,
        generic_instance_identity_len,
        "expression".len(),
        path_len,
    )
}

pub(super) fn scan_ast_capacity<'a>(
    roots: impl IntoIterator<Item = &'a crate::ast::Expr>,
    program: &Program,
    count_generic_calls: bool,
    stack: &mut [Option<(&'a crate::ast::Expr, usize, usize)>; MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<AstCapacityStats, Diagnostic> {
    let mut stats = AstCapacityStats::default();
    for root in roots {
        let mut stack_len = 1;
        stack[0] = Some((root, 1, 0));
        while stack_len != 0 {
            stack_len -= 1;
            let (expression, depth, next_child) = stack[stack_len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            if next_child == 0 {
                stats.nodes = stats
                    .nodes
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                stats.cumulative_depth = stats
                    .cumulative_depth
                    .checked_add(depth)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                stats.max_depth = stats.max_depth.max(depth);
                let indexed_children = match &expression.kind {
                    crate::ast::ExprKind::Call { args, .. } => args.len(),
                    crate::ast::ExprKind::MethodCall { args, .. } => args.len() + 1,
                    crate::ast::ExprKind::SuperMethod { args, .. } => args.len(),
                    crate::ast::ExprKind::Block { statements, .. } => {
                        if statements.iter().any(|statement| {
                            matches!(statement, crate::ast::Statement::While { .. })
                        }) {
                            statements
                                .len()
                                .checked_mul(2)
                                .and_then(|slots| slots.checked_add(1))
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
                        } else {
                            statements
                                .len()
                                .checked_add(1)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
                        }
                    }
                    crate::ast::ExprKind::ConstructRecord { fields, .. }
                    | crate::ast::ExprKind::ConstructVariant { fields, .. } => fields.len(),
                    crate::ast::ExprKind::Match { arms, .. } => {
                        stats.max_match_arms = stats.max_match_arms.max(arms.len());
                        if arms.iter().any(|arm| arm.guard.is_some()) {
                            arms.len()
                                .checked_mul(2)
                                .and_then(|slots| slots.checked_add(1))
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
                        } else {
                            arms.len()
                                .checked_add(1)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
                        }
                    }
                    crate::ast::ExprKind::UpdateRecord { fields, .. } => fields.len() + 1,
                    crate::ast::ExprKind::If { .. } => 3,
                    crate::ast::ExprKind::Binary { .. } => 2,
                    crate::ast::ExprKind::Unary { .. }
                    | crate::ast::ExprKind::Try { .. }
                    | crate::ast::ExprKind::Project { .. } => 1,
                    crate::ast::ExprKind::Int(_)
                    | crate::ast::ExprKind::Int32(_)
                    | crate::ast::ExprKind::Char(_)
                    | crate::ast::ExprKind::Uint8(_)
                    | crate::ast::ExprKind::Usize(_)
                    | crate::ast::ExprKind::ArrayU8(_)
                    | crate::ast::ExprKind::RepeatArrayU8 { .. }
                    | crate::ast::ExprKind::Float32(_)
                    | crate::ast::ExprKind::Float64(_)
                    | crate::ast::ExprKind::Bool(_)
                    | crate::ast::ExprKind::String(_)
                    | crate::ast::ExprKind::Var(_) => 0,
                };
                stats.max_indexed_children = stats.max_indexed_children.max(indexed_children);
                stats.max_index_digits = stats
                    .max_index_digits
                    .max(decimal_digits(indexed_children.saturating_sub(1)));
                if let crate::ast::ExprKind::Block { statements, .. } = &expression.kind {
                    let (binding_statements, binding_name_bytes) = statements
                        .iter()
                        .try_fold(
                            (0usize, 0usize),
                            |(count, bytes), statement| match statement {
                                crate::ast::Statement::Let { name, .. }
                                | crate::ast::Statement::Assign { name, .. } => {
                                    Some((count.checked_add(1)?, bytes.checked_add(name.len())?))
                                }
                                crate::ast::Statement::Unsafe { .. }
                                | crate::ast::Statement::While { .. } => Some((count, bytes)),
                            },
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    stats.local_bindings = stats
                        .local_bindings
                        .checked_add(binding_statements)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    stats.binding_name_bytes = stats
                        .binding_name_bytes
                        .checked_add(binding_name_bytes)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    stats.binding_depth_sum = stats
                        .binding_depth_sum
                        .checked_add(
                            depth
                                .checked_mul(binding_statements)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                stats.depth_width_product_sum = stats
                    .depth_width_product_sum
                    .checked_add(
                        depth
                            .checked_mul(indexed_children)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if let crate::ast::ExprKind::Match { arms, .. } = &expression.kind {
                    for arm in arms {
                        let (bindings, names) = ast_pattern_binding_stats(&arm.pattern)?;
                        stats.pattern_bindings = stats
                            .pattern_bindings
                            .checked_add(bindings)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        stats.binding_name_bytes = stats
                            .binding_name_bytes
                            .checked_add(names)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        stats.binding_depth_sum = stats
                            .binding_depth_sum
                            .checked_add(
                                depth
                                    .checked_mul(bindings)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                            )
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        stats.max_index_digits = stats
                            .max_index_digits
                            .max(ast_pattern_index_digits(&arm.pattern)?);
                    }
                    stats.depth_arm_product_sum = stats
                        .depth_arm_product_sum
                        .checked_add(
                            depth
                                .checked_mul(arms.len())
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                if let crate::ast::ExprKind::Call {
                    name,
                    type_arguments,
                    ..
                } = &expression.kind
                {
                    if count_generic_calls
                        && !type_arguments.is_empty()
                        && program.functions.iter().any(|function| {
                            !function.type_parameters.is_empty() && function.name == *name
                        })
                    {
                        stats.generic_calls = stats
                            .generic_calls
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                }
            }
            let mut child_cursor = next_child;
            if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
                if stack_len + 2 > stack.len() {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                stack[stack_len] = Some((expression, depth, child_cursor));
                stack[stack_len + 1] = Some((child, depth + 1, 0));
                stack_len += 2;
            }
        }
    }
    Ok(stats)
}

fn ast_pattern_index_digits(pattern: &crate::ast::MatchPattern) -> Result<usize, Diagnostic> {
    let crate::ast::MatchPattern::Record { fields, .. } = pattern else {
        return Ok(match pattern {
            crate::ast::MatchPattern::Variant { fields, .. } => {
                decimal_digits(fields.len().saturating_sub(1))
            }
            _ => 1,
        });
    };
    let mut pending: [Option<(&[crate::ast::RecordMatchPatternField], usize)>; MAX_FORMAT_NESTING] =
        [None; MAX_FORMAT_NESTING];
    pending[0] = Some((fields, 0));
    let mut len = 1;
    let mut digits = 1;
    while len != 0 {
        len -= 1;
        let (fields, next) = pending[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        digits = digits.max(decimal_digits(fields.len().saturating_sub(1)));
        let Some(field) = fields.get(next) else {
            continue;
        };
        pending[len] = Some((fields, next + 1));
        len += 1;
        if let crate::ast::RecordMatchFieldPattern::Record { fields, .. } = &field.pattern {
            if len == pending.len() {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
            pending[len] = Some((fields, 0));
            len += 1;
        }
    }
    Ok(digits)
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn ast_pattern_binding_stats(
    pattern: &crate::ast::MatchPattern,
) -> Result<(usize, usize), Diagnostic> {
    match pattern {
        // Refutable Match v1: literal/or patterns bind nothing; a binding
        // arm binds exactly the scrutinee.
        crate::ast::MatchPattern::Wildcard { .. } => Ok((0, 0)),
        crate::ast::MatchPattern::Literal { .. } => Ok((0, 0)),
        crate::ast::MatchPattern::Or { alternatives, .. } => {
            let mut total = (0usize, 0usize);
            for alternative in alternatives {
                let (count, names) = ast_pattern_binding_stats(alternative)?;
                total.0 = total.0.saturating_add(count);
                total.1 = total.1.saturating_add(names);
            }
            Ok(total)
        }
        crate::ast::MatchPattern::Binding { name, .. } => Ok((1, name.len())),
        crate::ast::MatchPattern::Variant { fields, .. } => Ok((
            fields.len(),
            fields.iter().map(|field| field.binding.len()).sum(),
        )),
        crate::ast::MatchPattern::Record { fields, .. } => {
            let mut pending: [Option<(&[crate::ast::RecordMatchPatternField], usize)>;
                MAX_FORMAT_NESTING] = [None; MAX_FORMAT_NESTING];
            pending[0] = Some((fields, 0));
            let mut len = 1;
            let mut count = 0usize;
            let mut names = 0usize;
            while len != 0 {
                len -= 1;
                let (fields, next) = pending[len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let Some(field) = fields.get(next) else {
                    continue;
                };
                pending[len] = Some((fields, next + 1));
                len += 1;
                match &field.pattern {
                    crate::ast::RecordMatchFieldPattern::Binding { .. } => {
                        count = count
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        if let crate::ast::RecordMatchFieldPattern::Binding { name, .. } =
                            &field.pattern
                        {
                            names = names
                                .checked_add(name.len())
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                    }
                    crate::ast::RecordMatchFieldPattern::Record { fields, .. } => {
                        if len == pending.len() {
                            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                        }
                        pending[len] = Some((fields, 0));
                        len += 1;
                    }
                    crate::ast::RecordMatchFieldPattern::Wildcard { .. } => {}
                }
            }
            Ok((count, names))
        }
    }
}

fn declaration_field_type(
    declaration: &crate::ast::TypeDeclaration,
    mut index: usize,
) -> Option<&crate::ast::Type> {
    match &declaration.kind {
        crate::ast::TypeDeclarationKind::Resource { .. } => None,
        crate::ast::TypeDeclarationKind::Record { fields }
        | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
            fields.get(index).map(|field| &field.ty)
        }
        crate::ast::TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                if index < case.fields.len() {
                    return Some(&case.fields[index].ty);
                }
                index -= case.fields.len();
            }
            None
        }
    }
}

fn declaration_field_identity_bytes(
    declaration: &crate::ast::TypeDeclaration,
    mut index: usize,
) -> Option<usize> {
    match &declaration.kind {
        crate::ast::TypeDeclarationKind::Resource { .. } => None,
        crate::ast::TypeDeclarationKind::Record { fields }
        | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
            fields.get(index).map(|field| field.stable_id.len())
        }
        crate::ast::TypeDeclarationKind::Variant { cases } => {
            for case in cases {
                if index < case.fields.len() {
                    return case
                        .stable_id
                        .len()
                        .checked_add(case.fields[index].stable_id.len());
                }
                index -= case.fields.len();
            }
            None
        }
    }
}

fn ast_resource_leaf_count(
    root: &crate::ast::Type,
    program: &Program,
) -> Result<usize, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a crate::ast::Type, usize),
        Children(&'a crate::ast::TypeDeclaration, usize, usize, usize),
        Add(&'a crate::ast::TypeDeclaration, usize, usize, usize),
    }
    let mut frames: [Option<Frame<'_>>; MAX_FORMAT_NESTING] = std::array::from_fn(|_| None);
    let mut ancestors: [Option<&str>; MAX_FORMAT_NESTING] = [None; MAX_FORMAT_NESTING];
    let mut values = [0usize; MAX_FORMAT_NESTING];
    frames[0] = Some(Frame::Enter(root, 0));
    let (mut frame_len, mut value_len) = (1usize, 0usize);
    while frame_len != 0 {
        frame_len -= 1;
        match frames[frame_len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        {
            Frame::Enter(
                crate::ast::Type::I64
                | crate::ast::Type::I32
                | crate::ast::Type::Char
                | crate::ast::Type::U8
                | crate::ast::Type::Usize
                | crate::ast::Type::F32
                | crate::ast::Type::F64
                | crate::ast::Type::Bool
                | crate::ast::Type::String
                | crate::ast::Type::Str
                | crate::ast::Type::ArrayU8(_)
                | crate::ast::Type::SliceU8,
                _,
            ) => {
                values[value_len] = 0;
                value_len += 1;
            }
            Frame::Enter(crate::ast::Type::Bytes, _) => {
                // Bytes is one compiler-owned cleanup leaf even though this
                // native-rust interop profile rejects it at admission.
                values[value_len] = 1;
                value_len += 1;
            }
            Frame::Enter(crate::ast::Type::Named { name, .. }, depth) => {
                let Some(declaration) = program.types.iter().find(|value| value.name == *name)
                else {
                    values[value_len] = 0;
                    value_len += 1;
                    continue;
                };
                if ancestors[..depth].contains(&Some(name.as_str())) {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                ancestors[depth] = Some(name);
                if matches!(
                    declaration.kind,
                    crate::ast::TypeDeclarationKind::Resource { .. }
                ) {
                    values[value_len] = 1;
                    value_len += 1;
                    ancestors[depth] = None;
                } else {
                    frames[frame_len] = Some(Frame::Children(declaration, 0, 0, depth));
                    frame_len += 1;
                }
            }
            Frame::Children(declaration, index, total, depth) => {
                if let Some(child) = declaration_field_type(declaration, index) {
                    if frame_len + 2 > frames.len() || depth + 1 >= ancestors.len() {
                        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                    }
                    frames[frame_len] = Some(Frame::Add(declaration, index + 1, total, depth));
                    frames[frame_len + 1] = Some(Frame::Enter(child, depth + 1));
                    frame_len += 2;
                } else {
                    ancestors[depth] = None;
                    values[value_len] = total;
                    value_len += 1;
                }
            }
            Frame::Add(declaration, index, total, depth) => {
                value_len = value_len
                    .checked_sub(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let total = total
                    .checked_add(values[value_len])
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if total > MAX_BUILDER_BYTES {
                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                }
                frames[frame_len] = Some(Frame::Children(declaration, index, total, depth));
                frame_len += 1;
            }
        }
    }
    (value_len == 1)
        .then_some(values[0])
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn maximum_resource_leaf_count(program: &Program) -> Result<usize, Diagnostic> {
    let mut maximum = 1usize;
    for declaration in &program.types {
        let leaves = match &declaration.kind {
            crate::ast::TypeDeclarationKind::Resource { .. } => 1,
            crate::ast::TypeDeclarationKind::Record { fields }
            | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                fields
                    .iter()
                    .try_fold(0usize, |total, field| -> Result<usize, Diagnostic> {
                        total
                            .checked_add(ast_resource_leaf_count(&field.ty, program)?)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                    })?
            }
            crate::ast::TypeDeclarationKind::Variant { cases } => {
                cases
                    .iter()
                    .try_fold(0usize, |total, case| -> Result<usize, Diagnostic> {
                        case.fields.iter().try_fold(
                            total,
                            |total, field| -> Result<usize, Diagnostic> {
                                total
                                    .checked_add(ast_resource_leaf_count(&field.ty, program)?)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                            },
                        )
                    })?
            }
        };
        maximum = maximum.max(leaves);
    }
    Ok(maximum)
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DeclarationDagExpansion {
    pub(super) maximum_resource_leaves: usize,
    pub(super) maximum_type_occurrences: usize,
    pub(super) maximum_shape_fields: usize,
    pub(super) maximum_projection_segments: usize,
    pub(super) maximum_shape_identity_bytes: usize,
    pub(super) maximum_lifecycle_identity_bytes: usize,
    pub(super) maximum_projection_identity_bytes: usize,
    pub(super) cleanup_retained: CleanupRetainedStats,
}

#[derive(Clone, Copy, Default)]
struct CleanupTypeFacts {
    leaves: usize,
    occurrences: usize,
    shape_fields: usize,
    projection_segments: usize,
    shape_ids: usize,
    lifecycle_ids: usize,
    projection_ids: usize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CleanupRetainedStats {
    pub(super) roots: usize,
    pub(super) occurrences: usize,
    pub(super) shape_fields: usize,
    pub(super) leaves: usize,
    pub(super) projection_segments: usize,
    pub(super) shape_ids: usize,
    pub(super) lifecycle_ids: usize,
    pub(super) projection_ids: usize,
    pub(super) finalizer_copies: usize,
    pub(super) finalizer_projection_segments: usize,
    pub(super) finalizer_lifecycle_ids: usize,
    pub(super) finalizer_projection_ids: usize,
    pub(super) place_copies: usize,
    pub(super) place_projection_segments: usize,
    pub(super) place_projection_ids: usize,
    pub(super) call_arguments: usize,
    pub(super) call_argument_owned_bytes: usize,
    pub(super) parent_local_epochs: usize,
    pub(super) parent_local_zero_lifetime_transfers: usize,
    pub(super) parent_local_partial_fields: usize,
    pub(super) parent_local_finalizer_copies: usize,
    pub(super) parent_local_finalizer_projection_segments: usize,
    pub(super) parent_local_finalizer_lifecycle_ids: usize,
    pub(super) parent_local_finalizer_projection_ids: usize,
    pub(super) parent_local_finalizer_storage_bytes: usize,
    pub(super) parent_local_projection_epochs: usize,
    pub(super) parent_local_projection_exit_groups: usize,
    pub(super) parent_local_projection_finalizer_copies: usize,
    pub(super) parent_local_projection_finalizer_projection_segments: usize,
    pub(super) parent_local_projection_finalizer_lifecycle_ids: usize,
    pub(super) parent_local_projection_finalizer_projection_ids: usize,
    pub(super) parent_local_projection_finalizer_storage_bytes: usize,
    pub(super) parent_local_update_prefix_fields: usize,
    pub(super) parent_local_update_prefix_exit_groups: usize,
    pub(super) parent_local_update_prefix_finalizer_copies: usize,
    pub(super) parent_local_update_prefix_finalizer_projection_segments: usize,
    pub(super) parent_local_update_prefix_finalizer_lifecycle_ids: usize,
    pub(super) parent_local_update_prefix_finalizer_projection_ids: usize,
    pub(super) parent_local_update_prefix_finalizer_storage_bytes: usize,
    pub(super) ordinary_slot_payload_bytes: usize,
    pub(super) ordinary_place_storage_bytes: usize,
    pub(super) ordinary_finalizer_storage_bytes: usize,
    pub(super) staged_results: usize,
    pub(super) variant_edges: usize,
    pub(super) stage_identity_and_type_bytes: usize,
    pub(super) variant_identity_bytes: usize,
    pub(super) fallback_roots: usize,
    pub(super) exit_events: usize,
}

impl CleanupRetainedStats {
    fn add_root(&mut self, facts: CleanupTypeFacts) -> Option<()> {
        if facts.leaves == 0 {
            return Some(());
        }
        self.roots = self.roots.checked_add(1)?;
        self.occurrences = self.occurrences.checked_add(facts.occurrences)?;
        self.shape_fields = self.shape_fields.checked_add(facts.shape_fields)?;
        self.leaves = self.leaves.checked_add(facts.leaves)?;
        self.projection_segments = self
            .projection_segments
            .checked_add(facts.projection_segments)?;
        self.shape_ids = self.shape_ids.checked_add(facts.shape_ids)?;
        self.lifecycle_ids = self.lifecycle_ids.checked_add(facts.lifecycle_ids)?;
        self.projection_ids = self.projection_ids.checked_add(facts.projection_ids)?;
        Some(())
    }

    fn merge(&mut self, other: Self) -> Option<()> {
        self.roots = self.roots.checked_add(other.roots)?;
        self.occurrences = self.occurrences.checked_add(other.occurrences)?;
        self.shape_fields = self.shape_fields.checked_add(other.shape_fields)?;
        self.leaves = self.leaves.checked_add(other.leaves)?;
        self.projection_segments = self
            .projection_segments
            .checked_add(other.projection_segments)?;
        self.shape_ids = self.shape_ids.checked_add(other.shape_ids)?;
        self.lifecycle_ids = self.lifecycle_ids.checked_add(other.lifecycle_ids)?;
        self.projection_ids = self.projection_ids.checked_add(other.projection_ids)?;
        self.finalizer_copies = self.finalizer_copies.checked_add(other.finalizer_copies)?;
        self.finalizer_projection_segments = self
            .finalizer_projection_segments
            .checked_add(other.finalizer_projection_segments)?;
        self.finalizer_lifecycle_ids = self
            .finalizer_lifecycle_ids
            .checked_add(other.finalizer_lifecycle_ids)?;
        self.finalizer_projection_ids = self
            .finalizer_projection_ids
            .checked_add(other.finalizer_projection_ids)?;
        self.place_copies = self.place_copies.checked_add(other.place_copies)?;
        self.place_projection_segments = self
            .place_projection_segments
            .checked_add(other.place_projection_segments)?;
        self.place_projection_ids = self
            .place_projection_ids
            .checked_add(other.place_projection_ids)?;
        self.call_arguments = self.call_arguments.checked_add(other.call_arguments)?;
        self.call_argument_owned_bytes = self
            .call_argument_owned_bytes
            .checked_add(other.call_argument_owned_bytes)?;
        self.parent_local_epochs = self
            .parent_local_epochs
            .checked_add(other.parent_local_epochs)?;
        self.parent_local_zero_lifetime_transfers = self
            .parent_local_zero_lifetime_transfers
            .checked_add(other.parent_local_zero_lifetime_transfers)?;
        self.parent_local_partial_fields = self
            .parent_local_partial_fields
            .checked_add(other.parent_local_partial_fields)?;
        self.parent_local_finalizer_copies = self
            .parent_local_finalizer_copies
            .checked_add(other.parent_local_finalizer_copies)?;
        self.parent_local_finalizer_projection_segments = self
            .parent_local_finalizer_projection_segments
            .checked_add(other.parent_local_finalizer_projection_segments)?;
        self.parent_local_finalizer_lifecycle_ids = self
            .parent_local_finalizer_lifecycle_ids
            .checked_add(other.parent_local_finalizer_lifecycle_ids)?;
        self.parent_local_finalizer_projection_ids = self
            .parent_local_finalizer_projection_ids
            .checked_add(other.parent_local_finalizer_projection_ids)?;
        self.parent_local_finalizer_storage_bytes = self
            .parent_local_finalizer_storage_bytes
            .checked_add(other.parent_local_finalizer_storage_bytes)?;
        self.parent_local_projection_epochs = self
            .parent_local_projection_epochs
            .checked_add(other.parent_local_projection_epochs)?;
        self.parent_local_projection_exit_groups = self
            .parent_local_projection_exit_groups
            .checked_add(other.parent_local_projection_exit_groups)?;
        self.parent_local_projection_finalizer_copies = self
            .parent_local_projection_finalizer_copies
            .checked_add(other.parent_local_projection_finalizer_copies)?;
        self.parent_local_projection_finalizer_projection_segments = self
            .parent_local_projection_finalizer_projection_segments
            .checked_add(other.parent_local_projection_finalizer_projection_segments)?;
        self.parent_local_projection_finalizer_lifecycle_ids = self
            .parent_local_projection_finalizer_lifecycle_ids
            .checked_add(other.parent_local_projection_finalizer_lifecycle_ids)?;
        self.parent_local_projection_finalizer_projection_ids = self
            .parent_local_projection_finalizer_projection_ids
            .checked_add(other.parent_local_projection_finalizer_projection_ids)?;
        self.parent_local_projection_finalizer_storage_bytes = self
            .parent_local_projection_finalizer_storage_bytes
            .checked_add(other.parent_local_projection_finalizer_storage_bytes)?;
        self.parent_local_update_prefix_fields = self
            .parent_local_update_prefix_fields
            .checked_add(other.parent_local_update_prefix_fields)?;
        self.parent_local_update_prefix_exit_groups =
            self.parent_local_update_prefix_exit_groups
                .checked_add(other.parent_local_update_prefix_exit_groups)?;
        self.parent_local_update_prefix_finalizer_copies = self
            .parent_local_update_prefix_finalizer_copies
            .checked_add(other.parent_local_update_prefix_finalizer_copies)?;
        self.parent_local_update_prefix_finalizer_projection_segments = self
            .parent_local_update_prefix_finalizer_projection_segments
            .checked_add(other.parent_local_update_prefix_finalizer_projection_segments)?;
        self.parent_local_update_prefix_finalizer_lifecycle_ids = self
            .parent_local_update_prefix_finalizer_lifecycle_ids
            .checked_add(other.parent_local_update_prefix_finalizer_lifecycle_ids)?;
        self.parent_local_update_prefix_finalizer_projection_ids = self
            .parent_local_update_prefix_finalizer_projection_ids
            .checked_add(other.parent_local_update_prefix_finalizer_projection_ids)?;
        self.parent_local_update_prefix_finalizer_storage_bytes = self
            .parent_local_update_prefix_finalizer_storage_bytes
            .checked_add(other.parent_local_update_prefix_finalizer_storage_bytes)?;
        self.ordinary_slot_payload_bytes = self
            .ordinary_slot_payload_bytes
            .checked_add(other.ordinary_slot_payload_bytes)?;
        self.ordinary_place_storage_bytes = self
            .ordinary_place_storage_bytes
            .checked_add(other.ordinary_place_storage_bytes)?;
        self.ordinary_finalizer_storage_bytes = self
            .ordinary_finalizer_storage_bytes
            .checked_add(other.ordinary_finalizer_storage_bytes)?;
        self.staged_results = self.staged_results.checked_add(other.staged_results)?;
        self.variant_edges = self.variant_edges.checked_add(other.variant_edges)?;
        self.stage_identity_and_type_bytes = self
            .stage_identity_and_type_bytes
            .checked_add(other.stage_identity_and_type_bytes)?;
        self.variant_identity_bytes = self
            .variant_identity_bytes
            .checked_add(other.variant_identity_bytes)?;
        self.fallback_roots = self.fallback_roots.checked_add(other.fallback_roots)?;
        self.exit_events = self.exit_events.checked_add(other.exit_events)?;
        Some(())
    }

    fn scaled(self, multiplier: usize) -> Option<Self> {
        Some(Self {
            roots: self.roots.checked_mul(multiplier)?,
            occurrences: self.occurrences.checked_mul(multiplier)?,
            shape_fields: self.shape_fields.checked_mul(multiplier)?,
            leaves: self.leaves.checked_mul(multiplier)?,
            projection_segments: self.projection_segments.checked_mul(multiplier)?,
            shape_ids: self.shape_ids.checked_mul(multiplier)?,
            lifecycle_ids: self.lifecycle_ids.checked_mul(multiplier)?,
            projection_ids: self.projection_ids.checked_mul(multiplier)?,
            finalizer_copies: self.finalizer_copies.checked_mul(multiplier)?,
            finalizer_projection_segments: self
                .finalizer_projection_segments
                .checked_mul(multiplier)?,
            finalizer_lifecycle_ids: self.finalizer_lifecycle_ids.checked_mul(multiplier)?,
            finalizer_projection_ids: self.finalizer_projection_ids.checked_mul(multiplier)?,
            place_copies: self.place_copies.checked_mul(multiplier)?,
            place_projection_segments: self.place_projection_segments.checked_mul(multiplier)?,
            place_projection_ids: self.place_projection_ids.checked_mul(multiplier)?,
            call_arguments: self.call_arguments.checked_mul(multiplier)?,
            call_argument_owned_bytes: self.call_argument_owned_bytes.checked_mul(multiplier)?,
            parent_local_epochs: self.parent_local_epochs.checked_mul(multiplier)?,
            parent_local_zero_lifetime_transfers: self
                .parent_local_zero_lifetime_transfers
                .checked_mul(multiplier)?,
            parent_local_partial_fields: self
                .parent_local_partial_fields
                .checked_mul(multiplier)?,
            parent_local_finalizer_copies: self
                .parent_local_finalizer_copies
                .checked_mul(multiplier)?,
            parent_local_finalizer_projection_segments: self
                .parent_local_finalizer_projection_segments
                .checked_mul(multiplier)?,
            parent_local_finalizer_lifecycle_ids: self
                .parent_local_finalizer_lifecycle_ids
                .checked_mul(multiplier)?,
            parent_local_finalizer_projection_ids: self
                .parent_local_finalizer_projection_ids
                .checked_mul(multiplier)?,
            parent_local_finalizer_storage_bytes: self
                .parent_local_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            parent_local_projection_epochs: self
                .parent_local_projection_epochs
                .checked_mul(multiplier)?,
            parent_local_projection_exit_groups: self
                .parent_local_projection_exit_groups
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_copies: self
                .parent_local_projection_finalizer_copies
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_projection_segments: self
                .parent_local_projection_finalizer_projection_segments
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_lifecycle_ids: self
                .parent_local_projection_finalizer_lifecycle_ids
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_projection_ids: self
                .parent_local_projection_finalizer_projection_ids
                .checked_mul(multiplier)?,
            parent_local_projection_finalizer_storage_bytes: self
                .parent_local_projection_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            parent_local_update_prefix_fields: self
                .parent_local_update_prefix_fields
                .checked_mul(multiplier)?,
            parent_local_update_prefix_exit_groups: self
                .parent_local_update_prefix_exit_groups
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_copies: self
                .parent_local_update_prefix_finalizer_copies
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_projection_segments: self
                .parent_local_update_prefix_finalizer_projection_segments
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_lifecycle_ids: self
                .parent_local_update_prefix_finalizer_lifecycle_ids
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_projection_ids: self
                .parent_local_update_prefix_finalizer_projection_ids
                .checked_mul(multiplier)?,
            parent_local_update_prefix_finalizer_storage_bytes: self
                .parent_local_update_prefix_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            ordinary_slot_payload_bytes: self
                .ordinary_slot_payload_bytes
                .checked_mul(multiplier)?,
            ordinary_place_storage_bytes: self
                .ordinary_place_storage_bytes
                .checked_mul(multiplier)?,
            ordinary_finalizer_storage_bytes: self
                .ordinary_finalizer_storage_bytes
                .checked_mul(multiplier)?,
            staged_results: self.staged_results.checked_mul(multiplier)?,
            variant_edges: self.variant_edges.checked_mul(multiplier)?,
            stage_identity_and_type_bytes: self
                .stage_identity_and_type_bytes
                .checked_mul(multiplier)?,
            variant_identity_bytes: self.variant_identity_bytes.checked_mul(multiplier)?,
            fallback_roots: self.fallback_roots.checked_mul(multiplier)?,
            exit_events: self.exit_events.checked_mul(multiplier)?,
        })
    }
}

fn retained_vec_capacity_extra(logical_entries: usize, container_upper: usize) -> Option<usize> {
    if logical_entries == 0 {
        return Some(0);
    }
    let nonempty_containers = container_upper.min(logical_entries);
    nonempty_containers
        .checked_mul(8)
        .and_then(|capacity| capacity.checked_add(logical_entries.checked_mul(2)?))
        .and_then(|capacity| capacity.checked_sub(logical_entries))
}

pub(super) fn declaration_dag_expansion(
    program: &Program,
    generic_instance_upper: usize,
) -> Result<DeclarationDagExpansion, Diagnostic> {
    fn add_child(
        parent: &mut CleanupTypeFacts,
        child: CleanupTypeFacts,
        edge_ids: usize,
    ) -> Option<()> {
        parent.leaves = parent.leaves.checked_add(child.leaves)?;
        parent.occurrences = parent.occurrences.checked_add(child.occurrences)?;
        parent.shape_fields = parent
            .shape_fields
            .checked_add(1)?
            .checked_add(child.shape_fields)?;
        parent.projection_segments = parent
            .projection_segments
            .checked_add(child.projection_segments)?
            .checked_add(child.leaves)?;
        parent.shape_ids = parent
            .shape_ids
            .checked_add(edge_ids)?
            .checked_add(child.shape_ids)?;
        parent.lifecycle_ids = parent.lifecycle_ids.checked_add(child.lifecycle_ids)?;
        parent.projection_ids = parent
            .projection_ids
            .checked_add(child.projection_ids)?
            .checked_add(child.leaves.checked_mul(edge_ids)?)?;
        Some(())
    }

    let mut cleanup_node_count = 0usize;
    let mut cleanup_scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for function in source_functions(program) {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            let mut len = 1usize;
            cleanup_scan[0] = Some((root, 0usize, 0usize));
            while len != 0 {
                len -= 1;
                let (expression, next_child, _) = cleanup_scan[len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if next_child == 0 {
                    cleanup_node_count = cleanup_node_count
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                let mut child_cursor = next_child;
                if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
                    if len + 2 > cleanup_scan.len() {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                    cleanup_scan[len] = Some((expression, child_cursor, 0));
                    cleanup_scan[len + 1] = Some((child, 0, 0));
                    len += 2;
                }
            }
        }
    }
    let cleanup_node_capacity = cleanup_node_count.max(1);
    let count = program.types.len().max(1);
    let table_bytes = count
        .checked_mul(
            std::mem::size_of::<u8>()
                + std::mem::size_of::<CleanupTypeFacts>()
                + std::mem::size_of::<Option<(usize, usize, CleanupTypeFacts)>>(),
        )
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup_node_capacity.checked_mul(std::mem::size_of::<CleanupTypeKey>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let _table_budget = reserve_temporary_exact(table_bytes)?;
    let mut state = Vec::with_capacity(count);
    let mut facts = Vec::with_capacity(count);
    let mut stack: Vec<Option<(usize, usize, CleanupTypeFacts)>> = Vec::with_capacity(count);
    state.resize(count, 0u8);
    facts.resize(count, CleanupTypeFacts::default());
    stack.resize(count, None);
    if state.capacity() != count || facts.capacity() != count || stack.capacity() != count {
        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    let mut maximum_resource_leaves = 0usize;
    let mut maximum_type_occurrences = 1usize;
    let mut maximum_shape_fields = 0usize;
    let mut maximum_projection_segments = 0usize;
    let mut maximum_shape_identity_bytes = 0usize;
    let mut maximum_lifecycle_identity_bytes = 0usize;
    let mut maximum_projection_identity_bytes = 0usize;
    for root in 0..program.types.len() {
        if state[root] == 2 {
            continue;
        }
        stack[0] = Some((
            root,
            0,
            CleanupTypeFacts {
                occurrences: 1,
                shape_ids: program.types[root].stable_id.len(),
                ..CleanupTypeFacts::default()
            },
        ));
        state[root] = 1;
        let mut len = 1usize;
        while len != 0 {
            len -= 1;
            let (index, next, total) = stack[len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            let declaration = &program.types[index];
            if matches!(
                declaration.kind,
                crate::ast::TypeDeclarationKind::Resource { .. }
            ) {
                let lifecycle_bytes = match &declaration.kind {
                    crate::ast::TypeDeclarationKind::Resource { lifecycles } => lifecycles
                        .iter()
                        .filter_map(|lifecycle| lifecycle.stable_id.as_deref())
                        .try_fold(0usize, |bytes, id| bytes.checked_add(id.len()))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    _ => unreachable!(),
                };
                facts[index] = CleanupTypeFacts {
                    leaves: 1,
                    occurrences: 1,
                    shape_ids: lifecycle_bytes,
                    lifecycle_ids: lifecycle_bytes,
                    projection_ids: 0,
                    ..CleanupTypeFacts::default()
                };
                maximum_resource_leaves = maximum_resource_leaves.max(1);
                maximum_type_occurrences = maximum_type_occurrences.max(1);
                maximum_shape_identity_bytes = maximum_shape_identity_bytes.max(lifecycle_bytes);
                maximum_lifecycle_identity_bytes =
                    maximum_lifecycle_identity_bytes.max(lifecycle_bytes);
                state[index] = 2;
                if let Some(parent) = len.checked_sub(1).and_then(|parent| stack[parent].as_mut()) {
                    let parent_decl = &program.types[parent.0];
                    let edge = declaration_field_identity_bytes(parent_decl, parent.1 - 1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_child(&mut parent.2, facts[index], edge)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                continue;
            }
            let Some(child) = declaration_field_type(declaration, next) else {
                facts[index] = total;
                state[index] = 2;
                maximum_resource_leaves = maximum_resource_leaves.max(total.leaves);
                maximum_type_occurrences = maximum_type_occurrences.max(total.occurrences);
                maximum_shape_fields = maximum_shape_fields.max(total.shape_fields);
                maximum_projection_segments =
                    maximum_projection_segments.max(total.projection_segments);
                maximum_shape_identity_bytes = maximum_shape_identity_bytes.max(total.shape_ids);
                maximum_lifecycle_identity_bytes =
                    maximum_lifecycle_identity_bytes.max(total.lifecycle_ids);
                maximum_projection_identity_bytes =
                    maximum_projection_identity_bytes.max(total.projection_ids);
                if let Some(parent) = len.checked_sub(1).and_then(|parent| stack[parent].as_mut()) {
                    let parent_decl = &program.types[parent.0];
                    let edge = declaration_field_identity_bytes(parent_decl, parent.1 - 1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_child(&mut parent.2, total, edge)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                continue;
            };
            stack[len] = Some((index, next + 1, total));
            len += 1;
            let crate::ast::Type::Named { name, .. } = child else {
                let parent = stack[len - 1].as_mut().expect("parent retained");
                parent.2.occurrences = parent
                    .2
                    .occurrences
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_fields = parent
                    .2
                    .shape_fields
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_ids = parent
                    .2
                    .shape_ids
                    .checked_add(
                        declaration_field_identity_bytes(declaration, next)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                continue;
            };
            let Some(child_index) = program.types.iter().position(|value| value.name == *name)
            else {
                let parent = stack[len - 1].as_mut().expect("parent retained");
                parent.2.occurrences = parent
                    .2
                    .occurrences
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_fields = parent
                    .2
                    .shape_fields
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                parent.2.shape_ids = parent
                    .2
                    .shape_ids
                    .checked_add(
                        declaration_field_identity_bytes(declaration, next)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                continue;
            };
            match state[child_index] {
                2 => {
                    let parent = stack[len - 1].as_mut().expect("parent retained");
                    let edge = declaration_field_identity_bytes(declaration, next)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_child(&mut parent.2, facts[child_index], edge)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                1 => return Err(b107("selected identity missing")),
                _ => {
                    if len == stack.len() {
                        return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                    }
                    state[child_index] = 1;
                    stack[len] = Some((
                        child_index,
                        0,
                        CleanupTypeFacts {
                            occurrences: 1,
                            shape_ids: program.types[child_index].stable_id.len(),
                            ..CleanupTypeFacts::default()
                        },
                    ));
                    len += 1;
                }
            }
        }
    }
    let cleanup_retained = cleanup_retained_stats(
        program,
        &facts,
        cleanup_node_capacity,
        generic_instance_upper,
    )?;
    Ok(DeclarationDagExpansion {
        maximum_resource_leaves,
        maximum_type_occurrences,
        maximum_shape_fields,
        maximum_projection_segments,
        maximum_shape_identity_bytes,
        maximum_lifecycle_identity_bytes,
        maximum_projection_identity_bytes,
        cleanup_retained,
    })
}

#[derive(Clone, Copy)]
enum CleanupTypeKey {
    Scalar,
    Declaration(usize),
    Unknown,
}

pub(super) fn cleanup_source_exit_events(expression: &crate::ast::Expr) -> usize {
    match &expression.kind {
        crate::ast::ExprKind::Call { .. }
        | crate::ast::ExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            ..
        }
        | crate::ast::ExprKind::Binary {
            op:
                crate::ast::BinaryOp::Add
                | crate::ast::BinaryOp::Sub
                | crate::ast::BinaryOp::Mul
                | crate::ast::BinaryOp::Div
                | crate::ast::BinaryOp::Rem,
            ..
        }
        | crate::ast::ExprKind::Block { .. }
        | crate::ast::ExprKind::Try { .. }
        | crate::ast::ExprKind::UpdateRecord { .. } => 1,
        // If, lazy boolean, and Match are lowered in their active region.
        // Their authored Block children, when present, own the corresponding
        // lexical scope exits and are counted independently above.
        _ => 0,
    }
}

fn cleanup_source_failure_events(expression: &crate::ast::Expr) -> usize {
    match &expression.kind {
        crate::ast::ExprKind::Call { .. }
        | crate::ast::ExprKind::Unary {
            op: crate::ast::UnaryOp::Neg,
            ..
        }
        | crate::ast::ExprKind::Binary {
            op:
                crate::ast::BinaryOp::Add
                | crate::ast::BinaryOp::Sub
                | crate::ast::BinaryOp::Mul
                | crate::ast::BinaryOp::Div
                | crate::ast::BinaryOp::Rem,
            ..
        }
        | crate::ast::ExprKind::Try { .. } => 1,
        _ => 0,
    }
}

pub(super) fn cleanup_function_exit_events<'a>(
    function: &'a crate::ast::Function,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = function
        .requires
        .len()
        .checked_add(function.ensures.len())
        .and_then(|contracts| contracts.checked_mul(2))
        .and_then(|events| events.checked_add(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        let mut len = 1usize;
        traversal[0] = Some((root, 0, 0));
        while len != 0 {
            len -= 1;
            let (expression, next_child, _) = traversal[len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            if next_child == 0 {
                // lower_root_body reuses the function's root region instead
                // of creating an authored Block region for the outer body.
                if !std::ptr::eq(expression, &function.body) {
                    events = events
                        .checked_add(cleanup_source_exit_events(expression))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            let mut child_cursor = next_child;
            if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
                if len + 2 > traversal.len() {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                traversal[len] = Some((expression, child_cursor, 0));
                traversal[len + 1] = Some((child, 0, 0));
                len += 2;
            }
        }
    }
    Ok(events)
}

fn cleanup_expression_exit_events<'a>(
    root: &'a crate::ast::Expr,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut len = 1usize;
    traversal[0] = Some((root, 0, 0));
    while len != 0 {
        len -= 1;
        let (expression, next_child, _) = traversal[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next_child == 0 {
            events = events
                .checked_add(cleanup_source_exit_events(expression))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let mut child_cursor = next_child;
        if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
            if len + 2 > traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            traversal[len] = Some((expression, child_cursor, 0));
            traversal[len + 1] = Some((child, 0, 0));
            len += 2;
        }
    }
    Ok(events)
}

fn cleanup_expression_failure_events<'a>(
    root: &'a crate::ast::Expr,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut len = 1usize;
    traversal[0] = Some((root, 0, 0));
    while len != 0 {
        len -= 1;
        let (expression, next_child, _) = traversal[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next_child == 0 {
            events = events
                .checked_add(cleanup_source_failure_events(expression))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let mut child_cursor = next_child;
        if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
            if len + 2 > traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            traversal[len] = Some((expression, child_cursor, 0));
            traversal[len + 1] = Some((child, 0, 0));
            len += 2;
        }
    }
    Ok(events)
}

fn cleanup_expression_call_events<'a>(
    root: &'a crate::ast::Expr,
    program: &Program,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut len = 1usize;
    traversal[0] = Some((root, 0, 0));
    while len != 0 {
        len -= 1;
        let (expression, next_child, _) = traversal[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next_child == 0 {
            if let crate::ast::ExprKind::Call { name, .. } = &expression.kind {
                if !program
                    .interfaces
                    .iter()
                    .any(|interface| interface.imports.iter().any(|import| import.name == *name))
                {
                    events = events
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
        }
        let mut child_cursor = next_child;
        if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
            if len + 2 > traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            traversal[len] = Some((expression, child_cursor, 0));
            traversal[len + 1] = Some((child, 0, 0));
            len += 2;
        }
    }
    Ok(events)
}

fn cleanup_expression_boolean_branch_events<'a>(
    root: &'a crate::ast::Expr,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut len = 1usize;
    traversal[0] = Some((root, 0, 0));
    while len != 0 {
        len -= 1;
        let (expression, next_child, _) = traversal[len]
            .take()
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if next_child == 0
            && matches!(
                expression.kind,
                crate::ast::ExprKind::If { .. }
                    | crate::ast::ExprKind::Binary {
                        op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
                        ..
                    }
            )
        {
            events = events
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let mut child_cursor = next_child;
        if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
            if len + 2 > traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            traversal[len] = Some((expression, child_cursor, 0));
            traversal[len + 1] = Some((child, 0, 0));
            len += 2;
        }
    }
    Ok(events)
}

fn cleanup_plan_variable_identity_bytes(
    function: &crate::ast::Function,
    program: &Program,
    cleanup_path_copies: usize,
) -> Result<(usize, usize), Diagnostic> {
    fn child_path_increment(
        expression: &crate::ast::Expr,
        child_index: usize,
        program: &Program,
    ) -> usize {
        ast_child_identity_path_increment(expression, child_index, program)
    }

    let generic_instance_identity_len = generic_function_instance_identity_upper(program, function)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut bytes = 0usize;
    let mut all_expression_bytes = 0usize;
    for (root_index, (root, contract)) in function
        .requires
        .iter()
        .map(|root| (root, true))
        .chain(std::iter::once((&function.body, false)))
        .chain(function.ensures.iter().map(|root| (root, true)))
        .enumerate()
    {
        let path_len = match root_index.cmp(&function.requires.len()) {
            std::cmp::Ordering::Less => "requires.".len() + decimal_digits(root_index),
            std::cmp::Ordering::Equal => "body".len(),
            std::cmp::Ordering::Greater => {
                "ensures.".len() + decimal_digits(root_index - function.requires.len() - 1)
            }
        };
        let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        let mut len = 1usize;
        traversal[0] = Some((root, path_len, 0));
        while len != 0 {
            len -= 1;
            let (expression, path_len, next_child) = traversal[len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            if next_child == 0 {
                let mut copies = usize::from(contract && std::ptr::eq(expression, root))
                    .checked_mul(5)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                match &expression.kind {
                    crate::ast::ExprKind::Call { name, .. } => {
                        if !program.interfaces.iter().any(|interface| {
                            interface.imports.iter().any(|import| import.name == *name)
                        }) {
                            // StatusSource, two status edges, SelectFailure,
                            // ReturnFailure, and CallCommit.
                            copies = copies
                                .checked_add(6)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                    }
                    crate::ast::ExprKind::Unary {
                        op: crate::ast::UnaryOp::Neg,
                        ..
                    }
                    | crate::ast::ExprKind::Binary {
                        op:
                            crate::ast::BinaryOp::Add
                            | crate::ast::BinaryOp::Sub
                            | crate::ast::BinaryOp::Mul
                            | crate::ast::BinaryOp::Div
                            | crate::ast::BinaryOp::Rem,
                        ..
                    } => {
                        copies = copies
                            .checked_add(5)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                    crate::ast::ExprKind::If { .. }
                    | crate::ast::ExprKind::Binary {
                        op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
                        ..
                    } => {
                        copies = copies
                            .checked_add(2)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                    _ => {}
                }
                if std::ptr::eq(expression, &function.body) {
                    copies = copies
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
                let uncovered = copies
                    .checked_sub(copies.min(cleanup_path_copies))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let identity_bytes = scoped_expression_identity_upper(
                    function,
                    generic_instance_identity_len,
                    path_len,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                all_expression_bytes = all_expression_bytes
                    .checked_add(identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                bytes = bytes
                    .checked_add(
                        uncovered
                            .checked_mul(identity_bytes)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            let mut child_cursor = next_child;
            if let Some((child_index, child)) = ast_child(expression, &mut child_cursor) {
                if len + 2 > traversal.len() {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_path_len = path_len
                    .checked_add(child_path_increment(expression, child_index, program))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                traversal[len] = Some((expression, path_len, child_cursor));
                traversal[len + 1] = Some((child, child_path_len, 0));
                len += 2;
            }
        }
    }
    Ok((all_expression_bytes, bytes))
}

fn cleanup_function_finalizer_events<'a>(
    function: &'a crate::ast::Function,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = function
        .requires
        .len()
        .checked_add(function.ensures.len())
        .and_then(|events| events.checked_add(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        events = events
            .checked_add(cleanup_expression_failure_events(root, traversal)?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    Ok(events)
}

fn cleanup_function_region_depth<'a>(
    function: &'a crate::ast::Function,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut maximum = 1usize;
    for (root, contract_region) in function
        .requires
        .iter()
        .map(|root| (root, true))
        .chain(std::iter::once((&function.body, false)))
        .chain(function.ensures.iter().map(|root| (root, true)))
    {
        let root_region = 1usize
            .checked_add(usize::from(contract_region))
            .and_then(|depth| {
                depth.checked_add(usize::from(
                    !std::ptr::eq(root, &function.body)
                        && matches!(
                            root.kind,
                            crate::ast::ExprKind::Block { .. }
                                | crate::ast::ExprKind::UpdateRecord { .. }
                        ),
                ))
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        maximum = maximum.max(root_region);
        let mut len = 1usize;
        traversal[0] = Some((root, 0, root_region));
        while len != 0 {
            len -= 1;
            let (expression, next_child, region_depth) = traversal[len]
                .take()
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            let mut child_cursor = next_child;
            if let Some((_, child)) = ast_child(expression, &mut child_cursor) {
                if len + 2 > traversal.len() {
                    return Err(b109(
                        "max_semantic_expression_depth",
                        MAX_SEMANTIC_EXPRESSION_DEPTH,
                    ));
                }
                let child_depth = region_depth
                    .checked_add(usize::from(matches!(
                        child.kind,
                        crate::ast::ExprKind::Block { .. }
                            | crate::ast::ExprKind::UpdateRecord { .. }
                    )))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                maximum = maximum.max(child_depth);
                traversal[len] = Some((expression, child_cursor, region_depth));
                traversal[len + 1] = Some((child, 0, child_depth));
                len += 2;
            }
        }
    }
    Ok(maximum)
}

#[derive(Clone, Copy, Default)]
struct CleanupBindingFlow {
    failure_finalizers: usize,
    live_after: bool,
}

fn cleanup_binding_flow<'a>(
    root: &'a crate::ast::Expr,
    binding: &str,
    consumes_result: bool,
    program: &Program,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<CleanupBindingFlow, Diagnostic> {
    let mut consumes = [false; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut flows = [CleanupBindingFlow::default(); MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut branch_live = [false; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut stack_len = 1usize;
    traversal[0] = Some((root, 0, 0));
    consumes[0] = consumes_result;
    flows[0].live_after = true;
    let mut returned: Option<CleanupBindingFlow> = None;
    while stack_len != 0 {
        let frame_index = stack_len - 1;
        let consume = consumes[frame_index];
        let (expression, next_child, _) =
            traversal[frame_index].ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        if let Some(child) = returned.take() {
            let child_index = ast_previous_child_path_index(next_child)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            let flow = &mut flows[frame_index];
            let sequence = |flow: &mut CleanupBindingFlow,
                            child: CleanupBindingFlow|
             -> Result<(), Diagnostic> {
                if flow.live_after {
                    flow.failure_finalizers = flow
                        .failure_finalizers
                        .checked_add(child.failure_finalizers)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    flow.live_after = child.live_after;
                }
                Ok(())
            };
            match &expression.kind {
                crate::ast::ExprKind::If { .. } | crate::ast::ExprKind::Match { .. }
                    if child_index != 0 =>
                {
                    if flow.live_after {
                        flow.failure_finalizers = flow
                            .failure_finalizers
                            .checked_add(child.failure_finalizers)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        branch_live[frame_index] |= child.live_after;
                    }
                }
                crate::ast::ExprKind::Binary {
                    op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
                    ..
                } if child_index == 1 => {
                    if flow.live_after {
                        flow.failure_finalizers = flow
                            .failure_finalizers
                            .checked_add(child.failure_finalizers)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        // The lazy short-circuit path retains the binding even
                        // if the right operand consumes it.
                    }
                }
                _ => sequence(flow, child)?,
            }
        }

        let mut child_cursor = next_child;
        if let Some((child_index, child)) = ast_child(expression, &mut child_cursor) {
            if stack_len == traversal.len() {
                return Err(b109(
                    "max_semantic_expression_depth",
                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                ));
            }
            let child_consumes = match &expression.kind {
                crate::ast::ExprKind::Call { name, .. } => program
                    .functions
                    .iter()
                    .find(|function| function.name == *name)
                    .and_then(|function| function.params.get(child_index))
                    .is_some_and(|parameter| parameter.mode == crate::ast::ParamMode::Own),
                crate::ast::ExprKind::MethodCall { method, .. } => program
                    .types
                    .iter()
                    .find_map(|declaration| match &declaration.kind {
                        crate::ast::TypeDeclarationKind::Class { methods, .. } => {
                            methods.iter().find(|candidate| candidate.name == *method)
                        }
                        _ => None,
                    })
                    .and_then(|method_function| method_function.params.get(child_index))
                    .is_some_and(|parameter| parameter.mode == crate::ast::ParamMode::Own),
                crate::ast::ExprKind::Block { statements, .. } => {
                    let statement_index = ast_block_statement_index(statements, child_index);
                    statement_index < statements.len() || consume
                }
                crate::ast::ExprKind::If { .. } => child_index != 0 && consume,
                crate::ast::ExprKind::ConstructRecord { .. }
                | crate::ast::ExprKind::ConstructVariant { .. }
                | crate::ast::ExprKind::UpdateRecord { .. } => true,
                crate::ast::ExprKind::Match { arms, .. } => {
                    let is_guard = arms.iter().any(|arm| arm.guard.is_some())
                        && child_index != 0
                        && (child_index - 1) & 1 == 0;
                    child_index == 0 || (!is_guard && consume)
                }
                crate::ast::ExprKind::Try { .. } => true,
                crate::ast::ExprKind::Project { .. }
                | crate::ast::ExprKind::Unary { .. }
                | crate::ast::ExprKind::Binary { .. } => false,
                crate::ast::ExprKind::SuperMethod { .. } => child_index != 0 && consume,
                crate::ast::ExprKind::Int(_)
                | crate::ast::ExprKind::Int32(_)
                | crate::ast::ExprKind::Char(_)
                | crate::ast::ExprKind::Uint8(_)
                | crate::ast::ExprKind::Usize(_)
                | crate::ast::ExprKind::ArrayU8(_)
                | crate::ast::ExprKind::RepeatArrayU8 { .. }
                | crate::ast::ExprKind::Float32(_)
                | crate::ast::ExprKind::Float64(_)
                | crate::ast::ExprKind::Bool(_)
                | crate::ast::ExprKind::String(_)
                | crate::ast::ExprKind::Var(_) => false,
            };
            traversal[frame_index] = Some((expression, child_cursor, 0));
            traversal[stack_len] = Some((child, 0, 0));
            consumes[stack_len] = child_consumes;
            flows[stack_len] = CleanupBindingFlow {
                failure_finalizers: 0,
                live_after: true,
            };
            branch_live[stack_len] = false;
            stack_len += 1;
            continue;
        }

        let mut flow = flows[frame_index];
        match &expression.kind {
            crate::ast::ExprKind::If { .. } | crate::ast::ExprKind::Match { .. }
                if flow.live_after =>
            {
                flow.live_after = branch_live[frame_index];
            }
            _ => {}
        }
        if flow.live_after {
            flow.failure_finalizers = flow
                .failure_finalizers
                .checked_add(cleanup_source_failure_events(expression))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            if consume
                && matches!(&expression.kind, crate::ast::ExprKind::Var(name) if name == binding)
            {
                flow.live_after = false;
            }
        }
        traversal[frame_index] = None;
        stack_len -= 1;
        returned = Some(flow);
    }
    returned.ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn cleanup_block_binding_finalizer_events<'a>(
    function: &'a crate::ast::Function,
    block: &'a crate::ast::Expr,
    next_child: usize,
    binding: &str,
    program: &Program,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut live = true;
    let mut child_cursor = next_child;
    while let Some((_, child)) = ast_child(block, &mut child_cursor) {
        if live {
            let flow = cleanup_binding_flow(child, binding, true, program, traversal)?;
            events = events
                .checked_add(flow.failure_finalizers)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            live = flow.live_after;
        }
    }
    if live && std::ptr::eq(block, &function.body) {
        for ensure in &function.ensures {
            let flow = cleanup_binding_flow(ensure, binding, false, program, traversal)?;
            events = events
                .checked_add(flow.failure_finalizers)
                .and_then(|events| events.checked_add(1))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            live = flow.live_after;
            if !live {
                break;
            }
        }
    }
    events
        .checked_add(usize::from(live))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

pub(super) fn cleanup_parameter_finalizer_events<'a>(
    function: &'a crate::ast::Function,
    binding: &str,
    program: &Program,
    traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    let mut live = true;
    for require in &function.requires {
        let flow = cleanup_binding_flow(require, binding, false, program, traversal)?;
        events = events
            .checked_add(flow.failure_finalizers)
            .and_then(|events| events.checked_add(usize::from(live)))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        live &= flow.live_after;
    }
    if matches!(function.body.kind, crate::ast::ExprKind::Block { .. }) {
        return events
            .checked_add(cleanup_block_binding_finalizer_events(
                function,
                &function.body,
                0,
                binding,
                program,
                traversal,
            )?)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES));
    }
    if live {
        let flow = cleanup_binding_flow(&function.body, binding, true, program, traversal)?;
        events = events
            .checked_add(flow.failure_finalizers)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        live = flow.live_after;
    }
    for ensure in &function.ensures {
        if live {
            let flow = cleanup_binding_flow(ensure, binding, false, program, traversal)?;
            events = events
                .checked_add(flow.failure_finalizers)
                .and_then(|events| events.checked_add(1))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            live = flow.live_after;
        }
    }
    events
        .checked_add(usize::from(live))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn cleanup_parent_local_remaining_finalizer_events<'a>(
    function: &'a crate::ast::Function,
    root: &'a crate::ast::Expr,
    traversal: &[Option<(&'a crate::ast::Expr, usize, usize)>],
    stack_len: usize,
    event_traversal: &mut [Option<(&'a crate::ast::Expr, usize, usize)>;
             MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<usize, Diagnostic> {
    let mut events = 0usize;
    for (ancestor, next_child, _) in traversal[..stack_len].iter().rev().flatten().copied() {
        let active_child = ast_previous_child_path_index(next_child)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let mut add_later_child = |child_cursor: usize| -> Result<(), Diagnostic> {
            let mut child_cursor = child_cursor;
            if let Some((_, child)) = ast_child(ancestor, &mut child_cursor) {
                events = events
                    .checked_add(cleanup_expression_failure_events(child, event_traversal)?)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
            Ok(())
        };
        match &ancestor.kind {
            crate::ast::ExprKind::If { .. } => {
                if active_child == 0 {
                    add_later_child(1)?;
                    add_later_child(2)?;
                }
            }
            crate::ast::ExprKind::Match { arms, .. } => {
                if active_child == 0 {
                    let _ = arms;
                    let mut child_cursor = next_child;
                    while let Some((_, child)) = ast_child(ancestor, &mut child_cursor) {
                        events = events
                            .checked_add(cleanup_expression_failure_events(child, event_traversal)?)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                }
            }
            crate::ast::ExprKind::Binary {
                op: crate::ast::BinaryOp::And | crate::ast::BinaryOp::Or,
                ..
            } => {
                if active_child == 0 {
                    add_later_child(1)?;
                }
            }
            _ => {
                let mut child_cursor = next_child;
                while let Some((_, child)) = ast_child(ancestor, &mut child_cursor) {
                    events = events
                        .checked_add(cleanup_expression_failure_events(child, event_traversal)?)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
        }
        events = events
            .checked_add(cleanup_source_failure_events(ancestor))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if matches!(
            ancestor.kind,
            crate::ast::ExprKind::Block { .. } | crate::ast::ExprKind::UpdateRecord { .. }
        ) {
            if matches!(ancestor.kind, crate::ast::ExprKind::Block { .. })
                && std::ptr::eq(ancestor, &function.body)
            {
                for ensure in &function.ensures {
                    events = events
                        .checked_add(cleanup_expression_failure_events(ensure, event_traversal)?)
                        .and_then(|events| events.checked_add(1))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                }
            }
            return events
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
    }
    if std::ptr::eq(root, &function.body) {
        for ensure in &function.ensures {
            events = events
                .checked_add(cleanup_expression_failure_events(ensure, event_traversal)?)
                .and_then(|events| events.checked_add(1))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
    }
    events
        .checked_add(1)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
}

fn cleanup_retained_stats(
    program: &Program,
    declaration_facts: &[CleanupTypeFacts],
    node_capacity: usize,
    generic_instance_upper: usize,
) -> Result<CleanupRetainedStats, Diagnostic> {
    fn key_for_type(program: &Program, ty: &crate::ast::Type) -> CleanupTypeKey {
        match ty {
            crate::ast::Type::I64
            | crate::ast::Type::I32
            | crate::ast::Type::Char
            | crate::ast::Type::U8
            | crate::ast::Type::Usize
            | crate::ast::Type::F32
            | crate::ast::Type::F64
            | crate::ast::Type::Bool
            | crate::ast::Type::String
            | crate::ast::Type::Str
            | crate::ast::Type::ArrayU8(_)
            | crate::ast::Type::SliceU8 => CleanupTypeKey::Scalar,
            crate::ast::Type::Bytes => CleanupTypeKey::Unknown,
            crate::ast::Type::Named { name, .. } => {
                if let Some(index) = program
                    .types
                    .iter()
                    .position(|declaration| declaration.name == *name)
                {
                    CleanupTypeKey::Declaration(index)
                } else if matches!(name.as_str(), "Option" | "Result")
                    || program.types.iter().any(|declaration| {
                        declaration
                            .type_parameters
                            .iter()
                            .any(|parameter| parameter.name == *name)
                    })
                    || program.functions.iter().any(|function| {
                        function
                            .type_parameters
                            .iter()
                            .any(|parameter| parameter.name == *name)
                    })
                {
                    // Prelude Option/Result and admitted direct generic
                    // arguments are Copy-only at this boundary.
                    CleanupTypeKey::Scalar
                } else {
                    CleanupTypeKey::Unknown
                }
            }
        }
    }

    fn pattern_binding_key(
        program: &Program,
        pattern: &crate::ast::MatchPattern,
        name: &str,
    ) -> Result<Option<CleanupTypeKey>, Diagnostic> {
        Ok(match pattern {
            crate::ast::MatchPattern::Variant {
                type_name,
                case_name,
                fields,
                ..
            } => {
                let Some(declaration) = program
                    .types
                    .iter()
                    .find(|declaration| declaration.name == *type_name)
                else {
                    return Ok(None);
                };
                let crate::ast::TypeDeclarationKind::Variant { cases } = &declaration.kind else {
                    return Ok(None);
                };
                let Some(case) = cases.iter().find(|case| case.name == *case_name) else {
                    return Ok(None);
                };
                fields.iter().find_map(|binding| {
                    (binding.binding == name).then(|| {
                        case.fields
                            .iter()
                            .find(|field| field.name == binding.name)
                            .map(|field| key_for_type(program, &field.ty))
                    })?
                })
            }
            crate::ast::MatchPattern::Record {
                type_name, fields, ..
            } => {
                let Some(declaration) = program
                    .types
                    .iter()
                    .find(|declaration| declaration.name == *type_name)
                else {
                    return Ok(None);
                };
                let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
                let mut len = 1usize;
                stack[0] = Some((declaration, fields.as_slice(), 0usize, 1usize));
                let mut found = None;
                while len != 0 {
                    len -= 1;
                    let (declaration, fields, index, depth) = stack[len]
                        .take()
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    let crate::ast::TypeDeclarationKind::Record {
                        fields: declarations,
                    } = &declaration.kind
                    else {
                        continue;
                    };
                    let Some(field) = fields.get(index) else {
                        continue;
                    };
                    let Some(declaration_field) = declarations
                        .iter()
                        .find(|candidate| candidate.name == field.name)
                    else {
                        continue;
                    };
                    match &field.pattern {
                        crate::ast::RecordMatchFieldPattern::Binding { name: binding, .. }
                            if binding == name =>
                        {
                            found = Some(key_for_type(program, &declaration_field.ty));
                            break;
                        }
                        crate::ast::RecordMatchFieldPattern::Record {
                            type_name,
                            fields: child_fields,
                            ..
                        } => {
                            let Some(child) = program
                                .types
                                .iter()
                                .find(|candidate| candidate.name == *type_name)
                            else {
                                continue;
                            };
                            let child_depth = depth.checked_add(1).ok_or_else(|| {
                                b109(
                                    "max_semantic_expression_depth",
                                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                                )
                            })?;
                            if child_depth > MAX_SEMANTIC_EXPRESSION_DEPTH || len + 2 > stack.len()
                            {
                                return Err(b109(
                                    "max_semantic_expression_depth",
                                    MAX_SEMANTIC_EXPRESSION_DEPTH,
                                ));
                            }
                            stack[len] = Some((declaration, fields, index + 1, depth));
                            stack[len + 1] = Some((child, child_fields.as_slice(), 0, child_depth));
                            len += 2;
                        }
                        _ => {
                            stack[len] = Some((declaration, fields, index + 1, depth));
                            len += 1;
                        }
                    }
                }
                found
            }
            // Refutable Match v1: scalar patterns reference no named type.
            crate::ast::MatchPattern::Wildcard { .. }
            | crate::ast::MatchPattern::Literal { .. }
            | crate::ast::MatchPattern::Binding { .. } => None,
            crate::ast::MatchPattern::Or { alternatives, .. } => {
                let mut found = None;
                for alternative in alternatives {
                    found = pattern_binding_key(program, alternative, name)?;
                    if found.is_some() {
                        break;
                    }
                }
                found
            }
        })
    }

    fn facts_for_key(
        key: CleanupTypeKey,
        declaration_facts: &[CleanupTypeFacts],
        fallback: CleanupTypeFacts,
    ) -> CleanupTypeFacts {
        match key {
            CleanupTypeKey::Scalar => CleanupTypeFacts::default(),
            CleanupTypeKey::Declaration(index) => declaration_facts[index],
            CleanupTypeKey::Unknown => fallback,
        }
    }

    fn add_root(
        target: &mut CleanupRetainedStats,
        key: CleanupTypeKey,
        declaration_facts: &[CleanupTypeFacts],
        fallback: CleanupTypeFacts,
        storage_identity_bytes: usize,
        resolved_type_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if matches!(key, CleanupTypeKey::Unknown) {
            target.fallback_roots = target
                .fallback_roots
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let facts = facts_for_key(key, declaration_facts, fallback);
        if facts.leaves != 0 {
            target.ordinary_slot_payload_bytes = target
                .ordinary_slot_payload_bytes
                .checked_add(
                    storage_identity_bytes
                        .checked_add(resolved_type_bytes)
                        .and_then(|bytes| bytes.checked_mul(2))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            target.ordinary_place_storage_bytes = target
                .ordinary_place_storage_bytes
                .checked_add(
                    storage_identity_bytes
                        // Initialize, Transfer source/destination, and the
                        // region's raw StorageId each own the full identity.
                        .checked_mul(4)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        target
            .add_root(facts)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
    }

    fn add_finalizer_upper(
        target: &mut CleanupRetainedStats,
        key: CleanupTypeKey,
        declaration_facts: &[CleanupTypeFacts],
        fallback: CleanupTypeFacts,
        exits_after_initialization: usize,
        storage_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        let facts = facts_for_key(key, declaration_facts, fallback);
        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(
                facts
                    .leaves
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(
                facts
                    .projection_segments
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(
                facts
                    .lifecycle_ids
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(
                facts
                    .projection_ids
                    .checked_mul(exits_after_initialization)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(
                storage_identity_bytes
                    .checked_mul(facts.leaves)
                    .and_then(|bytes| bytes.checked_mul(exits_after_initialization))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn add_parent_local_record_prefix(
        target: &mut CleanupRetainedStats,
        facts: CleanupTypeFacts,
        later_failure_events: usize,
        storage_identity_bytes: usize,
        field_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if facts.leaves == 0 || later_failure_events == 0 {
            return Ok(());
        }
        let finalizer_copies = facts
            .leaves
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_segments = facts
            .projection_segments
            .checked_add(facts.leaves)
            .and_then(|segments| segments.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let lifecycle_ids = facts
            .lifecycle_ids
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_ids = facts
            .projection_ids
            .checked_add(
                facts
                    .leaves
                    .checked_mul(field_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let storage_bytes = storage_identity_bytes
            .checked_mul(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.parent_local_partial_fields = target
            .parent_local_partial_fields
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_copies = target
            .parent_local_finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_projection_segments = target
            .parent_local_finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_lifecycle_ids = target
            .parent_local_finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_projection_ids = target
            .parent_local_finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_finalizer_storage_bytes = target
            .parent_local_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn add_parent_local_update_prefix(
        target: &mut CleanupRetainedStats,
        facts: CleanupTypeFacts,
        later_failure_events: usize,
        storage_identity_bytes: usize,
        field_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if facts.leaves == 0 || later_failure_events == 0 {
            return Ok(());
        }
        let finalizer_copies = facts
            .leaves
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_segments = facts
            .projection_segments
            .checked_add(facts.leaves)
            .and_then(|segments| segments.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let lifecycle_ids = facts
            .lifecycle_ids
            .checked_mul(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_ids = facts
            .projection_ids
            .checked_add(
                facts
                    .leaves
                    .checked_mul(field_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_mul(later_failure_events))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let storage_bytes = storage_identity_bytes
            .checked_mul(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.parent_local_update_prefix_fields = target
            .parent_local_update_prefix_fields
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_exit_groups = target
            .parent_local_update_prefix_exit_groups
            .checked_add(later_failure_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_copies = target
            .parent_local_update_prefix_finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_projection_segments = target
            .parent_local_update_prefix_finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_lifecycle_ids = target
            .parent_local_update_prefix_finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_projection_ids = target
            .parent_local_update_prefix_finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_update_prefix_finalizer_storage_bytes = target
            .parent_local_update_prefix_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn add_parent_local_projection_residual(
        target: &mut CleanupRetainedStats,
        residual: CleanupTypeFacts,
        remaining_events: usize,
        storage_identity_bytes: usize,
    ) -> Result<(), Diagnostic> {
        if residual.leaves == 0 || remaining_events == 0 {
            return Ok(());
        }
        let finalizer_copies = residual
            .leaves
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_segments = residual
            .projection_segments
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let lifecycle_ids = residual
            .lifecycle_ids
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_ids = residual
            .projection_ids
            .checked_mul(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let storage_bytes = storage_identity_bytes
            .checked_mul(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.finalizer_copies = target
            .finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_segments = target
            .finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_lifecycle_ids = target
            .finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.finalizer_projection_ids = target
            .finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.ordinary_finalizer_storage_bytes = target
            .ordinary_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;

        target.parent_local_projection_epochs = target
            .parent_local_projection_epochs
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_exit_groups = target
            .parent_local_projection_exit_groups
            .checked_add(remaining_events)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_copies = target
            .parent_local_projection_finalizer_copies
            .checked_add(finalizer_copies)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_projection_segments = target
            .parent_local_projection_finalizer_projection_segments
            .checked_add(projection_segments)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_lifecycle_ids = target
            .parent_local_projection_finalizer_lifecycle_ids
            .checked_add(lifecycle_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_projection_ids = target
            .parent_local_projection_finalizer_projection_ids
            .checked_add(projection_ids)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        target.parent_local_projection_finalizer_storage_bytes = target
            .parent_local_projection_finalizer_storage_bytes
            .checked_add(storage_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        Ok(())
    }

    fn variable_key(
        program: &Program,
        function: &crate::ast::Function,
        name: &str,
        traversal: &[Option<(&crate::ast::Expr, usize, usize)>],
        stack_len: usize,
        results: &[CleanupTypeKey],
    ) -> Result<CleanupTypeKey, Diagnostic> {
        for (ancestor, next_child, result_start) in
            traversal[..stack_len].iter().rev().flatten().copied()
        {
            match &ancestor.kind {
                crate::ast::ExprKind::Block { statements, .. } => {
                    let active_child = ast_previous_child_path_index(next_child)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    let completed_statements =
                        ast_block_statement_index(statements, active_child).min(statements.len());
                    for index in (0..completed_statements).rev() {
                        let crate::ast::Statement::Let { name: binding, .. } = &statements[index]
                        else {
                            continue;
                        };
                        if binding == name {
                            let result_index = ast_block_statement_result_index(statements, index)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            return Ok(results
                                .get(result_start + result_index)
                                .copied()
                                .unwrap_or(CleanupTypeKey::Unknown));
                        }
                    }
                }
                crate::ast::ExprKind::Match { arms, .. } => {
                    let active_child = ast_previous_child_path_index(next_child)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    if let Some(arm_index) = ast_match_arm_index(arms, active_child) {
                        if let Some(key) = arms
                            .get(arm_index)
                            .map(|arm| pattern_binding_key(program, &arm.pattern, name))
                            .transpose()?
                            .flatten()
                        {
                            return Ok(key);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(function
            .params
            .iter()
            .rev()
            .find(|parameter| parameter.name == name)
            .map(|parameter| key_for_type(program, &parameter.ty))
            .unwrap_or(CleanupTypeKey::Unknown))
    }

    let fallback =
        declaration_facts
            .iter()
            .copied()
            .fold(CleanupTypeFacts::default(), |maximum, facts| {
                CleanupTypeFacts {
                    leaves: maximum.leaves.max(facts.leaves),
                    occurrences: maximum.occurrences.max(facts.occurrences),
                    shape_fields: maximum.shape_fields.max(facts.shape_fields),
                    projection_segments: maximum.projection_segments.max(facts.projection_segments),
                    shape_ids: maximum.shape_ids.max(facts.shape_ids),
                    lifecycle_ids: maximum.lifecycle_ids.max(facts.lifecycle_ids),
                    projection_ids: maximum.projection_ids.max(facts.projection_ids),
                }
            });
    // Staged Result/Option records retain compiler-owned identities even in a
    // program with no user resource declarations. Keep this list adjacent to
    // the source prelude contract; tests below bind its exact spellings.
    let prelude_identity_bytes = crate::private_capacity_contract::PRELUDE_CAPACITY_IDENTITIES
        .into_iter()
        .map(str::len)
        .max()
        .expect("private prelude identities are nonempty");
    let authored_identity_bytes = program.types.iter().fold(0usize, |maximum, declaration| {
        let maximum = maximum.max(declaration.stable_id.len());
        match &declaration.kind {
            crate::ast::TypeDeclarationKind::Resource { lifecycles } => {
                lifecycles.iter().fold(maximum, |maximum, lifecycle| {
                    maximum.max(lifecycle.stable_id.as_deref().map(str::len).unwrap_or(0))
                })
            }
            crate::ast::TypeDeclarationKind::Record { fields }
            | crate::ast::TypeDeclarationKind::Class { fields, .. } => fields
                .iter()
                .fold(maximum, |maximum, field| maximum.max(field.stable_id.len())),
            crate::ast::TypeDeclarationKind::Variant { cases } => {
                cases.iter().fold(maximum, |maximum, case| {
                    case.fields
                        .iter()
                        .fold(maximum.max(case.stable_id.len()), |maximum, field| {
                            maximum.max(field.stable_id.len())
                        })
                })
            }
        }
    });
    let maximum_declaration_identity_bytes = authored_identity_bytes.max(prelude_identity_bytes);
    let maximum_type_arguments = program
        .types
        .iter()
        .map(|declaration| declaration.type_parameters.len())
        .max()
        .unwrap_or(0)
        .max(2);
    let maximum_resolved_type_owned_bytes = maximum_declaration_identity_bytes
        .checked_add(
            maximum_type_arguments
                .checked_mul(std::mem::size_of::<ResolvedType>())
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut total = CleanupRetainedStats::default();
    let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let mut event_traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];

    for function in source_functions(program) {
        let generic_instance_identity_len =
            generic_function_instance_identity_upper(program, function)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let function_roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let function_node_total =
            scan_ast_capacity(function_roots, program, false, &mut traversal)?.nodes;
        let path_segment_bytes = 32usize
            .checked_add(decimal_digits(function_node_total))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let value_storage_identity_bytes_for_path = |path_len: usize| {
            scoped_value_identity_upper(function, generic_instance_identity_len, path_len)
        };
        let expression_storage_identity_bytes_for_path = |path_len: usize| {
            scoped_expression_identity_upper(function, generic_instance_identity_len, path_len)
        };
        let type_bytes_for_key = |key: CleanupTypeKey| match key {
            CleanupTypeKey::Scalar => Some(0),
            CleanupTypeKey::Declaration(index) => program.types[index].stable_id.len().checked_add(
                program.types[index]
                    .type_parameters
                    .len()
                    .checked_mul(std::mem::size_of::<ResolvedType>())?,
            ),
            CleanupTypeKey::Unknown => Some(maximum_resolved_type_owned_bytes),
        };
        let function_exit_upper = cleanup_function_exit_events(function, &mut traversal)?;
        // These are exactly the source forms that can ask the lowerer for
        // an exit: operation failure, postfix residual, authored/update
        // scope, contract false/scope, and final success.
        let mut function_stats = CleanupRetainedStats {
            exit_events: function_exit_upper,
            ..CleanupRetainedStats::default()
        };
        let mut function_nodes = 0usize;
        let mut owned_parameters = 0usize;
        let mut has_try = false;
        for (parameter_index, parameter) in function.params.iter().enumerate() {
            if parameter.mode == crate::ast::ParamMode::Own {
                let key = key_for_type(program, &parameter.ty);
                let storage_identity_bytes =
                    value_storage_identity_bytes_for_path(decimal_digits(parameter_index))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                add_root(
                    &mut function_stats,
                    key,
                    declaration_facts,
                    fallback,
                    storage_identity_bytes,
                    type_bytes_for_key(key)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )?;
                add_finalizer_upper(
                    &mut function_stats,
                    key,
                    declaration_facts,
                    fallback,
                    cleanup_parameter_finalizer_events(
                        function,
                        &parameter.name,
                        program,
                        &mut event_traversal,
                    )?,
                    storage_identity_bytes,
                )?;
                function_stats.ordinary_place_storage_bytes = function_stats
                    .ordinary_place_storage_bytes
                    .checked_add(storage_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                owned_parameters = owned_parameters
                    .checked_add(1)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            }
        }

        let roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let mut traversal_path_lengths = [0usize; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
        for (root_index, root) in roots.enumerate() {
            let mut stack_len = 1usize;
            traversal[0] = Some((root, 0usize, 0usize));
            traversal_path_lengths[0] = ast_root_identity_path_len(function, root_index);
            let mut results = Vec::<CleanupTypeKey>::with_capacity(node_capacity);
            if results.capacity() != node_capacity {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
            while stack_len != 0 {
                stack_len -= 1;
                let expression_path_len = traversal_path_lengths[stack_len];
                let (expression, next_child, result_start) = traversal[stack_len]
                    .take()
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                if next_child != 0 {
                    if let crate::ast::ExprKind::Block { statements, .. } = &expression.kind {
                        let previous = ast_previous_child_path_index(next_child)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        let statement_index = ast_block_statement_index(statements, previous);
                        if let Some(crate::ast::Statement::Let { name, .. }) =
                            statements.get(statement_index)
                        {
                            let key = results
                                .last()
                                .copied()
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            let storage_identity_bytes = value_storage_identity_bytes_for_path(
                                expression_path_len
                                    .checked_add(".s".len())
                                    .and_then(|bytes| {
                                        bytes.checked_add(decimal_digits(statement_index))
                                    })
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                            )
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            add_root(
                                &mut function_stats,
                                key,
                                declaration_facts,
                                fallback,
                                storage_identity_bytes,
                                type_bytes_for_key(key)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                            )?;
                            if facts_for_key(key, declaration_facts, fallback).leaves != 0 {
                                function_stats.parent_local_epochs = function_stats
                                    .parent_local_epochs
                                    .checked_add(1)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            }
                            let remaining = cleanup_block_binding_finalizer_events(
                                function,
                                expression,
                                next_child,
                                name,
                                program,
                                &mut event_traversal,
                            )?;
                            add_finalizer_upper(
                                &mut function_stats,
                                key,
                                declaration_facts,
                                fallback,
                                remaining,
                                storage_identity_bytes,
                            )?;
                        }
                    }
                }
                if next_child == 0 {
                    function_nodes = function_nodes
                        .checked_add(1)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    match &expression.kind {
                        crate::ast::ExprKind::Call { args, .. } => {
                            if let crate::ast::ExprKind::Call { name, .. } = &expression.kind {
                                if let Some(candidate) = program
                                    .functions
                                    .iter()
                                    .find(|candidate| candidate.name == *name)
                                {
                                    for (argument_index, parameter) in
                                        candidate.params.iter().take(args.len()).enumerate()
                                    {
                                        let key = key_for_type(program, &parameter.ty);
                                        if parameter.mode != crate::ast::ParamMode::Own
                                            || facts_for_key(key, declaration_facts, fallback)
                                                .leaves
                                                == 0
                                        {
                                            continue;
                                        }
                                        // The caller retains a distinct
                                        // CallArgument epoch in addition to
                                        // the argument expression temporary.
                                        add_root(
                                            &mut function_stats,
                                            key,
                                            declaration_facts,
                                            fallback,
                                            0,
                                            0,
                                        )?;
                                        let later_argument_events = args[argument_index + 1..]
                                            .iter()
                                            .try_fold(0usize, |events, argument| {
                                                events
                                                    .checked_add(cleanup_expression_failure_events(
                                                        argument,
                                                        &mut event_traversal,
                                                    )?)
                                                    .ok_or_else(|| {
                                                        b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                                    })
                                            })?;
                                        let argument_identity_bytes = function
                                            .stable_id
                                            .len()
                                            .checked_add(
                                                stack_len
                                                    .checked_add(2)
                                                    .and_then(|depth| {
                                                        depth.checked_mul(path_segment_bytes)
                                                    })
                                                    .ok_or_else(|| {
                                                        b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                                    })?,
                                            )
                                            .and_then(|bytes| bytes.checked_mul(2))
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        let argument_facts =
                                            facts_for_key(key, declaration_facts, fallback);
                                        // Four paired CallArgument StorageId
                                        // copies coexist (slot, region,
                                        // Transfer destination, CallCommit
                                        // source). Transfer::at and
                                        // CallCommit::call add two single
                                        // expression IDs, equal to one more
                                        // paired upper.
                                        let fixed_storage_copies = argument_identity_bytes
                                            .checked_mul(5)
                                            .and_then(|bytes| {
                                                bytes.checked_add(maximum_resolved_type_owned_bytes)
                                            })
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        let failure_storage_copies = argument_identity_bytes
                                            .checked_mul(argument_facts.leaves)
                                            .and_then(|bytes| {
                                                bytes.checked_mul(later_argument_events)
                                            })
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        function_stats.call_argument_owned_bytes = function_stats
                                            .call_argument_owned_bytes
                                            .checked_add(fixed_storage_copies)
                                            .and_then(|bytes| {
                                                bytes.checked_add(failure_storage_copies)
                                            })
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        add_finalizer_upper(
                                            &mut function_stats,
                                            key,
                                            declaration_facts,
                                            fallback,
                                            later_argument_events,
                                            0,
                                        )?;
                                        function_stats.call_arguments = function_stats
                                            .call_arguments
                                            .checked_add(1)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                        function_stats.parent_local_epochs = function_stats
                                            .parent_local_epochs
                                            .checked_add(1)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?;
                                    }
                                }
                            }
                        }
                        crate::ast::ExprKind::Match { arms, .. } => {
                            function_stats.variant_edges =
                                function_stats
                                    .variant_edges
                                    .checked_add(arms.len().checked_mul(2).ok_or_else(|| {
                                        b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                    })?)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                        crate::ast::ExprKind::Try { .. } => {
                            has_try = true;
                            function_stats.staged_results = function_stats
                                .staged_results
                                .checked_add(1)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            function_stats.variant_edges = function_stats
                                .variant_edges
                                .checked_add(2)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                        _ => {}
                    }
                }
                let mut child_cursor = next_child;
                if let Some((child_index, child)) = ast_child(expression, &mut child_cursor) {
                    if stack_len + 2 > traversal.len() {
                        return Err(b109(
                            "max_semantic_expression_depth",
                            MAX_SEMANTIC_EXPRESSION_DEPTH,
                        ));
                    }
                    traversal[stack_len] = Some((expression, child_cursor, result_start));
                    traversal_path_lengths[stack_len] = expression_path_len;
                    traversal[stack_len + 1] = Some((child, 0, results.len()));
                    traversal_path_lengths[stack_len + 1] = expression_path_len
                        .checked_add(ast_child_identity_path_increment(
                            expression,
                            child_index,
                            program,
                        ))
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    stack_len += 2;
                    continue;
                }

                let children = &results[result_start..];
                let key = match &expression.kind {
                    crate::ast::ExprKind::Int(_)
                    | crate::ast::ExprKind::Int32(_)
                    | crate::ast::ExprKind::Char(_)
                    | crate::ast::ExprKind::Uint8(_)
                    | crate::ast::ExprKind::Usize(_)
                    | crate::ast::ExprKind::ArrayU8(_)
                    | crate::ast::ExprKind::RepeatArrayU8 { .. }
                    | crate::ast::ExprKind::Float32(_)
                    | crate::ast::ExprKind::Float64(_)
                    | crate::ast::ExprKind::Bool(_)
                    | crate::ast::ExprKind::String(_) => CleanupTypeKey::Scalar,
                    crate::ast::ExprKind::Var(name) => {
                        variable_key(program, function, name, &traversal, stack_len, &results)?
                    }
                    crate::ast::ExprKind::Call { name, .. } => program
                        .functions
                        .iter()
                        .find(|candidate| candidate.name == *name)
                        .map(|candidate| key_for_type(program, &candidate.return_type))
                        .unwrap_or(CleanupTypeKey::Scalar),
                    crate::ast::ExprKind::MethodCall { method, .. } => program
                        .types
                        .iter()
                        .find_map(|declaration| match &declaration.kind {
                            crate::ast::TypeDeclarationKind::Class { methods, .. } => {
                                methods.iter().find(|candidate| candidate.name == *method)
                            }
                            _ => None,
                        })
                        .map(|candidate| key_for_type(program, &candidate.return_type))
                        .unwrap_or(CleanupTypeKey::Scalar),
                    crate::ast::ExprKind::SuperMethod { method, .. } => program
                        .types
                        .iter()
                        .find_map(|declaration| match &declaration.kind {
                            crate::ast::TypeDeclarationKind::Class { methods, .. } => {
                                methods.iter().find(|candidate| candidate.name == *method)
                            }
                            _ => None,
                        })
                        .map(|candidate| key_for_type(program, &candidate.return_type))
                        .unwrap_or(CleanupTypeKey::Scalar),
                    crate::ast::ExprKind::Unary { .. } | crate::ast::ExprKind::Binary { .. } => {
                        CleanupTypeKey::Scalar
                    }
                    crate::ast::ExprKind::Block { .. } => {
                        children.last().copied().unwrap_or(CleanupTypeKey::Scalar)
                    }
                    crate::ast::ExprKind::If { .. } => {
                        children.get(1).copied().unwrap_or(CleanupTypeKey::Unknown)
                    }
                    crate::ast::ExprKind::ConstructRecord {
                        type_name, fields, ..
                    } => {
                        let declaration_index = program
                            .types
                            .iter()
                            .position(|declaration| declaration.name == *type_name);
                        if let Some(declaration_index) = declaration_index {
                            let declaration = &program.types[declaration_index];
                            let declared_fields = match &declaration.kind {
                                crate::ast::TypeDeclarationKind::Record { fields }
                                | crate::ast::TypeDeclarationKind::Class { fields, .. } => fields,
                                _ => {
                                    return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
                                }
                            };
                            let storage_identity_bytes =
                                expression_storage_identity_bytes_for_path(expression_path_len)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            for (field_index, initializer) in fields.iter().enumerate() {
                                let field_key = children
                                    .get(field_index)
                                    .copied()
                                    .unwrap_or(CleanupTypeKey::Unknown);
                                let facts = facts_for_key(field_key, declaration_facts, fallback);
                                if facts.leaves == 0 {
                                    continue;
                                }
                                let later_failure_events = fields[field_index + 1..]
                                    .iter()
                                    .try_fold(0usize, |events, later| {
                                        events
                                            .checked_add(cleanup_expression_failure_events(
                                                &later.value,
                                                &mut event_traversal,
                                            )?)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })
                                    })?;
                                let field_identity_bytes = declared_fields
                                    .iter()
                                    .find(|field| field.name == initializer.name)
                                    .map(|field| field.stable_id.len())
                                    .unwrap_or(maximum_declaration_identity_bytes);
                                add_parent_local_record_prefix(
                                    &mut function_stats,
                                    facts,
                                    later_failure_events,
                                    storage_identity_bytes,
                                    field_identity_bytes,
                                )?;
                            }
                            CleanupTypeKey::Declaration(declaration_index)
                        } else {
                            CleanupTypeKey::Unknown
                        }
                    }
                    crate::ast::ExprKind::ConstructVariant { type_name, .. } => program
                        .types
                        .iter()
                        .position(|declaration| declaration.name == *type_name)
                        .map(CleanupTypeKey::Declaration)
                        .unwrap_or_else(|| {
                            if matches!(type_name.as_str(), "Option" | "Result") {
                                CleanupTypeKey::Scalar
                            } else {
                                CleanupTypeKey::Unknown
                            }
                        }),
                    crate::ast::ExprKind::Match { arms, .. } => {
                        if let Some(arm) = arms.first() {
                            if let crate::ast::ExprKind::Var(name) = &arm.value.kind {
                                pattern_binding_key(program, &arm.pattern, name)?
                                    .unwrap_or(CleanupTypeKey::Unknown)
                            } else {
                                ast_match_arm_value_result_index(arms, 0)
                                    .and_then(|index| children.get(index).copied())
                                    .unwrap_or(CleanupTypeKey::Unknown)
                            }
                        } else {
                            CleanupTypeKey::Unknown
                        }
                    }
                    crate::ast::ExprKind::Try { .. } => CleanupTypeKey::Scalar,
                    crate::ast::ExprKind::UpdateRecord { fields, .. } => {
                        let base = children.first().copied().unwrap_or(CleanupTypeKey::Unknown);
                        let destination_storage_identity_bytes =
                            expression_storage_identity_bytes_for_path(expression_path_len)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        for (field_index, initializer) in fields.iter().enumerate() {
                            let replacement_key = children
                                .get(field_index + 1)
                                .copied()
                                .unwrap_or(CleanupTypeKey::Unknown);
                            let replacement_facts =
                                facts_for_key(replacement_key, declaration_facts, fallback);
                            if replacement_facts.leaves == 0 {
                                continue;
                            }
                            let later_failure_events = fields[field_index + 1..].iter().try_fold(
                                0usize,
                                |events, later| {
                                    events
                                        .checked_add(cleanup_expression_failure_events(
                                            &later.value,
                                            &mut event_traversal,
                                        )?)
                                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                                },
                            )?;
                            let field_identity_bytes = match base {
                                CleanupTypeKey::Declaration(index) => {
                                    match &program.types[index].kind {
                                        crate::ast::TypeDeclarationKind::Record { fields }
                                        | crate::ast::TypeDeclarationKind::Class {
                                            fields, ..
                                        } => fields
                                            .iter()
                                            .find(|field| field.name == initializer.name)
                                            .map(|field| field.stable_id.len())
                                            .unwrap_or(maximum_declaration_identity_bytes),
                                        _ => maximum_declaration_identity_bytes,
                                    }
                                }
                                _ => maximum_declaration_identity_bytes,
                            };
                            add_parent_local_update_prefix(
                                &mut function_stats,
                                replacement_facts,
                                later_failure_events,
                                destination_storage_identity_bytes,
                                field_identity_bytes,
                            )?;
                        }
                        let storage_identity_bytes = expression_storage_identity_bytes_for_path(
                            expression_path_len
                                .checked_add(".base".len())
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        add_root(
                            &mut function_stats,
                            base,
                            declaration_facts,
                            fallback,
                            storage_identity_bytes,
                            type_bytes_for_key(base)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                        )?;
                        let staged_base_exits = fields.iter().try_fold(
                            1usize,
                            |events, field| -> Result<usize, Diagnostic> {
                                events
                                    .checked_add(cleanup_expression_failure_events(
                                        &field.value,
                                        &mut event_traversal,
                                    )?)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                            },
                        )?;
                        add_finalizer_upper(
                            &mut function_stats,
                            base,
                            declaration_facts,
                            fallback,
                            staged_base_exits,
                            storage_identity_bytes,
                        )?;
                        if facts_for_key(base, declaration_facts, fallback).leaves != 0 {
                            function_stats.parent_local_epochs = function_stats
                                .parent_local_epochs
                                .checked_add(1)
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        }
                        base
                    }
                    crate::ast::ExprKind::Project { base, field, .. } => {
                        let base_key = children.first().copied().unwrap_or(CleanupTypeKey::Unknown);
                        let selected = match base_key {
                            CleanupTypeKey::Declaration(index) => {
                                let declaration = &program.types[index];
                                match &declaration.kind {
                                    crate::ast::TypeDeclarationKind::Record { fields }
                                    | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                                        fields
                                            .iter()
                                            .find(|candidate| candidate.name == *field)
                                            .map(|candidate| key_for_type(program, &candidate.ty))
                                    }
                                    _ => None,
                                }
                            }
                            _ => None,
                        }
                        .unwrap_or(CleanupTypeKey::Unknown);
                        if !matches!(base.kind, crate::ast::ExprKind::Var(_)) {
                            let base_facts = facts_for_key(base_key, declaration_facts, fallback);
                            let residual = if let CleanupTypeKey::Declaration(index) = base_key {
                                let selected_facts =
                                    facts_for_key(selected, declaration_facts, fallback);
                                let field_identity_bytes = match &program.types[index].kind {
                                    crate::ast::TypeDeclarationKind::Record { fields }
                                    | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                                        fields
                                            .iter()
                                            .find(|candidate| candidate.name == *field)
                                            .map(|candidate| candidate.stable_id.len())
                                            .unwrap_or(maximum_declaration_identity_bytes)
                                    }
                                    _ => maximum_declaration_identity_bytes,
                                };
                                let selected_projection_segments = selected_facts
                                    .projection_segments
                                    .checked_add(selected_facts.leaves)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                                let selected_projection_ids = selected_facts
                                    .projection_ids
                                    .checked_add(
                                        selected_facts
                                            .leaves
                                            .checked_mul(field_identity_bytes)
                                            .ok_or_else(|| {
                                                b109("max_builder_bytes", MAX_BUILDER_BYTES)
                                            })?,
                                    )
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                                if selected_facts.leaves <= base_facts.leaves
                                    && selected_projection_segments
                                        <= base_facts.projection_segments
                                    && selected_facts.lifecycle_ids <= base_facts.lifecycle_ids
                                    && selected_projection_ids <= base_facts.projection_ids
                                {
                                    CleanupTypeFacts {
                                        leaves: base_facts.leaves - selected_facts.leaves,
                                        projection_segments: base_facts.projection_segments
                                            - selected_projection_segments,
                                        lifecycle_ids: base_facts.lifecycle_ids
                                            - selected_facts.lifecycle_ids,
                                        projection_ids: base_facts.projection_ids
                                            - selected_projection_ids,
                                        ..CleanupTypeFacts::default()
                                    }
                                } else {
                                    // Generic field substitution is not yet
                                    // materialized in this source census.
                                    // Keeping the complete base is the exact
                                    // admitted fallback, never a subtraction
                                    // from unrelated declaration facts.
                                    base_facts
                                }
                            } else {
                                // A valid unresolved generic projection may
                                // still instantiate to the maximum admitted
                                // resource aggregate. Retain the whole fallback
                                // rather than assuming which field transferred.
                                base_facts
                            };
                            let base_path_len = expression_path_len
                                .checked_add(ast_child_identity_path_increment(
                                    expression, 0, program,
                                ))
                                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            let storage_identity_bytes =
                                expression_storage_identity_bytes_for_path(base_path_len)
                                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                            let remaining_events = cleanup_parent_local_remaining_finalizer_events(
                                function,
                                root,
                                &traversal,
                                stack_len,
                                &mut event_traversal,
                            )?;
                            add_parent_local_projection_residual(
                                &mut function_stats,
                                residual,
                                remaining_events,
                                storage_identity_bytes,
                            )?;
                        }
                        selected
                    }
                };
                if !matches!(expression.kind, crate::ast::ExprKind::Var(_)) {
                    let storage_identity_bytes =
                        expression_storage_identity_bytes_for_path(expression_path_len)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    add_root(
                        &mut function_stats,
                        key,
                        declaration_facts,
                        fallback,
                        storage_identity_bytes,
                        type_bytes_for_key(key)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                    )?;
                    if facts_for_key(key, declaration_facts, fallback).leaves != 0 {
                        function_stats.parent_local_epochs = function_stats
                            .parent_local_epochs
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                        function_stats.parent_local_zero_lifetime_transfers = function_stats
                            .parent_local_zero_lifetime_transfers
                            .checked_add(1)
                            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                    }
                }
                results.truncate(result_start);
                results.push(key);
            }
            if results.len() != 1 {
                return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
            }
        }
        if function_nodes != function_node_total {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }

        let result_key = key_for_type(program, &function.return_type);
        add_root(
            &mut function_stats,
            result_key,
            declaration_facts,
            fallback,
            0,
            type_bytes_for_key(result_key)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )?;
        if facts_for_key(result_key, declaration_facts, fallback).leaves != 0 {
            function_stats.parent_local_epochs = function_stats
                .parent_local_epochs
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        if facts_for_key(result_key, declaration_facts, fallback).leaves != 0 {
            function_stats.ordinary_slot_payload_bytes = function_stats
                .ordinary_slot_payload_bytes
                .checked_add(
                    value_storage_identity_bytes_for_path(0)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
                )
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let result_finalizer_events =
            function
                .ensures
                .iter()
                .try_fold(function.ensures.len(), |events, ensure| {
                    events
                        .checked_add(cleanup_expression_failure_events(
                            ensure,
                            &mut event_traversal,
                        )?)
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))
                })?;
        add_finalizer_upper(
            &mut function_stats,
            result_key,
            declaration_facts,
            fallback,
            result_finalizer_events,
            0,
        )?;
        if has_try {
            // The plan retains one Body staging source in addition to every
            // residual source materialized by a postfix `?`.
            function_stats.staged_results = function_stats
                .staged_results
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        let expression_identity_bytes = function
            .stable_id
            .len()
            .checked_add(
                function_nodes
                    .checked_mul(path_segment_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .and_then(|bytes| bytes.checked_add(fallback.shape_ids))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let staged_owned_bytes = expression_identity_bytes
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(maximum_resolved_type_owned_bytes.checked_mul(2)?))
            .and_then(|bytes| bytes.checked_add(maximum_declaration_identity_bytes.checked_mul(5)?))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        function_stats.stage_identity_and_type_bytes = function_stats
            .staged_results
            .checked_mul(staged_owned_bytes)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        function_stats.variant_identity_bytes = function_stats
            .variant_edges
            .checked_mul(
                expression_identity_bytes
                    .checked_add(maximum_declaration_identity_bytes)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if function_stats.leaves != 0 {
            // Each cleanup storage epoch is initialized once and transferred
            // at most once; its inventory/plan slot is accounted separately.
            // CallCommit argument sources are additional projected places.
            let root_transition_copies = function_stats
                .roots
                .checked_mul(2)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            let projected_place_copies = root_transition_copies
                .checked_add(function_stats.call_arguments)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            function_stats.place_copies = function_stats
                .roots
                .checked_add(projected_place_copies)
                .and_then(|value| value.checked_add(owned_parameters))
                .and_then(|value| value.checked_add(1))
                .and_then(|value| value.checked_add(function_stats.finalizer_copies))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            function_stats.place_projection_segments = function_stats
                .projection_segments
                .checked_mul(2)
                .and_then(|segments| {
                    segments.checked_add(
                        fallback
                            .projection_segments
                            .checked_mul(function_stats.call_arguments)?,
                    )
                })
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
            function_stats.place_projection_ids = function_stats
                .projection_ids
                .checked_mul(2)
                .and_then(|bytes| {
                    bytes.checked_add(
                        fallback
                            .projection_ids
                            .checked_mul(function_stats.call_arguments)?,
                    )
                })
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }

        let multiplicity = if function.type_parameters.is_empty() {
            1
        } else {
            generic_instance_upper
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        };
        total
            .merge(
                function_stats
                    .scaled(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    Ok(total)
}

#[derive(Clone, Copy)]
pub(super) struct HirPreResolveCapacity {
    pub(super) retained_upper: usize,
    pub(super) scratch_upper: usize,
    pub(super) declaration_index_upper: usize,
    pub(super) cleanup_retained_upper: usize,
    pub(super) cleanup_authority_upper: usize,
    pub(super) cleanup_exit_events_upper: usize,
    pub(super) cleanup_fallback_roots: usize,
    pub(super) cleanup_call_argument_owned_upper: usize,
    pub(super) cleanup_plan_structural_upper: usize,
    #[cfg(test)]
    pub(super) cleanup_parent_local_lifetime_upper: usize,
    #[cfg(test)]
    pub(super) cleanup_parent_local_projection_lifetime_upper: usize,
    #[cfg(test)]
    pub(super) cleanup_parent_local_update_prefix_lifetime_upper: usize,
    #[cfg(test)]
    pub(super) cleanup_proof: CleanupCapacityProofTerms,
    pub(super) phase_peaks: [usize; 8],
    pub(super) disposal_frames: usize,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
pub(super) struct CleanupCapacityProofTerms {
    pub(super) stats: CleanupRetainedStats,
    pub(super) inventory_slot_capacity_entries: usize,
    pub(super) inventory_flag_capacity_entries: usize,
    pub(super) inventory_entry_capacity_entries: usize,
    pub(super) plan_slot_capacity_entries: usize,
    pub(super) plan_entry_capacity_entries: usize,
    pub(super) shape_field_capacity_entries: usize,
    pub(super) flag_projection_capacity_entries: usize,
    pub(super) place_projection_capacity_entries: usize,
    pub(super) finalizer_projection_capacity_entries: usize,
    pub(super) finalizer_capacity_entries: usize,
    pub(super) block_capacity_entries: usize,
    pub(super) edge_capacity_entries: usize,
    pub(super) region_capacity_entries: usize,
    pub(super) exit_capacity_entries: usize,
    pub(super) status_capacity_entries: usize,
    pub(super) transition_capacity_entries: usize,
    pub(super) branch_edge_capacity_entries: usize,
    pub(super) region_slot_capacity_entries: usize,
    pub(super) exit_region_capacity_entries: usize,
    pub(super) status_case_capacity_entries: usize,
}

impl HirPreResolveCapacity {
    pub(super) fn complete(self) -> Option<usize> {
        self.retained_upper.checked_add(self.scratch_upper)
    }

    #[cfg(test)]
    pub(super) fn phase_peaks(self) -> [usize; 8] {
        self.phase_peaks
    }
}

#[cfg(test)]
pub(super) fn hir_capacity_terms_for_test(
    program: &Program,
    source_bytes: usize,
) -> Result<(usize, usize, usize), Diagnostic> {
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(program, source_bytes, &mut stack)?;
    Ok((
        capacity.retained_upper,
        capacity.scratch_upper,
        capacity.cleanup_retained_upper,
    ))
}

pub(super) fn hir_pre_resolve_capacity<'a>(
    program: &'a Program,
    source_bytes: usize,
    stack: &mut [Option<(&'a crate::ast::Expr, usize, usize)>; MAX_SEMANTIC_EXPRESSION_DEPTH + 1],
) -> Result<HirPreResolveCapacity, Diagnostic> {
    let all_roots = source_functions(program).flat_map(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
    });
    let stats = scan_ast_capacity(all_roots, program, false, stack)?;
    let contract_index_digits = source_functions(program).fold(1usize, |digits, function| {
        digits
            .max(decimal_digits(function.requires.len().saturating_sub(1)))
            .max(decimal_digits(function.ensures.len().saturating_sub(1)))
            .max(decimal_digits(function.params.len().saturating_sub(1)))
    });
    let monomorphic_roots = source_functions(program)
        .filter(|function| function.type_parameters.is_empty())
        .flat_map(|function| {
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
        });
    let reachable_generic_calls =
        scan_ast_capacity(monomorphic_roots, program, true, stack)?.generic_calls;
    let mut largest_template = AstCapacityStats::default();
    for function in source_functions(program) {
        if function.type_parameters.is_empty() {
            continue;
        }
        let roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let template = scan_ast_capacity(roots, program, false, stack)?;
        largest_template.nodes = largest_template.nodes.max(template.nodes);
        largest_template.cumulative_depth = largest_template
            .cumulative_depth
            .max(template.cumulative_depth);
        largest_template.max_depth = largest_template.max_depth.max(template.max_depth);
        largest_template.max_match_arms =
            largest_template.max_match_arms.max(template.max_match_arms);
        largest_template.max_indexed_children = largest_template
            .max_indexed_children
            .max(template.max_indexed_children);
        largest_template.depth_arm_product_sum = largest_template
            .depth_arm_product_sum
            .max(template.depth_arm_product_sum);
        largest_template.depth_width_product_sum = largest_template
            .depth_width_product_sum
            .max(template.depth_width_product_sum);
        largest_template.local_bindings =
            largest_template.local_bindings.max(template.local_bindings);
        largest_template.pattern_bindings = largest_template
            .pattern_bindings
            .max(template.pattern_bindings);
        largest_template.binding_name_bytes = largest_template
            .binding_name_bytes
            .max(template.binding_name_bytes);
        largest_template.binding_depth_sum = largest_template
            .binding_depth_sum
            .max(template.binding_depth_sum);
        largest_template.max_index_digits = largest_template
            .max_index_digits
            .max(template.max_index_digits);
    }
    let declarations = program
        .types
        .len()
        .checked_add(program.interfaces.len())
        .and_then(|value| value.checked_add(source_functions(program).count()))
        .and_then(|value| {
            program
                .interfaces
                .iter()
                .try_fold(value, |value, interface| {
                    value.checked_add(interface.imports.len())
                })
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let nested_declarations = program
        .types
        .iter()
        .try_fold(declarations, |count, declaration| {
            let count = count.checked_add(declaration.type_parameters.len())?;
            match &declaration.kind {
                crate::ast::TypeDeclarationKind::Resource { lifecycles } => {
                    count.checked_add(lifecycles.len())
                }
                crate::ast::TypeDeclarationKind::Record { fields }
                | crate::ast::TypeDeclarationKind::Class { fields, .. } => {
                    count.checked_add(fields.len())
                }
                crate::ast::TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .try_fold(count.checked_add(cases.len())?, |count, case| {
                        count.checked_add(case.fields.len())
                    }),
            }
        })
        .and_then(|count| {
            source_functions(program).try_fold(count, |count, function| {
                count
                    .checked_add(function.type_parameters.len())?
                    .checked_add(function.params.len())
            })
        })
        .and_then(|count| {
            program
                .interfaces
                .iter()
                .try_fold(count, |count, interface| {
                    interface.imports.iter().try_fold(count, |count, import| {
                        count.checked_add(import.params.len())
                    })
                })
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The longest indexed segment is `.arm.<i>.binding.<j>`; derive its digit
    // widths from the widest admitted authored node instead of assuming a
    // machine-usize textual width. Resolved
    // expression identity, value identity, cleanup inventory, cleanup plan,
    // and validation/index ownership can retain at most six path-bearing
    // copies. Fixed node/declaration terms cover enum/vector/BTree node bodies.
    let maximum_index_digits = stats.max_index_digits.max(contract_index_digits);
    let indexed_path_segment_bytes = 15usize
        .checked_add(
            maximum_index_digits
                .checked_mul(2)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_node_inline = std::mem::size_of::<semaprax::cleanup::CleanupStorageSlot>()
        .checked_add(std::mem::size_of::<semaprax::cleanup::CleanupFlag>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>())
        })
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let retained_node_inline = std::mem::size_of::<ResolvedExpr>()
        .checked_add(cleanup_node_inline)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let type_expansion = declaration_dag_expansion(program, reachable_generic_calls)?;
    let maximum_resource_leaves = type_expansion.maximum_resource_leaves;
    let disposal_frames = stats
        .max_depth
        .checked_mul(4)
        .and_then(|frames| {
            frames.checked_add(type_expansion.maximum_type_occurrences.checked_mul(2)?)
        })
        .and_then(|frames| frames.checked_add(16))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let path_copy_upper = 1usize
        .checked_add(maximum_resource_leaves.min(5))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_path_copies = maximum_resource_leaves.min(5);
    let mut exact_expression_identity_bytes = 0usize;
    let mut cleanup_plan_uncovered_identity_bytes = 0usize;
    for function in source_functions(program) {
        let multiplicity = if function.type_parameters.is_empty() {
            1
        } else {
            reachable_generic_calls
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        };
        let (function_expression_bytes, function_plan_bytes) =
            cleanup_plan_variable_identity_bytes(function, program, cleanup_path_copies)?;
        exact_expression_identity_bytes = exact_expression_identity_bytes
            .checked_add(
                function_expression_bytes
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_plan_uncovered_identity_bytes = cleanup_plan_uncovered_identity_bytes
            .checked_add(
                function_plan_bytes
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let node_bytes = stats
        .nodes
        .checked_mul(retained_node_inline)
        .and_then(|bytes| {
            bytes.checked_add(exact_expression_identity_bytes.checked_mul(path_copy_upper)?)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Peak iterative resolver/validator/cleanup scratch. The declaration
    // census is a conservative upper for simultaneously live bindings/flags.
    // Branch continuations retain at most depth copies; Match retains one
    // FlowState per authored arm. Indexed child vectors/commit lists are
    // bounded by the widest authored node.
    let parameter_bindings = source_functions(program)
        .try_fold(0usize, |count, function| {
            count.checked_add(function.params.len())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let maximum_declared_fields = program
        .types
        .iter()
        .try_fold(0usize, |total, declaration| {
            let fields = match &declaration.kind {
                crate::ast::TypeDeclarationKind::Resource { .. } => 1,
                crate::ast::TypeDeclarationKind::Record { fields }
                | crate::ast::TypeDeclarationKind::Class { fields, .. } => fields.len(),
                crate::ast::TypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .try_fold(0usize, |count, case| count.checked_add(case.fields.len()))?,
            };
            total.checked_add(fields)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        .max(1);
    let binding_slots = parameter_bindings
        .checked_add(stats.local_bindings)
        .and_then(|width| width.checked_add(stats.pattern_bindings))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // A binding of an aggregate can contribute one ownership/partial-place
    // fact per resource leaf. The declaration-field sum is a no-allocation
    // upper for an acyclic declaration graph, while the declaration verifier
    // rejects cycles before semantic admission.
    let live_state_width = binding_slots
        .checked_mul(maximum_resource_leaves)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        .max(1);
    let branch_scope_copies = stats
        .depth_arm_product_sum
        .checked_add(stats.max_depth)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let parameter_name_bytes = program
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            function.params.iter().try_fold(bytes, |bytes, parameter| {
                bytes.checked_add(parameter.name.len())
            })
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let binding_identity_bytes = stats
        .binding_name_bytes
        .checked_add(parameter_name_bytes)
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .binding_depth_sum
                    .checked_mul(indexed_path_segment_bytes)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let scope_entry_inline =
        std::mem::size_of::<(crate::hir::ValueId, ResolvedType, OwnershipMode)>();
    let scope_payload_bytes = live_state_width
        .checked_mul(scope_entry_inline)
        .and_then(|bytes| {
            bytes.checked_add(binding_identity_bytes.checked_mul(maximum_declared_fields)?)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let indexed_result_bytes = std::mem::size_of::<ResolvedExpr>()
        .checked_add(std::mem::size_of::<ResolvedStatement>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<crate::hir::ResolvedFieldInitializer>())
        })
        .and_then(|bytes| bytes.checked_add(CLEANUP_EVAL_RESULT_BYTES))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let declaration_work_bytes = std::mem::size_of::<crate::hir::ResolvedTypeDeclaration>()
        .checked_add(std::mem::size_of::<crate::hir::ResolvedFieldDeclaration>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<
                crate::hir::ResolvedVariantCaseDeclaration,
            >())
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let source_phase = stats
        .max_depth
        .checked_mul(SOURCE_VERIFIER_FRAME_BYTES)
        .and_then(|bytes| bytes.checked_add(branch_scope_copies.checked_mul(scope_payload_bytes)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let resolver_phase = stats
        .max_depth
        .checked_mul(HIR_RESOLVER_FRAME_BYTES)
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .max_depth
                    .checked_mul(stats.max_indexed_children.max(1))?
                    .checked_mul(indexed_result_bytes)?,
            )
        })
        .and_then(|bytes| bytes.checked_add(branch_scope_copies.checked_mul(scope_payload_bytes)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let validator_phase = stats
        .max_depth
        .checked_mul(HIR_VALIDATOR_FRAME_BYTES)
        .and_then(|bytes| bytes.checked_add(branch_scope_copies.checked_mul(scope_payload_bytes)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let inventory_phase = maximum_resource_leaves
        .checked_mul(
            CLEANUP_INVENTORY_SHAPE_FRAME_BYTES
                + std::mem::size_of::<DeclarationId>()
                + std::mem::size_of::<semaprax::cleanup::FieldLivenessShape>(),
        )
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .max_depth
                    .checked_mul(CLEANUP_INVENTORY_EXPR_FRAME_BYTES)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let plan_entry_bytes = std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>()
        + std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>()
        + std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>()
        + std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>()
        + std::mem::size_of::<semaprax::cleanup_plan::StatusSource>();
    let cleanup_phase = stats
        .max_depth
        .checked_mul(CLEANUP_LOWER_FRAME_BYTES)
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .max_depth
                    .checked_mul(stats.max_indexed_children.max(1))?
                    .checked_mul(CLEANUP_EVAL_RESULT_BYTES)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                stats
                    .nodes
                    .checked_mul(maximum_resource_leaves)?
                    .checked_mul(plan_entry_bytes)?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let call_index_phase = stats
        .max_depth
        .checked_mul(CALL_INDEX_FRAME_BYTES)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let closure_identity_entries = program
        .functions
        .len()
        .checked_add(program.interfaces.len())
        .and_then(|entries| entries.checked_add(nested_declarations))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        .max(1);
    let closure_btree_entry_overhead = std::mem::size_of::<BTreeMap<String, usize>>();
    let closure_reference_headers = program
        .functions
        .len()
        .checked_mul(std::mem::size_of::<&ResolvedFunction>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let closure_phase = Some(closure_reference_headers)
        // The selected closure borrows functions from the live resolved
        // program. Only the sorted reference vector is retained; expression
        // and cleanup trees are neither cloned nor separately dropped.
        // by_id, state, depths, reached-imports, pending/visited/direct-call
        // sets, and contract traversal sets can overlap. One separately
        // allocated BTree node per identity plus the full authored source as
        // every key payload is conservative for each of the nine containers.
        .and_then(|bytes| {
            bytes.checked_add(
                closure_identity_entries
                    .checked_mul(
                        std::mem::size_of::<(String, usize)>()
                            .checked_add(closure_btree_entry_overhead)?,
                    )?
                    .checked_mul(9)?,
            )
        })
        .and_then(|bytes| bytes.checked_add(source_bytes.checked_mul(9)?))
        // DFS retains one ID and one indexed direct-call vector per depth.
        .and_then(|bytes| {
            bytes.checked_add(
                MAX_CALL_DEPTH.checked_mul(
                    std::mem::size_of::<SelectedClosureFrame>()
                        .checked_add(indexed_path_segment_bytes)?,
                )?,
            )
        })
        // While converting a direct-call set into the frame Vec, both
        // container backings and all ID strings coexist.
        .and_then(|bytes| {
            bytes.checked_add(
                closure_identity_entries.checked_mul(
                    std::mem::size_of::<String>()
                        .checked_add(std::mem::size_of::<DeclarationId>())?
                        .checked_add(closure_btree_entry_overhead)?,
                )?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let frame_machine_scratch = source_phase
        .max(resolver_phase)
        .max(validator_phase)
        .max(inventory_phase)
        .max(cleanup_phase)
        .max(call_index_phase)
        .max(closure_phase)
        .checked_add(
            nested_declarations
                .checked_mul(declaration_work_bytes)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let declaration_phase_overlap = nested_declarations
        .checked_mul(declaration_work_bytes)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Each distinct reachable specialization may clone and resolve one whole
    // template while the resolved template remains live. Count every call
    // site (even duplicate instances) against the largest template, which is
    // conservative without allocating a pre-resolution identity set.
    let specialization_bytes = largest_template
        .nodes
        .checked_mul(
            retained_node_inline
                .checked_mul(2)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .cumulative_depth
                    .checked_mul(indexed_path_segment_bytes.checked_mul(2 * 6)?)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .max_depth
                    .checked_mul(HIR_RESOLVER_FRAME_BYTES)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .depth_arm_product_sum
                    .checked_add(largest_template.max_depth)?
                    .checked_mul(live_state_width)?
                    .checked_mul(scope_entry_inline)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                largest_template
                    .max_depth
                    .checked_mul(largest_template.max_indexed_children.max(1))?
                    .checked_mul(indexed_result_bytes)?,
            )
        })
        .and_then(|bytes| bytes.checked_mul(reachable_generic_calls))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // TypeFacts layout keys recursively embed each child key. The fixed
    // per-occurrence syntax consists of four decimal lengths/separators plus
    let type_fact_layout_upper = crate::private_capacity_contract::type_facts_layout_upper(
        source_bytes,
        program.types.len(),
        type_expansion.maximum_type_occurrences,
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let type_facts_frame_bytes = std::mem::size_of::<(
        ResolvedType,
        String,
        DeclarationId,
        crate::hir::DeclarationKind,
        usize,
    )>();
    let type_facts_scratch = type_expansion
        .maximum_type_occurrences
        .checked_mul(type_facts_frame_bytes)
        .and_then(|bytes| bytes.checked_add(type_fact_layout_upper.checked_mul(2)?))
        .and_then(|bytes| {
            bytes.checked_add(
                program
                    .types
                    .len()
                    .checked_mul(std::mem::size_of::<(String, crate::hir::TypeFacts)>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let declaration_index_upper = crate::private_capacity_contract::declaration_index_upper(
        source_bytes,
        program.types.len(),
        program.interfaces.len(),
        program.functions.len(),
        type_fact_layout_upper,
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // The declaration-DAG pass also performs a typed source-flow census while
    // its exact temporary memo is still authorized. Unlike the former
    // `all roots * largest type * all nodes` product, every persistent shape,
    // flag and projection below is charged against the authored type of the
    // storage root that can create it. Plan places and exits are separate
    // copies because they coexist with the inventory and plan-slot shapes.
    let cleanup = type_expansion.cleanup_retained;
    let cleanup_function_instance_upper = program
        .functions
        .iter()
        .try_fold(0usize, |instances, function| {
            let multiplicity = if function.type_parameters.is_empty() {
                1
            } else {
                reachable_generic_calls.checked_add(1)?
            };
            instances.checked_add(multiplicity)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let flag_capacity_extra =
        retained_vec_capacity_extra(cleanup.leaves, cleanup_function_instance_upper)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let shape_field_capacity_extra = retained_vec_capacity_extra(
        cleanup.shape_fields,
        cleanup.occurrences.min(cleanup.shape_fields),
    )
    .and_then(|extra| extra.checked_mul(2))
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let flag_projection_capacity_extra = retained_vec_capacity_extra(
        cleanup.projection_segments,
        cleanup.leaves.min(cleanup.projection_segments),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let place_projection_capacity_extra = retained_vec_capacity_extra(
        cleanup.place_projection_segments,
        cleanup.place_copies.min(cleanup.place_projection_segments),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let finalizer_projection_capacity_extra = retained_vec_capacity_extra(
        cleanup.finalizer_projection_segments,
        cleanup
            .finalizer_copies
            .min(cleanup.finalizer_projection_segments),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let finalizer_capacity_extra = retained_vec_capacity_extra(
        cleanup.finalizer_copies,
        cleanup.exit_events.min(cleanup.finalizer_copies),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let entry_state_capacity_extra = retained_vec_capacity_extra(
        cleanup.roots,
        cleanup_function_instance_upper.min(cleanup.roots),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let inventory_slot_capacity_extra = retained_vec_capacity_extra(
        cleanup.roots,
        cleanup_function_instance_upper.min(cleanup.roots),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let plan_slot_capacity_extra = retained_vec_capacity_extra(
        cleanup.roots,
        cleanup_function_instance_upper.min(cleanup.roots),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let inventory_entry_capacity_entries = cleanup
        .roots
        .checked_add(entry_state_capacity_extra)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let cleanup_parent_local_lifetime_upper = cleanup
        .parent_local_finalizer_copies
        .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .parent_local_finalizer_projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.parent_local_finalizer_lifecycle_ids))
        .and_then(|bytes| bytes.checked_add(cleanup.parent_local_finalizer_projection_ids))
        .and_then(|bytes| bytes.checked_add(cleanup.parent_local_finalizer_storage_bytes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let cleanup_parent_local_projection_lifetime_upper = {
        let action_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_projection_finalizer_copies,
            cleanup
                .parent_local_projection_exit_groups
                .min(cleanup.parent_local_projection_finalizer_copies),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_projection_finalizer_projection_segments,
            cleanup
                .parent_local_projection_finalizer_copies
                .min(cleanup.parent_local_projection_finalizer_projection_segments),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup
            .parent_local_projection_finalizer_copies
            .checked_add(action_capacity_extra)
            .and_then(|entries| {
                entries.checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    cleanup
                        .parent_local_projection_finalizer_projection_segments
                        .checked_add(projection_capacity_extra)?
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_projection_finalizer_lifecycle_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_projection_finalizer_projection_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_projection_finalizer_storage_bytes)
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
    };
    #[cfg(test)]
    let cleanup_parent_local_update_prefix_lifetime_upper = {
        let action_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_update_prefix_finalizer_copies,
            cleanup
                .parent_local_update_prefix_exit_groups
                .min(cleanup.parent_local_update_prefix_finalizer_copies),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let projection_capacity_extra = retained_vec_capacity_extra(
            cleanup.parent_local_update_prefix_finalizer_projection_segments,
            cleanup
                .parent_local_update_prefix_finalizer_copies
                .min(cleanup.parent_local_update_prefix_finalizer_projection_segments),
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup
            .parent_local_update_prefix_finalizer_copies
            .checked_add(action_capacity_extra)
            .and_then(|entries| {
                entries.checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())
            })
            .and_then(|bytes| {
                bytes.checked_add(
                    cleanup
                        .parent_local_update_prefix_finalizer_projection_segments
                        .checked_add(projection_capacity_extra)?
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_update_prefix_finalizer_lifecycle_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_update_prefix_finalizer_projection_ids)
            })
            .and_then(|bytes| {
                bytes.checked_add(cleanup.parent_local_update_prefix_finalizer_storage_bytes)
            })
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
    };
    let cleanup_retained_upper = cleanup
        .roots
        .checked_mul(
            std::mem::size_of::<semaprax::cleanup::CleanupStorageSlot>()
                + std::mem::size_of::<semaprax::cleanup_plan::CleanupSlot>(),
        )
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .occurrences
                    .checked_mul(2)?
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::FieldLivenessShape>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .shape_fields
                    .checked_mul(2)?
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::FieldLiveness>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.shape_ids.checked_mul(2)?))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .leaves
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupFlag>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.lifecycle_ids))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.projection_ids))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .place_copies
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupPlace>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .place_projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.place_projection_ids))
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .finalizer_copies
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .finalizer_projection_segments
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.finalizer_lifecycle_ids))
        .and_then(|bytes| bytes.checked_add(cleanup.finalizer_projection_ids))
        .and_then(|bytes| {
            bytes.checked_add(cleanup.staged_results.checked_mul(std::mem::size_of::<
                semaprax::cleanup_plan::StagedCopyResultSource,
            >())?)
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup
                    .variant_edges
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::EdgeCondition>())?,
            )
        })
        .and_then(|bytes| bytes.checked_add(cleanup.stage_identity_and_type_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.variant_identity_bytes))
        .and_then(|bytes| {
            bytes.checked_add(cleanup.call_arguments.checked_mul(std::mem::size_of::<
                semaprax::cleanup_plan::CallArgumentTransfer,
            >())?)
        })
        .and_then(|bytes| bytes.checked_add(cleanup.call_argument_owned_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.ordinary_slot_payload_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.ordinary_place_storage_bytes))
        .and_then(|bytes| bytes.checked_add(cleanup.ordinary_finalizer_storage_bytes))
        .and_then(|bytes| {
            bytes.checked_add(
                flag_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupFlag>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                shape_field_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::FieldLiveness>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                flag_projection_capacity_extra.checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                place_projection_capacity_extra.checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                finalizer_projection_capacity_extra
                    .checked_mul(std::mem::size_of::<DeclarationId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                finalizer_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                entry_state_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupPlace>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                inventory_slot_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupStorageSlot>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                plan_slot_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupSlot>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                inventory_entry_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup::CleanupStorageId>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let mut cleanup_structural_nodes = 0usize;
    let mut cleanup_structural_depth = 0usize;
    let mut cleanup_failure_events = 0usize;
    let mut cleanup_call_events = 0usize;
    let mut cleanup_boolean_branch_events = 0usize;
    let mut cleanup_contracts = 0usize;
    let mut cleanup_function_instances = 0usize;
    let cleanup_expression_identity_bytes = exact_expression_identity_bytes;
    for function in source_functions(program) {
        let function_stats = scan_ast_capacity(
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures),
            program,
            false,
            stack,
        )?;
        let multiplicity = if function.type_parameters.is_empty() {
            1
        } else {
            reachable_generic_calls
                .checked_add(1)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
        };
        cleanup_structural_nodes = cleanup_structural_nodes
            .checked_add(
                function_stats
                    .nodes
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_structural_depth =
            cleanup_structural_depth.max(cleanup_function_region_depth(function, stack)?);
        cleanup_function_instances = cleanup_function_instances
            .checked_add(multiplicity)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_contracts = cleanup_contracts
            .checked_add(
                function
                    .requires
                    .len()
                    .checked_add(function.ensures.len())
                    .and_then(|contracts| contracts.checked_mul(multiplicity))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        cleanup_failure_events = cleanup_failure_events
            .checked_add(
                cleanup_function_finalizer_events(function, stack)?
                    .checked_sub(1)
                    .and_then(|events| events.checked_mul(multiplicity))
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let mut function_call_events = 0usize;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            function_call_events = function_call_events
                .checked_add(cleanup_expression_call_events(root, program, stack)?)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        cleanup_call_events = cleanup_call_events
            .checked_add(
                function_call_events
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        let mut function_boolean_branch_events = 0usize;
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            function_boolean_branch_events = function_boolean_branch_events
                .checked_add(cleanup_expression_boolean_branch_events(root, stack)?)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        }
        cleanup_boolean_branch_events = cleanup_boolean_branch_events
            .checked_add(
                function_boolean_branch_events
                    .checked_mul(multiplicity)
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    }
    let cleanup_structural_upper = cleanup_structural_nodes
        .checked_mul(cleanup_node_inline)
        .and_then(|bytes| {
            bytes.checked_add(cleanup_expression_identity_bytes.checked_mul(cleanup_path_copies)?)
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    // Retained CleanupPlan container backing and identity payloads are
    // distinct from inventory/slot shapes above. Derive each family from the
    // source events that can create it. Four headers per logical entry covers
    // the current target's minimum-capacity floor for independently allocated
    // small Vecs as well as geometric growth.
    let transition_entries = cleanup
        .occurrences
        .checked_mul(2)
        // Every failing status path owns one SelectFailure transition. Only
        // ordinary calls additionally own CallCommit; checked arithmetic and
        // contract-false paths do not. Native Rust imports have neither.
        .and_then(|entries| entries.checked_add(cleanup_failure_events))
        .and_then(|entries| entries.checked_add(cleanup_call_events))
        .and_then(|entries| entries.checked_add(cleanup.call_arguments))
        .and_then(|entries| entries.checked_add(cleanup.staged_results))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_callee_identity_bytes = program
        .functions
        .iter()
        .map(|function| function.stable_id.len())
        .chain(program.interfaces.iter().flat_map(|interface| {
            interface
                .imports
                .iter()
                .map(|import| import.stable_id.len())
        }))
        .max()
        .unwrap_or(0);
    let expression_identity_fixed_bytes = "function-execution:"
        .len()
        .checked_add("semaprax.function-execution.v1:generic:".len())
        .and_then(|bytes| bytes.checked_add("declaration:".len()))
        .and_then(|bytes| bytes.checked_add(":expression:".len()))
        .and_then(|bytes| bytes.checked_add(decimal_digits(source_bytes).checked_mul(4)?))
        .and_then(|bytes| bytes.checked_add(8))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_block_headers = cleanup_structural_nodes
        .checked_mul(2)
        .and_then(|entries| entries.checked_add(cleanup_contracts))
        .and_then(|entries| entries.checked_add(cleanup_function_instances))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_edge_headers = cleanup_structural_nodes
        .checked_mul(3)
        .and_then(|entries| entries.checked_add(cleanup_contracts.checked_mul(2)?))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_region_headers = cleanup_contracts
        .checked_add(cleanup_function_instances)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let extra_exit_headers = cleanup
        .exit_events
        .checked_sub(cleanup.exit_events.min(cleanup_structural_nodes))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let block_entries = cleanup_structural_nodes
        .checked_add(extra_block_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let edge_entries = cleanup_structural_nodes
        .checked_add(extra_edge_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let region_entries = cleanup_structural_nodes
        .checked_add(extra_region_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_entries = cleanup_structural_nodes
        .checked_add(extra_exit_headers)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let block_capacity_extra =
        retained_vec_capacity_extra(block_entries, cleanup_function_instances.min(block_entries))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let edge_capacity_extra =
        retained_vec_capacity_extra(edge_entries, cleanup_function_instances.min(edge_entries))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let region_capacity_extra = retained_vec_capacity_extra(
        region_entries,
        cleanup_function_instances.min(region_entries),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_capacity_extra =
        retained_vec_capacity_extra(exit_entries, cleanup_function_instances.min(exit_entries))
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_capacity_extra = retained_vec_capacity_extra(
        cleanup_failure_events,
        cleanup_function_instances.min(cleanup_failure_events),
    )
    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let plan_expression_identity_copies = cleanup
        .occurrences
        .checked_mul(2)
        .and_then(|copies| copies.checked_add(cleanup_failure_events.checked_mul(5)?))
        .and_then(|copies| copies.checked_add(cleanup_call_events))
        .and_then(|copies| copies.checked_add(cleanup_boolean_branch_events.checked_mul(2)?))
        .and_then(|copies| copies.checked_add(cleanup_function_instances))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let transition_capacity_entries = transition_entries
        .checked_add(
            retained_vec_capacity_extra(transition_entries, block_entries.min(transition_entries))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let branch_edge_entries = cleanup_structural_nodes
        .checked_mul(3)
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let branch_edge_capacity_entries = branch_edge_entries
        .checked_add(
            retained_vec_capacity_extra(
                branch_edge_entries,
                block_entries.min(branch_edge_entries),
            )
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let region_slot_capacity_entries = cleanup
        .roots
        .checked_add(
            retained_vec_capacity_extra(cleanup.roots, region_entries.min(cleanup.roots))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_region_entries = cleanup
        .exit_events
        .checked_mul(cleanup_structural_depth.max(1))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let exit_region_capacity_entries = exit_region_entries
        .checked_add(
            retained_vec_capacity_extra(exit_region_entries, exit_entries.min(exit_region_entries))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let status_case_capacity_entries = cleanup_failure_events
        .checked_add(
            retained_vec_capacity_extra(cleanup_failure_events, cleanup_failure_events)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
        )
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_plan_structural_upper = transition_capacity_entries
        .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupTransition>())
        .and_then(|bytes| {
            bytes.checked_add(
                branch_edge_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::EdgeId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_block_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_edge_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_region_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                extra_exit_headers
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                region_slot_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StorageId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                exit_region_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegionId>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                cleanup_failure_events
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StatusSource>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                status_case_capacity_entries
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StatusCase>())?,
            )
        })
        // The full path payload has one source-derived copy in
        // cleanup_structural_upper. Each status/edge/continuation clone also
        // owns the fixed scoped-identity framing around that path.
        .and_then(|bytes| {
            bytes.checked_add(
                plan_expression_identity_copies.checked_mul(expression_identity_fixed_bytes)?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(cleanup_failure_events.checked_mul(cleanup_callee_identity_bytes)?)
        })
        .and_then(|bytes| bytes.checked_add(cleanup_plan_uncovered_identity_bytes))
        .and_then(|bytes| {
            bytes.checked_add(
                block_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupBlock>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                edge_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupEdge>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                region_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::CleanupRegion>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                exit_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::ExitTarget>())?,
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                status_capacity_extra
                    .checked_mul(std::mem::size_of::<semaprax::cleanup_plan::StatusSource>())?,
            )
        })
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let cleanup_authority_upper = cleanup_retained_upper
        .checked_add(cleanup_structural_upper)
        .and_then(|bytes| bytes.checked_add(cleanup_plan_structural_upper))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    #[cfg(test)]
    let cleanup_proof =
        CleanupCapacityProofTerms {
            stats: cleanup,
            inventory_slot_capacity_entries: cleanup
                .roots
                .checked_add(inventory_slot_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            inventory_flag_capacity_entries: cleanup
                .leaves
                .checked_add(flag_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            inventory_entry_capacity_entries,
            plan_slot_capacity_entries: cleanup
                .roots
                .checked_add(plan_slot_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            plan_entry_capacity_entries: cleanup
                .roots
                .checked_add(entry_state_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            shape_field_capacity_entries: cleanup
                .shape_fields
                .checked_mul(2)
                .and_then(|entries| entries.checked_add(shape_field_capacity_extra))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            flag_projection_capacity_entries: cleanup
                .projection_segments
                .checked_add(flag_projection_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            place_projection_capacity_entries: cleanup
                .place_projection_segments
                .checked_add(place_projection_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            finalizer_projection_capacity_entries: cleanup
                .finalizer_projection_segments
                .checked_add(finalizer_projection_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            finalizer_capacity_entries: cleanup
                .finalizer_copies
                .checked_add(finalizer_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            block_capacity_entries: block_entries
                .checked_add(block_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            edge_capacity_entries: edge_entries
                .checked_add(edge_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            region_capacity_entries: region_entries
                .checked_add(region_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            exit_capacity_entries: exit_entries
                .checked_add(exit_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            status_capacity_entries: cleanup_failure_events
                .checked_add(status_capacity_extra)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            transition_capacity_entries,
            branch_edge_capacity_entries,
            region_slot_capacity_entries,
            exit_region_capacity_entries,
            status_case_capacity_entries,
        };
    let disposal_workspace_bytes = disposal_frames
        .checked_mul(std::mem::size_of::<ResolvedDisposeFrame>())
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let resolved_function_headers = program
        .functions
        .len()
        .checked_add(reachable_generic_calls)
        .and_then(|functions| functions.checked_mul(std::mem::size_of::<ResolvedFunction>()))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    let retained_upper = source_bytes
        .checked_mul(8)
        .and_then(|bytes| bytes.checked_add(node_bytes))
        .and_then(|bytes| bytes.checked_add(specialization_bytes))
        .and_then(|bytes| {
            bytes.checked_add(nested_declarations.checked_mul(declaration_work_bytes)?)
        })
        .and_then(|bytes| bytes.checked_add(declaration_index_upper))
        .and_then(|bytes| bytes.checked_add(cleanup_retained_upper))
        .and_then(|bytes| bytes.checked_add(cleanup_plan_structural_upper))
        .and_then(|bytes| bytes.checked_add(disposal_workspace_bytes))
        .and_then(|bytes| bytes.checked_add(resolved_function_headers))
        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
    Ok(HirPreResolveCapacity {
        retained_upper,
        scratch_upper: frame_machine_scratch.max(type_facts_scratch),
        declaration_index_upper,
        cleanup_retained_upper,
        cleanup_authority_upper,
        cleanup_exit_events_upper: cleanup.exit_events,
        cleanup_fallback_roots: cleanup.fallback_roots,
        cleanup_call_argument_owned_upper: cleanup.call_argument_owned_bytes,
        cleanup_plan_structural_upper,
        #[cfg(test)]
        cleanup_parent_local_lifetime_upper,
        #[cfg(test)]
        cleanup_parent_local_projection_lifetime_upper,
        #[cfg(test)]
        cleanup_parent_local_update_prefix_lifetime_upper,
        #[cfg(test)]
        cleanup_proof,
        phase_peaks: [
            source_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            resolver_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            validator_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            inventory_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            cleanup_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            call_index_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            closure_phase
                .checked_add(declaration_phase_overlap)
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?,
            type_facts_scratch,
        ],
        disposal_frames,
    })
}

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

pub(super) fn hir_loan_plan_owned_capacity(plan: &semaprax::loan_plan::LoanPlan) -> Option<usize> {
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

pub(super) fn hir_owned_capacity(resolved: &ResolvedProgram) -> Result<usize, Diagnostic> {
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

#[cfg(test)]
pub(super) fn validate_native_rust_expression_budget(
    resolved: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    let functions = resolved.functions.iter().collect::<Vec<_>>();
    validate_native_rust_expression_budget_for_closure(&functions, false)
}

pub(super) fn validate_native_rust_expression_budget_for_closure(
    functions: &[&ResolvedFunction],
    preauthorized: bool,
) -> Result<(), Diagnostic> {
    note_hir_post_resolve_phase(1);
    let mut pending = Vec::new();
    for function in functions {
        pending.extend(
            function
                .requires
                .iter()
                .map(|expression| (expression, 1_usize)),
        );
        pending.push((&function.body, 1));
        pending.extend(
            function
                .ensures
                .iter()
                .map(|expression| (expression, 1_usize)),
        );
    }
    let mut visited = 0_usize;
    while let Some((expression, depth)) = pending.pop() {
        note_hir_post_resolve_capacity(
            0,
            pending.capacity() * std::mem::size_of::<(&ResolvedExpr, usize)>(),
        );
        visited = visited
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        if !preauthorized {
            debit(std::mem::size_of::<&ResolvedExpr>())?;
        }
        if visited > MAX_SOURCE_BYTES {
            return Err(b109("max_builder_bytes", MAX_BUILDER_BYTES));
        }
        if depth > MAX_SEMANTIC_EXPRESSION_DEPTH {
            return Err(b109(
                "max_semantic_expression_depth",
                MAX_SEMANTIC_EXPRESSION_DEPTH,
            ));
        }
        let child_depth = depth
            .checked_add(1)
            .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
        match &expression.kind {
            ResolvedExprKind::Call { args, .. } => {
                pending.extend(args.iter().map(|value| (value, child_depth)))
            }
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                pending.push((source, child_depth));
                pending.push((start, child_depth));
                pending.push((end, child_depth));
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                pending.extend(call.args.iter().map(|value| (value, child_depth)))
            }
            ResolvedExprKind::HostCommandCall(call) => {
                pending.extend(call.args.iter().map(|value| (value, child_depth)))
            }
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value } => pending.push((value, child_depth)),
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push((left, child_depth));
                pending.push((right, child_depth));
            }
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    pending.extend(
                        (0..statement.child_count())
                            .filter_map(|index| statement.child(index))
                            .map(|child| (child, child_depth)),
                    );
                }
                pending.push((tail, child_depth));
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push((condition, child_depth));
                pending.push((then_branch, child_depth));
                pending.push((else_branch, child_depth));
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                pending.extend(fields.iter().map(|field| (&field.value, child_depth)));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                pending.push((scrutinee, child_depth));
                pending.extend(
                    arms.iter()
                        .filter_map(|arm| arm.guard.as_deref())
                        .map(|guard| (guard, child_depth)),
                );
                pending.extend(arms.iter().map(|arm| (&arm.value, child_depth)));
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                pending.push((base, child_depth));
                pending.extend(fields.iter().map(|field| (&field.value, child_depth)));
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
    Ok(())
}
