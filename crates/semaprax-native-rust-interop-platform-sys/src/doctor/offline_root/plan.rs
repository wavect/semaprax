//! Fallible, effect-free preparation. No path or payload is discovered on disk.
use super::Error;
use crate::DoctorOfflineBundle;
use std::ffi::{CStr, CString};

const MAX_FILES: usize = 4096;
const MAX_DIRECTORIES: usize = MAX_FILES * 31;
const MAX_PATH_STORAGE: usize = 32 * (1024 * 1024 + MAX_FILES);

pub(super) struct File<'a> {
    pub(super) path: CString,
    pub(super) bytes: &'a [u8],
    pub(super) executable: bool,
}

/// Only this module can construct a plan. Paths are derived from an opaque
/// parsed bundle, and payload borrows keep that exact snapshot alive.
pub(super) struct Plan<'a> {
    directories: Vec<CString>,
    files: Vec<File<'a>>,
    page_size: usize,
    block_count: usize,
    inode_count: usize,
    size_value: CString,
    inode_value: CString,
}

impl<'a> Plan<'a> {
    /// Run before any child boundary. The materializer independently checks the
    /// actual tmpfs block size before trusting this page-rounded allocation cap.
    pub(super) fn prepare(
        bundle: &'a DoctorOfflineBundle,
        page_size: usize,
    ) -> Result<Self, Error> {
        if !(4096..=65_536).contains(&page_size) || !page_size.is_power_of_two() {
            return Err(Error::Invalid);
        }
        let count = bundle.files().len();
        if count == 0 || count > MAX_FILES {
            return Err(Error::Limit);
        }
        let mut prefix_count = 0usize;
        let mut path_storage = 0usize;
        let mut size = 0usize;
        for file in bundle.files() {
            path_storage = charge_path(path_storage, file.path().len())?;
            size = size
                .checked_add(rounded_size(file.bytes().len(), page_size)?)
                .ok_or(Error::Limit)?;
            for (offset, byte) in file.path().bytes().enumerate() {
                if byte == b'/' {
                    prefix_count = prefix_count.checked_add(1).ok_or(Error::Limit)?;
                    path_storage = charge_path(path_storage, offset)?;
                }
            }
        }
        if prefix_count > MAX_DIRECTORIES {
            return Err(Error::Limit);
        }
        // Temporary prefix slices do not copy paths. Deduplication precedes
        // owned C-string allocation and preserves parents before descendants.
        let mut prefixes = Vec::new();
        reserve(&mut prefixes, prefix_count)?;
        for file in bundle.files() {
            for (offset, byte) in file.path().bytes().enumerate() {
                if byte == b'/' {
                    prefixes.push(&file.path()[..offset]);
                }
            }
        }
        prefixes.sort_unstable();
        prefixes.dedup();
        let mut directories = Vec::new();
        reserve(&mut directories, prefixes.len())?;
        for prefix in prefixes {
            directories.push(c_string(prefix.as_bytes())?);
        }
        let mut files = Vec::new();
        reserve(&mut files, count)?;
        for file in bundle.files() {
            files.push(File {
                path: c_string(file.path().as_bytes())?,
                bytes: file.bytes(),
                executable: file.is_executable(),
            });
        }
        let inode_count = directories
            .len()
            .checked_add(count)
            .and_then(|value| value.checked_add(1))
            .ok_or(Error::Limit)?;
        Ok(Self {
            directories,
            files,
            page_size,
            block_count: size.max(page_size) / page_size,
            inode_count,
            // tmpfs interprets zero as unlimited, not an empty filesystem.
            size_value: decimal(size.max(page_size))?,
            inode_value: decimal(inode_count)?,
        })
    }

    pub(super) fn directories(&self) -> &[CString] {
        &self.directories
    }

    pub(super) fn files(&self) -> &[File<'a>] {
        &self.files
    }

    pub(super) fn page_size(&self) -> usize {
        self.page_size
    }

    pub(super) fn block_count(&self) -> usize {
        self.block_count
    }

    pub(super) fn inode_count(&self) -> usize {
        self.inode_count
    }

    pub(super) fn size_value(&self) -> &CStr {
        &self.size_value
    }

    pub(super) fn inode_value(&self) -> &CStr {
        &self.inode_value
    }
}

fn reserve<T>(items: &mut Vec<T>, count: usize) -> Result<(), Error> {
    items
        .try_reserve_exact(count)
        .map_err(|_| Error::Allocation)
}

fn charge_path(total: usize, length: usize) -> Result<usize, Error> {
    total
        .checked_add(length)
        .and_then(|value| value.checked_add(1))
        .filter(|value| *value <= MAX_PATH_STORAGE)
        .ok_or(Error::Limit)
}

fn rounded_size(length: usize, page_size: usize) -> Result<usize, Error> {
    length
        .checked_add(page_size - 1)
        .map(|value| value / page_size * page_size)
        .ok_or(Error::Limit)
}

fn c_string(bytes: &[u8]) -> Result<CString, Error> {
    let mut buffer = Vec::new();
    reserve(&mut buffer, bytes.len().checked_add(1).ok_or(Error::Limit)?)?;
    buffer.extend_from_slice(bytes);
    buffer.push(0);
    CString::from_vec_with_nul(buffer).map_err(|_| Error::Invalid)
}

fn decimal(mut value: usize) -> Result<CString, Error> {
    let mut buffer = [0_u8; 20];
    let mut start = buffer.len();
    loop {
        start -= 1;
        buffer[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            return c_string(&buffer[start..]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_storage_and_page_arithmetic_reject_overflow_at_exact_boundaries() {
        assert_eq!(MAX_PATH_STORAGE, 33_685_504);
        assert_eq!(MAX_DIRECTORIES, 126_976);
        assert_eq!(charge_path(MAX_PATH_STORAGE - 2, 1), Ok(MAX_PATH_STORAGE));
        assert_eq!(charge_path(MAX_PATH_STORAGE - 1, 1), Err(Error::Limit));
        assert_eq!(charge_path(0, usize::MAX), Err(Error::Limit));
        assert_eq!(charge_path(usize::MAX, 0), Err(Error::Limit));
        for page in [4096, 8192, 16_384, 32_768, 65_536] {
            for (length, expected) in [(0, 0), (1, page), (page, page), (page + 1, 2 * page)] {
                assert_eq!(rounded_size(length, page), Ok(expected));
            }
            assert_eq!(rounded_size(usize::MAX, page), Err(Error::Limit));
        }
    }

    #[test]
    fn decimal_and_c_string_preparation_are_exact_and_fallible() {
        for value in [0, 1, 4096, 536_870_912, usize::MAX] {
            assert_eq!(
                decimal(value).unwrap().to_bytes(),
                value.to_string().as_bytes()
            );
        }
        assert_eq!(c_string(b"a/b").unwrap().to_bytes_with_nul(), b"a/b\0");
        assert_eq!(c_string(b"a\0b").unwrap_err(), Error::Invalid);
        let mut bytes: Vec<u8> = Vec::new();
        assert_eq!(reserve(&mut bytes, usize::MAX), Err(Error::Allocation));
        assert!(bytes.is_empty());
    }
}
