//! Source census: expression, pattern, and resource-leaf counts taken
//! from the parsed program before resolution.

use super::*;

pub(in crate::implementation) fn scan_ast_capacity<'a>(
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

pub(super) fn decimal_digits(mut value: usize) -> usize {
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

pub(super) fn declaration_field_type(
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

pub(super) fn declaration_field_identity_bytes(
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
