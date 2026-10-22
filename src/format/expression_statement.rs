//! Conservative canonical projection: a deserialized marker cannot hide a reader.
use super::{Expr, Statement};
use std::cell::Cell;
thread_local! { static SAFE: Cell<Option<bool>> = const { Cell::new(None) }; }
pub(super) struct Scope(Option<bool>);
impl Drop for Scope {
    fn drop(&mut self) {
        SAFE.with(|slot| slot.set(self.0));
    }
}
pub(super) fn scope(root: &Expr) -> Scope {
    let old = SAFE.with(Cell::get);
    if old.is_none() {
        SAFE.with(|slot| {
            slot.set(Some(
                crate::ast::expression_statement::projection_is_hygienic(
                    root,
                    scratch_reservations(),
                ),
            ))
        });
    }
    Scope(old)
}
pub(super) fn function_scope(root: &Expr, params: &[crate::ast::Param]) -> Scope {
    let old = SAFE.with(Cell::get);
    if old.is_none() {
        SAFE.with(|slot| {
            slot.set(Some(
                crate::ast::expression_statement::projection_with_bindings(
                    root,
                    params.iter().map(|p| p.name.as_str()),
                    scratch_reservations(),
                ),
            ))
        });
    }
    Scope(old)
}
pub(super) fn value(statement: &Statement) -> Option<&Expr> {
    SAFE.with(Cell::get)
        .unwrap_or(false)
        .then(|| crate::ast::expression_statement::value(statement))
        .flatten()
}
pub(super) fn write_let_prefix(output: &mut impl std::fmt::Write, statement: &Statement) {
    if value(statement).is_some() {
        return;
    }
    let Statement::Let {
        name,
        mutable,
        declared,
        ..
    } = statement
    else {
        unreachable!()
    };
    if *mutable {
        write!(output, "let mut {name}").unwrap();
    } else {
        write!(output, "let {name}").unwrap();
    }
    if let Some(ty) = declared {
        write!(output, ": ").unwrap();
        super::write_type(output, ty);
    }
    write!(output, " = ").unwrap();
}

pub(super) fn hygienic() -> bool {
    SAFE.with(Cell::get).unwrap_or(false)
}

pub(super) fn whole_function_scope(function: &crate::ast::Function) -> Scope {
    let old = SAFE.with(Cell::get);
    if old.is_none() {
        let roots = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures);
        let names = function
            .params
            .iter()
            .map(|p| p.name.as_str())
            .chain(function.type_parameters.iter().map(|p| p.name.as_str()));
        SAFE.with(|slot| {
            slot.set(Some(crate::ast::expression_statement::projection_roots(
                roots,
                names,
                scratch_reservations(),
            )))
        });
    }
    Scope(old)
}

// Private writers already hold this scratch bank across both formatting passes.
// Hygiene finishes and drops all its buffers before formatter stacks start, so
// those sequential allocations may borrow the same reserved backing. Ordinary
// writers have no credit; every excess byte still reaches the live ledger.
fn scratch_reservations() -> impl FnMut(usize) -> bool {
    let mut credit = super::PRIVATE_SCRATCH_CAPACITY
        .with(|capacity| capacity.get().map_or(0, |capacity| capacity.bytes()));
    move |bytes| {
        if credit == 0 {
            return crate::bounded_output::reserve_active_required(bytes);
        }
        // Prepaid storage cannot recover an already refused invocation.
        if !crate::bounded_output::reserve_active_required(0) {
            return false;
        }
        let prepaid = credit.min(bytes);
        credit -= prepaid;
        let excess = bytes - prepaid;
        excess == 0 || crate::bounded_output::reserve_active_required(excess)
    }
}

// The hygiene verifier retains its original fixed 129-pattern-frame bound.
// Its largest slot is a slice/index payload plus a machine-word enum tag.
// This storage is sequential with the formatter's recursive frame stacks.
pub(super) fn pattern_scratch_bytes(pattern_depth: usize) -> Option<usize> {
    if pattern_depth == 0 {
        return Some(0);
    }
    129usize.checked_mul(std::mem::size_of::<(
        &Expr,
        &[crate::ast::RecordMatchPatternField],
        usize,
    )>())
}
