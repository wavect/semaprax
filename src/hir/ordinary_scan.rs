//! Physical scratch for ordinary preflight scans; inline slots are not an admission limit.
use super::{hir_error_static, Diagnostic, ResolvedExpr, ResolvedExprKind};
use std::collections::BTreeSet;

pub(super) fn fund(bytes: Option<usize>) -> Result<(), Diagnostic> {
    match bytes {
        Some(bytes) if crate::bounded_output::reserve_active_required(bytes) => Ok(()),
        Some(_) => Err(hir_error_static(
            "ordinary HIR scan exceeds the active validation budget",
        )),
        None => {
            crate::bounded_output::refuse_active_required(usize::MAX);
            Err(hir_error_static(
                "ordinary HIR scan storage layout overflowed",
            ))
        }
    }
}

pub(super) struct Stack<T> {
    inline: [Option<T>; 16],
    len: usize,
    spill: Vec<T>,
}
impl<T> Stack<T> {
    pub(super) fn new() -> Result<Self, Diagnostic> {
        fund(Some(0))?;
        Ok(Self {
            inline: std::array::from_fn(|_| None),
            len: 0,
            spill: Vec::new(),
        })
    }
    pub(super) fn push(&mut self, value: T) -> Result<(), Diagnostic> {
        fund(Some(0))?;
        if self.spill.is_empty() && self.len < self.inline.len() {
            self.inline[self.len] = Some(value);
            self.len += 1;
        } else {
            if self.spill.len() == self.spill.capacity() {
                let Some(capacity) = self.spill.capacity().checked_mul(2).map(|n| n.max(4)) else {
                    return fund(None);
                };
                fund(capacity.checked_mul(std::mem::size_of::<T>()))?;
                // The full replacement is paid while the old backing is still live.
                let mut next = Vec::new();
                if next.try_reserve_exact(capacity).is_err() || next.capacity() != capacity {
                    crate::bounded_output::refuse_active_required(usize::MAX);
                    return Err(hir_error_static(
                        "ordinary HIR scan backing allocation failed",
                    ));
                }
                next.append(&mut self.spill);
                self.spill = next;
            }
            self.spill.push(value);
        }
        Ok(())
    }
    pub(super) fn pop(&mut self) -> Option<T> {
        if let Some(value) = self.spill.pop() {
            return Some(value);
        }
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        self.inline[self.len].take()
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = &T> {
        self.inline[..self.len]
            .iter()
            .filter_map(Option::as_ref)
            .chain(self.spill.iter())
    }
}

pub(super) fn insert<'a, K: Ord>(set: &mut BTreeSet<&'a K>, key: &'a K) -> Result<(), Diagnostic> {
    // Preserve the original single insert comparison path, including duplicates.
    fund(crate::cleanup_plan::replay_btree_node_upper::<&'a K, ()>())?;
    set.insert(key);
    Ok(())
}

pub(super) fn children<'a>(
    expression: &'a ResolvedExpr,
    push: &mut impl FnMut(&'a ResolvedExpr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    children_with(expression, push)
}

#[inline(never)]
fn children_with<'a>(
    expression: &'a ResolvedExpr,
    push: &mut dyn FnMut(&'a ResolvedExpr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Closure { captures, .. } => {
            for child in captures.iter().rev().map(|capture| &capture.value) {
                push(child)?;
            }
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            push(tail)?;
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        push(child)?;
                    }
                }
            }
        }
        ResolvedExprKind::FunctionReference { .. } => {}
        ResolvedExprKind::Invoke { callable, args } => {
            {
                for child in args.iter().rev() {
                    push(child)?;
                }
            };
            push(callable)?;
        }
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => {
            for child in args.iter().rev() {
                push(child)?;
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for child in call.args.iter().rev() {
                push(child)?;
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for child in call.args.iter().rev() {
                push(child)?;
            }
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            push(end)?;
            push(start)?;
            push(source)?;
        }
        ResolvedExprKind::LexicalExit { value, .. } => {
            if let Some(value) = value {
                push(value)?;
            }
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => push(value)?,
        ResolvedExprKind::Binary { left, right, .. } => {
            push(right)?;
            push(left)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            push(else_branch)?;
            push(then_branch)?;
            push(condition)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            {
                for child in fields.iter().rev().map(|field| &field.value) {
                    push(child)?;
                }
            };
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            for arm in arms.iter().rev() {
                push(&arm.value)?;
                if let Some(guard) = &arm.guard {
                    push(guard)?;
                }
            }
            push(scrutinee)?;
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            {
                for child in fields.iter().rev().map(|field| &field.value) {
                    push(child)?;
                }
            };
            push(base)?;
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
mod tests;
