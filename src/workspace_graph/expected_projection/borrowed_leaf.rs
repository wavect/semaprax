//! Exact-source borrowed input for the generated scalar-main leaf route.
//! Imported/private layouts use their exact overlay producer; invariant
//! transformation acquires writable AST only at its ordinary HIR boundary.
use super::{
    active_builder_limit, checked_usage, graph_error, limit_error, private_layout,
    reserve_builder_structure, synthetic_main_runtime_cost, synthetic_program_with_registry,
    AuthoredDeclaration,
};
use crate::ast::program_source::SyntheticProgramView;
use crate::ast::Span;
use crate::ast::{Expr, ExprKind, Function, Program, Type, TypeDeclaration};
use crate::bounded_output::budgeted_format;
use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
use crate::diagnostic::Diagnostic;
use crate::hir;
use std::collections::BTreeMap;

pub(in crate::workspace_graph) fn eligible(program: &Program) -> bool {
    program.module_uses.is_empty()
}

pub(in crate::workspace_graph) fn construct(
    program: &Program,
) -> Result<SyntheticProgramView<'_>, Vec<Diagnostic>> {
    if !eligible(program) {
        return Err(vec![graph_error(
            "SPX-G173",
            "borrowed projection requires the owned route",
        )]);
    }
    // Preserve the owned producer's type-name preflight and its diagnostic
    // order. This directory borrows actual authored declarations; no body or
    // declaration is materialized merely to run the ordinary check.
    let index_bytes = type_index_bytes(program)?;
    reserve_builder_structure(index_bytes)?;
    let mut types = Vec::with_capacity(program.types.len());
    for declaration in &program.types {
        types.push((declaration.name.as_str(), declaration));
    }
    // Duplicate keys always refuse with the same diagnostic; equal-key order
    // is never consumed. Unstable sorting retains that verdict without a
    // hidden stable-sort scratch allocation.
    types.sort_unstable_by(|left, right| left.0.cmp(right.0));
    if types.windows(2).any(|items| items[0].0 == items[1].0) {
        return Err(vec![graph_error(
            "SPX-G173",
            "synthetic workspace type-name index is not unique",
        )]);
    }
    let mut generated = Vec::new();
    if !program
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        reserve_builder_structure(std::mem::size_of::<Function>())?;
        reserve_builder_structure(std::mem::size_of::<Expr>())?;
        generated = Vec::with_capacity(1);
        generated.push(Function {
            stable_id: budgeted_format(format_args!("workspace.synthetic.main.{}", program.module)),
            explicit_id: true,
            name: crate::bounded_output::budgeted_clone("main"),
            name_span: Span::default(),
            type_parameters: Vec::new(),
            params: Vec::new(),
            return_type: Type::I64,
            effects: Vec::new(),
            yields: None,
            follows: None,
            requires: Vec::new(),
            ensures: Vec::new(),
            body: Expr {
                kind: ExprKind::Int(0),
                span: Span::default(),
            },
            span: Span::default(),
        });
    }
    // Only a leaf Int body is owned, so ordinary field Drop allocates nothing
    // and never descends through an authored recursive subtree.
    SyntheticProgramView::new(program, generated)
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

pub(in crate::workspace_graph) fn ast_peak(program: &Program) -> Result<usize, Vec<Diagnostic>> {
    let generated = if program
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        0
    } else {
        synthetic_main_runtime_cost(&program.module)?.total
    };
    let owned = checked_usage(
        std::mem::size_of::<Input<'_>>(),
        generated,
        "builder_bytes",
        active_builder_limit(),
    )?;
    checked_usage(
        owned,
        type_index_bytes(program)?,
        "builder_bytes",
        active_builder_limit(),
    )
}
fn type_index_bytes(program: &Program) -> Result<usize, Vec<Diagnostic>> {
    program
        .types
        .len()
        .checked_mul(std::mem::size_of::<(&str, &TypeDeclaration)>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

/// Extra inline carrier over the ordinary owned Program. The resolved-core
/// invocation retains one reused input slot, never one header per module.
pub(in crate::workspace_graph) fn input_carrier_delta() -> Result<usize, Vec<Diagnostic>> {
    std::mem::size_of::<Input<'_>>()
        .checked_sub(std::mem::size_of::<Program>())
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])
}

/// Hits/reuse candidates and invariant transforms keep the owned input.
/// First checked cold keys become owned only at the retention boundary.
// The reused inline owning fallback is priced by input_carrier_delta using
// actual sizeof; a Box would introduce a separate unpriced allocation.
#[allow(clippy::large_enum_variant)]
pub(in crate::workspace_graph) enum Input<'a> {
    Owned(Program),
    Transient(crate::ast::program_source::function_alias::TransientFunctions<'a>),
    Borrowed(SyntheticProgramView<'a>),
}
impl<'a> Input<'a> {
    #[cfg(test)]
    pub(in crate::workspace_graph) fn construct(
        program: &'a Program,
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
        programs: &[Program],
        registry: &private_layout::registry::LayoutRegistry<'_>,
        _uncached: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        if eligible(program) {
            construct(program).map(Self::Borrowed)
        } else if super::borrowed_imports::eligible(program, authored, programs) {
            super::borrowed_imports::construct(program, authored, programs, registry)
                .map(Self::Borrowed)
        } else {
            synthetic_program_with_registry(program, authored, programs, Some(registry))
                .map(Self::Owned)
        }
    }
    /// A live uncached resolver will drop its projection rather than publish a key.
    /// Acquire only generated backing; authored declarations remain source-held.
    pub(in crate::workspace_graph) fn construct_transient(
        program: &'a Program,
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
        programs: &[Program],
        registry: &private_layout::registry::LayoutRegistry<'_>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if eligible(program) {
            construct(program).map(Self::Borrowed)
        } else if super::borrowed_imports::eligible(program, authored, programs) {
            super::borrowed_imports::construct(program, authored, programs, registry)
                .map(Self::Borrowed)
        } else {
            synthetic_program_with_registry(program, authored, programs, Some(registry))
                .map(Self::Owned)
        }
    }

    // CORE_TRANSIENT_BEGIN
    pub(in crate::workspace_graph) fn construct_transient_core(
        program: &'a Program,
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
        registry: &private_layout::registry::LayoutRegistry<'_>,
        core: &mut super::CoreProviderProofs<'a>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if eligible(program) {
            construct(program).map(Self::Borrowed)
        } else if super::borrowed_imports::eligible(program, authored, programs) {
            super::borrowed_imports::construct_core(program, authored, programs, registry, core)
                .map(Self::Borrowed)
        } else {
            super::held_projection::synthetic_program_core(
                program,
                authored,
                programs,
                Some(registry),
                core,
            )
            .map(Self::Owned)
        }
    }
    // CORE_TRANSIENT_END

    pub(in crate::workspace_graph) fn construct_receiving(
        program: &'a Program,
        authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
        programs: &[Program],
        registry: &private_layout::registry::LayoutRegistry<'_>,
        _uncached: bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        if eligible(program) {
            construct(program).map(Self::Borrowed)
        } else if super::borrowed_imports::eligible(program, authored, programs) {
            super::borrowed_imports::construct_receiving(program, authored, programs, registry)
                .map(Self::Borrowed)
        } else {
            synthetic_program_with_registry(program, authored, programs, Some(registry))
                .map(Self::Owned)
        }
    }
    pub(in crate::workspace_graph) fn borrowed(&self) -> Option<&SyntheticProgramView<'_>> {
        match self {
            Self::Owned(_) | Self::Transient(_) => None,
            Self::Borrowed(program) => Some(program),
        }
    }

    #[cfg(test)]
    pub(in crate::workspace_graph) fn into_checked_key(self) -> Result<Program, Vec<Diagnostic>> {
        match self {
            Self::Owned(program) => Ok(program),
            Self::Borrowed(view) => checked_key::materialize(view),
            Self::Transient(_) => unreachable!("transient wrapper view cannot publish a cache key"),
        }
    }

    pub(in crate::workspace_graph) fn into_split_key(
        self,
        source_slot: usize,
    ) -> Result<crate::ast::program_source::checked_key::CheckedSource, Vec<Diagnostic>> {
        reserve_builder_structure(0)?;
        match self {
            Self::Owned(program) => {
                Ok(crate::ast::program_source::checked_key::CheckedSource::Owned(program))
            }
            Self::Transient(_) => unreachable!("transient wrapper view cannot publish a cache key"),
            Self::Borrowed(view) => {
                let (program, generated) = view.into_overlay_owner();
                Ok(
                    crate::ast::program_source::checked_key::CheckedSource::deferred(
                        program,
                        generated,
                        source_slot,
                    ),
                )
            }
        }
    }

    #[cfg(test)]
    pub(in crate::workspace_graph) fn into_receiving_key(self) -> Result<Program, Vec<Diagnostic>> {
        match self {
            Self::Owned(program) => Ok(program),
            Self::Borrowed(view) => checked_key::materialize_receiving(view),
            Self::Transient(_) => unreachable!("transient wrapper view cannot publish a cache key"),
        }
    }

    pub(in crate::workspace_graph) fn owned(&self) -> Option<&Program> {
        match self {
            Self::Owned(program) => Some(program),
            Self::Borrowed(_) | Self::Transient(_) => None,
        }
    }
    pub(in crate::workspace_graph) fn source(
        &self,
    ) -> &dyn crate::ast::program_source::ProgramSource {
        match self {
            Self::Owned(program) => program,
            Self::Borrowed(program) => program,
            Self::Transient(program) => program,
        }
    }
    #[cfg(test)]
    pub(in crate::workspace_graph) fn into_owned(self) -> Result<Program, Vec<Diagnostic>> {
        match self {
            Self::Owned(program) => Ok(program),
            Self::Borrowed(_) | Self::Transient(_) => Err(vec![graph_error(
                "SPX-G173",
                "borrowed projection cannot become a checked cache key",
            )]),
        }
    }
    /// Fresh resolution of the actual Owned/Borrowed case with the caller's
    /// already acquired identity owner; no Program is materialized.
    pub(in crate::workspace_graph) fn resolve_paid<'s>(
        &'s self,
        private: Option<crate::source_verify::private_names::PrivateNames<'s>>,
        owner: &mut IdentityReservation,
    ) -> Result<
        (hir::ResolvedProgram, hir::FunctionResolutionWork),
        hir::identity_resolution::Failure<Refused>,
    > {
        match self {
            Self::Owned(program) => hir::identity_resolution::resolve_checked_nominal(
                program,
                None,
                None,
                None,
                private,
                |bytes| owner.reserve(bytes),
            ),
            Self::Borrowed(program) => {
                hir::identity_resolution::resolve_source_nominal(program, private, |bytes| {
                    owner.reserve(bytes)
                })
            }
            Self::Transient(program) => {
                hir::identity_resolution::resolve_source_nominal(program, private, |bytes| {
                    owner.reserve(bytes)
                })
            }
        }
    }
    pub(in crate::workspace_graph) fn resolve_paid_with_base<'s>(
        &'s self,
        private: Option<crate::source_verify::private_names::PrivateNames<'s>>,
        invocation: &mut hir::prelude_bundle::Invocation<Refused>,
        owner: &mut IdentityReservation,
    ) -> Result<
        (hir::ResolvedProgram, hir::FunctionResolutionWork),
        hir::identity_resolution::NominalReceiverFailure,
    > {
        hir::identity_resolution::resolve_source_checked_previous_with_base(
            self.source(),
            None,
            None,
            None,
            private,
            invocation,
            |bytes| owner.reserve(bytes),
        )
    }
}

#[cfg(test)]
#[path = "borrowed_leaf/tests.rs"]
mod tests;

#[path = "borrowed_leaf/checked_key.rs"]
mod checked_key;

pub(in crate::workspace_graph) fn checked_key_owned(
    view: SyntheticProgramView<'_>,
) -> Result<Program, Vec<Diagnostic>> {
    checked_key::materialize(view)
}
