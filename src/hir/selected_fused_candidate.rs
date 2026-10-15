//! Private, experimental `FusedConstructionV1` factory (OPT #747).
//!
//! [`validate_selected`] is the only constructor of a
//! [`ValidatedFusedSelection`]. It takes one parsed source program, with no
//! reader, path, cache, graph or ambient authority, and runs these steps in
//! order:
//!
//! 1. `source_verify::fused_construction::candidate::check_candidate` derives
//!    the source proof from the actual AST and runs every source pass.
//! 2. The original source is resolved without rescaling its row count. The
//!    only change is that the HIR while-body gate admits the held producer
//!    call by exact pointer. Typing, ownership and every other resolution
//!    rule run as usual.
//! 3. The sibling `hir::fused_construction` derives the typed selection
//!    independently. Its `join` authenticates a freshly derived current
//!    source proof and joins every prefix and suffix scalar, owner and
//!    identity.
//! 4. Full ordinary HIR validation (types, ownership, loans, cleanup, cleanup
//!    plans, status, agent and function-value checks) runs with the joined
//!    selection. Capacity projects only the joined loop as
//!    `FusedConstruction` at its exact producer. Every other loop stays
//!    Legacy, under the same 32-event and 2 MiB limits.
//! 5. Only then is the immutable owner published.
//!
//! Resolution itself runs the capacity and validation hooks of steps 3 and 4
//! before it returns: a join and selected capacity before any loan, inventory
//! or plan attachment, then full selected validation over the attached HIR.
//! The factory then repeats the join, full validation and selected capacity
//! over the finished HIR. That is deliberate, visible duplicate work. It may
//! only be removed together with an immutable sealed owner proof, never by
//! weakening a gate. Any invariant-bearing type refuses at step 2, because
//! the ordinary route would resolve different, desugared ASTs. Nothing is
//! written to a graph, cache or artifact.
//!
//! The owner privately holds the `ResolvedProgram` it resolved and borrows
//! the original source. It stores no source/HIR proof that refers to itself.
//! [`ValidatedFusedSelection::with_joined`] derives and joins afresh on every
//! use and lends the `Joined` only for the duration of a callback. No decoded
//! or caller-built HIR can become an owner, because no constructor accepts
//! HIR. Ordinary `check`, `resolve` and `validate` never reach this module.
//! The factory is not enabled by default: the interpreter, native and
//! Core-Wasm executors must each pass their lease-domain gates before any
//! route may call it.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "experimental factory awaits the three executor gates"
    )
)]

use crate::ast;
use crate::diagnostic::Diagnostic;
use crate::hir::fused_construction::join::{Joined, SelectedFlow};
use crate::hir::fused_construction::Refusal;
use crate::hir::{ResolvedExpr, ResolvedProgram};
use crate::source_verify::fused_construction::candidate::{check_candidate, CandidateRefusal};

pub(in crate::hir) mod resolution;
#[cfg(test)]
mod tests;

/// A joined selection lent to the validator and capacity projection. It can
/// only be built from a `Joined`, so its rows, width and producer are joined
/// facts rather than caller-supplied numbers.
#[derive(Clone, Copy)]
pub(crate) struct Selected<'a> {
    program: &'a ResolvedProgram,
    function: &'a str,
    flow: SelectedFlow<'a>,
}

impl<'a> Selected<'a> {
    fn from_joined(joined: &Joined<'a>) -> Self {
        let selection = joined.selection();
        Self {
            program: selection.program(),
            function: selection.function().id.as_str(),
            flow: joined.flow(),
        }
    }
    pub(crate) fn function(&self) -> &'a str {
        self.function
    }
    pub(crate) fn flow(&self) -> SelectedFlow<'a> {
        self.flow
    }
    /// Pointer identity with the program the join read.
    pub(crate) fn held_by(&self, program: &ResolvedProgram) -> bool {
        std::ptr::eq(self.program, program)
    }
    /// True only for the joined loop of this exact program.
    pub(crate) fn selects(
        &self,
        program: &ResolvedProgram,
        condition: &ResolvedExpr,
        body: &ResolvedExpr,
    ) -> bool {
        self.held_by(program)
            && std::ptr::eq(self.flow.condition, condition)
            && std::ptr::eq(self.flow.body, body)
    }
}

/// Why the factory published nothing. Each step refuses before the next.
#[derive(Debug)]
pub(crate) enum SelectedRefusal {
    /// Step 1: no candidate, or a source pass refused.
    // Kept in the refusal so Debug and callers can inspect the original source reason.
    #[allow(dead_code)]
    Source(CandidateRefusal),
    /// Step 2: resolution of the original source refused.
    Resolve(Diagnostic),
    /// Step 3: typed derivation or the exact source/HIR join refused.
    #[allow(
        dead_code,
        reason = "retained original receiver or refusal payload; not every legacy entry is selected by current tests"
    )]
    Join(Refusal),
    /// Step 4: full HIR validation under the selection refused.
    Validation(Diagnostic),
    /// Step 4: selected capacity refused.
    Capacity(Diagnostic),
}

/// Capacity facts observed in step 4 for the selected function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SelectedCapacity {
    pub(crate) rows: u32,
    pub(crate) width: u64,
    pub(crate) extent: u64,
    pub(crate) bytes_copy_sites: u32,
    pub(crate) fused_backing_events: u32,
    pub(crate) owned_byte_payload_bytes: u64,
}

/// Immutable, validated selection owner. It grants no emission, cache, graph
/// or publication permission, and engines must re-join through
/// [`Self::with_joined`].
pub(crate) struct ValidatedFusedSelection<'s> {
    source: &'s ast::Program,
    producer: &'s ast::Expr,
    program: ResolvedProgram,
    capacity: SelectedCapacity,
}

impl<'s> ValidatedFusedSelection<'s> {
    pub(crate) fn source(&self) -> &'s ast::Program {
        self.source
    }
    /// The validated typed program, read-only.
    pub(crate) fn program(&self) -> &ResolvedProgram {
        &self.program
    }
    pub(crate) fn capacity(&self) -> SelectedCapacity {
        self.capacity
    }
    /// Re-run every selected gate over the immutable held source/program.
    /// Receivers cannot substitute a mutable or merely joined typed program.
    pub(crate) fn revalidate_current(&self) -> Result<SelectedCapacity, SelectedRefusal> {
        let current = revalidate(self.source, self.producer, &self.program)?;
        if current != self.capacity {
            return Err(SelectedRefusal::Validation(super::hir_error(
                "selected owner capacity no longer agrees",
            )));
        }
        Ok(current)
    }
    /// Test-only real join receiver for independent final-boundary controls.
    /// A candidate still passes the complete current source/HIR join; this
    /// neither publishes it nor changes production constructor visibility.
    #[cfg(test)]
    pub(in crate::hir) fn with_selected_for_test<R>(
        &self,
        program: &ResolvedProgram,
        receive: impl FnOnce(&Joined<'_>, Selected<'_>) -> R,
    ) -> Result<R, Refusal> {
        let joined = resolution::join_current(self.source, self.producer, program)?;
        Ok(receive(&joined, Selected::from_joined(&joined)))
    }
    /// Freshly derive and join over the held source and program, then lend
    /// the `Joined` to `receive` for the duration of the call.
    pub(crate) fn with_joined<R>(
        &self,
        receive: impl FnOnce(&Joined<'_>) -> R,
    ) -> Result<R, Refusal> {
        let joined = resolution::join_current(self.source, self.producer, &self.program)?;
        Ok(receive(&joined))
    }
}

/// The only constructor of a [`ValidatedFusedSelection`].
pub(crate) fn validate_selected(
    source: &ast::Program,
) -> Result<ValidatedFusedSelection<'_>, SelectedRefusal> {
    // 1. Source proof and every source pass, from the actual AST.
    let candidate = check_candidate(source).map_err(SelectedRefusal::Source)?;
    let producer = candidate.proof().producer().call;
    // 2. Resolve the original source under the exact producer gate.
    let program =
        resolution::resolve_selected(source, producer).map_err(SelectedRefusal::Resolve)?;
    // 3 and 4. Independent typed join, then full selected validation.
    let capacity = revalidate(source, producer, &program)?;
    // 5. Publish.
    Ok(ValidatedFusedSelection {
        source,
        producer,
        program,
        capacity,
    })
}

fn revalidate(
    source: &ast::Program,
    producer: &ast::Expr,
    program: &ResolvedProgram,
) -> Result<SelectedCapacity, SelectedRefusal> {
    let joined =
        resolution::join_current(source, producer, program).map_err(SelectedRefusal::Join)?;
    let selected = Selected::from_joined(&joined);
    super::validate_selected_with_static_text(program, Some(selected))
        .map_err(SelectedRefusal::Validation)?;
    let summary = super::byte_capacity::analyze_selected_byte_data_capacity(program, selected)
        .map_err(SelectedRefusal::Capacity)?;
    let function = summary.function(selected.function()).ok_or_else(|| {
        SelectedRefusal::Capacity(super::hir_error(
            "selected function has no capacity summary",
        ))
    })?;
    let selection = joined.selection();
    Ok(SelectedCapacity {
        rows: selection.rows(),
        width: selection.width(),
        extent: selection.extent(),
        bytes_copy_sites: function.bytes_copy_sites,
        fused_backing_events: function.fused_backing_events,
        owned_byte_payload_bytes: function.owned_byte_payload_bytes,
    })
}
