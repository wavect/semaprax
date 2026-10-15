//! Actual workspace module-resolution receiver for initial declaration-index
//! identity payment. One fixed report owner is acquired on the already active
//! builder ledger BEFORE verification, SourceIndex or DeclarationIndex work,
//! then forwarded through the selective or fresh route and both invariant
//! passes. Cached modules use only the live prelude replay sibling. Setup
//! refusal stays Copy; the
//! outer `Vec<Diagnostic>` graph/report layer remains explicitly unqualified.
use super::expected_projection::borrowed_leaf::Input;
use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
use crate::bounded_output::staged;
use crate::diagnostic::Diagnostic;
use crate::hir::identity_resolution::Failure;
use crate::project::incremental::paid_resolution::SelectiveFailure;
use crate::project::incremental::FrontendPass;
use std::collections::BTreeMap;

/// Resolved HIR, per-function costs, reused function count, static-text flag.
pub(super) type Parts = (
    crate::hir::ResolvedProgram,
    BTreeMap<String, usize>,
    usize,
    bool,
);

#[derive(Debug)]
pub(super) enum ModuleFailure {
    /// Fixed report acquisition refused; no report backing exists.
    Setup(Refused),
    /// Owned source/semantic diagnostics (legacy) or the published identity
    /// report that moved the acquired slot/text.
    Report(Vec<Diagnostic>),
}

impl ModuleFailure {
    /// Adapts into the legacy outer graph error layer, outside the funded
    /// resolution boundary. Setup refusal has already made the active budget
    /// sticky, so the outer retry loop renders the original refusal.
    pub(super) fn into_legacy(self) -> Vec<Diagnostic> {
        match self {
            Self::Report(report) => report,
            Self::Setup(_) => vec![super::limit_error(
                "builder_bytes",
                super::active_builder_limit(),
            )],
        }
    }
}

pub(super) fn resolve_module<'s>(
    frontend: Option<&FrontendPass>,
    path: &str,
    synthetic: &'s Input<'_>,
    private: Option<crate::source_verify::private_names::PrivateNames<'s>>,
) -> Result<Parts, ModuleFailure> {
    // Every non-hit module acquires exactly once, on the same Rc<Budget> the
    // caller's resolver-cost interval measures; no fresh quota is created.
    let mut owner = if frontend.is_some_and(FrontendPass::retains_checked_hir) {
        staged!(SelectiveResolution, IdentityReservation::acquire())
    } else {
        staged!(FreshResolution, IdentityReservation::acquire())
    }
    .map_err(ModuleFailure::Setup)?;
    #[cfg(test)]
    observe_acquired(&owner);
    // Both routes execute inside the same linked-source authentication scope.
    // Evaluating a selective Result before these guards would reject retained
    // imported wrapper bodies before the guards could authenticate them.
    crate::vec_ops::with_authenticated_linked_source(|| {
        crate::box_ops::with_authenticated_linked_source(|| {
            let selective = staged!(
                SelectiveResolution,
                frontend.and_then(|cache| {
                    synthetic
                        .owned()
                        .and_then(|input| {
                            cache.resolve_functions_paid(path, input, private, &mut owner)
                        })
                        .or_else(|| {
                            synthetic.borrowed().and_then(|view| {
                                if cache.permits_borrowed_cold_source(path) {
                                    cache.resolve_borrowed_cold_source_paid(
                                        path, view, private, &mut owner,
                                    )
                                } else {
                                    cache.resolve_borrowed_source_paid(
                                        path, view, private, &mut owner,
                                    )
                                }
                            })
                        })
                })
            );
            if let Some(selective) = selective {
                selective.map(|resolved| resolved.into_parts())
            } else {
                staged!(FreshResolution, {
                    synthetic
                        .resolve_paid(private, &mut owner)
                        .map(|(resolved, work)| {
                            let selected = work.static_text();
                            (resolved, BTreeMap::new(), 0, selected)
                        })
                        .map_err(SelectiveFailure::Resolve)
                })
            }
        })
    })
    .map_err(|failure| match failure {
        SelectiveFailure::Project(report) => ModuleFailure::Report(report),
        SelectiveFailure::Resolve(Failure::Source(report)) => ModuleFailure::Report(report),
        SelectiveFailure::Resolve(Failure::Semantic(error)) => ModuleFailure::Report(vec![error]),
        SelectiveFailure::Resolve(Failure::Identity(_)) => ModuleFailure::Report(owner.publish()),
    })
}

#[cfg(test)]
thread_local! {
    /// Test-only: acquisition count and original report/text pointers of the
    /// most recent actual acquisition on this thread. Const fixed storage, so
    /// the callback never allocates; it carries no authority.
    static ACQUIRED: std::cell::Cell<(usize, Option<(usize, usize)>)> =
        const { std::cell::Cell::new((0, None)) };
}

#[cfg(test)]
fn observe_acquired(owner: &IdentityReservation) {
    let (report, text) = owner.pointers();
    ACQUIRED.with(|acquired| {
        acquired.set((acquired.get().0 + 1, Some((report as usize, text as usize))));
    });
}

/// Test-only count of actual acquisitions on this thread; never authority.
#[cfg(test)]
pub(super) fn acquisitions() -> usize {
    ACQUIRED.with(|acquired| acquired.get().0)
}

#[cfg(test)]
mod tests;

mod base;
pub(super) use base::resolve_module_with_core;

#[cfg(test)]
pub(super) use base::cache_billing::source_reference::{
    historical_inline_inventory, historical_inline_root, historical_root,
};
pub(super) use base::cache_billing::{local_resolution_bytes, replay_cached_prelude};
