use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::fs::{self, FileType, Mode, OFlags};

use super::{
    validate_path, FileAccess, FileFailure, FileKind, FileMetadata, FileProvider, MAX_FILE_BYTES,
};

static NEXT_ATOMIC_TEMP: AtomicU64 = AtomicU64::new(0);
const ATOMIC_TEMP_ATTEMPTS: usize = 64;

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

    fn stat(&mut self, path: &[u8]) -> Result<FileMetadata, FileFailure> {
        if self.access == FileAccess::WriteOnly {
            return Err(FileFailure::AuthorityDenied);
        }
        if path.is_empty() {
            return Ok(FileMetadata {
                kind: FileKind::Directory,
                size: 0,
            });
        }
        let (parent, name) = self.parent(path)?;
        let metadata = fs::statat(
            &parent,
            OsStr::from_bytes(&name),
            fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(io_failure)?;
        match FileType::from_raw_mode(metadata.st_mode) {
            FileType::RegularFile => {
                let size =
                    u64::try_from(metadata.st_size).map_err(|_| FileFailure::CapacityExceeded)?;
                if size > u64::MAX / 4 {
                    return Err(FileFailure::CapacityExceeded);
                }
                Ok(FileMetadata {
                    kind: FileKind::File,
                    size,
                })
            }
            FileType::Directory => Ok(FileMetadata {
                kind: FileKind::Directory,
                size: 0,
            }),
            _ => Err(FileFailure::InvalidFileType),
        }
    }

    fn list(&mut self, path: &[u8], max: usize) -> Result<Vec<u8>, FileFailure> {
        if self.access == FileAccess::WriteOnly {
            return Err(FileFailure::AuthorityDenied);
        }
        if !path.is_empty() {
            validate_path(path)?;
        }
        if max > MAX_FILE_BYTES {
            return Err(FileFailure::CapacityExceeded);
        }
        let directory = if path.is_empty() {
            rustix::io::dup(&self.root).map_err(io_failure)?
        } else {
            let (parent, name) = self.parent(path)?;
            fs::openat(
                &parent,
                OsStr::from_bytes(&name),
                directory_flags(),
                Mode::empty(),
            )
            .map_err(io_failure)?
        };
        let mut directory = fs::Dir::read_from(&directory).map_err(io_failure)?;
        let mut names = Vec::new();
        while let Some(entry) = directory.read() {
            let entry = entry.map_err(io_failure)?;
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            if name.is_empty() || name.contains(&0) || name.contains(&b'/') {
                return Err(FileFailure::IoFailure);
            }
            if names.len() >= 1024 {
                return Err(FileFailure::CapacityExceeded);
            }
            names.push(name.to_vec());
        }
        names.sort();
        let mut out = Vec::new();
        for name in names {
            let next = out
                .len()
                .checked_add(name.len() + 1)
                .ok_or(FileFailure::CapacityExceeded)?;
            if next > max {
                return Err(FileFailure::CapacityExceeded);
            }
            out.extend_from_slice(&name);
            out.push(0);
        }
        Ok(out)
    }

    fn create_dir(&mut self, path: &[u8]) -> Result<usize, FileFailure> {
        if self.access == FileAccess::ReadOnly {
            return Err(FileFailure::AuthorityDenied);
        }
        let (parent, name) = self.parent(path)?;
        fs::mkdirat(
            &parent,
            OsStr::from_bytes(&name),
            Mode::RUSR | Mode::WUSR | Mode::XUSR,
        )
        .map_err(io_failure)?;
        Ok(0)
    }

    fn remove(&mut self, path: &[u8]) -> Result<usize, FileFailure> {
        if self.access == FileAccess::ReadOnly {
            return Err(FileFailure::AuthorityDenied);
        }
        let (parent, name) = self.parent(path)?;
        let metadata = fs::statat(
            &parent,
            OsStr::from_bytes(&name),
            fs::AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(io_failure)?;
        let flags = match FileType::from_raw_mode(metadata.st_mode) {
            FileType::Directory => fs::AtFlags::REMOVEDIR,
            FileType::RegularFile => fs::AtFlags::empty(),
            _ => return Err(FileFailure::InvalidFileType),
        };
        fs::unlinkat(&parent, OsStr::from_bytes(&name), flags).map_err(io_failure)?;
        Ok(0)
    }

    fn write_atomic(&mut self, path: &[u8], data: &[u8]) -> Result<usize, FileFailure> {
        if self.access == FileAccess::ReadOnly {
            return Err(FileFailure::AuthorityDenied);
        }
        validate_path(path)?;
        if data.len() > MAX_FILE_BYTES {
            return Err(FileFailure::CapacityExceeded);
        }
        let (parent, name) = self.parent(path)?;
        match fs::statat(
            &parent,
            OsStr::from_bytes(&name),
            fs::AtFlags::SYMLINK_NOFOLLOW,
        ) {
            Ok(metadata) if FileType::from_raw_mode(metadata.st_mode) == FileType::RegularFile => {}
            Ok(_) => return Err(FileFailure::InvalidFileType),
            Err(rustix::io::Errno::NOENT) => {}
            Err(error) => return Err(io_failure(error)),
        }
        let mut temporary = None;
        let mut file = None;
        for _ in 0..ATOMIC_TEMP_ATTEMPTS {
            let candidate = format!(
                ".semaprax-atomic-{}-{}",
                std::process::id(),
                NEXT_ATOMIC_TEMP.fetch_add(1, Ordering::Relaxed)
            );
            if candidate.as_bytes() == name {
                continue;
            }
            match fs::openat(
                &parent,
                OsStr::new(&candidate),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ) {
                Ok(opened) => {
                    temporary = Some(candidate);
                    file = Some(opened);
                    break;
                }
                Err(rustix::io::Errno::EXIST) => continue,
                Err(error) => return Err(io_failure(error)),
            }
        }
        let temp = temporary.ok_or(FileFailure::IoFailure)?;
        let file = file.expect("atomic temporary file accompanies its name");
        let written = std::fs::File::from(file).write_all(data);
        if written.is_err() {
            let _ = fs::unlinkat(&parent, OsStr::new(&temp), fs::AtFlags::empty());
            return Err(FileFailure::IoFailure);
        }
        if let Err(error) = fs::renameat(
            &parent,
            OsStr::new(&temp),
            &parent,
            OsStr::from_bytes(&name),
        ) {
            let _ = fs::unlinkat(&parent, OsStr::new(&temp), fs::AtFlags::empty());
            return Err(io_failure(error));
        }
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
