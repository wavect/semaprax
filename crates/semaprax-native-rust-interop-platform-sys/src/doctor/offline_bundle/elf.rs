//! Bounded ELF header/interpreter metadata, not loadability or library closure.
use super::{DoctorOfflineArchitecture, DoctorOfflineBundleError};

#[cfg(test)]
#[path = "elf/tests.rs"]
mod tests;

type Error = DoctorOfflineBundleError;

pub(super) fn validate(
    bytes: &[u8],
    architecture: DoctorOfflineArchitecture,
) -> Result<Option<&str>, Error> {
    let header = range(bytes, 0, 64)?;
    let machine = match architecture {
        DoctorOfflineArchitecture::LinuxX86_64 => 62,
        DoctorOfflineArchitecture::LinuxAarch64 => 183,
    };
    if &header[..7] != b"\x7fELF\x02\x01\x01"
        || !matches!(u16_at(header, 16)?, 2 | 3)
        || u16_at(header, 18)? != machine
        || u32_at(header, 20)? != 1
        || u16_at(header, 52)? != 64
        || u16_at(header, 54)? != 56
    {
        return Err(Error::Invalid);
    }
    let count = usize::from(u16_at(header, 56)?);
    if !(1..=128).contains(&count) {
        return Err(Error::Invalid);
    }
    let offset = usize::try_from(u64_at(header, 32)?).map_err(|_| Error::Invalid)?;
    let table = range(bytes, offset, count.checked_mul(56).ok_or(Error::Invalid)?)?;
    let mut interpreter = None;
    for entry in table.as_chunks::<56>().0 {
        if u32_at(entry, 0)? != 3 {
            continue;
        }
        if interpreter.is_some() {
            return Err(Error::Invalid);
        }
        let size = u64_at(entry, 32)?;
        if !(3..=1026).contains(&size) {
            return Err(Error::Invalid);
        }
        let offset = usize::try_from(u64_at(entry, 8)?).map_err(|_| Error::Invalid)?;
        let size = usize::try_from(size).map_err(|_| Error::Invalid)?;
        let payload = range(bytes, offset, size)?;
        let (terminator, path) = payload.split_last().ok_or(Error::Invalid)?;
        if *terminator != 0 || path.contains(&0) {
            return Err(Error::Invalid);
        }
        let path = std::str::from_utf8(path).map_err(|_| Error::Invalid)?;
        if !path.starts_with('/') {
            return Err(Error::Invalid);
        }
        interpreter = Some(path);
    }
    Ok(interpreter)
}

fn range(bytes: &[u8], offset: usize, length: usize) -> Result<&[u8], Error> {
    let end = offset.checked_add(length).ok_or(Error::Invalid)?;
    bytes.get(offset..end).ok_or(Error::Invalid)
}

fn word<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], Error> {
    range(bytes, offset, N)?
        .try_into()
        .map_err(|_| Error::Invalid)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(word(bytes, offset)?))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(word(bytes, offset)?))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(word(bytes, offset)?))
}
