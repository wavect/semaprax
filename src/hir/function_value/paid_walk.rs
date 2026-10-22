//! Production pending references: inline common path, prepaid fallible spill.
//! Old root pop order and the complete authored child dispatcher are retained.
use super::*;
const INLINE: usize = 32;
struct Pending<'a> {
    inline: [Option<&'a ResolvedExpr>; INLINE],
    len: usize,
    spill: Vec<&'a ResolvedExpr>,
}
fn refused() -> Diagnostic {
    super::super::hir_error_static("function-value traversal exceeds the active validation budget")
}
fn reserve(bytes: usize) -> Result<(), Diagnostic> {
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(refused())
    }
}
fn physical() -> Diagnostic {
    crate::bounded_output::refuse_active_required(usize::MAX);
    refused()
}
impl<'a> Pending<'a> {
    fn new() -> Result<Self, Diagnostic> {
        reserve(0)?;
        Ok(Self {
            inline: [None; INLINE],
            len: 0,
            spill: Vec::new(),
        })
    }
    fn push(&mut self, value: &'a ResolvedExpr) -> Result<(), Diagnostic> {
        reserve(0)?;
        if self.spill.is_empty() && self.len < INLINE {
            self.inline[self.len] = Some(value);
            self.len += 1;
            return Ok(());
        }
        if self.spill.len() == self.spill.capacity() {
            let capacity = self
                .spill
                .capacity()
                .checked_mul(2)
                .ok_or_else(physical)?
                .max(1);
            let bytes = std::alloc::Layout::array::<&ResolvedExpr>(capacity)
                .map_err(|_| physical())?
                .size();
            reserve(bytes)?;
            self.spill
                .try_reserve_exact(capacity - self.spill.len())
                .map_err(|_| physical())?;
            if self.spill.capacity() != capacity {
                let excess = self
                    .spill
                    .capacity()
                    .checked_sub(capacity)
                    .and_then(|n| n.checked_mul(std::mem::size_of::<&ResolvedExpr>()))
                    .unwrap_or(usize::MAX);
                if excess != 0 {
                    reserve(excess)?;
                }
                return Err(physical());
            }
        }
        self.spill.push(value);
        Ok(())
    }
    fn extend(
        &mut self,
        values: impl IntoIterator<Item = &'a ResolvedExpr>,
    ) -> Result<(), Diagnostic> {
        for value in values {
            self.push(value)?;
        }
        Ok(())
    }
    fn pop(&mut self) -> Option<&'a ResolvedExpr> {
        if let Some(value) = self.spill.pop() {
            return Some(value);
        }
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        self.inline[self.len].take()
    }
}
pub(super) fn roots<'a>(
    roots: impl IntoIterator<Item = &'a ResolvedExpr>,
    mut visit: impl FnMut(&'a ResolvedExpr),
) -> Result<(), Diagnostic> {
    let mut pending = Pending::new()?;
    pending.extend(roots)?;
    walk_pending(&mut pending, &mut visit)
}

#[inline(never)]
fn walk_pending<'a>(
    pending: &mut Pending<'a>,
    visit: &mut dyn FnMut(&'a ResolvedExpr),
) -> Result<(), Diagnostic> {
    while let Some(expression) = pending.pop() {
        visit(expression);
        push_children(expression, pending)?;
    }
    Ok(())
}
pub(crate) fn walk<'a>(
    function: &'a ResolvedFunction,
    visit: impl FnMut(&'a ResolvedExpr),
) -> Result<(), Diagnostic> {
    roots(
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures),
        visit,
    )
}
pub(super) fn requires(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        let mut found = matches!(
            function.return_type,
            ResolvedType::Function { .. }
                | ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64
        ) || function.params.iter().any(|p| {
            matches!(
                p.ty,
                ResolvedType::Function { .. }
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
                    | ResolvedType::MutFunctionI64
            )
        });
        walk(function, |e| {
            found |= matches!(
                e.kind,
                ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::FunctionReference { .. }
                    | ResolvedExprKind::Invoke { .. }
            )
        })?;
        if found {
            return Ok(true);
        }
    }
    for template in &program.function_templates {
        // The original initializes pending roots before this metadata shortcut.
        let mut pending = Pending::new()?;
        pending.extend(
            template
                .requires
                .iter()
                .chain(std::iter::once(&template.body))
                .chain(&template.ensures),
        )?;
        if matches!(template.return_type, ResolvedType::Function { .. })
            || template.params.iter().any(|p| {
                matches!(
                    p.ty,
                    ResolvedType::Function { .. }
                        | ResolvedType::OnceFunction
                        | ResolvedType::OnceFunctionI64
                        | ResolvedType::OnceFunctionI64Pair
                        | ResolvedType::MutFunctionI64
                )
            })
        {
            return Ok(true);
        }
        while let Some(e) = pending.pop() {
            if matches!(
                e.kind,
                ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::FunctionReference { .. }
                    | ResolvedExprKind::Invoke { .. }
            ) {
                return Ok(true);
            }
            push_children(e, &mut pending)?;
        }
    }
    Ok(false)
}
#[cfg(test)]
mod tests;

fn push_children<'a>(
    expression: &'a ResolvedExpr,
    pending: &mut Pending<'a>,
) -> Result<(), Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Closure { captures, .. } => {
            pending.extend(captures.iter().rev().map(|capture| &capture.value))?
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            pending.push(tail)?;
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        pending.push(child)?;
                    }
                }
            }
        }
        ResolvedExprKind::FunctionReference { .. } => {}
        ResolvedExprKind::Invoke { callable, args } => {
            pending.extend(args.iter().rev())?;
            pending.push(callable)?;
        }
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => pending.extend(args.iter().rev())?,
        ResolvedExprKind::NativeRustImportCall(call) => pending.extend(call.args.iter().rev())?,
        ResolvedExprKind::HostCommandCall(call) => pending.extend(call.args.iter().rev())?,
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            pending.push(end)?;
            pending.push(start)?;
            pending.push(source)?;
        }
        ResolvedExprKind::LexicalExit { value, .. } => {
            if let Some(value) = value {
                pending.push(value)?;
            }
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => pending.push(value)?,
        ResolvedExprKind::Binary { left, right, .. } => {
            pending.push(right)?;
            pending.push(left)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            pending.push(else_branch)?;
            pending.push(then_branch)?;
            pending.push(condition)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value))?;
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            for arm in arms.iter().rev() {
                pending.push(&arm.value)?;
                if let Some(guard) = &arm.guard {
                    pending.push(guard)?;
                }
            }
            pending.push(scrutinee)?;
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            pending.extend(fields.iter().rev().map(|field| &field.value))?;
            pending.push(base)?;
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
        | ResolvedExprKind::BorrowedUtf8Literal(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
use crate::hir::{function_value, push_resolved_expression_children_in_authored_order};
