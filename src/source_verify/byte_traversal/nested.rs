//! Nested byte-only admission borrows the existing paid verifier frames and
//! the OUTERMOST traversal entry scope. No new clone, map, frame or allowance.
use crate::ast::{Expr, ExprKind, Statement, Type};
use crate::source_verify::binding::Availability;
use crate::source_verify::scope::VerifierFrame;
use crate::source_verify::IterativeVerifier;

impl<'a, 'v, 'p> IterativeVerifier<'a, 'v, 'p> {
    /// None also covers mixed source-for ancestry, including through while.
    pub(in crate::source_verify) fn active_byte_entry_scope(&self) -> Option<usize> {
        let mut outer = None;
        let boundary = self
            .frames
            .iter()
            .rposition(|frame| matches!(frame, VerifierFrame::ResumeClosure { .. }))
            .map_or(0, |index| index + 1);
        for frame in &self.frames[boundary..] {
            if let VerifierFrame::ResumeForBody {
                statements,
                index,
                block_scope,
                source,
                ..
            } = frame
            {
                if !matches!(statements[*index], Statement::For { .. })
                    || !source.is_some_and(|source| {
                        self.scopes[*block_scope]
                            .bindings
                            .get(source)
                            .is_some_and(|binding| binding.ty == Type::SliceU8 && !binding.mutable)
                    })
                {
                    return None;
                }
                outer.get_or_insert(*block_scope);
            }
        }
        outer
    }
    pub(in crate::source_verify) fn byte_body_entry_scope(
        &self,
        current_scope: usize,
        byte_source: bool,
    ) -> Option<usize> {
        if !byte_source {
            return None;
        }
        // Any enclosing for must itself be byte-only. A while preserves this
        // ancestry rather than manufacturing a byte admission context.
        let boundary = self
            .frames
            .iter()
            .rposition(|frame| matches!(frame, VerifierFrame::ResumeClosure { .. }))
            .map_or(0, |index| index + 1);
        if self.frames[boundary..]
            .iter()
            .any(|frame| matches!(frame, VerifierFrame::ResumeForBody { .. }))
        {
            self.active_byte_entry_scope()
        } else {
            Some(current_scope)
        }
    }
    pub(in crate::source_verify) fn nested_byte_source(&self, scope: usize, source: &Expr) -> bool {
        let ExprKind::Var(name) = &source.kind else {
            return false;
        };
        self.scopes[scope]
            .bindings
            .get(name)
            .is_some_and(|binding| {
                binding.ty == Type::SliceU8
                    && !binding.mutable
                    && binding.availability == Availability::Available
                    && binding.traversal_row.is_none()
            })
    }
}

#[cfg(test)]
thread_local! { static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(in crate::source_verify) fn before_selected() -> bool {
    BEFORE.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(in crate::source_verify) fn with_before<T>(f: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BEFORE.with(|slot| slot.set(self.0));
        }
    }
    let guard = Restore(BEFORE.with(|slot| slot.replace(true)));
    let result = f();
    drop(guard);
    result
}

#[cfg(test)]
mod tests;
