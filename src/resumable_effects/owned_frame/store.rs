//! Caller-registered authoritative history. No HMAC/global anti-rollback claim.
use super::{codec, OwnedFrameError as Error};
use crate::resumable_effects::source_checkpoint::SourceCheckpointScope;
use serde_json::{json, Value};
use std::fs::File;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OwnedFrameStoreIdentity {
    pub(crate) directory_device: u64,
    pub(crate) directory_inode: u64,
    pub(crate) file_device: u64,
    pub(crate) file_inode: u64,
}
impl OwnedFrameStoreIdentity {
    pub(super) fn json(self) -> Value {
        json!({"directory_device":self.directory_device,"directory_inode":self.directory_inode,"file_device":self.file_device,"file_inode":self.file_inode})
    }
}
/// Explicit caller authority, not a fact derived from authenticated input.
/// The caller protects ONE history from copy/rebinding/full-tail rollback for
/// the complete invocation lifetime, including after result claim. Revocation
/// or unavailable protection must prevent construction/reopen. This assertion
/// is not a runtime proof of global uniqueness or hardware monotonic storage.
pub(crate) struct OwnedFrameStoreRegistration {
    identity: OwnedFrameStoreIdentity,
    scope: Value,
}
impl OwnedFrameStoreRegistration {
    pub(crate) fn grant_for_trusted_host(
        identity: OwnedFrameStoreIdentity,
        scope: &SourceCheckpointScope,
        authoritative_history_available: bool,
    ) -> Result<Self, Error> {
        if !authoritative_history_available {
            return Err(Error::Policy);
        }
        Ok(Self {
            identity,
            scope: codec::scope(scope)?,
        })
    }
    pub(crate) fn identity(&self) -> OwnedFrameStoreIdentity {
        self.identity
    }
}
#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use unix::RegisteredJournalLease;
#[cfg(not(unix))]
pub(crate) struct RegisteredJournalLease;

#[cfg(not(unix))]
impl RegisteredJournalLease {
    pub(crate) fn fresh(_: File, _: (u64, u64), _: &SourceCheckpointScope) -> Result<Self, Error> {
        Err(Error::UnsupportedStore)
    }
    pub(crate) fn recover(
        _: File,
        _: OwnedFrameStoreRegistration,
        _: &SourceCheckpointScope,
    ) -> Result<Self, Error> {
        Err(Error::UnsupportedStore)
    }
    pub(crate) fn validate_current(&self) -> Result<(), Error> {
        Err(Error::UnsupportedStore)
    }
    pub(crate) fn identity(&self) -> OwnedFrameStoreIdentity {
        unreachable!("unsupported store")
    }
    pub(crate) fn read(&mut self) -> Result<Vec<u8>, Error> {
        Err(Error::UnsupportedStore)
    }
    pub(crate) fn append(&mut self, _: &[u8]) -> Result<(), Error> {
        Err(Error::UnsupportedStore)
    }
}
fn name(scope: &SourceCheckpointScope) -> String {
    let digest = codec::digest(
        b"semaprax.source-owned-frame-journal-name.v1\0",
        scope.invocation_id().as_bytes(),
    );
    format!("{}.owned-frame.jsonl", &digest[7..])
}
