//! Private byte decoder; all retained facts are ranges into the original input.
use super::{elf, DoctorOfflineArchitecture, DoctorOfflineBundleError as Error};
use std::ops::Range;

const MAGIC: &[u8; 8] = b"SPXDOC1\0";
const MAX_FILES: usize = 4096;
const MAX_PATH_BYTES: usize = 1024;
const MAX_TOTAL_PATH_BYTES: usize = 1024 * 1024;
const ABSENT: u32 = u32::MAX;

#[derive(Debug)]
pub(super) struct Index {
    pub(super) selector: Range<usize>,
    pub(super) architecture: DoctorOfflineArchitecture,
    pub(super) files: Vec<FileIndex>,
    pub(super) tools: [Option<usize>; 3],
}

#[derive(Debug)]
pub(super) struct FileIndex {
    pub(super) path: Range<usize>,
    pub(super) content: Range<usize>,
    pub(super) executable: bool,
}

impl FileIndex {
    pub(super) fn path<'a>(&self, bytes: &'a [u8]) -> &'a str {
        std::str::from_utf8(&bytes[self.path.clone()]).expect("validated offline inventory path")
    }
}

pub(super) fn valid_selector(selector: &str) -> bool {
    let bytes = selector.as_bytes();
    (1..=64).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
}

pub(super) fn parse(
    bytes: &[u8],
    selector: &str,
    architecture: DoctorOfflineArchitecture,
) -> Result<Index, Error> {
    if !valid_selector(selector) {
        return Err(Error::Invalid);
    }
    if bytes.len() > super::DOCTOR_OFFLINE_INPUT_MAX_BYTES {
        return Err(Error::Limit);
    }
    let mut cursor = Cursor { bytes, offset: 0 };
    if cursor.take(8)? != MAGIC {
        return Err(Error::Invalid);
    }
    let encoded_architecture = match cursor.byte()? {
        1 => DoctorOfflineArchitecture::LinuxX86_64,
        2 => DoctorOfflineArchitecture::LinuxAarch64,
        _ => return Err(Error::Invalid),
    };
    if encoded_architecture != architecture {
        return Err(Error::ArchitectureMismatch);
    }
    let roles = cursor.byte()?;
    if roles == 0 || roles & !7 != 0 {
        return Err(Error::Invalid);
    }
    let selector_length = usize::from(cursor.u16()?);
    if selector_length == 0 {
        return Err(Error::Invalid);
    }
    if selector_length > 64 {
        return Err(Error::Limit);
    }
    let file_count = usize::try_from(cursor.u32()?).map_err(|_| Error::Limit)?;
    if file_count == 0 {
        return Err(Error::Invalid);
    }
    if file_count > MAX_FILES {
        return Err(Error::Limit);
    }
    let role_indices = [cursor.u32()?, cursor.u32()?, cursor.u32()?];
    let selector_range = cursor.range(selector_length)?;
    let encoded_selector =
        std::str::from_utf8(&bytes[selector_range.clone()]).map_err(|_| Error::Invalid)?;
    if !valid_selector(encoded_selector) {
        return Err(Error::Invalid);
    }
    if encoded_selector != selector {
        return Err(Error::SelectorMismatch);
    }

    let mut files: Vec<FileIndex> = Vec::new();
    files
        .try_reserve_exact(file_count)
        .map_err(|_| Error::Allocation)?;
    let mut total_path_bytes = 0usize;
    for _ in 0..file_count {
        let path_length = usize::from(cursor.u16()?);
        if path_length == 0 {
            return Err(Error::Invalid);
        }
        if path_length > MAX_PATH_BYTES {
            return Err(Error::Limit);
        }
        let executable = match cursor.byte()? {
            0 => false,
            1 => true,
            _ => return Err(Error::Invalid),
        };
        if cursor.byte()? != 0 {
            return Err(Error::Invalid);
        }
        let content_length = usize::try_from(cursor.u64()?).map_err(|_| Error::Limit)?;
        if content_length > super::DOCTOR_OFFLINE_INPUT_MAX_BYTES {
            return Err(Error::Limit);
        }
        total_path_bytes = total_path_bytes
            .checked_add(path_length)
            .ok_or(Error::Limit)?;
        if total_path_bytes > MAX_TOTAL_PATH_BYTES {
            return Err(Error::Limit);
        }
        let path = cursor.range(path_length)?;
        validate_path(&bytes[path.clone()])?;
        if let Some(previous) = files.last() {
            if bytes[previous.path.clone()] >= bytes[path.clone()] {
                return Err(Error::Invalid);
            }
        }
        let content = cursor.range(content_length)?;
        files.push(FileIndex {
            path,
            content,
            executable,
        });
    }
    if cursor.offset != bytes.len() {
        return Err(Error::Invalid);
    }

    // Adjacent-path checks alone miss a file ancestor separated by other names,
    // for example a, a-, a/x. Check every slash prefix against the full inventory.
    for file in &files {
        let path = file.path(bytes);
        for (offset, byte) in path.bytes().enumerate() {
            if byte == b'/' && find_file(&files, bytes, &path[..offset]).is_some() {
                return Err(Error::Invalid);
            }
        }
    }

    let mut tools = [None; 3];
    for (role, name) in ["clang", "node", "rustc"].into_iter().enumerate() {
        let raw = role_indices[role];
        if roles & (1 << role) == 0 {
            if raw != ABSENT {
                return Err(Error::Invalid);
            }
            continue;
        }
        let index = usize::try_from(raw).map_err(|_| Error::Invalid)?;
        let file = files.get(index).ok_or(Error::Invalid)?;
        if !file.executable || file.path(bytes).rsplit('/').next() != Some(name) {
            return Err(Error::Invalid);
        }
        tools[role] = Some(index);
    }

    // Temporary borrowed interpreter paths avoid copying input or validating an
    // executable repeatedly. This second metadata allocation is bounded too.
    let mut interpreters = Vec::new();
    interpreters
        .try_reserve_exact(file_count)
        .map_err(|_| Error::Allocation)?;
    for file in &files {
        interpreters.push(if file.executable {
            elf::validate(&bytes[file.content.clone()], architecture)?
        } else {
            None
        });
    }
    for interpreter in interpreters.iter().flatten() {
        let path = interpreter.strip_prefix('/').ok_or(Error::Invalid)?;
        validate_path(path.as_bytes())?;
        let index = find_file(&files, bytes, path).ok_or(Error::Invalid)?;
        if !files[index].executable || interpreters[index].is_some() {
            return Err(Error::Invalid);
        }
    }
    Ok(Index {
        selector: selector_range,
        architecture,
        files,
        tools,
    })
}

fn find_file(files: &[FileIndex], bytes: &[u8], path: &str) -> Option<usize> {
    files
        .binary_search_by(|file| file.path(bytes).cmp(path))
        .ok()
}

fn validate_path(path: &[u8]) -> Result<(), Error> {
    if path.is_empty() {
        return Err(Error::Invalid);
    }
    if path.len() > MAX_PATH_BYTES {
        return Err(Error::Limit);
    }
    let mut depth = 0usize;
    for component in path.split(|byte| *byte == b'/') {
        depth += 1;
        if depth > 32 || component.len() > 255 {
            return Err(Error::Limit);
        }
        if component.is_empty() || component == b"." || component == b".." {
            return Err(Error::Invalid);
        }
        if !component
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'+' | b'-'))
        {
            return Err(Error::Invalid);
        }
    }
    Ok(())
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn range(&mut self, length: usize) -> Result<Range<usize>, Error> {
        let end = self.offset.checked_add(length).ok_or(Error::Invalid)?;
        self.bytes.get(self.offset..end).ok_or(Error::Invalid)?;
        let range = self.offset..end;
        self.offset = end;
        Ok(range)
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], Error> {
        let range = self.range(length)?;
        Ok(&self.bytes[range])
    }

    fn byte(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, Error> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| Error::Invalid)?,
        ))
    }

    fn u32(&mut self) -> Result<u32, Error> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| Error::Invalid)?,
        ))
    }

    fn u64(&mut self) -> Result<u64, Error> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| Error::Invalid)?,
        ))
    }
}
