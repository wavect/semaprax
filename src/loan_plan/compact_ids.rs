//! Exact immutable proof lists; order and duplicate entries remain untouched.
use super::{error, LoanId};
use crate::diagnostic::Diagnostic;

pub(super) fn from_vec(ids: Vec<LoanId>) -> Result<Box<[LoanId]>, Diagnostic> {
    // into_boxed_slice may move a nonempty over-capacity vector into a new
    // exact buffer. Charge that new buffer while the old one is still live.
    // Empty conversion releases storage; exact-capacity conversion reuses it.
    if !ids.is_empty() && ids.capacity() != ids.len() {
        let bytes = ids
            .len()
            .checked_mul(std::mem::size_of::<LoanId>())
            .ok_or_else(|| error("immutable loan list allocation overflows"))?;
        if !crate::bounded_output::reserve_active_required(bytes) {
            return Err(error(
                "immutable loan list exceeds its active allocation budget",
            ));
        }
    }
    Ok(ids.into_boxed_slice())
}

#[cfg(test)]
mod tests;
