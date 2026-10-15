//! One actual core-attempt ledger owns the single-diagnostic receiving slot.
//! This certifies neither existing multi-error helpers nor final legacy output.
use super::Failure;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::bounded_output::{active_remaining, reserve_active_required};
use crate::diagnostic::Diagnostic;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::workspace_graph) enum Setup {
    NoOwner,
    Budget { maximum: usize },
    Allocation,
    Capacity,
    Detached,
    Arithmetic,
}

/// Affine physical backing; no Clone, factory budget, or source authority.
pub(in crate::workspace_graph) struct Receiver {
    ledger: ClassifierLedger,
    report: Vec<Diagnostic>,
    detached: bool,
    maximum: usize,
    first: Option<Setup>,
}
impl std::fmt::Debug for Receiver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConstructorReceiver")
            .field("len", &self.report.len())
            .finish_non_exhaustive()
    }
}
#[derive(Debug)]
#[allow(
    clippy::large_enum_variant,
    reason = "the complete primary and unreceived owner must remain inline after refusal"
)]
pub(in crate::workspace_graph) enum CoreFailure {
    /// Only this one slot has genuine constructor receiving ownership.
    Received {
        report: Vec<Diagnostic>,
        ledger: ClassifierLedgerDebug,
    },
    /// Existing full ordered Vec backing; explicitly not recertified paid.
    Legacy(Vec<Diagnostic>),
    Setup(Setup),
    Unreceived {
        primary: Failure,
        owner: Receiver,
        refusal: Setup,
    },
}
/// Diagnostic formatting must not expose ledger access or allocate a proxy.
pub(in crate::workspace_graph) struct ClassifierLedgerDebug(ClassifierLedger);
impl std::fmt::Debug for ClassifierLedgerDebug {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let _held = &self.0;
        f.write_str("held original constructor ledger")
    }
}
impl From<Vec<Diagnostic>> for CoreFailure {
    fn from(report: Vec<Diagnostic>) -> Self {
        Self::Legacy(report)
    }
}
impl From<Setup> for CoreFailure {
    fn from(failure: Setup) -> Self {
        Self::Setup(failure)
    }
}
impl CoreFailure {
    /// Public compatibility only; setup/final/help producers are NOT funded by
    /// the constructor slot. Called after the attempt's ledger has restored.
    pub(in crate::workspace_graph) fn into_legacy_unfunded(self) -> Vec<Diagnostic> {
        match self {
            Self::Received { report, ledger } => {
                drop(ledger);
                report
            }
            Self::Legacy(report) => report,
            Self::Unreceived {
                primary,
                owner,
                refusal: _,
            } => {
                drop(owner);
                primary.into_legacy_unfunded()
            }
            Self::Setup(Setup::Budget { maximum }) => {
                vec![super::super::limit_error("builder_bytes", maximum)]
            }
            Self::Setup(refusal) => vec![super::super::graph_error(
                "SPX-G173",
                match refusal {
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
                },
            )],
        }
    }
}
impl Receiver {
    pub(in crate::workspace_graph) fn setup_bytes() -> Option<usize> {
        [
            size_of::<Self>(),
            // Fixed small refusal controls exist even when the first request
            // refuses. Typed primitive adapters never return a large Failure.
            size_of::<Result<(), Setup>>(),
            size_of::<Setup>(),
            size_of::<&mut Self>(),
            size_of::<Option<Setup>>(),
            size_of::<usize>(),
            size_of::<Option<Self>>(),
            size_of::<Result<Self, Setup>>(),
            size_of::<CoreFailure>(),
            size_of::<Vec<Diagnostic>>(),
            size_of::<Result<super::super::ResolvedCore, CoreFailure>>(),
            size_of::<
                Result<
                    (
                        super::super::ResolvedCore,
                        super::super::custody::SourceCustody,
                    ),
                    CoreFailure,
                >,
            >(),
            size_of::<Diagnostic>(),
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
    }
    pub(in crate::workspace_graph) fn acquire() -> Result<Self, Setup> {
        if active_remaining().is_none() {
            return Err(Setup::NoOwner);
        }
        let bytes = Self::setup_bytes().ok_or(Setup::Arithmetic)?;
        if !reserve_active_required(bytes) {
            return Err(Setup::Budget {
                maximum: super::super::active_builder_limit(),
            });
        }
        let ledger = ClassifierLedger::capture();
        let mut report = Vec::new();
        report.try_reserve_exact(1).map_err(|_| Setup::Allocation)?;
        if report.capacity() != 1 {
            let excess = report
                .capacity()
                .saturating_sub(1)
                .checked_mul(size_of::<Diagnostic>())
                .ok_or(Setup::Arithmetic)?;
            if !reserve_active_required(excess) {
                return Err(Setup::Budget {
                    maximum: super::super::active_builder_limit(),
                });
            }
            return Err(Setup::Capacity);
        }
        Ok(Self {
            ledger,
            report,
            detached: false,
            maximum: super::super::active_builder_limit(),
            first: None,
        })
    }
    /// A selected primary may move into existing backing even when the held
    /// ledger is sticky. This authenticates identity and preserves ledger state;
    /// performs NO new reservation/formatting/output acquisition after Err.
    pub(in crate::workspace_graph) fn receive(mut self, failure: Failure) -> CoreFailure {
        if self.detached || !self.ledger.current() {
            self.detached = true;
            return CoreFailure::Unreceived {
                primary: failure,
                owner: self,
                refusal: Setup::Detached,
            };
        }
        match failure {
            Failure::One(primary) => {
                self.report.push(primary);
                CoreFailure::Received {
                    report: self.report,
                    ledger: ClassifierLedgerDebug(self.ledger),
                }
            }
            Failure::Many(report) => CoreFailure::Legacy(report),
        }
    }
}

#[cfg(test)]
pub(in crate::workspace_graph) mod before;
#[cfg(test)]
mod tests;

mod typed;

// Only an opaque report::Published can supply this backing and ledger. This
// does not certify arbitrary legacy Vecs or turn an unreceived error into paid
// output. The complete compatibility conversion above remains unchanged.
impl CoreFailure {
    pub(super) fn receive_published(&mut self, published: super::report::Published) {
        let (report, ledger) = published.into_parts();
        *self = Self::Received {
            report,
            ledger: ClassifierLedgerDebug(ledger),
        };
    }
}
