//! Closed no-binding roots omit only the first Names traversal machinery.
use super::{Expr, ExprKind, Proof, Statement};
use std::mem::size_of;
// Two helper argument/formal+boolean-return tuples; both match-place loans;
// bounded Vec::is_empty actual/formals and length/boolean receivers; Box tail
// pattern/Deref loans and returned expression; all selected/branch booleans.
// This bank is ADDITIONAL on every attempted root, including fallback.
pub(in crate::workspace_graph::expected_projection) const SELECTOR: usize =
    2 * size_of::<(&Expr, &mut Proof, bool)>()
        + size_of::<(
            [&ExprKind; 2],
            [&Vec<Statement>; 3],
            [&Vec<Expr>; 4],
            [&Box<Expr>; 3],
            [&Expr; 3],
            [usize; 8],
            [bool; 12],
        )>();

pub(in crate::workspace_graph::expected_projection) fn accounted(
    root: &Expr,
    proof: &mut Proof,
) -> bool {
    #[cfg(test)]
    note(0);
    // The original callback's charge(&expression) is retained once per node,
    // in original root-before-tail order. No body/parameter/callee result is
    // reused; every other root still runs the entire original bounded walker.
    let closed = match &root.kind {
        ExprKind::Int(_)
        | ExprKind::Int32(_)
        | ExprKind::Char(_)
        | ExprKind::Uint8(_)
        | ExprKind::Usize(_)
        | ExprKind::ArrayU8(_)
        | ExprKind::RepeatArrayU8 { .. }
        | ExprKind::Float32(_)
        | ExprKind::Float64(_)
        | ExprKind::Bool(_)
        | ExprKind::String(_)
        | ExprKind::BorrowedUtf8Literal(_) => {
            proof.charge(&root);
            true
        }
        ExprKind::Call { args, .. } if args.is_empty() => {
            proof.charge(&root);
            true
        }
        ExprKind::Block { statements, tail } if statements.is_empty() => {
            let child: &Expr = tail;
            match &child.kind {
                ExprKind::Int(_)
                | ExprKind::Int32(_)
                | ExprKind::Char(_)
                | ExprKind::Uint8(_)
                | ExprKind::Usize(_)
                | ExprKind::ArrayU8(_)
                | ExprKind::RepeatArrayU8 { .. }
                | ExprKind::Float32(_)
                | ExprKind::Float64(_)
                | ExprKind::Bool(_)
                | ExprKind::String(_)
                | ExprKind::BorrowedUtf8Literal(_) => {
                    proof.charge(&root);
                    proof.charge(&child);
                    true
                }
                ExprKind::Call { args, .. } if args.is_empty() => {
                    proof.charge(&root);
                    proof.charge(&child);
                    true
                }
                _ => false,
            }
        }
        _ => false,
    };
    #[cfg(test)]
    if closed {
        note(1);
    }
    closed
}

#[cfg(test)]
thread_local! {
    static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static COUNTS: std::cell::Cell<[usize; 2]> = const { std::cell::Cell::new([0; 2]) };
}
#[cfg(test)]
pub(super) fn before_selected() -> bool {
    BEFORE.with(|flag| flag.get())
}
#[cfg(test)]
pub(super) struct Before(bool);
#[cfg(test)]
impl Before {
    pub(super) fn hold() -> Self {
        Self(BEFORE.with(|flag| flag.replace(true)))
    }
}
#[cfg(test)]
impl Drop for Before {
    fn drop(&mut self) {
        BEFORE.with(|flag| flag.set(self.0));
    }
}
#[cfg(test)]
fn note(index: usize) {
    COUNTS.with(|count| {
        let mut n = count.get();
        n[index] += 1;
        count.set(n);
    });
}

#[cfg(test)]
#[path = "../../identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod reader;
#[cfg(test)]
#[path = "closed_roots_tests.rs"]
mod tests;
