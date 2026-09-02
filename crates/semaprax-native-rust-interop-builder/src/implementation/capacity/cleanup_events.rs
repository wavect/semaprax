//! Cleanup event counting: exit, failure, call, branch, and finalizer
//! events per source and resolved function.

use super::*;

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
