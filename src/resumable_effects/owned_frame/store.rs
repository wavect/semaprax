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
pub(crate) mod source_v8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StoreProfile {
    OwnedFrameV1,
    SourceOwnedWaitV8,
}
impl StoreProfile {
    fn journal_limit(self) -> usize {
        match self {
            Self::OwnedFrameV1 => codec::MAX_JOURNAL,
            Self::SourceOwnedWaitV8 => 16 * 1024 * 1024,
        }
    }
    fn record_limit(self) -> usize {
        match self {
            Self::OwnedFrameV1 => codec::MAX_RECORD,
            Self::SourceOwnedWaitV8 => self.journal_limit(),
        }
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
    pub(super) fn fresh_profile(
        _: File,
        _: (u64, u64),
        _: &SourceCheckpointScope,
        _: StoreProfile,
        _: String,
    ) -> Result<Self, Error> {
        Err(Error::UnsupportedStore)
    }
    pub(super) fn recover_profile(
        _: File,
        _: OwnedFrameStoreRegistration,
        _: &SourceCheckpointScope,
        _: StoreProfile,
        _: String,
    ) -> Result<Self, Error> {
        Err(Error::UnsupportedStore)
    }
    pub(super) fn validate_process(&self) -> Result<(), Error> {
        Err(Error::UnsupportedStore)
    }
    pub(super) fn validate_profile(&self, _: StoreProfile) -> Result<(), Error> {
        Err(Error::UnsupportedStore)
    }
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
    pub(crate) fn validate_scope(&self, _: &SourceCheckpointScope) -> Result<(), Error> {
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
