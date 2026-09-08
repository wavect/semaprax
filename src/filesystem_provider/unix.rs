use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use rustix::fs::{self, FileType, Mode, OFlags};

use super::{validate_path, FileAccess, FileFailure, FileProvider, MAX_FILE_BYTES};

/// A retained directory descriptor is the authority root. Every relative path
/// component is opened through a retained parent descriptor without following
/// symlinks. Renaming the caller's root pathname does not redirect this scope.
/// Read/write operations close their transient descriptors on every exit.
pub struct ScopedFileProvider {
    root: OwnedFd,
    access: FileAccess,
}

impl ScopedFileProvider {
    pub fn open(root: impl AsRef<Path>, access: FileAccess) -> Result<Self, FileFailure> {
        let root = root.as_ref();
        if !root.is_absolute() {
            return Err(FileFailure::InvalidPath);
        }
        let root = fs::open(root, directory_flags(), Mode::empty()).map_err(io_failure)?;
        Ok(Self { root, access })
    }

    fn parent(&self, path: &[u8]) -> Result<(OwnedFd, Vec<u8>), FileFailure> {
        validate_path(path)?;
        let mut parts = path.split(|byte| *byte == b'/').peekable();
        let mut parent = rustix::io::dup(&self.root).map_err(io_failure)?;
        while let Some(part) = parts.next() {
            if parts.peek().is_none() {
                return Ok((parent, part.to_vec()));
            }
            parent = fs::openat(
                &parent,
                OsStr::from_bytes(part),
                directory_flags(),
                Mode::empty(),
            )
            .map_err(io_failure)?;
        }
        Err(FileFailure::InvalidPath)
    }
}

impl FileProvider for ScopedFileProvider {
    fn read(&mut self, path: &[u8], max: usize) -> Result<Vec<u8>, FileFailure> {
        validate_path(path)?;
        if self.access == FileAccess::WriteOnly {
            return Err(FileFailure::AuthorityDenied);
        }
        if max > MAX_FILE_BYTES {
            return Err(FileFailure::CapacityExceeded);
        }
        let (parent, name) = self.parent(path)?;
        let file = fs::openat(
            &parent,
            OsStr::from_bytes(&name),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(io_failure)?;
        let metadata = fs::fstat(&file).map_err(io_failure)?;
        if FileType::from_raw_mode(metadata.st_mode) != FileType::RegularFile {
            return Err(FileFailure::InvalidFileType);
        }
        let mut bytes = Vec::new();
        std::fs::File::from(file)
            .take((max + 1) as u64)
            .read_to_end(&mut bytes)
            .map_err(|_| FileFailure::IoFailure)?;
        if bytes.len() > max {
            return Err(FileFailure::CapacityExceeded);
        }
        Ok(bytes)
    }

    fn write_new(&mut self, path: &[u8], data: &[u8]) -> Result<usize, FileFailure> {
        validate_path(path)?;
        if self.access == FileAccess::ReadOnly {
            return Err(FileFailure::AuthorityDenied);
        }
        if data.len() > MAX_FILE_BYTES {
            return Err(FileFailure::CapacityExceeded);
        }
        let (parent, name) = self.parent(path)?;
        let file = fs::openat(
            &parent,
            OsStr::from_bytes(&name),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::RUSR | Mode::WUSR,
        )
        .map_err(io_failure)?;
        // The name is create-new and no existing object is ever overwritten.
        // A failed physical write may leave a partial newly created file; a
        // later language failure cannot retract the published filesystem name.
        std::fs::File::from(file)
            .write_all(data)
            .map_err(|_| FileFailure::IoFailure)?;
        Ok(data.len())
    }
}

fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK
}

fn io_failure(error: rustix::io::Errno) -> FileFailure {
    match error {
        rustix::io::Errno::NOENT => FileFailure::NotFound,
        rustix::io::Errno::EXIST => FileFailure::AlreadyExists,
        rustix::io::Errno::ACCESS | rustix::io::Errno::PERM => FileFailure::AuthorityDenied,
        rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR => FileFailure::InvalidFileType,
        _ => FileFailure::IoFailure,
    }
}
