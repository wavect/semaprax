//! Byte-only receiving authority for changed source presentation boundaries.
//! The ordinary verifier keeps its historical API; no work allowance is created.
use crate::bounded_output::identity_reservation::{IdentityReservation, Refused};
use crate::diagnostic::Diagnostic;
use std::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SourceStop {
    Reservation(Refused),
    /// Private presentation input bypassed the preselected owning invariant route.
    InvariantRoute,
}
#[derive(Debug)]
pub(crate) struct AbortPayload {
    prefix: Vec<Diagnostic>,
    stop: Option<SourceStop>,
}
/// Fixed one-element physical owner, prepared before source verification.
/// Its Vec-width outward carrier never grows while transferring a refusal.
#[derive(Debug)]
pub(crate) struct SourceAbort {
    storage: Vec<AbortPayload>,
}
impl SourceAbort {
    #[cfg(test)]
    pub(crate) fn prefix(&self) -> &[Diagnostic] {
        &self.storage[0].prefix
    }
    #[cfg(test)]
    pub(crate) fn stop(&self) -> SourceStop {
        self.storage[0].stop.expect("sealed source abort")
    }
    pub(crate) fn into_parts(mut self) -> (Vec<Diagnostic>, SourceStop) {
        let payload = self.storage.pop().expect("prepared source abort slot");
        (payload.prefix, payload.stop.expect("sealed source abort"))
    }
}
pub(crate) struct SourceFunding<'o> {
    owner: Option<&'o mut IdentityReservation>,
    first: Option<SourceStop>,
    abort: Option<SourceAbort>,
}
impl<'o> SourceFunding<'o> {
    fn bridge_bytes() -> usize {
        size_of::<Self>()
            + size_of::<Result<Self, Refused>>()
            + size_of::<&mut Self>()
            + size_of::<SourceAbort>()
            + size_of::<Option<SourceAbort>>()
            + size_of::<Result<Vec<Diagnostic>, SourceAbort>>() // profile return
            + size_of::<Result<Vec<Diagnostic>, SourceAbort>>() // facade return
            + size_of::<Result<Vec<Diagnostic>, SourceAbort>>() // owner receiver
            + size_of::<Result<Vec<AbortPayload>, Refused>>() // storage call receiver
            + size_of::<SourceStop>()
            + size_of::<Result<(), SourceStop>>()
    }
    pub(crate) fn entry_receiving_bytes() -> usize {
        size_of::<Result<Self, Refused>>()
            + size_of::<Self>()
            + size_of::<&mut IdentityReservation>()
    }
    /// Fixed bridge and actual one-element abort storage, with no work bank.
    #[cfg(test)]
    pub(crate) fn setup_bytes() -> usize {
        Self::bridge_bytes() + IdentityReservation::source_abort_storage_bytes()
    }
    pub(crate) fn ordinary() -> Self {
        Self {
            owner: None,
            first: None,
            abort: None,
        }
    }
    pub(crate) fn funded(owner: &'o mut IdentityReservation) -> Result<Self, Refused> {
        owner.reserve(Self::bridge_bytes())?;
        let mut storage = owner.source_abort_storage()?;
        storage.push(AbortPayload {
            prefix: Vec::new(),
            stop: None,
        });
        Ok(Self {
            owner: Some(owner),
            first: None,
            abort: Some(SourceAbort { storage }),
        })
    }
    pub(crate) fn abort(&mut self, prefix: Vec<Diagnostic>, stop: SourceStop) -> SourceAbort {
        let mut abort = self.abort.take().expect("ordinary funding never refuses");
        let payload = &mut abort.storage[0];
        payload.prefix = prefix;
        payload.stop = Some(self.first.unwrap_or(stop));
        abort
    }
    pub(crate) fn reserve(&mut self, bytes: usize) -> Result<(), SourceStop> {
        if let Some(first) = self.first {
            return Err(first);
        }
        if let Some(owner) = &mut self.owner {
            if let Err(refusal) = owner.reserve(bytes) {
                let stop = SourceStop::Reservation(refusal);
                self.first = Some(stop);
                return Err(stop);
            }
        }
        Ok(())
    }
    pub(crate) fn specialized_presentation(&mut self) -> Result<(), SourceStop> {
        use crate::ast::{program_source::FunctionRef, Function};
        // Constructor value, return and receiving local; self and payload
        // parameters are borrowed. Reserve before specialized_payload is called.
        self.reserve(
            size_of::<FunctionRef<'_>>()
                + size_of::<FunctionRef<'_>>()
                + size_of::<FunctionRef<'_>>()
                + size_of::<&FunctionRef<'_>>()
                + size_of::<&Function>(),
        )
    }
}
#[cfg(test)]
#[path = "funding/tests.rs"]
mod tests;
