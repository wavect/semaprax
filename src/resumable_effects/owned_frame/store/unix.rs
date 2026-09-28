//! Relative, no-follow store operations on independently registered identities.
use super::*;
use rustix::fd::AsFd;
use rustix::fs::{FileType, FlockOperation, Mode, OFlags};
use std::io::{Read, Seek, SeekFrom, Write};

pub(crate) struct RegisteredJournalLease {
    directory: File,
    file: File,
    identity: OwnedFrameStoreIdentity,
    name: String,
    scope: Value,
    profile: StoreProfile,
    length: u64,
    poisoned: bool,
    creator_process: u32,
    #[cfg(test)]
    pub(super) fault: Option<(usize, bool)>,
    #[cfg(test)]
    writes: usize,
}
fn failure(_: impl Sized) -> Error {
    Error::Storage
}
fn directory_id(file: &File) -> Result<(u64, u64), Error> {
    let stat = rustix::fs::fstat(file.as_fd()).map_err(failure)?;
    if !FileType::from_raw_mode(stat.st_mode).is_dir()
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode & 0o077 != 0
    {
        return Err(Error::Binding);
    }
    Ok((stat.st_dev as u64, stat.st_ino as u64))
}
pub(super) fn validate_directory_identity(file: &File, expected: (u64, u64)) -> Result<(), Error> {
    if directory_id(file)? != expected {
        return Err(Error::Binding);
    }
    Ok(())
}
fn file_id(file: &File) -> Result<(u64, u64, u64), Error> {
    let stat = rustix::fs::fstat(file.as_fd()).map_err(failure)?;
    if !FileType::from_raw_mode(stat.st_mode).is_file()
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode & 0o077 != 0
        || stat.st_nlink != 1
    {
        return Err(Error::Binding);
    }
    Ok((stat.st_dev as u64, stat.st_ino as u64, stat.st_size as u64))
}
fn lock(file: &File) -> Result<(), Error> {
    rustix::fs::flock(file.as_fd(), FlockOperation::NonBlockingLockExclusive).map_err(|error| {
        if error == rustix::io::Errno::WOULDBLOCK {
            Error::Busy
        } else {
            Error::Storage
        }
    })
}
fn sync_directory(file: &File) -> Result<(), Error> {
    #[cfg(target_vendor = "apple")]
    if rustix::fs::fcntl_fullfsync(file).is_ok() {
        return Ok(());
    }
    rustix::fs::fsync(file).map_err(failure)
}
impl RegisteredJournalLease {
    pub(crate) fn fresh(
        directory: File,
        expected_directory: (u64, u64),
        scope: &SourceCheckpointScope,
    ) -> Result<Self, Error> {
        Self::fresh_profile(
            directory,
            expected_directory,
            scope,
            StoreProfile::OwnedFrameV1,
            super::name(scope),
        )
    }
    pub(super) fn fresh_profile(
        directory: File,
        expected_directory: (u64, u64),
        scope: &SourceCheckpointScope,
        profile: StoreProfile,
        name: String,
    ) -> Result<Self, Error> {
        codec::scope(scope)?;
        if directory_id(&directory)? != expected_directory {
            return Err(Error::Binding);
        }
        let fd = rustix::fs::openat(
            &directory,
            name.as_str(),
            OFlags::RDWR
                | OFlags::APPEND
                | OFlags::CREATE
                | OFlags::EXCL
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map_err(|e| {
            if e == rustix::io::Errno::EXIST {
                Error::Binding
            } else {
                Error::Storage
            }
        })?;
        let file = File::from(fd);
        if profile == StoreProfile::SourceOwnedWaitV8 {
            lock(&file)?;
        }
        let (file_device, file_inode, length) = file_id(&file)?;
        if profile == StoreProfile::OwnedFrameV1 {
            lock(&file)?;
        }
        sync_directory(&directory)?;
        Ok(Self {
            directory,
            file,
            identity: OwnedFrameStoreIdentity {
                directory_device: expected_directory.0,
                directory_inode: expected_directory.1,
                file_device,
                file_inode,
            },
            name,
            scope: codec::scope(scope)?,
            profile,
            length,
            poisoned: false,
            creator_process: std::process::id(),
            #[cfg(test)]
            fault: None,
            #[cfg(test)]
            writes: 0,
        })
    }
    pub(crate) fn recover(
        directory: File,
        registration: OwnedFrameStoreRegistration,
        scope: &SourceCheckpointScope,
    ) -> Result<Self, Error> {
        Self::recover_profile(
            directory,
            registration,
            scope,
            StoreProfile::OwnedFrameV1,
            super::name(scope),
        )
    }
    pub(super) fn recover_profile(
        directory: File,
        registration: OwnedFrameStoreRegistration,
        scope: &SourceCheckpointScope,
        profile: StoreProfile,
        name: String,
    ) -> Result<Self, Error> {
        if registration.scope != codec::scope(scope)?
            || directory_id(&directory)?
                != (
                    registration.identity.directory_device,
                    registration.identity.directory_inode,
                )
        {
            return Err(Error::Binding);
        }
        let fd = rustix::fs::openat(
            &directory,
            name.as_str(),
            OFlags::RDWR | OFlags::APPEND | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(failure)?;
        let file = File::from(fd);
        let (device, inode, length) = file_id(&file)?;
        if (device, inode)
            != (
                registration.identity.file_device,
                registration.identity.file_inode,
            )
        {
            return Err(Error::Binding);
        }
        lock(&file)?;
        let lease = Self {
            directory,
            file,
            identity: registration.identity,
            name,
            scope: codec::scope(scope)?,
            profile,
            length,
            poisoned: false,
            creator_process: std::process::id(),
            #[cfg(test)]
            fault: None,
            #[cfg(test)]
            writes: 0,
        };
        lease.check()?;
        Ok(lease)
    }
    pub(super) fn validate_process(&self) -> Result<(), Error> {
        if self.creator_process != std::process::id() {
            return Err(Error::Policy);
        }
        Ok(())
    }
    pub(super) fn validate_profile(&self, expected: StoreProfile) -> Result<(), Error> {
        if self.creator_process != std::process::id() {
            return Err(Error::Policy);
        }
        if self.profile != expected {
            return Err(Error::Binding);
        }
        self.check()
    }
    pub(crate) fn validate_current(&self) -> Result<(), Error> {
        self.check()
    }
    pub(crate) fn validate_scope(&self, expected: &SourceCheckpointScope) -> Result<(), Error> {
        if self.creator_process != std::process::id() {
            return Err(Error::Policy);
        }
        if self.scope != codec::scope(expected)? {
            return Err(Error::Binding);
        }
        self.validate_current()
    }
    pub(crate) fn identity(&self) -> OwnedFrameStoreIdentity {
        self.identity
    }
    fn check(&self) -> Result<(), Error> {
        if self.creator_process != std::process::id() {
            return Err(Error::Policy);
        }
        if self.poisoned {
            return Err(Error::InDoubt);
        }
        if directory_id(&self.directory)?
            != (
                self.identity.directory_device,
                self.identity.directory_inode,
            )
        {
            return Err(Error::Binding);
        }
        let (device, inode, length) = file_id(&self.file)?;
        if (device, inode) != (self.identity.file_device, self.identity.file_inode)
            || length != self.length
        {
            return Err(Error::Binding);
        }
        let entry = rustix::fs::statat(
            &self.directory,
            self.name.as_str(),
            rustix::fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(failure)?;
        if !FileType::from_raw_mode(entry.st_mode).is_file()
            || (entry.st_dev as u64, entry.st_ino as u64) != (device, inode)
        {
            return Err(Error::Binding);
        }
        Ok(())
    }
    pub(crate) fn read(&mut self) -> Result<Vec<u8>, Error> {
        self.check()?;
        if self.length > self.profile.journal_limit() as u64 {
            return Err(Error::Capacity);
        }
        self.file.seek(SeekFrom::Start(0)).map_err(failure)?;
        let mut bytes = Vec::new();
        Read::by_ref(&mut self.file)
            .take(self.profile.journal_limit() as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(failure)?;
        if bytes.len() > self.profile.journal_limit() || bytes.len() as u64 != self.length {
            return Err(Error::Capacity);
        }
        Ok(bytes)
    }
    pub(crate) fn append(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.check()?;
        if bytes.is_empty()
            || bytes.len() > self.profile.record_limit()
            || self
                .length
                .checked_add(bytes.len() as u64)
                .filter(|n| *n <= self.profile.journal_limit() as u64)
                .is_none()
        {
            return Err(Error::Capacity);
        }
        #[cfg(test)]
        {
            self.writes += 1;
            if self.fault == Some((self.writes, false)) {
                self.poisoned = true;
                return Err(Error::InDoubt);
            }
        }
        // From the first write attempt every error is ambiguous, even if the
        // caller later sees no complete row. Only new authenticated recovery
        // may decide its tail; this live description never retries.
        self.poisoned = true;
        self.file.write_all(bytes).map_err(|_| Error::InDoubt)?;
        self.file.sync_all().map_err(|_| Error::InDoubt)?;
        self.length += bytes.len() as u64;
        #[cfg(test)]
        if self.fault == Some((self.writes, true)) {
            return Err(Error::InDoubt);
        }
        self.poisoned = false;
        Ok(())
    }
}
impl RegisteredJournalLease {
    fn unlock_creator_only(&self) {
        if self.creator_process == std::process::id() {
            let _ = rustix::fs::flock(self.file.as_fd(), FlockOperation::Unlock);
        }
        // Inherited open descriptions are close-only here; LOCK_UN would
        // unlock the creator process's still-active lease.
    }
}
impl Drop for RegisteredJournalLease {
    fn drop(&mut self) {
        self.unlock_creator_only();
    }
}

#[cfg(test)]
impl RegisteredJournalLease {
    pub(super) fn mark_foreign_process_for_test(&mut self) {
        self.creator_process = std::process::id().wrapping_add(1);
    }
    pub(crate) fn fail_append(&mut self, number: usize, after_persistence: bool) {
        self.fault = Some((number, after_persistence));
    }
}
#[cfg(test)]
mod tests;
