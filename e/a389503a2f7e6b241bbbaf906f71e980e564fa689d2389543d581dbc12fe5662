//! Prepaid dynamic final-refusal publication on the original core ledger.
use super::receiving::Setup;
use crate::bounded_output::classifier_string::ClassifierLedger;
use crate::bounded_output::{active_remaining, reserve_active_required, ReservationRefusal};
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::diagnostics::BorrowedBuilderHelp;
use std::borrow::Cow;
use std::fmt::{self, Write};
use std::mem::size_of;
mod stages;

pub(super) struct Owner {
    ledger: ClassifierLedger,
    report: Vec<Diagnostic>,
    message: String,
    help: String,
    failure: Option<Setup>,
}
pub(super) struct Published {
    report: Vec<Diagnostic>,
    ledger: ClassifierLedger,
}
pub(super) struct Unpublished {
    pub(super) owner: Owner,
    pub(super) refusal: Setup,
}
struct Count(usize);
impl Write for Count {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        self.0 = self.0.checked_add(value.len()).ok_or(fmt::Error)?;
        Ok(())
    }
}
struct Fixed<'a>(&'a mut String);
impl Write for Fixed<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if value.len() > self.0.capacity() - self.0.len() {
            return Err(fmt::Error);
        }
        self.0.push_str(value);
        Ok(())
    }
}
fn message(out: &mut dyn Write, maximum: usize) -> fmt::Result {
    write!(
        out,
        "Workspace Semantic Graph `builder_bytes` exceeds {maximum}"
    )
}
fn fields(
    out: &mut dyn Write,
    requested: usize,
    remaining: usize,
    floor: usize,
    stage: &str,
) -> fmt::Result {
    write!(
        out,
        "resolved-core phase first sticky ledger refusal: requested {} bytes with {} remaining and {} reserved floor. Authenticated internal stage: {}. These are cumulative reservation bytes, not a retained-memory forecast or process RSS; the exact inner operation is unknown.",
        requested, remaining, floor, stage
    )
}
fn evidence(out: &mut dyn Write, first: Option<ReservationRefusal>) -> fmt::Result {
    match first {
        Some(first)=>fields(out,first.requested,first.remaining,first.floor,first.stage.map_or("unknown",|stage|stage.label())),
        None=>out.write_str("resolved-core phase refused; the first reservation and exact inner operation are unknown."),
    }
}
fn counted(write: impl FnOnce(&mut dyn Write) -> fmt::Result) -> Result<usize, Setup> {
    let mut out = Count(0);
    write(&mut out).map_err(|_| Setup::Arithmetic)?;
    Ok(out.0)
}
fn budget() -> Setup {
    Setup::Budget {
        maximum: super::super::active_builder_limit(),
    }
}
fn reserve(bytes: usize) -> Result<(), Setup> {
    if reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(budget())
    }
}
impl Owner {
    pub(super) fn control_bytes(extra: usize) -> Option<usize> {
        [
            size_of::<Self>(),
            size_of::<Result<Self, Setup>>(),
            size_of::<Option<Self>>(),
            size_of::<Published>(),
            size_of::<Unpublished>(),
            size_of::<Result<Published, Unpublished>>(),
            size_of::<Vec<Diagnostic>>(),
            // The actual tuple initializer/receiver transfers the opaque report.
            // A unit producer then initializes the existing Err payload in place;
            // no new Diagnostic-bearing Result or returned CoreFailure survives.
            2 * size_of::<(Vec<Diagnostic>, ClassifierLedger)>(),
            size_of::<super::receiving::CoreFailure>(),
            size_of::<super::receiving::ClassifierLedgerDebug>(),
            size_of::<&mut super::receiving::CoreFailure>(),
            size_of::<Diagnostic>(),
            size_of::<Option<ReservationRefusal>>(),
            size_of::<fmt::Result>(),
            2 * size_of::<String>(),
            4 * size_of::<Count>(),
            4 * size_of::<Fixed<'_>>(),
            4 * size_of::<Result<usize, Setup>>(),
            extra,
        ]
        .into_iter()
        .try_fold(0usize, usize::checked_add)
    }
    pub(super) fn acquire(maximum: usize, extra_controls: usize) -> Result<Self, Setup> {
        if active_remaining().is_none() {
            return Err(Setup::NoOwner);
        }
        reserve(Self::control_bytes(extra_controls).ok_or(Setup::Arithmetic)?)?;
        // Reserve the full width of the captured usize maximum. Otherwise the
        // same successful construction acquires different report backing at
        // each decimal limit boundary, so its exact minimum changes its bill.
        // Rendering below still uses the actual maximum and original help.
        let message_len = counted(|out| message(out, usize::MAX))?;
        let base_len = counted(|out| write!(out, "{}", BorrowedBuilderHelp(usize::MAX)))?;
        // This is a finite CAPACITY bound, not a claimed live payload length.
        // All three actual fields have usize representations; no receipt is
        // synthesized and no value is inserted into a ledger for this count.
        let suffix_max =
            counted(|out| fields(out, usize::MAX, usize::MAX, usize::MAX, stages::longest()))?
                .max(counted(|out| evidence(out, None))?);
        let help_capacity = base_len
            .checked_add(usize::from(base_len != 0))
            .and_then(|n| n.checked_add(suffix_max))
            .ok_or(Setup::Arithmetic)?;
        let backing = message_len
            .checked_add(help_capacity)
            .and_then(|n| n.checked_add(size_of::<Diagnostic>()))
            .ok_or(Setup::Arithmetic)?;
        reserve(backing)?;
        let ledger = ClassifierLedger::capture();
        let mut report = Vec::new();
        let mut text = String::new();
        let mut help = String::new();
        report.try_reserve_exact(1).map_err(|_| Setup::Allocation)?;
        text.try_reserve_exact(message_len)
            .map_err(|_| Setup::Allocation)?;
        help.try_reserve_exact(help_capacity)
            .map_err(|_| Setup::Allocation)?;
        let excess = report
            .capacity()
            .saturating_sub(1)
            .checked_mul(size_of::<Diagnostic>())
            .and_then(|n| n.checked_add(text.capacity().saturating_sub(message_len)))
            .and_then(|n| n.checked_add(help.capacity().saturating_sub(help_capacity)))
            .ok_or(Setup::Arithmetic)?;
        if excess != 0 {
            reserve(excess)?;
            return Err(Setup::Capacity);
        }
        message(&mut Fixed(&mut text), maximum).map_err(|_| Setup::Capacity)?;
        write!(&mut Fixed(&mut help), "{}", BorrowedBuilderHelp(maximum))
            .map_err(|_| Setup::Capacity)?;
        Ok(Self {
            ledger,
            report,
            message: text,
            help,
            failure: None,
        })
    }
    /// Reuse the report's actual backing for a non-sticky setup primary before
    /// the child restores. No reservation or formatting follows this selection.
    /// Arbitrary diagnostics/foreign receivers/multi-error Vecs are not admitted.
    pub(super) fn seal_setup(mut self, setup: Setup) -> Result<Published, Unpublished> {
        if let Some(refusal) = self.failure {
            return Err(Unpublished {
                owner: self,
                refusal,
            });
        }
        // Dynamic budget help can change during a core attempt. Its existing
        // sticky publication stays separate; do not substitute an earlier TLS
        // snapshot for exceptional non-sticky legacy output.
        if matches!(setup, Setup::Budget { .. }) {
            self.failure = Some(setup);
            return Err(Unpublished {
                owner: self,
                refusal: setup,
            });
        }
        if !self.ledger.current() {
            self.failure = Some(Setup::Detached);
            return Err(Unpublished {
                owner: self,
                refusal: Setup::Detached,
            });
        }
        if crate::bounded_output::active_refused() {
            let refusal = budget();
            self.failure = Some(refusal);
            return Err(Unpublished {
                owner: self,
                refusal,
            });
        }
        let primary = Diagnostic::io_static(
            "SPX-G173",
            match setup {
                Setup::NoOwner => "constructor report receiver has no active builder owner",
                Setup::Allocation => "constructor report receiving allocation refused",
                Setup::Capacity => {
                    "constructor report receiving capacity differs from its paid request"
                }
                Setup::Detached => {
                    "constructor report receiver is detached from its original builder owner"
                }
                Setup::Arithmetic => "constructor report receiving layout overflowed",
                Setup::Budget { .. } => unreachable!("budget form retains its legacy owner"),
            },
        );
        self.report.push(primary);
        Ok(Published {
            report: self.report,
            ledger: self.ledger,
        })
    }
    pub(super) fn seal(mut self) -> Result<Published, Unpublished> {
        if let Some(refusal) = self.failure {
            return Err(Unpublished {
                owner: self,
                refusal,
            });
        }
        if !self.ledger.current() {
            self.failure = Some(Setup::Detached);
            return Err(Unpublished {
                owner: self,
                refusal: Setup::Detached,
            });
        }
        let first = self.ledger.first_refusal();
        let result = (|| {
            if !self.help.is_empty() {
                Fixed(&mut self.help).write_char(' ')?;
            }
            evidence(&mut Fixed(&mut self.help), first)
        })();
        if result.is_err() {
            self.failure = Some(Setup::Capacity);
            return Err(Unpublished {
                owner: self,
                refusal: Setup::Capacity,
            });
        }
        self.report.push(Diagnostic {
            code: "SPX-G171",
            severity: crate::diagnostic::Severity::Error,
            message: Cow::Owned(self.message),
            path: None,
            span: None,
            help: Some(self.help),
        });
        Ok(Published {
            report: self.report,
            ledger: self.ledger,
        })
    }
}
impl Published {
    pub(super) fn into_parts(self) -> (Vec<Diagnostic>, ClassifierLedger) {
        (self.report, self.ledger)
    }
    pub(super) fn into_report(self) -> Vec<Diagnostic> {
        drop(self.ledger);
        self.report
    }
}

#[cfg(test)]
mod tests;
