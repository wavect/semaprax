//! Invocation-local result of comparing independently rebuilt complete plans.
//! No attached evidence constructs this receipt and no storage projection is
//! sufficient to replace it. The final consumer supplies the exact subject.
use super::*;
use crate::hir::{hir_error, ResolvedFunction};
use crate::loan_plan::LoanPlan;

pub(in crate::hir) struct Deferred<'a> {
    program: &'a ResolvedProgram,
    first_mismatch: Option<&'a ResolvedFunction>,
}
impl<'a> Deferred<'a> {
    pub(super) fn new(program: &'a ResolvedProgram) -> Self {
        Self {
            program,
            first_mismatch: None,
        }
    }
    pub(super) fn compare(&mut self, function: &'a ResolvedFunction, rebuilt: &LoanPlan) {
        // Complete equality includes schema, every loan field, endpoints and
        // all edges. Never derive a rebuild or an origin from the attachment.
        let matches = function.loan_plan == *rebuilt;
        if self.first_mismatch.is_none() && !matches {
            self.first_mismatch = Some(function);
        }
    }
    pub(super) fn compare_match(&mut self, function: &'a ResolvedFunction, matches: bool) {
        if self.first_mismatch.is_none() && !matches {
            self.first_mismatch = Some(function);
        }
    }
    pub(in crate::hir) fn finish(self, program: &ResolvedProgram) -> Result<(), Diagnostic> {
        if !std::ptr::eq(self.program, program) {
            return Err(hir_error(
                "deferred shared-loan validation belongs to another program",
            ));
        }
        match self.first_mismatch {
            Some(function) => Err(crate::loan_plan::forged_evidence(function)),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests;
