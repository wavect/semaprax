//! Explicit filesystem authority for checked filesystem invocations.
//!
//! Providers receive already bounded relative byte paths. They are also safe
//! to call directly: the shipped providers repeat path and payload validation.
//! No default provider discovers a current directory or opens ambient files.

use std::collections::{BTreeMap, BTreeSet};

pub const MAX_PATH_BYTES: usize = 4096;
pub const MAX_FILE_BYTES: usize = 65_536;
pub const MAX_TOTAL_BYTES: usize = 1_048_576;
pub const MAX_OPERATIONS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileFailure {
    InvalidPath,
    NotFound,
    AlreadyExists,
    CapacityExceeded,
    IoFailure,
    AuthorityDenied,
    InvalidFileType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileKind {
    File,
    Directory,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileMetadata {
    pub kind: FileKind,
    pub size: u64,
}

impl FileFailure {
    pub const fn status_code(self) -> u32 {
        match self {
            Self::InvalidPath => 1,
            Self::NotFound => 2,
            Self::AlreadyExists => 3,
            Self::CapacityExceeded => 4,
            Self::IoFailure => 5,
            Self::AuthorityDenied => 6,
            Self::InvalidFileType => 7,
        }
    }
}

/// Admit only nonempty relative slash-separated components. This is a byte
/// grammar, not Unicode normalization or permission to access a host path.
pub fn validate_path(path: &[u8]) -> Result<(), FileFailure> {
    if path.is_empty()
        || path.len() > MAX_PATH_BYTES
        || path.iter().any(|byte| matches!(byte, 0 | b'\\' | b':'))
        || path
            .split(|byte| *byte == b'/')
            .any(|part| part.is_empty() || part == b"." || part == b"..")
    {
        return Err(FileFailure::InvalidPath);
    }
    Ok(())
}

fn parent_path(path: &[u8]) -> Option<&[u8]> {
    path.iter()
        .rposition(|byte| *byte == b'/')
        .map(|index| &path[..index])
}

/// The caller explicitly supplies authority. Read results transfer their owned
/// bytes; write input is borrowed only for that synchronous operation. Physical
/// writes cannot be rolled back by a later language failure. `settle` releases
/// invocation transients; it does not imply transaction commit or durability.
pub trait FileProvider {
    fn read(&mut self, path: &[u8], max: usize) -> Result<Vec<u8>, FileFailure>;
    fn write_new(&mut self, path: &[u8], data: &[u8]) -> Result<usize, FileFailure>;
    fn stat(&mut self, _path: &[u8]) -> Result<FileMetadata, FileFailure> {
        Err(FileFailure::AuthorityDenied)
    }
    fn list(&mut self, _path: &[u8], _max: usize) -> Result<Vec<u8>, FileFailure> {
        Err(FileFailure::AuthorityDenied)
    }
    fn create_dir(&mut self, _path: &[u8]) -> Result<usize, FileFailure> {
        Err(FileFailure::AuthorityDenied)
    }
    fn remove(&mut self, _path: &[u8]) -> Result<usize, FileFailure> {
        Err(FileFailure::AuthorityDenied)
    }
    fn write_atomic(&mut self, _path: &[u8], _data: &[u8]) -> Result<usize, FileFailure> {
        Err(FileFailure::AuthorityDenied)
    }
    fn settle(&mut self) {}
}

#[derive(Default)]
pub struct DeniedFileProvider;

impl FileProvider for DeniedFileProvider {
    fn read(&mut self, _path: &[u8], _max: usize) -> Result<Vec<u8>, FileFailure> {
        Err(FileFailure::AuthorityDenied)
    }
    fn write_new(&mut self, _path: &[u8], _data: &[u8]) -> Result<usize, FileFailure> {
        Err(FileFailure::AuthorityDenied)
    }
}

/// A deterministic explicit in-memory filesystem. It makes no physical-file
/// claim and has no ambient access. Files persist across settled invocations.
pub struct FixtureFileProvider {
    files: BTreeMap<Vec<u8>, Vec<u8>>,
    directories: BTreeSet<Vec<u8>>,
    writable: bool,
    settlements: usize,
}

impl FixtureFileProvider {
    pub fn new(
        files: impl IntoIterator<Item = (Vec<u8>, Vec<u8>)>,
        writable: bool,
    ) -> Result<Self, FileFailure> {
        let mut result = Self {
            files: BTreeMap::new(),
            directories: BTreeSet::new(),
            writable,
            settlements: 0,
        };
        let mut bytes = 0usize;
        for (path, data) in files {
            validate_path(&path)?;
            bytes = bytes
                .checked_add(data.len())
                .ok_or(FileFailure::CapacityExceeded)?;
            if data.len() > MAX_FILE_BYTES || bytes > MAX_TOTAL_BYTES || result.files.len() >= 1024
            {
                return Err(FileFailure::CapacityExceeded);
            }
            for (index, byte) in path.iter().enumerate() {
                if *byte == b'/' {
                    result.directories.insert(path[..index].to_vec());
                }
            }
            if result.files.insert(path, data).is_some() {
                return Err(FileFailure::AlreadyExists);
            }
        }
        Ok(result)
    }

    pub fn files(&self) -> &BTreeMap<Vec<u8>, Vec<u8>> {
        &self.files
    }
    pub fn settlements(&self) -> usize {
        self.settlements
    }

    fn add_parent_directories(&mut self, path: &[u8]) {
        for (index, byte) in path.iter().enumerate() {
            if *byte == b'/' {
                self.directories.insert(path[..index].to_vec());
            }
        }
    }
}

impl FileProvider for FixtureFileProvider {
    fn read(&mut self, path: &[u8], max: usize) -> Result<Vec<u8>, FileFailure> {
        validate_path(path)?;
        if max > MAX_FILE_BYTES {
            return Err(FileFailure::CapacityExceeded);
        }
        let bytes = self.files.get(path).ok_or(FileFailure::NotFound)?;
        if bytes.len() > max {
            return Err(FileFailure::CapacityExceeded);
        }
        Ok(bytes.clone())
    }
    fn write_new(&mut self, path: &[u8], data: &[u8]) -> Result<usize, FileFailure> {
        validate_path(path)?;
        if !self.writable {
            return Err(FileFailure::AuthorityDenied);
        }
        if self.files.contains_key(path) || self.directories.contains(path) {
            return Err(FileFailure::AlreadyExists);
        }
        if data.len() > MAX_FILE_BYTES
            || self.files.len() >= 1024
            || self.files.values().map(Vec::len).sum::<usize>() + data.len() > MAX_TOTAL_BYTES
        {
            return Err(FileFailure::CapacityExceeded);
        }
        self.add_parent_directories(path);
        self.files.insert(path.to_vec(), data.to_vec());
        Ok(data.len())
    }
    fn stat(&mut self, path: &[u8]) -> Result<FileMetadata, FileFailure> {
        if path.is_empty() {
            return Ok(FileMetadata {
                kind: FileKind::Directory,
                size: 0,
            });
        }
        validate_path(path)?;
        if let Some(data) = self.files.get(path) {
            return Ok(FileMetadata {
                kind: FileKind::File,
                size: data.len() as u64,
            });
        }
        if self.directories.contains(path) {
            return Ok(FileMetadata {
                kind: FileKind::Directory,
                size: 0,
            });
        }
        Err(FileFailure::NotFound)
    }
    fn list(&mut self, path: &[u8], max: usize) -> Result<Vec<u8>, FileFailure> {
        if !path.is_empty() {
            validate_path(path)?;
        }
        if max > MAX_FILE_BYTES {
            return Err(FileFailure::CapacityExceeded);
        }
        if !path.is_empty() {
            if self.files.contains_key(path) {
                return Err(FileFailure::InvalidFileType);
            }
            if !self.directories.contains(path) {
                return Err(FileFailure::NotFound);
            }
        }
        let prefix = if path.is_empty() {
            Vec::new()
        } else {
            [path, b"/"].concat()
        };
        let mut names: BTreeSet<Vec<u8>> = BTreeSet::new();
        for key in self.files.keys().chain(self.directories.iter()) {
            if let Some(rest) = key.strip_prefix(prefix.as_slice()) {
                if let Some(name) = rest.split(|b| *b == b'/').next() {
                    if !name.is_empty() && !names.iter().any(|known| known.as_slice() == name) {
                        if names.len() >= 1024 {
                            return Err(FileFailure::CapacityExceeded);
                        }
                        names.insert(name.to_vec());
                    }
                }
            }
        }
        let mut out = Vec::new();
        for name in names {
            let next = out
                .len()
                .checked_add(name.len() + 1)
                .ok_or(FileFailure::CapacityExceeded)?;
            if next > max {
                return Err(FileFailure::CapacityExceeded);
            }
            out.extend(name);
            out.push(0);
        }
        Ok(out)
    }
    fn create_dir(&mut self, path: &[u8]) -> Result<usize, FileFailure> {
        validate_path(path)?;
        if !self.writable {
            return Err(FileFailure::AuthorityDenied);
        }
        if self.files.contains_key(path) || self.directories.contains(path) {
            return Err(FileFailure::AlreadyExists);
        }
        if let Some(parent) = parent_path(path) {
            if self.files.contains_key(parent) {
                return Err(FileFailure::InvalidFileType);
            }
            if !parent.is_empty() && !self.directories.contains(parent) {
                return Err(FileFailure::NotFound);
            }
        }
        self.directories.insert(path.to_vec());
        Ok(0)
    }
    fn remove(&mut self, path: &[u8]) -> Result<usize, FileFailure> {
        validate_path(path)?;
        if !self.writable {
            return Err(FileFailure::AuthorityDenied);
        }
        if self.files.remove(path).is_some() {
            return Ok(0);
        }
        if self.directories.contains(path) {
            let p = [path, b"/"].concat();
            if self.files.keys().any(|x| x.starts_with(&p))
                || self.directories.iter().any(|x| x.starts_with(&p))
            {
                return Err(FileFailure::IoFailure);
            }
            self.directories.remove(path);
            return Ok(0);
        }
        Err(FileFailure::NotFound)
    }
    fn write_atomic(&mut self, path: &[u8], data: &[u8]) -> Result<usize, FileFailure> {
        validate_path(path)?;
        if !self.writable {
            return Err(FileFailure::AuthorityDenied);
        }
        if data.len() > MAX_FILE_BYTES {
            return Err(FileFailure::CapacityExceeded);
        }
        if self.directories.contains(path) {
            return Err(FileFailure::InvalidFileType);
        }
        if let Some(parent) = parent_path(path) {
            if self.files.contains_key(parent) {
                return Err(FileFailure::InvalidFileType);
            }
            if !parent.is_empty() && !self.directories.contains(parent) {
                return Err(FileFailure::NotFound);
            }
        }
        let replaced = self.files.get(path).map_or(0, Vec::len);
        let total = self
            .files
            .values()
            .map(Vec::len)
            .sum::<usize>()
            .checked_sub(replaced)
            .and_then(|total| total.checked_add(data.len()))
            .ok_or(FileFailure::CapacityExceeded)?;
        if total > MAX_TOTAL_BYTES || (!self.files.contains_key(path) && self.files.len() >= 1024) {
            return Err(FileFailure::CapacityExceeded);
        }
        self.files.insert(path.to_vec(), data.to_vec());
        Ok(data.len())
    }
    fn settle(&mut self) {
        self.settlements += 1;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileAccess {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub use unix::ScopedFileProvider;

#[cfg(test)]
mod tests;
