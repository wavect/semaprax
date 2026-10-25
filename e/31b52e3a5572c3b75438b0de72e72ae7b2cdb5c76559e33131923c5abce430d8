//! Constructor-covered forward-reachability storage, never a resolver discount.
//!
//! The existing paid modular census supplies complete scalar functions. The
//! full resolver and fixed-function bundles are unchanged. Uncovered functions,
//! defaults, generic materializations and synthetic constructors keep all terms.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Counts {
    pub expressions: usize,
    pub maximum_function: usize,
    pub functions: usize,
}
impl Counts {
    pub(super) fn bytes(self) -> Option<usize> {
        self.expressions
            .checked_mul(crate::loan_plan::REACHABILITY_BYTES_PER_EXPRESSION)
    }
    pub(super) fn later(self) -> Option<usize> {
        self.maximum_function
            .checked_mul(crate::loan_plan::REACHABILITY_BYTES_PER_EXPRESSION)
    }
}

// Actual/formal Counts receivers, checked-byte Options, retained/global cursor
// fields and the terminal diagnostic receivers of the new peak calculation.
// This is paid in addition to every unchanged full/fixed allowance.
pub(super) const RECEIVING: usize = std::mem::size_of::<(
    [Counts; 8],
    [usize; 32],
    [Option<usize>; 16],
    [bool; 32],
    [Result<usize, Vec<crate::diagnostic::Diagnostic>>; 8],
)>();
