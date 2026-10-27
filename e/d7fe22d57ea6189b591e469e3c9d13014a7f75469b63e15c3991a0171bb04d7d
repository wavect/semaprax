//! Actual report output is ordinary caller IO, not acquired builder backing.
use super::*;
impl WorkspaceReportFailure {
    pub fn write_lines(&self, output: &mut impl fmt::Write, json: bool) -> fmt::Result {
        if let Some(report) = self.reported() {
            return crate::diagnostic::writer::write_lines(output, report, json);
        }
        match &self.0 {
            Owner::Initial(failure) => {
                if json {
                    failure.write_json(output)?;
                } else {
                    failure.write_human(output)?;
                }
            }
            Owner::Selected { form, .. } => {
                Diagnostic::io_static(form.code(), form.message()).write_to(output, json)?
            }
            Owner::Existing(_) | Owner::Unreceived(_) => {
                unreachable!("existing backing was handled above")
            }
        }
        output.write_char('\n')
    }
}

impl WorkspaceReportFailure {
    pub(crate) fn map_change_limit(
        self,
        internal: usize,
        maximum: usize,
        legacy: impl FnOnce(Vec<Diagnostic>) -> Vec<Diagnostic>,
    ) -> Self {
        match self {
            Self(Owner::Initial(mut initial), None) => {
                initial.map_change(internal, maximum);
                Self::initial(initial)
            }
            other => other.map_legacy(legacy),
        }
    }
    pub(crate) fn map_structural_limit(
        self,
        internal: usize,
        candidate_maximum: usize,
        analysis_maximum: usize,
        legacy: impl FnOnce(Vec<Diagnostic>) -> Vec<Diagnostic>,
    ) -> Self {
        match self {
            Self(Owner::Initial(mut initial), None) => {
                initial.map_structural(internal, candidate_maximum, analysis_maximum);
                Self::initial(initial)
            }
            other => other.map_legacy(legacy),
        }
    }
    pub(crate) fn map_operations_limit(
        self,
        internal: usize,
        candidate_maximum: usize,
        operations_maximum: usize,
        legacy: impl FnOnce(Vec<Diagnostic>) -> Vec<Diagnostic>,
    ) -> Self {
        match self {
            Self(Owner::Initial(mut initial), None) => {
                initial.map_operations(internal, candidate_maximum, operations_maximum);
                Self::initial(initial)
            }
            other => other.map_legacy(legacy),
        }
    }
}
