//! Structural producer failure consumes the same already-paid report carrier.
use super::*;
const PRODUCER_MESSAGE: &str = "base prelude producer acquisition refused";
const _: () = assert!(PRODUCER_MESSAGE.len() <= MESSAGE.len());
impl IdentityReservation {
    pub(crate) fn publish_base_prelude_producer(mut self) -> Vec<Diagnostic> {
        self.text.push_str(PRODUCER_MESSAGE);
        self.report.push(Diagnostic::io("SPX-H006", self.text));
        self.report
    }
}
