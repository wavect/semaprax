//! Affine outward report failure. No initial refusal implicitly renders a Vec.
//! Dynamic graph limits/attribution remain existing legacy reports, not funded
//! by selected replay. Output storage belongs to the caller of the renderer.
use crate::cleanup_plan::{SelectedReplayRefusal, SelectedReplayReportForm};
use crate::diagnostic::Diagnostic;
use std::fmt;

#[allow(
    clippy::large_enum_variant,
    reason = "the original affine ledger moves inline; a refusal must not allocate a Box"
)]
enum Owner {
    Existing(Vec<Diagnostic>),
    Selected {
        failure: SelectedReplayRefusal,
        form: SelectedReplayReportForm,
    },
}

/// A sealed, non-Clone failure retaining one exact failed replay owner.
/// `reported()` borrows already-owned diagnostics. When no report was paid,
/// `reported()` returns None; it never allocates a vector or dynamic text.
/// Explicit `into_legacy_unfunded()` is the allocating compatibility boundary.
pub struct WorkspaceReportFailure(Owner, Option<Vec<Diagnostic>>);
impl WorkspaceReportFailure {
    pub(crate) fn existing(diagnostics: Vec<Diagnostic>) -> Self {
        Self(Owner::Existing(diagnostics), None)
    }
    pub(crate) fn selected(
        failure: SelectedReplayRefusal,
        expected: Option<SelectedReplayReportForm>,
    ) -> Self {
        let form = expected.unwrap_or_else(|| failure.form());
        Self(Owner::Selected { failure, form }, None)
    }
    /// Only existing backing is borrowed. Some does not by itself establish
    /// funding: ordinary legacy reports remain ordinary legacy reports.
    pub fn reported(&self) -> Option<&[Diagnostic]> {
        if let Some(report) = &self.1 {
            return Some(report);
        }
        self.retained_report()
    }
    /// The original primary backing is retained even when unlock selected
    /// an existing legacy outward report. This borrow grants no authority.
    pub fn retained_report(&self) -> Option<&[Diagnostic]> {
        match &self.0 {
            Owner::Existing(diagnostics) => Some(diagnostics),
            Owner::Selected { failure, form } => failure.published_for(*form),
        }
    }
    /// Whether this carrier contains an exact selected-replay paid report.
    pub fn has_paid_report(&self) -> bool {
        if self.1.is_some() {
            return false;
        }
        match &self.0 {
            Owner::Selected { failure, form } => failure.published_for(*form).is_some(),
            _ => false,
        }
    }
    /// Transfer an exact paid Vec/String without rebuilding it. Refusal
    /// returns this same affine failure, including its first receipt/ledger.
    pub fn into_paid_report(self) -> Result<Vec<Diagnostic>, Self> {
        if self.1.is_some() {
            return Err(self);
        }
        match self.0 {
            Owner::Selected { failure, form } => failure
                .into_report_retaining(form)
                .map_err(|failure| Self(Owner::Selected { failure, form }, None)),
            owner => Err(Self(owner, None)),
        }
    }
    /// Explicit legacy materialization. Initial/unmatched/unfunded forms may
    /// allocate, outside the paid protocol; existing reports move unchanged.
    /// Calling this does not grant funding, admission or publication authority.
    pub fn into_legacy_unfunded(self) -> Vec<Diagnostic> {
        if let Some(report) = self.1 {
            return report;
        }
        match self.0 {
            Owner::Existing(diagnostics) => diagnostics,
            Owner::Selected { failure, form } => failure
                .into_report(form)
                .unwrap_or_else(crate::cleanup_plan::SelectedReplayUnpaid::into_unfunded),
        }
    }
    #[cfg(test)]
    pub(crate) fn selected_owner_mut(&mut self) -> Option<&mut SelectedReplayRefusal> {
        match &mut self.0 {
            Owner::Selected { failure, .. } => Some(failure),
            _ => None,
        }
    }
}
// This conversion only moves existing legacy backing. There is deliberately
// no conversion from an inline failure to Diagnostic, String or Vec.
impl From<Vec<Diagnostic>> for WorkspaceReportFailure {
    fn from(diagnostics: Vec<Diagnostic>) -> Self {
        Self::existing(diagnostics)
    }
}
impl fmt::Display for WorkspaceReportFailure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(diagnostics) = self.reported() {
            for (index, diagnostic) in diagnostics.iter().enumerate() {
                if index != 0 {
                    writeln!(out)?;
                }
                diagnostic.write_to(out, false)?;
            }
            return Ok(());
        }
        let form = match &self.0 {
            Owner::Selected { form, .. } => *form,
            Owner::Existing(_) => unreachable!(),
        };
        fmt::Display::fmt(&Diagnostic::io_static(form.code(), form.message()), out)
    }
}
impl fmt::Debug for WorkspaceReportFailure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("WorkspaceReportFailure")
            .field("paid", &self.has_paid_report())
            .field("reported", &self.reported())
            .finish_non_exhaustive()
    }
}
impl std::error::Error for WorkspaceReportFailure {}

#[cfg(test)]
mod tests;

impl WorkspaceReportFailure {
    /// Same legacy unlock precedence, WITHOUT releasing or reformatting the
    /// original primary owner. The existing unlock Vec moves unchanged.
    pub(crate) fn with_unlock_override(mut self, diagnostics: Vec<Diagnostic>) -> Self {
        self.1 = Some(diagnostics);
        self
    }
}
#[cfg(test)]
#[allow(
    dead_code,
    reason = "complete historical receiver retained for independent test comparisons"
)]
mod original;

impl WorkspaceReportFailure {
    pub(crate) fn map_legacy(
        self,
        mapper: impl FnOnce(Vec<Diagnostic>) -> Vec<Diagnostic>,
    ) -> Self {
        match self {
            Self(Owner::Existing(report), None) => Self::existing(mapper(report)),
            owner => owner,
        }
    }
}

impl WorkspaceReportFailure {
    pub(crate) fn is_selected_refusal(&self) -> bool {
        matches!(self.0, Owner::Selected { .. })
    }
}
