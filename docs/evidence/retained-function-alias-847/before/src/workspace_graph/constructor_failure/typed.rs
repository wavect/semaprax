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
