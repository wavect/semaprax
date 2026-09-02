//! Positional AST child traversal and the identity-path and type-identity
//! length upper bounds derived from it.

use super::*;

#[derive(Clone, Copy, Default)]
pub(in crate::implementation) struct AstCapacityStats {
    pub(in crate::implementation) nodes: usize,
    pub(in crate::implementation) cumulative_depth: usize,
    pub(in crate::implementation) generic_calls: usize,
    pub(in crate::implementation) max_depth: usize,
    pub(in crate::implementation) max_match_arms: usize,
    pub(in crate::implementation) max_indexed_children: usize,
    pub(in crate::implementation) depth_arm_product_sum: usize,
    pub(in crate::implementation) depth_width_product_sum: usize,
    pub(in crate::implementation) local_bindings: usize,
    pub(in crate::implementation) pattern_bindings: usize,
    pub(in crate::implementation) binding_name_bytes: usize,
    pub(in crate::implementation) binding_depth_sum: usize,
    pub(in crate::implementation) max_index_digits: usize,
}

const AST_COMPLEX_CURSOR: usize = 1usize << (usize::BITS - 1);
const AST_CURSOR_INDEX_MASK: usize = AST_COMPLEX_CURSOR - 1;

pub(super) fn ast_previous_child_path_index(cursor: usize) -> Option<usize> {
    (cursor & AST_CURSOR_INDEX_MASK).checked_sub(1)
}

pub(super) fn ast_block_statement_index(
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

pub(super) fn ast_block_statement_result_index(
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

pub(super) fn ast_match_arm_index(
    arms: &[crate::ast::MatchArm],
    child_path_index: usize,
) -> Option<usize> {
    let arm_path_index = child_path_index.checked_sub(1)?;
    Some(if arms.iter().any(|arm| arm.guard.is_some()) {
        arm_path_index / 2
    } else {
        arm_path_index
    })
}

pub(super) fn ast_match_arm_value_result_index(
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

pub(in crate::implementation) fn ast_child<'a>(
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

pub(super) fn ast_child_identity_path_increment(
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

pub(super) fn ast_root_identity_path_len(
    function: &crate::ast::Function,
    root_index: usize,
) -> usize {
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

pub(in crate::implementation) fn generic_function_instance_identity_upper(
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

pub(super) fn scoped_value_identity_upper(
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

pub(super) fn scoped_expression_identity_upper(
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
