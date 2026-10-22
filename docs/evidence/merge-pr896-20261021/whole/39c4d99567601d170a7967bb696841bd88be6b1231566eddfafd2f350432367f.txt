//! Original reverse-stack lookup order, now admitted before stack growth.
//! Each fresh helper frame includes its actual intermediate Option and terminal
//! budget conversion argument/result roles in caller and existing From callee.
//! A terminal budget failure uses those four slots once; text retains its
//! original funded provenance. The initial invoice failure is unreceived ABI.
use super::*;
use crate::hir::{
    ResolvedClosureCapture, ResolvedExpr, ResolvedExprKind as K, ResolvedFieldInitializer,
    ResolvedMatchArm, ResolvedStatement,
};
type Roots<'a> = std::iter::Chain<
    std::iter::Chain<std::slice::Iter<'a, ResolvedExpr>, std::iter::Once<&'a ResolvedExpr>>,
    std::slice::Iter<'a, ResolvedExpr>,
>;
pub(super) fn find<'a>(
    function: &'a ResolvedFunction,
    id: &ExpressionId,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a ResolvedExpr>, Error> {
    receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &ExpressionId,
            &mut ReplayBudget,
            Result<Option<&ResolvedExpr>, Error>,
            Vec<&ResolvedExpr>,
            Roots<'_>,
            usize,
            Option<&ResolvedExpr>,
            Diagnostic,
            Error,
            Diagnostic,
            Error
        ),
        12,
        "selected renewal expression lookup formal and roots receiving",
        ()
    );
    let root_count = function
        .requires
        .len()
        .saturating_add(1)
        .saturating_add(function.ensures.len());
    budget.reserve_array::<&ResolvedExpr>(function, root_count, "selected renewal lookup roots")?;
    let mut stack = Vec::with_capacity(root_count);
    let mut roots = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures);
    while let Some(root) = dynamic_next!(
        budget,
        function,
        roots,
        Roots<'_>,
        &ResolvedExpr,
        (&ResolvedExpr),
        3
    ) {
        stack.push(root);
    }
    while let Some(expression) = receive!(
        budget,
        function,
        (
            &mut Vec<&ResolvedExpr>,
            Option<&ResolvedExpr>,
            &ResolvedExpr
        ),
        3,
        "selected renewal lookup stack pop receiving",
        stack.pop()
    ) {
        if receive!(
            budget,
            function,
            bool,
            1,
            "selected renewal lookup identity comparison receiving",
            expression.id == *id
        ) {
            return Ok(Some(expression));
        }
        receive!(
            budget,
            function,
            usize,
            1,
            "selected renewal lookup child index receiving",
            ()
        );
        let mut index = 0usize;
        while let Some(child) = receive!(
            budget,
            function,
            (
                &ResolvedFunction,
                &ResolvedExpr,
                usize,
                &mut ReplayBudget,
                Result<Option<&ResolvedExpr>, Error>,
                &ResolvedExpr
            ),
            6,
            "selected renewal child lookup caller receiving",
            child(function, expression, index, budget)
        )? {
            budget.reserve_vec(
                function,
                &mut stack,
                1,
                "selected renewal lookup child stack",
            )?;
            stack.push(child);
            index += 1;
        }
    }
    Ok(None)
}
macro_rules! get {
    ($budget:ident,$function:ident,$values:expr,$index:expr,$item:ty) => {
        dynamic_get!(
            $budget,
            $function,
            $values,
            &[$item],
            $index,
            usize,
            Option<&$item>,
            (&$item),
            4
        )
    };
}
fn child<'a>(
    function: &ResolvedFunction,
    expression: &'a ResolvedExpr,
    index: usize,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a ResolvedExpr>, Error> {
    receive!(
        budget,
        function,
        (
            &ResolvedFunction,
            &ResolvedExpr,
            usize,
            &mut ReplayBudget,
            Result<Option<&ResolvedExpr>, Error>,
            Option<&ResolvedExpr>,
            Diagnostic,
            Error,
            Diagnostic,
            Error
        ),
        10,
        "selected renewal child lookup formal receiving",
        ()
    );
    // This compatibility query has no allocation for admitted selected shapes;
    // affine Closure/Invoke are rejected by the earlier closed source profile.
    if let Some((_, args)) = receive!(
        budget,
        function,
        (
            &ResolvedExpr,
            Option<(&'static DeclarationId, &[ResolvedExpr])>,
            &[ResolvedExpr]
        ),
        3,
        "selected renewal affine child query receiving",
        crate::hir::closure::once::call(expression)
    ) {
        return Ok(get!(budget, function, args, index, ResolvedExpr));
    }
    Ok(match &expression.kind {
        K::LexicalExit { value, .. } => {
            if index == 0 {
                receive!(
                    budget,
                    function,
                    (&Option<Box<ResolvedExpr>>, Option<&ResolvedExpr>),
                    2,
                    "selected renewal lexical child option receiving",
                    value.as_deref()
                )
            } else {
                None
            }
        }
        K::Closure { captures, .. } => {
            match get!(budget, function, captures, index, ResolvedClosureCapture) {
                Some(capture) => Some(&capture.value),
                None => None,
            }
        }
        K::FunctionReference { .. } => None,
        K::Invoke { args, .. }
        | K::Call { args, .. }
        | K::LiteralFormat { args, .. }
        | K::VecFieldRead { args, .. } => get!(budget, function, args, index, ResolvedExpr),
        K::NativeRustImportCall(call) => get!(budget, function, call.args, index, ResolvedExpr),
        K::HostCommandCall(call) => get!(budget, function, call.args, index, ResolvedExpr),
        K::ByteRange {
            source, start, end, ..
        } => {
            receive!(
                budget,
                function,
                [&ResolvedExpr; 3],
                1,
                "selected renewal byte child array receiving",
                ()
            );
            get!(
                budget,
                function,
                [source.as_ref(), start.as_ref(), end.as_ref()],
                index,
                &ResolvedExpr
            )
            .copied()
        }
        K::Unary { value, .. }
        | K::Try { operand: value, .. }
        | K::TryOption { operand: value, .. }
        | K::Project { base: value, .. }
        | K::Upcast { source: value }
        | K::Yield { request: value } => {
            if index == 0 {
                Some(value)
            } else {
                None
            }
        }
        K::Binary { left, right, .. } => {
            receive!(
                budget,
                function,
                [&ResolvedExpr; 2],
                1,
                "selected renewal binary child array receiving",
                ()
            );
            get!(
                budget,
                function,
                [left.as_ref(), right.as_ref()],
                index,
                &ResolvedExpr
            )
            .copied()
        }
        K::Block { statements, tail }
        | K::VecConstructor {
            statements, tail, ..
        } => {
            receive!(
                budget,
                function,
                (usize, std::slice::Iter<'_, ResolvedStatement>),
                2,
                "selected renewal block child cursor receiving",
                ()
            );
            let mut offset = 0usize;
            let mut entries = statements.iter();
            while let Some(statement) = dynamic_next!(
                budget,
                function,
                entries,
                std::slice::Iter<'_, ResolvedStatement>,
                &ResolvedStatement,
                (&ResolvedStatement),
                3
            ) {
                let count = receive!(
                    budget,
                    function,
                    (&ResolvedStatement, usize),
                    2,
                    "selected renewal statement child count receiving",
                    statement.child_count()
                );
                if index < offset + count {
                    return Ok(receive!(
                        budget,
                        function,
                        (&ResolvedStatement, usize, Option<&ResolvedExpr>),
                        3,
                        "selected renewal statement child receiving",
                        statement.child(index - offset)
                    ));
                }
                offset += count;
            }
            if index == offset {
                Some(tail)
            } else {
                None
            }
        }
        K::If {
            condition,
            then_branch,
            else_branch,
        } => {
            receive!(
                budget,
                function,
                [&ResolvedExpr; 3],
                1,
                "selected renewal branch child array receiving",
                ()
            );
            get!(
                budget,
                function,
                [
                    condition.as_ref(),
                    then_branch.as_ref(),
                    else_branch.as_ref()
                ],
                index,
                &ResolvedExpr
            )
            .copied()
        }
        K::ConstructRecord { fields, .. } | K::ConstructVariant { fields, .. } => {
            match get!(budget, function, fields, index, ResolvedFieldInitializer) {
                Some(field) => Some(&field.value),
                None => None,
            }
        }
        K::Match {
            scrutinee, arms, ..
        } => {
            if index == 0 {
                Some(scrutinee.as_ref())
            } else {
                receive!(
                    budget,
                    function,
                    (
                        usize,
                        std::slice::Iter<'_, ResolvedMatchArm>,
                        &Box<ResolvedExpr>
                    ),
                    3,
                    "selected renewal match child cursor receiving",
                    ()
                );
                let mut cursor = index - 1;
                let mut entries = arms.iter();
                while let Some(arm) = dynamic_next!(
                    budget,
                    function,
                    entries,
                    std::slice::Iter<'_, ResolvedMatchArm>,
                    &ResolvedMatchArm,
                    (&ResolvedMatchArm),
                    3
                ) {
                    if let Some(guard) = &arm.guard {
                        if cursor == 0 {
                            return Ok(Some(guard.as_ref()));
                        }
                        cursor -= 1;
                    }
                    if cursor == 0 {
                        return Ok(Some(&arm.value));
                    }
                    cursor -= 1;
                }
                None
            }
        }
        K::UpdateRecord { base, fields, .. } => {
            if index == 0 {
                Some(base.as_ref())
            } else {
                match get!(
                    budget,
                    function,
                    fields,
                    index - 1,
                    ResolvedFieldInitializer
                ) {
                    Some(field) => Some(&field.value),
                    None => None,
                }
            }
        }
        K::Int(_)
        | K::Int32(_)
        | K::Char(_)
        | K::Uint8(_)
        | K::Usize(_)
        | K::ArrayU8(_)
        | K::RepeatArrayU8 { .. }
        | K::Float32(_)
        | K::Float64(_)
        | K::Bool(_)
        | K::String(_)
        | K::BorrowedUtf8Literal(_)
        | K::Place(_)
        | K::BorrowPlace { .. }
        | K::RowLoan { .. } => None,
    })
}
