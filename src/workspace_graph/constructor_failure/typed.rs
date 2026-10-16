//! Explicit original receiver authority; old One/Many helpers remain independent.
use super::receiving::{CoreFailure, Receiver, Setup};
use crate::diagnostic::Diagnostic;
#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "selected diagnostic stays inline after refusal"
)]
pub(in crate::workspace_graph) enum Failure {
    One(Diagnostic),
    Many(Vec<Diagnostic>),
    Setup(Setup),
}
impl Failure {
    pub(in crate::workspace_graph) fn one(diagnostic: Diagnostic) -> Self {
        Self::One(diagnostic)
    }
}
impl From<Vec<Diagnostic>> for Failure {
    fn from(report: Vec<Diagnostic>) -> Self {
        Self::Many(report)
    }
}
impl From<Setup> for Failure {
    fn from(failure: Setup) -> Self {
        Self::Setup(failure)
    }
}
impl From<Diagnostic> for Failure {
    fn from(primary: Diagnostic) -> Self {
        Self::One(primary)
    }
}
pub(in crate::workspace_graph) fn construct_transient_core<'a>(
    program: &'a crate::ast::Program,
    authored: &'a std::collections::BTreeMap<&'a str, super::super::AuthoredDeclaration<'a>>,
    programs: &'a [crate::ast::Program],
    registry: &super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
    core: &mut super::super::expected_projection::CoreProviderProofs<'a>,
    owner: &mut Receiver,
) -> Result<super::super::expected_projection::borrowed_leaf::Input<'a>, Failure> {
    use super::super::expected_projection::{borrowed_imports, borrowed_leaf};
    use borrowed_leaf::Input;
    owner.entry::<Input<'a>>()?;
    if borrowed_leaf::eligible(program) {
        borrowed_leaf::construct(program)
            .map(Input::Borrowed)
            .map_err(Failure::from)
    } else if borrowed_imports::eligible(program, authored, programs) {
        borrowed_imports::typed::construct_using(
            program,
            authored,
            programs,
            registry,
            Some(core),
            owner,
        )
        .map(Input::Borrowed)
    } else {
        // Explicit legacy dependency: its Vec producers are not recertified.
        Input::construct_transient_core(program, authored, programs, registry, core)
            .map_err(Failure::from)
    }
}
/// The caller admits this only for an uncached transient resolution. A cache
/// or invariant-rewriting route keeps the complete original constructor above.
pub(in crate::workspace_graph) fn construct_transient_alias_core<'a>(
    program: &'a crate::ast::Program,
    authored: &'a std::collections::BTreeMap<&'a str, super::super::AuthoredDeclaration<'a>>,
    programs: &'a [crate::ast::Program],
    registry: &super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
    core: &mut super::super::expected_projection::CoreProviderProofs<'a>,
    owner: &mut Receiver,
) -> Result<super::super::expected_projection::borrowed_leaf::Input<'a>, Failure> {
    use super::super::expected_projection::{borrowed_imports, borrowed_leaf::Input};
    owner.entry::<Input<'a>>()?;
    // Before admission scans or forwarding arguments: the new boundary has
    // one real Result in addition to the selected old/new producer's Result.
    owner.reserve(
        std::mem::size_of::<&crate::ast::Program>()
            + std::mem::size_of::<
                &std::collections::BTreeMap<&str, super::super::AuthoredDeclaration<'_>>,
            >()
            + std::mem::size_of::<&[crate::ast::Program]>()
            + std::mem::size_of::<
                &super::super::expected_projection::private_layout::registry::LayoutRegistry<'_>,
            >()
            + std::mem::size_of::<&mut super::super::expected_projection::CoreProviderProofs<'_>>()
            + std::mem::size_of::<&mut Receiver>()
            + 2 * std::mem::size_of::<bool>(),
    )?;
    if !borrowed_imports::eligible(program, authored, programs) {
        return construct_transient_core(program, authored, programs, registry, core, owner);
    }
    // Invariant passes may require a writable whole Program. Preserve their
    // complete ordinary producer rather than materializing views later.
    owner.reserve(
        std::mem::size_of::<std::slice::Iter<'_, crate::ast::Program>>()
            + programs.len()
                * (std::mem::size_of::<Option<&crate::ast::Program>>()
                    + std::mem::size_of::<&crate::ast::Program>()),
    )?;
    for candidate in programs {
        // The backing length already fits an allocated TypeDeclaration Vec;
        // these smaller per-item controls cannot overflow that source bound.
        owner.reserve(
            std::mem::size_of::<std::slice::Iter<'_, crate::ast::TypeDeclaration>>()
                + candidate.types.len()
                    * (std::mem::size_of::<Option<&crate::ast::TypeDeclaration>>()
                        + 2 * std::mem::size_of::<&crate::ast::TypeDeclaration>()
                        + std::mem::size_of::<&[crate::ast::Expr]>()
                        + std::mem::size_of::<bool>()),
        )?;
        for declaration in &candidate.types {
            if !declaration.invariants().is_empty() {
                return construct_transient_core(
                    program, authored, programs, registry, core, owner,
                );
            }
        }
    }
    borrowed_imports::typed::construct_transient(
        program,
        authored,
        programs,
        registry,
        Some(core),
        owner,
    )
    .map(Input::Transient)
}
impl Receiver {
    /// Semantic selection precedes a secondary receiving/sticky association.
    pub(in crate::workspace_graph) fn receive_typed(self, failure: Failure) -> CoreFailure {
        match failure {
            Failure::One(primary) => self.receive(super::Failure::One(primary)),
            Failure::Many(report) => self.receive(super::Failure::Many(report)),
            Failure::Setup(refusal) => {
                drop(self);
                CoreFailure::Setup(refusal)
            }
        }
    }
}

#[cfg(test)]
#[path = "../function_alias/before_core.rs"]
pub(in crate::workspace_graph) mod before_function_alias_core;
