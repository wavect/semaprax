//! Independent typed contour replay before cleanup certificates may authorize exits.
use super::{child, contains, validate_latch, validate_owner};
use crate::diagnostic::Diagnostic;
use crate::hir::*;
fn error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}
fn bottom(expression: &ResolvedExpr) -> bool {
    match &expression.kind {
        ResolvedExprKind::LexicalExit { .. } => true,
        ResolvedExprKind::Block { tail, .. } | ResolvedExprKind::VecConstructor { tail, .. } => {
            tail.ty == ResolvedType::Never
        }
        ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } => then_branch.ty == ResolvedType::Never && else_branch.ty == ResolvedType::Never,
        ResolvedExprKind::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|a| a.value.ty == ResolvedType::Never)
        }
        _ => false,
    }
}
/// Reachable guard completion is exactly the existing scalar discard zero.
fn guard_completion(expression: &ResolvedExpr, depth: usize) -> bool {
    if depth > crate::cache_codec::MAX_DEPTH {
        return false;
    }
    match &expression.kind {
        ResolvedExprKind::LexicalExit { .. } => expression.ty == ResolvedType::Never,
        ResolvedExprKind::Int(0) => {
            expression.ty == ResolvedType::I64 && expression.ownership == OwnershipMode::Value
        }
        ResolvedExprKind::Block { tail, .. } | ResolvedExprKind::VecConstructor { tail, .. } => {
            guard_completion(tail, depth + 1)
        }
        ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } => guard_completion(then_branch, depth + 1) && guard_completion(else_branch, depth + 1),
        ResolvedExprKind::Match { arms, .. } => {
            !arms.is_empty() && arms.iter().all(|a| guard_completion(&a.value, depth + 1))
        }
        _ => false,
    }
}
fn no_read(expression: &ResolvedExpr, id: &ValueId, depth: usize) -> bool {
    if depth > crate::cache_codec::MAX_DEPTH {
        return false;
    }
    if matches!(&expression.kind,ResolvedExprKind::Place(p)|ResolvedExprKind::BorrowPlace{place:p,..} if &p.root==id)
    {
        return false;
    }
    let mut index = 0;
    while let Some(next) = child(expression, index) {
        if !no_read(next, id, depth + 1) {
            return false;
        }
        index += 1;
    }
    true
}
fn storage_has_bottom(ty: &ResolvedType, depth: usize) -> bool {
    if depth > crate::cache_codec::MAX_DEPTH {
        return true;
    }
    match ty {
        ResolvedType::Never => true,
        ResolvedType::Function { parameters, result } => {
            parameters.iter().any(|t| storage_has_bottom(t, depth + 1))
                || storage_has_bottom(result, depth + 1)
        }
        ResolvedType::Nominal { arguments, .. } => {
            arguments.iter().any(|t| storage_has_bottom(t, depth + 1))
        }
        _ => false,
    }
}
pub(crate) fn validate_function(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    owner: &FunctionExecutionId,
) -> Result<(), Diagnostic> {
    validate_function_with(program, function, owner, &mut |program, region| {
        crate::loan_plan::traversal::inspect(program, region).map(|proof| proof.is_some())
    })
}

pub(crate) fn validate_function_with<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    owner: &FunctionExecutionId,
    inspect: &mut dyn FnMut(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedExpr,
    ) -> Result<bool, Diagnostic>,
) -> Result<(), Diagnostic> {
    validate_owner(function, owner)?;
    if storage_has_bottom(&function.return_type, 0)
        || function.params.iter().any(|p| storage_has_bottom(&p.ty, 0))
    {
        return Err(error(
            "lexical completion has no public signature or storage type",
        ));
    }
    // Independently scan every root before declining the selective contour.
    // Forged bottom types and typed loop markers still select full replay.
    if !super::walk::requires_validation(function)? {
        return Ok(());
    }
    crate::hir::validation::byte_traversal::nested::slot::controls(
        std::mem::size_of::<Proofs<'_, '_>>()
            + std::mem::size_of::<Result<Proofs<'_, '_>, Diagnostic>>(),
    )?;
    let mut proofs = Proofs {
        bytes: crate::hir::ByteTraversalSlot::new(program, function)?,
        inspect,
    };
    for contract in function.requires.iter().chain(&function.ensures) {
        walk(
            program,
            function,
            owner,
            contract,
            false,
            None,
            None,
            0,
            &mut proofs,
        )?;
    }
    walk(
        program,
        function,
        owner,
        &function.body,
        function.yields.is_none(),
        None,
        None,
        0,
        &mut proofs,
    )
}
struct Proofs<'a, 'b> {
    bytes: crate::hir::ByteTraversalSlot<'a>,
    inspect: &'b mut dyn FnMut(
        &dyn crate::cleanup::ReplayContext,
        &ResolvedExpr,
    ) -> Result<bool, Diagnostic>,
}
fn walk<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    owner: &FunctionExecutionId,
    expression: &'a ResolvedExpr,
    route: bool,
    nearest: Option<&'a ExpressionId>,
    allowed: Option<&'a ResolvedExpr>,
    depth: usize,
    inspect: &mut Proofs<'a, '_>,
) -> Result<(), Diagnostic> {
    if depth > crate::cache_codec::MAX_DEPTH {
        return Err(error("lexical contour exceeds admitted HIR depth"));
    }
    let route = route || allowed.is_some_and(|a| std::ptr::eq(a, expression));
    if (expression.ty == ResolvedType::Never) != bottom(expression) {
        return Err(error(
            "lexical completion type differs from independently replayed reachability",
        ));
    }
    if expression.ty == ResolvedType::Never && expression.ownership != OwnershipMode::Value {
        return Err(error(
            "lexical completion cannot carry ownership or runtime storage",
        ));
    }
    match &expression.kind {
        ResolvedExprKind::LexicalExit {
            kind,
            target,
            value,
        } => {
            if !route {
                return Err(error(
                    "lexical exit crosses a value-evaluation or callable boundary",
                ));
            }
            match (kind, target, value) {
                (
                    crate::ast::LexicalExitKind::Return,
                    ControlTarget::Function(actual),
                    Some(operand),
                ) if actual == owner
                    && operand.ty == function.return_type
                    && operand.ty != ResolvedType::Never =>
                {
                    walk(
                        program,
                        function,
                        owner,
                        operand,
                        false,
                        nearest,
                        None,
                        depth + 1,
                        inspect,
                    )?;
                }
                (
                    crate::ast::LexicalExitKind::Break | crate::ast::LexicalExitKind::Continue,
                    ControlTarget::Loop {
                        owner: actual,
                        site,
                    },
                    None,
                ) if actual == owner && nearest == Some(site) => {}
                _ => {
                    return Err(error(
                        "lexical exit kind, value or exact nearest target is inconsistent",
                    ))
                }
            }
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            for (index, statement) in statements.iter().enumerate() {
                match statement {
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        if contains(body) {
                            return Err(error(
                                "legacy while cannot contain lexical exit authority",
                            ));
                        }
                        walk(
                            program,
                            function,
                            owner,
                            condition,
                            false,
                            nearest,
                            allowed,
                            depth + 1,
                            inspect,
                        )?;
                        walk(
                            program,
                            function,
                            owner,
                            body,
                            route,
                            Some(&condition.id),
                            allowed,
                            depth + 1,
                            inspect,
                        )?;
                    }
                    ResolvedStatement::LexicalWhile {
                        condition,
                        body,
                        latch,
                        ..
                    } => {
                        if !contains(body) {
                            return Err(error("lexical loop profile has no explicit exit"));
                        }
                        validate_latch(condition, body, latch)?;
                        walk(
                            program,
                            function,
                            owner,
                            condition,
                            false,
                            nearest,
                            allowed,
                            depth + 1,
                            inspect,
                        )?;
                        walk(
                            program,
                            function,
                            owner,
                            body,
                            route,
                            Some(&condition.id),
                            allowed,
                            depth + 1,
                            inspect,
                        )?;
                    }
                    ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        ..
                    } => {
                        if let Some(wrapper) = super::wrapper::source_for_wrapper_with_bytes(
                            program,
                            value,
                            Some(&mut inspect.bytes),
                            inspect.inspect,
                        )? {
                            if *mutable
                                || binding.ty != value.ty
                                || binding.ownership != OwnershipMode::Value
                            {
                                return Err(error(
                                    "source-for wrapper discard binding is inconsistent",
                                ));
                            }
                            walk(
                                program,
                                function,
                                owner,
                                value,
                                route,
                                nearest,
                                Some(wrapper.authored_body),
                                depth + 1,
                                inspect,
                            )?;
                        } else {
                            let guard = contains(value)
                                && matches!(
                                    value.kind,
                                    ResolvedExprKind::If { .. } | ResolvedExprKind::Match { .. }
                                )
                                && !*mutable
                                && binding.ty == ResolvedType::I64
                                && binding.ownership == OwnershipMode::Value
                                && value.ty == binding.ty
                                && value.ownership == binding.ownership
                                && guard_completion(value, 0)
                                && statements[index + 1..].iter().all(|s| {
                                    (0..s.child_count()).all(|i| {
                                        s.child(i).is_none_or(|e| no_read(e, &binding.id, 0))
                                    })
                                })
                                && no_read(tail, &binding.id, 0);
                            if binding.ty == ResolvedType::Never
                                && !allowed.is_some_and(|a| std::ptr::eq(a, value))
                            {
                                return Err(error("lexical completion cannot initialize storage"));
                            }
                            walk(
                                program,
                                function,
                                owner,
                                value,
                                route && guard,
                                nearest,
                                allowed,
                                depth + 1,
                                inspect,
                            )?;
                        }
                    }
                    ResolvedStatement::CopyDiscard { value, .. } => {
                        super::super::copy_discard::require(value)?;
                        walk(
                            program,
                            function,
                            owner,
                            value,
                            route,
                            nearest,
                            allowed,
                            depth + 1,
                            inspect,
                        )?;
                    }
                    ResolvedStatement::Assign { value, .. } => walk(
                        program,
                        function,
                        owner,
                        value,
                        false,
                        nearest,
                        allowed,
                        depth + 1,
                        inspect,
                    )?,
                    ResolvedStatement::Unsafe { body, .. } => walk(
                        program,
                        function,
                        owner,
                        body,
                        false,
                        nearest,
                        None,
                        depth + 1,
                        inspect,
                    )?,
                }
            }
            walk(
                program,
                function,
                owner,
                tail,
                route,
                nearest,
                allowed,
                depth + 1,
                inspect,
            )?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            walk(
                program,
                function,
                owner,
                condition,
                false,
                nearest,
                allowed,
                depth + 1,
                inspect,
            )?;
            walk(
                program,
                function,
                owner,
                then_branch,
                route,
                nearest,
                allowed,
                depth + 1,
                inspect,
            )?;
            walk(
                program,
                function,
                owner,
                else_branch,
                route,
                nearest,
                allowed,
                depth + 1,
                inspect,
            )?;
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            walk(
                program,
                function,
                owner,
                scrutinee,
                false,
                nearest,
                allowed,
                depth + 1,
                inspect,
            )?;
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    walk(
                        program,
                        function,
                        owner,
                        guard,
                        false,
                        nearest,
                        None,
                        depth + 1,
                        inspect,
                    )?;
                }
                walk(
                    program,
                    function,
                    owner,
                    &arm.value,
                    route,
                    nearest,
                    allowed,
                    depth + 1,
                    inspect,
                )?;
            }
        }
        ResolvedExprKind::Closure { captures, body, .. } => {
            for capture in captures {
                walk(
                    program,
                    function,
                    owner,
                    &capture.value,
                    false,
                    nearest,
                    None,
                    depth + 1,
                    inspect,
                )?;
            }
            walk(
                program,
                function,
                owner,
                body,
                false,
                None,
                None,
                depth + 1,
                inspect,
            )?;
        }
        _ => {
            let mut index = 0;
            while let Some(next) = child(expression, index) {
                walk(
                    program,
                    function,
                    owner,
                    next,
                    false,
                    nearest,
                    allowed,
                    depth + 1,
                    inspect,
                )?;
                index += 1;
            }
        }
    }
    Ok(())
}
