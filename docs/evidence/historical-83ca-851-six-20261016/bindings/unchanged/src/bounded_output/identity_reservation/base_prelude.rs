//! Distinct fixed-base work report from the existing affine prepaid owner.
//! No budget failure is fabricated, no receipt is formatted, and both original
//! buffers transfer unchanged. Root routing must preserve typed work failure.
use super::{IdentityReservation, MESSAGE};
use crate::diagnostic::Diagnostic;

const WORK_MESSAGE: &str = "base prelude producer work bound refused";
const _: () = assert!(WORK_MESSAGE.len() <= MESSAGE.len());

impl IdentityReservation {
    /// The root selects this only for its distinct typed base-producer work
    /// refusal. Acquisition already proved one empty Diagnostic slot and exact
    /// MESSAGE text capacity, which statically covers this shorter fixed form.
    pub(crate) fn publish_base_prelude_work(mut self) -> Vec<Diagnostic> {
        self.text.push_str(WORK_MESSAGE);
        self.report.push(Diagnostic::io("SPX-H006", self.text));
        self.report
    }
}

#[cfg(test)]
mod tests;

const EXTENSION_WORK_MESSAGE: &str = "prelude extension producer work bound refused";
const _: () = assert!(EXTENSION_WORK_MESSAGE.len() <= MESSAGE.len());
impl IdentityReservation {
    pub(crate) fn publish_prelude_extension_work(mut self) -> Vec<Diagnostic> {
        self.text.push_str(EXTENSION_WORK_MESSAGE);
        self.report.push(Diagnostic::io("SPX-H006", self.text));
        self.report
    }
}
