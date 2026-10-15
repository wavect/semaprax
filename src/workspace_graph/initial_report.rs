//! Sealed error policy at the actual retaining-source builder. Legacy errors
//! render at their original lexical producer, while its invocation still lives.
//! This does not certify initially unowned inline controls or ordinary output.
use super::*;
use crate::bounded_output::{ReservationRefusal, ReservationSite};
use crate::workspace_report::WorkspaceReportFailure;
use constructor_failure::receiving::{CoreFailure, Setup};
mod build;
pub(super) use build::build_owned_inner;
mod sealed {
    pub trait Sealed {}
    impl Sealed for Vec<crate::diagnostic::Diagnostic> {}
    impl Sealed for crate::workspace_report::WorkspaceReportFailure {}
}
/// Opaque dispatch storage, explicitly unfunded ordinary caller-stack controls.
pub(crate) struct Core(CoreFailure);
pub(super) fn core<E: Policy>(failure: CoreFailure) -> E {
    E::core(Core(failure))
}
pub(crate) trait Policy: sealed::Sealed + From<Vec<Diagnostic>> {
    fn terminal(
        maximum: usize,
        paid: Option<Vec<Diagnostic>>,
        first: Option<ReservationRefusal>,
        site: Option<ReservationSite>,
    ) -> Self;
    fn core(failure: Core) -> Self;
}
impl Policy for Vec<Diagnostic> {
    fn terminal(
        maximum: usize,
        paid: Option<Vec<Diagnostic>>,
        first: Option<ReservationRefusal>,
        site: Option<ReservationSite>,
    ) -> Self {
        paid.unwrap_or_else(|| vec![diagnostics::live_builder_refusal_at(maximum, first, site)])
    }
    fn core(failure: Core) -> Self {
        failure.0.into_legacy_unfunded()
    }
}
/// The complete original primary AND failed owner remain affine. This does
/// not turn an unreceived diagnostic into a paid report or grant child work.
pub(crate) struct Unreceived(CoreFailure);
impl Unreceived {
    pub(crate) fn diagnostics(&self) -> &[Diagnostic] {
        match &self.0 {
            CoreFailure::Unreceived {
                primary: constructor_failure::Failure::One(diagnostic),
                ..
            } => std::slice::from_ref(diagnostic),
            CoreFailure::Unreceived {
                primary: constructor_failure::Failure::Many(diagnostics),
                ..
            } => diagnostics,
            _ => unreachable!("only an unreceived primary enters this holder"),
        }
    }
    pub(crate) fn into_legacy_unfunded(self) -> Vec<Diagnostic> {
        self.0.into_legacy_unfunded()
    }
}
impl Policy for WorkspaceReportFailure {
    fn terminal(
        maximum: usize,
        paid: Option<Vec<Diagnostic>>,
        first: Option<ReservationRefusal>,
        site: Option<ReservationSite>,
    ) -> Self {
        match paid {
            Some(report) => Self::existing(report),
            None => Self::initial(diagnostics::initial::Captured::live(maximum, first, site)),
        }
    }
    fn core(failure: Core) -> Self {
        match failure.0 {
            CoreFailure::Setup(Setup::Budget { maximum }) => {
                Self::initial(diagnostics::initial::Captured::limit(maximum))
            }
            CoreFailure::Setup(setup) => {
                Self::initial(diagnostics::initial::Captured::fixed(match setup {
                    Setup::NoOwner => "constructor report receiver has no active builder owner",
                    Setup::Allocation => "constructor report receiving allocation refused",
                    Setup::Capacity => {
                        "constructor report receiving capacity differs from its paid request"
                    }
                    Setup::Detached => {
                        "constructor report receiver is detached from its original builder owner"
                    }
                    Setup::Arithmetic => "constructor report receiving layout overflowed",
                    Setup::Budget { .. } => unreachable!("budget form handled separately"),
                }))
            }
            whole @ CoreFailure::Unreceived { .. } => Self::unreceived(Unreceived(whole)),
            // Both variants move their exact existing Vec; received ownership
            // was authenticated at receive/seal before the child restored.
            whole @ (CoreFailure::Received { .. } | CoreFailure::Legacy(_)) => {
                Self::existing(whole.into_legacy_unfunded())
            }
        }
    }
}

#[cfg(test)]
#[path = "initial_report/before.rs"]
mod before;
#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "initial_report/before_mappers.rs"]
mod before_mappers;
