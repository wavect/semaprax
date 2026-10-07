//! Closed source admission for one invocation-scoped streaming reader.
use super::diagnostics::error;
use crate::ast::{Expr, ExprKind, ParamMode, Program, Statement, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::stdin_stream_ops::{ast_is_reader, ast_type_uses};
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn check(program: &Program, diagnostics: &mut Vec<Diagnostic>) {
    let refuse = |diagnostics: &mut Vec<Diagnostic>, message, span| {
        diagnostics.push(error(program, "SPX-T270", message, span))
    };
    for declaration in &program.types {
        if declaration.stable_id == crate::stdin_stream_ops::READER_ID {
            continue;
        }
        let fields: Vec<_> = match &declaration.kind {
            TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
                fields.iter().collect()
            }
            TypeDeclarationKind::Variant { cases } => {
                cases.iter().flat_map(|case| &case.fields).collect()
            }
            TypeDeclarationKind::Resource { .. } => Vec::new(),
        };
        for field in fields {
            if ast_type_uses(&field.ty) {
                refuse(
                    diagnostics,
                    "streaming reader cannot be stored in an aggregate",
                    field.span,
                );
            }
        }
    }
    for import in program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
    {
        if matches!(&import.result, crate::ast::ImportResult::OwnedResource { name } | crate::ast::ImportResult::OwnedResultResourceI64 { name } if name == "StdinReader")
            || matches!(&import.result, crate::ast::ImportResult::BorrowedStr { owner } if owner == "StdinReader")
            || import.params.iter().any(|p| ast_type_uses(&p.ty))
        {
            refuse(
                diagnostics,
                "streaming reader cannot cross an authored import ABI",
                import.span,
            );
        }
    }
    let streaming = crate::stdin_stream_ops::program_uses(program);
    for function in &program.functions {
        if streaming {
            function.body.visit_all_nodes(&mut |expression| {
                if matches!(expression.kind, ExprKind::Closure { .. }) {
                    refuse(
                        diagnostics,
                        "streaming stdin does not compose with function values",
                        expression.span,
                    );
                }
            });
        }
        let uses = ast_type_uses(&function.return_type)
            || function.params.iter().any(|p| ast_type_uses(&p.ty))
            || crate::stdin_stream_ops::ast_expression_uses(&function.body);
        if !uses {
            continue;
        }
        if (ast_is_reader(&function.return_type)
            || function
                .params
                .iter()
                .any(|param| ast_is_reader(&param.ty) && param.mode == ParamMode::Own))
            && !crate::stdin_stream_ops::ast_forward_signature(function)
        {
            refuse(
                diagnostics,
                "owned reader calls must forward one exact owned reader parameter",
                function.span,
            );
        }
        if crate::stdin_stream_ops::ast_forward_signature(function) {
            let mut tail = &function.body;
            while let ExprKind::Block { tail: next, .. } = &tail.kind {
                tail = next;
            }
            let owner = &function.params[0].name;
            if !matches!(&tail.kind, ExprKind::Var(name) if name == owner)
                && !crate::stdin_stream_ops::source_next_is_same_owner(program, owner, tail)
            {
                refuse(
                    diagnostics,
                    "reader forwarding tail must return its exact parameter or successor",
                    tail.span,
                );
            }
        }

        if !function.type_parameters.is_empty()
            || function.yields.is_some()
            || function.follows.is_some()
        {
            refuse(
                diagnostics,
                "streaming reader functions must be monomorphic and synchronous",
                function.span,
            );
        }
        if ast_type_uses(&function.return_type) && !ast_is_reader(&function.return_type) {
            refuse(
                diagnostics,
                "streaming reader cannot be nested in a return carrier",
                function.span,
            );
        }
        for param in &function.params {
            if ast_type_uses(&param.ty)
                && (!ast_is_reader(&param.ty)
                    || !matches!(param.mode, ParamMode::Own | ParamMode::Borrow))
            {
                refuse(
                    diagnostics,
                    "streaming reader parameters require exact own or borrow mode",
                    param.span,
                );
            }
        }
        if crate::stdin_stream_ops::pure_by_name(&function.name).is_some() {
            refuse(
                diagnostics,
                "streaming inspection function names are compiler reserved",
                function.name_span,
            );
        }
        let mut pending = vec![(&function.body, false)];
        pending.extend(function.requires.iter().map(|e| (e, true)));
        pending.extend(function.ensures.iter().map(|e| (e, true)));
        while let Some((expression, contract)) = pending.pop() {
            match &expression.kind {
                ExprKind::Call {
                    type_arguments,
                    name,
                    ..
                } => {
                    if type_arguments.iter().any(ast_type_uses) {
                        refuse(
                            diagnostics,
                            "streaming reader cannot instantiate a generic operation",
                            expression.span,
                        );
                    }
                    if contract
                        && (crate::stdin_stream_ops::pure_by_name(name).is_some()
                            || crate::stdin_stream_ops::host_by_name(name).is_some())
                    {
                        refuse(
                            diagnostics,
                            "streaming stdin operations are not admitted in contracts",
                            expression.span,
                        );
                    }
                }
                ExprKind::ConstructRecord {
                    type_name,
                    type_arguments,
                    ..
                }
                | ExprKind::ConstructVariant {
                    type_name,
                    type_arguments,
                    ..
                } => {
                    if type_name == "StdinReader" || type_arguments.iter().any(ast_type_uses) {
                        refuse(
                            diagnostics,
                            "streaming reader has no source constructor or generic carrier",
                            expression.span,
                        );
                    }
                }
                ExprKind::Closure { .. } => {
                    refuse(
                        diagnostics,
                        "streaming reader functions cannot construct closures",
                        expression.span,
                    );
                }
                ExprKind::Block { statements, .. } => {
                    for statement in statements {
                        if let Statement::Let {
                            declared: Some(ty),
                            span,
                            ..
                        } = statement
                        {
                            if ast_type_uses(ty) && !ast_is_reader(ty) {
                                refuse(
                                    diagnostics,
                                    "streaming reader local type must be exact",
                                    *span,
                                );
                            }
                        }
                    }
                }
                _ => {}
            }
            let mut index = 0;
            while let Some(child) = expression.child(index) {
                pending.push((child, contract));
                index += 1;
            }
        }
    }
    if crate::stdin_stream_ops::program_uses(program) {
        if let Err((message, span)) = open_bounds(program) {
            refuse(diagnostics, message, span);
        }
    }
}

fn open_bounds(program: &Program) -> Result<(), (&'static str, crate::ast::Span)> {
    if program.functions.len() > crate::byte_data_capacity::MAX_FUNCTIONS {
        return Err((
            "streaming admission exceeds function capacity",
            program.functions[crate::byte_data_capacity::MAX_FUNCTIONS].span,
        ));
    }
    let mut work = 0usize;
    let functions = program
        .functions
        .iter()
        .map(|f| (f.name.as_str(), f))
        .collect::<BTreeMap<_, _>>();
    let mut reverse = BTreeMap::<String, BTreeSet<&str>>::new();
    let mut relevant = BTreeSet::new();
    for function in &program.functions {
        if ast_is_reader(&function.return_type)
            || function.params.iter().any(|p| ast_is_reader(&p.ty))
            || crate::stdin_stream_ops::ast_expression_uses(&function.body)
        {
            relevant.insert(function.name.as_str());
        }
        let mut expressions = vec![&function.body];
        while let Some(expression) = expressions.pop() {
            work += 1;
            if work > crate::loan_plan::MAX_LOAN_PLAN_WORK_V1 {
                return Err(("streaming admission exceeds bounded work", expression.span));
            }
            if let ExprKind::Call { name: callee, .. } = &expression.kind {
                if functions.contains_key(callee.as_str()) {
                    reverse
                        .entry(callee.clone())
                        .or_default()
                        .insert(function.name.as_str());
                }
            }
            let mut index = 0;
            while let Some(child) = expression.child(index) {
                expressions.push(child);
                index += 1;
            }
        }
    }
    let mut pending = relevant.iter().copied().collect::<Vec<_>>();
    while let Some(callee) = pending.pop() {
        for caller in reverse.get(callee).into_iter().flatten() {
            if relevant.insert(*caller) {
                pending.push(*caller);
            }
        }
    }
    let mut summaries = BTreeMap::new();
    let mut active = BTreeSet::new();
    for function in &program.functions {
        if !relevant.contains(function.name.as_str()) {
            summaries.insert(function.name.as_str(), 0);
            continue;
        }
        let mut pending = vec![(function.name.as_str(), false)];
        while let Some((name, finishing)) = pending.pop() {
            work += 1;
            if work > crate::loan_plan::MAX_LOAN_PLAN_WORK_V1 {
                return Err(("streaming admission exceeds bounded work", function.span));
            }
            if summaries.contains_key(name) {
                continue;
            }
            let Some(function) = functions.get(name) else {
                continue;
            };
            if finishing {
                let cost = expression_open_bound(&function.body, &summaries, &mut work)?;
                if function.params.iter().any(|param| ast_is_reader(&param.ty)) && cost != 0 {
                    return Err((
                        "reader forwarding helpers cannot open a second reader",
                        function.span,
                    ));
                }
                summaries.insert(name, cost);
                active.remove(name);
                continue;
            }
            if !active.insert(name) {
                return Err(("streaming stdin call graph must be acyclic", function.span));
            }
            pending.push((name, true));
            let mut expressions = vec![&function.body];
            while let Some(expression) = expressions.pop() {
                work += 1;
                if work > crate::loan_plan::MAX_LOAN_PLAN_WORK_V1 {
                    return Err(("streaming admission exceeds bounded work", expression.span));
                }
                if let ExprKind::Call { name, .. } = &expression.kind {
                    if relevant.contains(name.as_str()) && !summaries.contains_key(name.as_str()) {
                        pending.push((name.as_str(), false));
                    }
                }
                let mut index = 0;
                while let Some(child) = expression.child(index) {
                    expressions.push(child);
                    index += 1;
                }
            }
        }
    }
    Ok(())
}
fn expression_open_bound(
    expression: &Expr,
    functions: &BTreeMap<&str, u8>,
    budget: &mut usize,
) -> Result<u8, (&'static str, crate::ast::Span)> {
    enum Work<'a> {
        Enter(&'a Expr),
        Finish(&'a Expr, usize),
    }
    let mut pending = vec![Work::Enter(expression)];
    let mut values = Vec::<u8>::new();
    while let Some(work) = pending.pop() {
        *budget += 1;
        if *budget > crate::loan_plan::MAX_LOAN_PLAN_WORK_V1 {
            return Err(("streaming admission exceeds bounded work", expression.span));
        }
        match work {
            Work::Enter(expression) => {
                let mut children = Vec::new();
                let mut index = 0;
                while let Some(child) = expression.child(index) {
                    children.push(child);
                    index += 1;
                }
                pending.push(Work::Finish(expression, children.len()));
                pending.extend(children.into_iter().rev().map(Work::Enter));
            }
            Work::Finish(expression, count) => {
                let offset = values.len() - count;
                let children = &values[offset..];
                let cost = match &expression.kind {
                    ExprKind::If { .. } if children.len() == 3 => {
                        children[0].saturating_add(children[1].max(children[2]))
                    }
                    ExprKind::Match { arms, .. } => {
                        let mut index = 1;
                        let mut guards = 0u8;
                        let mut arm_max = 0u8;
                        for arm in arms {
                            if arm.guard.is_some() {
                                guards = guards.saturating_add(children[index]);
                                index += 1;
                            }
                            arm_max = arm_max.max(children[index]);
                            index += 1;
                        }
                        children[0].saturating_add(guards).saturating_add(arm_max)
                    }
                    ExprKind::Call { name, .. } => children.iter().copied().fold(
                        if name == "stdin_stream_open" {
                            1
                        } else {
                            *functions.get(name.as_str()).unwrap_or(&0)
                        },
                        u8::saturating_add,
                    ),
                    ExprKind::Block { statements, .. } => {
                        let mut child = 0;
                        for statement in statements {
                            let count = match statement {
                                Statement::While { .. } => 2,
                                Statement::For { .. } | Statement::ForOwn { .. } => 2,
                                _ => 1,
                            };
                            if matches!(
                                statement,
                                Statement::While { .. }
                                    | Statement::For { .. }
                                    | Statement::ForOwn { .. }
                            ) && children[child..child + count]
                                .iter()
                                .any(|value| *value != 0)
                            {
                                return Err((
                                    "streaming stdin Open cannot execute in a loop",
                                    expression.span,
                                ));
                            }
                            child += count;
                        }
                        children.iter().copied().fold(0, u8::saturating_add)
                    }
                    _ => children.iter().copied().fold(0, u8::saturating_add),
                };
                if cost > 1 {
                    return Err((
                        "streaming stdin Open may execute at most once per invocation path",
                        expression.span,
                    ));
                }
                values.truncate(offset);
                values.push(cost);
            }
        }
    }
    Ok(values.pop().unwrap_or(0))
}
