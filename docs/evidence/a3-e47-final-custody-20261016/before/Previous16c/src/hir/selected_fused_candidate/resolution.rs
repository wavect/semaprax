//! Resolver-side hooks for the private selected factory.
//!
//! Ordinary resolution passes `None` and keeps the unchanged capacity and
//! validation calls. Only [`resolve_selected`] sets the held producer. When
//! it is set, both hooks join a freshly derived current source proof with
//! the HIR just built, and they refuse unless the proof's producer is that
//! exact source node.
use super::Selected;
use crate::ast;
use crate::diagnostic::Diagnostic;
use crate::hir::fused_construction::join::{self, Joined};
use crate::hir::fused_construction::Refusal;
use crate::hir::{DeclarationIndex, FunctionResolutionWork, ResolvedProgram, Resolver};

/// Derive the current source proof, require its producer to be `producer`,
/// and join it with `program`.
pub(in crate::hir) fn join_current<'a>(
    source: &'a ast::Program,
    producer: &ast::Expr,
    program: &'a ResolvedProgram,
) -> Result<Joined<'a>, Refusal> {
    let proof = join::current_source(source)?;
    if !std::ptr::eq(proof.producer().call, producer) {
        return Err(Refusal::Join(
            "held producer is not the current source producer",
        ));
    }
    join::join(source, proof, program)
}

fn refused(refusal: Refusal) -> Diagnostic {
    crate::hir::hir_error(format!("selected fused construction refused: {refusal:?}"))
}

/// Capacity hook at the resolver's ordinary capacity boundary.
pub(in crate::hir) fn capacity(
    source: &dyn ast::program_source::ProgramSource,
    producer: Option<&ast::Expr>,
    resolved: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    let Some(producer) = producer else {
        return crate::hir::analyze_byte_data_capacity(resolved).map(|_| ());
    };
    let ast::program_source::Subject::Owned(source) = source.subject() else {
        return Err(refused(Refusal::Join(
            "selected source is not the original owned AST",
        )));
    };
    let joined = join_current(source, producer, resolved).map_err(refused)?;
    crate::hir::byte_capacity::analyze_selected_byte_data_capacity(
        resolved,
        Selected::from_joined(&joined),
    )
    .map(|_| ())
}

/// Validation hook at the resolver's ordinary final validation boundary.
pub(in crate::hir) fn validation(
    source: &dyn ast::program_source::ProgramSource,
    producer: Option<&ast::Expr>,
    resolved: &ResolvedProgram,
) -> Result<bool, Diagnostic> {
    let Some(producer) = producer else {
        return crate::hir::validate_with_static_text(resolved);
    };
    let ast::program_source::Subject::Owned(source) = source.subject() else {
        return Err(refused(Refusal::Join(
            "selected source is not the original owned AST",
        )));
    };
    let joined = join_current(source, producer, resolved).map_err(refused)?;
    crate::hir::validate_selected_with_static_text(resolved, Some(Selected::from_joined(&joined)))
}

/// Resolve the original, already source-checked program with the exact
/// producer gate.
///
/// The ordinary `resolve_verified` route resolves an invariant clause
/// program and then a desugared program with synthesized functions. Those are
/// different ASTs, and the held producer pointer would not belong to them.
/// This closed profile therefore refuses any type that declares invariants,
/// even one the selected row never uses. The check reads the borrowed
/// declarations; it builds no clause program and skips no desugaring. The
/// route also never reuses prior HIR and publishes no `Analysis`.
pub(super) fn resolve_selected<'s>(
    source: &'s ast::Program,
    producer: &'s ast::Expr,
) -> Result<ResolvedProgram, Diagnostic> {
    if source
        .types
        .iter()
        .any(|declaration| !declaration.invariants().is_empty())
    {
        return Err(crate::hir::hir_error(
            "selected fused construction refuses invariant-bearing types",
        ));
    }
    Resolver {
        program: source,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(source),
        declarations: DeclarationIndex::from_verified(source)?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: Some(producer),
        partition_profile: false,
    }
    .resolve()
    .map(|(resolved, _)| resolved)
}
