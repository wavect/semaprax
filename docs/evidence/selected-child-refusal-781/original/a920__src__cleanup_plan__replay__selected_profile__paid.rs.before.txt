//! Paid selected-local static refusals; arbitrary child Diagnostics stay explicit.
use super::super::selected_context::paid::refusal;
use super::{
    DEPTH, Frame, OwnershipMode, ReplayBudget, ResolvedExprKind, ResolvedFunction,
    ResolvedMatchPattern, ResolvedType, collections, expression_facts, flat_records, row_loan,
    string_transfer,
};
pub(in super::super) fn validate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), expression_facts::errors::EngineError> {
    #[cfg(test)]
    if let Some(lease) = program.borrowed_helper_lease(function) {
        return lease
            .validate(program, function, budget)
            .map_err(Into::into);
    }
    if !budget.selected() {
        return Ok(());
    }
    flat_records::module(program, function, budget)?;
    if matches!(function.return_type, ResolvedType::Nominal { .. }) {
        collections::prepay(
            program,
            function,
            collections::type_key_bytes(&function.return_type)?,
            budget,
        )?;
    }
    if !program.interfaces().is_empty()
        || !program.function_templates().is_empty()
        || !program.function_instances().is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
        || !flat_records::admitted(program, &function.return_type)?
    {
        return Err(refusal(
            function,
            budget,
            "selected lexical replay requires a module-local primitive context",
        ));
    }
    for parameter in &function.params {
        if matches!(parameter.ty, ResolvedType::Nominal { .. }) {
            collections::prepay(
                program,
                function,
                collections::type_key_bytes(&parameter.ty)?,
                budget,
            )?;
        }
        if !flat_records::admitted(program, &parameter.ty)? {
            return Err(refusal(
                function,
                budget,
                "selected parameter type is outside direct records",
            ));
        }
    }
    budget.reserve_array::<Frame<'_>>(function, DEPTH, "selected primitive profile cursor")?;
    let mut frames = [None; DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        frames[0] = Some((crate::hir::lexical_exit::ChildCursor::new(root), false));
        let mut depth = 1;
        while depth != 0 {
            budget.charge(function, 1, "selected primitive profile node")?;
            let (cursor, entered) = frames[depth - 1]
                .ok_or_else(|| refusal(function, budget, "selected profile cursor is empty"))?;
            let expression = cursor.expression();
            if !entered {
                if matches!(expression.ty, ResolvedType::Nominal { .. }) {
                    collections::prepay(
                        program,
                        function,
                        collections::type_key_bytes(&expression.ty)?,
                        budget,
                    )?;
                }
                if expression.ty != ResolvedType::Never
                    && !flat_records::admitted(program, &expression.ty)?
                {
                    return Err(refusal(function, budget, "selected lexical replay does not admit nominal or callable expression storage"));
                }
                if let ResolvedExprKind::Project { base, .. } = &expression.kind {
                    collections::prepay(
                        program,
                        function,
                        collections::type_key_bytes(&base.ty)?,
                        budget,
                    )?;
                }
                flat_records::expression(program, function, expression)?;
                match &expression.kind {
                    ResolvedExprKind::ConstructVariant { .. }
                    | ResolvedExprKind::UpdateRecord { .. }
                    | ResolvedExprKind::Upcast { .. }
                    | ResolvedExprKind::LiteralFormat { .. }
                    | ResolvedExprKind::Try { .. }
                    | ResolvedExprKind::TryOption { .. }
                    | ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::Invoke { .. }
                    | ResolvedExprKind::FunctionReference { .. }
                    | ResolvedExprKind::NativeRustImportCall(_)
                    | ResolvedExprKind::HostCommandCall(_)
                    | ResolvedExprKind::Yield { .. } => {
                        return Err(refusal(function, budget, "selected lexical replay provider is outside the reserved primitive profile"));
                    }
                    ResolvedExprKind::VecFieldRead { .. } => {
                        collections::field(program, function, expression, budget)?
                    }
                    ResolvedExprKind::RowLoan { .. } => {
                        row_loan::validate(program, function, expression, budget)?
                    }
                    ResolvedExprKind::BorrowedUtf8Literal(value) => {
                        if expression.ty != ResolvedType::Str
                            || expression.ownership != OwnershipMode::Borrow
                            || !value.is_backed()
                        {
                            return Err(refusal(
                                function,
                                budget,
                                "selected static literal has invalid type, ownership or backing",
                            ));
                        }
                        crate::hir::static_text::validate_literal(value)?;
                    }
                    ResolvedExprKind::Block { statements, .. }
                    | ResolvedExprKind::VecConstructor { statements, .. } => {
                        budget.charge(
                            function,
                            statements.len(),
                            "selected primitive statements",
                        )?;
                        for statement in statements {
                            if let crate::hir::ResolvedStatement::Let { binding, .. }
                            | crate::hir::ResolvedStatement::Assign { binding, .. } = statement
                            {
                                if matches!(binding.ty, ResolvedType::Nominal { .. }) {
                                    collections::prepay(
                                        program,
                                        function,
                                        collections::type_key_bytes(&binding.ty)?,
                                        budget,
                                    )?;
                                }
                            }
                            match statement {
                                crate::hir::ResolvedStatement::Let { binding, .. }
                                | crate::hir::ResolvedStatement::Assign { binding, .. }
                                    if !flat_records::admitted(program, &binding.ty)? =>
                                {
                                    return Err(refusal(
                                        function,
                                        budget,
                                        "selected block binding requires unsupported storage",
                                    ));
                                }
                                crate::hir::ResolvedStatement::Assign {
                                    field: Some(_), ..
                                } => {
                                    return Err(refusal(function, budget, "selected primitive assignment cannot target a nominal field"));
                                }
                                crate::hir::ResolvedStatement::LexicalWhile {
                                    condition,
                                    body,
                                    latch,
                                    ..
                                } => {
                                    if matches!(
                                        latch,
                                        crate::hir::LoopLatchKind::OwnedIteratorStep { .. }
                                            | crate::hir::LoopLatchKind::SourceForStep {
                                                kind: crate::hir::SourceForKind::OwnedIterator,
                                                ..
                                            }
                                    ) {
                                        return Err(refusal(function, budget,"selected lexical replay owning iterator latch remains closed"));
                                    }
                                    if matches!(
                                        latch,
                                        crate::hir::LoopLatchKind::SourceForStep {
                                            kind: crate::hir::SourceForKind::Copy,
                                            ..
                                        }
                                    ) {
                                        let ResolvedExprKind::Block { statements, .. } = &body.kind
                                        else {
                                            return Err(refusal(
                                                function,
                                                budget,
                                                "selected Copy-for body is absent",
                                            ));
                                        };
                                        let Some(crate::hir::ResolvedStatement::Let {
                                            binding,
                                            ..
                                        }) = statements.first()
                                        else {
                                            return Err(refusal(
                                                function,
                                                budget,
                                                "selected Copy-for item is absent",
                                            ));
                                        };
                                        if !crate::vec_ops::resolved_element_is_admitted(
                                            &binding.ty,
                                        ) {
                                            return Err(refusal(function, budget,"selected Copy-for item is outside its scalar family"));
                                        }
                                        // Ordinary latch validation constructs exactly one Vec type
                                        // (one argument and its owned declaration spelling).
                                        budget.reserve_array::<ResolvedType>(
                                            function,
                                            1,
                                            "selected Copy-for latch Vec argument",
                                        )?;
                                        budget.reserve_bytes(
                                            function,
                                            crate::prelude::VEC_ID.len(),
                                            "selected Copy-for latch Vec identity",
                                        )?;
                                    }
                                    crate::hir::lexical_exit::validate_latch(
                                        condition, body, latch,
                                    )?;
                                }
                                crate::hir::ResolvedStatement::Assign {
                                    binding, value, ..
                                } if binding.ty == ResolvedType::String => {
                                    collections::append(function, binding, value, budget)?;
                                }
                                crate::hir::ResolvedStatement::Unsafe { .. } => {
                                    return Err(refusal(
                                        function,
                                        budget,
                                        "selected lexical replay has an unsafe statement",
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                    ResolvedExprKind::Match {
                        scrutinee, arms, ..
                    } => {
                        if matches!(scrutinee.ty, ResolvedType::Nominal { .. }) {
                            return Err(refusal(function, budget, "selected direct records do not admit owning or destructuring matches"));
                        }
                        for arm in arms {
                            validate_pattern(function, &arm.pattern, budget, 0)?;
                        }
                    }
                    ResolvedExprKind::Call {
                        callee,
                        instance,
                        type_arguments,
                        ..
                    } => {
                        string_transfer::operand(function, expression, budget)?;
                        if collections::call(program, function, expression, budget)? {
                            frames[depth - 1] = Some((cursor, true));
                            continue;
                        }
                        if instance.is_some() || !type_arguments.is_empty() {
                            return Err(refusal(
                                function,
                                budget,
                                "selected lexical replay call needs an unsupported provider",
                            ));
                        }
                        if crate::byte_ops::by_id(callee.as_str()).is_none()
                            && crate::string_ops::by_id(callee.as_str()).is_none()
                            && crate::str_ops::by_id(callee.as_str()).is_none()
                        {
                            budget.charge(
                                function,
                                program.function_count(),
                                "selected current callee lookup",
                            )?;
                            if program.resolve_call_target(callee, None).is_none() {
                                return Err(refusal(
                                    function,
                                    budget,
                                    "selected lexical replay call has no current primitive target",
                                ));
                            }
                        }
                    }
                    _ => {}
                }
                frames[depth - 1] = Some((cursor, true));
                continue;
            }
            // The entry census prepays block statement boundaries. Saved
            // cursor progress makes each advance (including None) constant.
            let mut cursor = cursor;
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Err(refusal(
                        function,
                        budget,
                        "selected primitive profile exceeds ordinary HIR depth",
                    ));
                }
                frames[depth - 1] = Some((cursor, true));
                frames[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(())
}

// Match bindings remain on the frozen primitive pattern boundary. Direct
// records are admitted as ordinary construction/calls/projection, not patterns.
fn primitive(ty: &ResolvedType) -> bool {
    crate::hir::is_scalar_resolved_type(ty)
        || matches!(
            ty,
            ResolvedType::Bytes
                | ResolvedType::String
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::ArrayU8(_)
        )
}

fn validate_pattern(
    function: &ResolvedFunction,
    pattern: &ResolvedMatchPattern,
    budget: &mut ReplayBudget,
    depth: usize,
) -> Result<(), expression_facts::errors::EngineError> {
    budget.charge(function, 1, "selected primitive match pattern")?;
    if depth > 256 {
        return Err(refusal(
            function,
            budget,
            "selected match pattern exceeds depth",
        ));
    }
    match pattern {
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => Ok(()),
        ResolvedMatchPattern::Binding(binding) if primitive(&binding.ty) => Ok(()),
        ResolvedMatchPattern::Or(alternatives) => {
            for pattern in alternatives {
                validate_pattern(function, pattern, budget, depth + 1)?;
            }
            Ok(())
        }
        ResolvedMatchPattern::Binding(_)
        | ResolvedMatchPattern::Variant { .. }
        | ResolvedMatchPattern::Record { .. } => Err(refusal(
            function,
            budget,
            "selected match pattern requires unsupported storage",
        )),
    }
}
