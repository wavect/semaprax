//! Explicit filesystem authority for checked filesystem invocations.
//!
//! Providers receive already bounded relative byte paths. They are also safe
//! to call directly: the shipped providers repeat path and payload validation.
//! No default provider discovers a current directory or opens ambient files.

use std::collections::BTreeMap;

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

/// The caller explicitly supplies authority. Read results transfer their owned
/// bytes; write input is borrowed only for that synchronous operation. Physical
/// writes cannot be rolled back by a later language failure. `settle` releases
/// invocation transients; it does not imply transaction commit or durability.
pub trait FileProvider {
    fn read(&mut self, path: &[u8], max: usize) -> Result<Vec<u8>, FileFailure>;
    fn write_new(&mut self, path: &[u8], data: &[u8]) -> Result<usize, FileFailure>;
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
        if data.len() > MAX_FILE_BYTES
            || self.files.len() >= 1024
            || self.files.values().map(Vec::len).sum::<usize>() + data.len() > MAX_TOTAL_BYTES
        {
            return Err(FileFailure::CapacityExceeded);
        }
        if self.files.contains_key(path) {
            return Err(FileFailure::AlreadyExists);
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
