//! Cleanup event counting: exit, failure, call, branch, and finalizer
//! events per source and resolved function.

use super::*;

const FOR_GENERATED_EXPRESSION_COUNT: usize = 16;
const FOR_OWN_GENERATED_EXPRESSION_COUNT: usize = 15;

fn longest_direct_expression_identity_suffix_len() -> usize {
    let index_digits = decimal_digits(usize::MAX);
    [
        ".arg.0.source".len(),
        ".callable".len(),
        ".capture.".len() + index_digits,
        ".native-rust-arg.".len() + index_digits,
        ".arg.".len() + index_digits,
        ".field.".len() + index_digits + ".value".len(),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

fn desugared_statement_identity_upper(
    function: &crate::ast::Function,
    generic_instance_identity_len: usize,
    block_path_len: usize,
    statements: &[crate::ast::Statement],
) -> Option<usize> {
    statements
        .iter()
        .enumerate()
        .try_fold(0usize, |bytes, (index, statement)| {
            let (count, suffix) = match statement {
                crate::ast::Statement::For { .. } => (
                    FOR_GENERATED_EXPRESSION_COUNT,
                    ".value.s2.body.s0.value.arg.1",
                ),
                crate::ast::Statement::ForOwn { .. } => (
                    FOR_OWN_GENERATED_EXPRESSION_COUNT,
                    ".value.s1.body.s0.value.arm.1.value.tail.arg.0",
                ),
                _ => return Some(bytes),
            };
            let generated_path_len = block_path_len
                .checked_add(".s".len())?
                .checked_add(decimal_digits(index))?
                .checked_add(suffix.len())?;
            let generated_expression_bytes = scoped_expression_backing_upper(
                function,
                generic_instance_identity_len,
                generated_path_len,
            )?
            .checked_mul(count)?;
            bytes.checked_add(generated_expression_bytes)
        })
}

#[derive(Clone, Copy)]
pub(super) enum CleanupTypeKey {
    Scalar,
    Declaration(usize),
    Unknown,
}

pub(in crate::implementation) fn cleanup_source_exit_events(
    expression: &crate::ast::Expr,
) -> usize {
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

pub(in crate::implementation) fn cleanup_function_exit_events<'a>(
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

pub(super) fn cleanup_expression_failure_events<'a>(
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

pub(super) fn cleanup_expression_call_events<'a>(
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

pub(super) fn cleanup_expression_boolean_branch_events<'a>(
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

pub(super) fn cleanup_plan_variable_identity_bytes(
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
                // Ordinary HIR retains at most three backing strings for an
                // authored expression. Direct generated identities include
                // indexed native arguments, fields, and captures; bound every
                // retained backing by the longest such direct suffix and the
                // allocator growth allowance. Closure-owner framing is a
                // separate structural bound and is not represented here.
                let retained_path_len = path_len
                    .checked_add(longest_direct_expression_identity_suffix_len())
                    .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let identity_bytes = scoped_expression_backing_upper(
                    function,
                    generic_instance_identity_len,
                    retained_path_len,
                )
                .and_then(|bytes| bytes.checked_mul(3))
                .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?;
                let desugared_statement_bytes = match &expression.kind {
                    crate::ast::ExprKind::Block { statements, .. } => {
                        desugared_statement_identity_upper(
                            function,
                            generic_instance_identity_len,
                            path_len,
                            statements,
                        )
                        .ok_or_else(|| b109("max_builder_bytes", MAX_BUILDER_BYTES))?
                    }
                    _ => 0,
                };
                all_expression_bytes = all_expression_bytes
                    .checked_add(identity_bytes)
                    .and_then(|bytes| bytes.checked_add(desugared_statement_bytes))
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

pub(super) fn cleanup_function_finalizer_events<'a>(
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

pub(super) fn cleanup_function_region_depth<'a>(
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
        let child = (!matches!(expression.kind, crate::ast::ExprKind::Closure { .. }))
            .then(|| ast_child(expression, &mut child_cursor))
            .flatten();
        if let Some((child_index, child)) = child {
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
                // A closure body is structurally retained, but construction
                // only snapshots its captures; the body has no creation-time
                // ownership or failure effect.
                crate::ast::ExprKind::Closure { .. } => false,
                // Resumable Effects v1 (issue #204): `yield`'s request is an
                // admitted Copy scalar, transparent in exactly the same way
                // an upcast is -- it does not consume the binding.
                crate::ast::ExprKind::Project { .. }
                | crate::ast::ExprKind::Unary { .. }
                | crate::ast::ExprKind::Yield { .. }
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

pub(super) fn cleanup_block_binding_finalizer_events<'a>(
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

pub(in crate::implementation) fn cleanup_parameter_finalizer_events<'a>(
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

pub(super) fn cleanup_parent_local_remaining_finalizer_events<'a>(
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

#[cfg(test)]
mod carrier_tests {
    use super::*;

    #[test]
    fn hir_capacity_identity_carriers_reserve_three_headers_per_expression() {
        let source =
            "module capacity.expression_carriers; @id(\"app.main\") fn main() -> i64 { 0 }";
        let program =
            crate::parse(source, std::path::Path::new("expression-carriers.spx")).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();

        let (retained_expression_bytes, _) =
            cleanup_plan_variable_identity_bytes(function, &program, 0).unwrap();
        let direct_suffix = longest_direct_expression_identity_suffix_len();
        assert!(direct_suffix > ".arg.0.source".len());
        let body_backing =
            scoped_expression_backing_upper(function, 0, "body".len() + direct_suffix).unwrap();
        let tail_backing =
            scoped_expression_backing_upper(function, 0, "body.tail".len() + direct_suffix)
                .unwrap();
        assert_eq!(retained_expression_bytes, 3 * (body_backing + tail_backing));

        let long_utf8_path = "λ-prefix-".repeat(64);
        let encoded_len =
            scoped_expression_identity_upper(function, 0, long_utf8_path.len()).unwrap();
        let receiver_path_len = long_utf8_path.len() + direct_suffix;
        let receiver_len =
            scoped_expression_identity_upper(function, 0, receiver_path_len).unwrap();
        let plain_backing =
            scoped_expression_backing_upper(function, 0, long_utf8_path.len()).unwrap();
        let receiver_backing =
            scoped_expression_backing_upper(function, 0, receiver_path_len).unwrap();
        assert!(
            plain_backing
                >= 2 * encoded_len.max(8)
                    + semaprax::hir::ExpressionId::OWNED_ALLOCATION_CARRIER_BYTES
        );
        assert!(
            receiver_backing
                >= 2 * receiver_len.max(8)
                    + semaprax::hir::ExpressionId::OWNED_ALLOCATION_CARRIER_BYTES
        );
        assert!(receiver_backing > plain_backing);
    }

    #[test]
    fn closure_owner_identity_growth_is_bounded_across_nested_path_widths() {
        let source =
            "module capacity.closure_growth; @id(\"app.main\") fn main<T>(value: T) -> i64 { 0 }";
        let program = crate::parse(source, std::path::Path::new("closure-growth.spx")).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        let generic_identity_len = 256;
        let increment = closure_body_identity_path_increment();
        assert_eq!(
            ast_child_identity_path_increment(
                &crate::ast::Expr {
                    kind: crate::ast::ExprKind::Closure {
                        params: Vec::new(),
                        return_type: crate::ast::Type::I64,
                        body: Box::new(crate::ast::Expr {
                            kind: crate::ast::ExprKind::Int(0),
                            span: crate::ast::Span::default(),
                        }),
                        owning: false,
                        retained: false,
                        mutable: false,
                    },
                    span: crate::ast::Span::default(),
                },
                0,
                &program,
            ),
            increment,
        );

        for mut path_len in [
            8,
            9,
            98,
            99,
            998,
            999,
            9_998,
            9_999,
            "λ-prefix-".repeat(64).len(),
        ] {
            for _ in 0..3 {
                let creation_len =
                    scoped_expression_identity_upper(function, generic_identity_len, path_len)
                        .unwrap();
                let actual_closure_body_len =
                    closure_body_identity_upper(creation_len, "body".len()).unwrap();
                path_len = path_len.checked_add(increment).unwrap();
                let synthetic_upper =
                    scoped_expression_identity_upper(function, generic_identity_len, path_len)
                        .unwrap();
                assert!(synthetic_upper >= actual_closure_body_len);

                let descendant_suffix_len = ".value".len();
                let actual_descendant_len =
                    closure_body_identity_upper(creation_len, "body".len() + descendant_suffix_len)
                        .unwrap();
                let synthetic_descendant = scoped_expression_identity_upper(
                    function,
                    generic_identity_len,
                    path_len + descendant_suffix_len,
                )
                .unwrap();
                assert!(synthetic_descendant >= actual_descendant_len);
                path_len += descendant_suffix_len;
            }
        }
    }

    #[test]
    fn loop_desugaring_identity_upper_covers_all_generated_paths() {
        let source = "module capacity.loop_carriers; @id(\"app.main\") fn main(values: i64) -> i64 { for item in values { item } for own item in values { item } 0 }";
        let program = crate::parse(source, std::path::Path::new("loop-carriers.spx")).unwrap();
        let function = program
            .functions
            .iter()
            .find(|function| function.name == "main")
            .unwrap();
        let crate::ast::ExprKind::Block { statements, .. } = &function.body.kind else {
            panic!("function body should remain a block");
        };
        assert_eq!(
            ast_child_identity_path_increment(&function.body, 0, &program),
            ".s0.values".len(),
        );
        assert_eq!(
            ast_child_identity_path_increment(&function.body, 1, &program),
            ".s0.value.s2.body.s1.value".len(),
        );
        assert_eq!(
            ast_child_identity_path_increment(&function.body, 2, &program),
            ".s1.value.s0.value.arg.0".len(),
        );
        assert_eq!(
            ast_child_identity_path_increment(&function.body, 3, &program),
            ".s1.value.s1.body.s0.value.arm.1.value.s0.value".len(),
        );
        let actual =
            desugared_statement_identity_upper(function, 0, "body".len(), statements).unwrap();
        let for_path = "body.s0.value.s2.body.s0.value.arg.1".len();
        let for_own_path = "body.s1.value.s1.body.s0.value.arm.1.value.tail.arg.0".len();
        let expected = scoped_expression_backing_upper(function, 0, for_path).unwrap()
            * FOR_GENERATED_EXPRESSION_COUNT
            + scoped_expression_backing_upper(function, 0, for_own_path).unwrap()
                * FOR_OWN_GENERATED_EXPRESSION_COUNT;
        assert_eq!(actual, expected);
    }
}
