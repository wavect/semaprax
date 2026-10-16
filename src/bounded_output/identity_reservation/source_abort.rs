//! The real invocation owns fixed source-abort storage before verification.
use super::*;
use crate::source_verify::funding::AbortPayload;
impl IdentityReservation {
    pub(crate) fn source_abort_storage_bytes() -> usize {
        size_of::<AbortPayload>() // one-element backing
            + size_of::<AbortPayload>() // initialized push operand
            + size_of::<Vec<AbortPayload>>()
            + size_of::<Result<Vec<AbortPayload>, Refused>>()
    }
    pub(crate) fn source_abort_storage(&mut self) -> Result<Vec<AbortPayload>, Refused> {
        self.reserve(Self::source_abort_storage_bytes())?;
        let mut storage = Vec::new();
        storage.try_reserve_exact(1).map_err(|_| {
            poison(&self.budget, 0);
            Refused::Allocation
        })?;
        if storage.capacity() != 1 {
            let excess = storage
                .capacity()
                .saturating_sub(1)
                .checked_mul(size_of::<AbortPayload>())
                .ok_or_else(|| {
                    poison(&self.budget, usize::MAX);
                    Refused::Arithmetic
                })?;
            self.reserve(excess)?;
            poison(&self.budget, 0);
            return Err(Refused::Capacity);
        }
        Ok(storage)
    }
}
