//! Audited operating-system quarantine for Native Rust Interop bundle builds.
//!
//! This crate is unpublished. Its public surface exists only so the sibling
//! safe facade can own opaque held objects without exposing handles upstream.

#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(unix)]
use std::ffi::CString;
use std::ffi::OsStr;
use std::fs::File;
use std::io::Write as _;
use std::path::Path;

#[cfg(test)]
static TEST_PREPARED_FILE_SYSCALL_ENTRIES: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn enter_prepared_file_syscalls<T>(resolved: Result<&T, Error>) -> Result<&T, Error> {
    let resolved = resolved?;
    #[cfg(test)]
    TEST_PREPARED_FILE_SYSCALL_ENTRIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    Ok(resolved)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Invalid,
    Exists,
    Changed,
    Unsupported,
    Spawn,
    Exit,
    OutputLimit,
}

pub const SDK_ARCHIVE_MAX_BYTES: u64 = 8_388_608;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ArchiveMemberKind {
    GnuLinkerIndex,
    BsdSortedLinkerIndex,
    LongNames,
    Input,
    Extended(usize),
}

fn archive_member_size(field: &[u8]) -> Result<u64, Error> {
    let digits = field
        .iter()
        .position(|byte| *byte == b' ')
        .unwrap_or(field.len());
    if digits == 0
        || field[digits..].iter().any(|byte| *byte != b' ')
        || !field[..digits].iter().all(u8::is_ascii_digit)
        || digits > 1 && field[0] == b'0'
    {
        return Err(Error::Invalid);
    }
    field[..digits].iter().try_fold(0_u64, |value, digit| {
        value
            .checked_mul(10)
            .and_then(|value| value.checked_add(u64::from(*digit - b'0')))
            .ok_or(Error::OutputLimit)
    })
}

fn archive_member_kind(field: &[u8], input: &[u8]) -> Result<ArchiveMemberKind, Error> {
    let mut name = field;
    while name.last() == Some(&b' ') {
        name = &name[..name.len() - 1];
    }
    if let Some(length) = name.strip_prefix(b"#1/") {
        let length =
            usize::try_from(archive_member_size(length)?).map_err(|_| Error::OutputLimit)?;
        if length == 0 || length > 255 {
            return Err(Error::Invalid);
        }
        return Ok(ArchiveMemberKind::Extended(length));
    }
    if name == b"/" {
        return Ok(ArchiveMemberKind::GnuLinkerIndex);
    }
    if name == b"//" {
        return Ok(ArchiveMemberKind::LongNames);
    }
    if name == b"__.SYMDEF SORTED" {
        return Ok(ArchiveMemberKind::BsdSortedLinkerIndex);
    }
    if name.strip_suffix(b"/").unwrap_or(name) == input {
        return Ok(ArchiveMemberKind::Input);
    }
    Err(Error::Invalid)
}

fn archive_extended_name(field: &[u8]) -> Result<&[u8], Error> {
    let nul = field
        .iter()
        .position(|byte| *byte == 0)
        .ok_or(Error::Invalid)?;
    let padding = field.len().checked_sub(nul).ok_or(Error::Invalid)?;
    if !(1..=4).contains(&padding)
        || !field.len().is_multiple_of(4)
        || field[nul..].iter().any(|byte| *byte != 0)
    {
        return Err(Error::Invalid);
    }
    Ok(&field[..nul])
}

fn exact_archive_member_metadata(
    header: &[u8; 60],
    kind: ArchiveMemberKind,
    input_mode: u32,
) -> Result<(), Error> {
    // GNU deterministic and COFF archives have canonical member modes; the
    // input filesystem mode is intentionally relevant only to Darwin below.
    #[cfg(any(target_os = "linux", target_family = "windows"))]
    let _ = input_mode;
    #[cfg(not(target_family = "windows"))]
    if header[16..28] != *b"0           " {
        return Err(Error::Invalid);
    }
    // Microsoft lib.exe serializes its /BREPRO time_t sentinel verbatim.
    // This is deliberately not a general signed-date admission.
    #[cfg(target_family = "windows")]
    if header[16..28] != *b"-1          " {
        return Err(Error::Invalid);
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if header[28..34] != *b"0     " || header[34..40] != *b"0     " {
        return Err(Error::Invalid);
    }
    #[cfg(target_family = "windows")]
    if header[28..34] != *b"      " || header[34..40] != *b"      " {
        return Err(Error::Invalid);
    }
    #[cfg(target_os = "linux")]
    let mode = match kind {
        ArchiveMemberKind::GnuLinkerIndex => 0,
        ArchiveMemberKind::Input => 0o644,
        _ => return Err(Error::Invalid),
    };
    #[cfg(target_os = "macos")]
    let mode = match kind {
        ArchiveMemberKind::Extended(20) => 0o100644,
        ArchiveMemberKind::Extended(12) => input_mode & 0o777,
        _ => return Err(Error::Invalid),
    };
    #[cfg(target_family = "windows")]
    let mode = match kind {
        ArchiveMemberKind::GnuLinkerIndex | ArchiveMemberKind::LongNames => 0,
        ArchiveMemberKind::Input => 0o100666,
        _ => return Err(Error::Invalid),
    };
    let mut encoded = [b' '; 8];
    let mut value = mode;
    let mut digits = [0_u8; 8];
    let mut count = 0usize;
    loop {
        digits[count] = b'0' + u8::try_from(value & 7).map_err(|_| Error::Invalid)?;
        count += 1;
        value >>= 3;
        if value == 0 {
            break;
        }
    }
    for index in 0..count {
        encoded[index] = digits[count - index - 1];
    }
    if header[40..48] != encoded {
        return Err(Error::Invalid);
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::enum_variant_names)]
#[repr(u8)]
#[derive(Clone, Copy)]
enum TestSettlementFailure {
    #[cfg(unix)]
    UnixWait,
    #[cfg(unix)]
    UnixGroup,
    #[cfg(unix)]
    UnixSettleClose,
    #[cfg(unix)]
    UnixSuccessReadClose,
    #[cfg(unix)]
    UnixParentWriteClose,
    #[cfg(unix)]
    UnixParentNullClose,
    #[cfg(unix)]
    UnixPipeReadFcntl,
    #[cfg(unix)]
    UnixPipeWriteFcntl,
    #[cfg(unix)]
    UnixDrainFcntl,
    #[cfg(unix)]
    UnixPoll,
    #[cfg(unix)]
    UnixRead,
    #[cfg(unix)]
    UnixReadConversion,
    #[cfg(unix)]
    UnixWaitpid,
    #[cfg(unix)]
    UnixDeadline,
    #[cfg(target_os = "macos")]
    DarwinActionsDestroy,
    #[cfg(target_os = "macos")]
    DarwinAttributesDestroy,
    #[cfg(target_os = "macos")]
    DarwinAttest,
    #[cfg(target_os = "macos")]
    DarwinSigcont,
    #[cfg(target_os = "windows")]
    WindowsImage,
    #[cfg(target_os = "windows")]
    WindowsAssign,
    #[cfg(target_os = "windows")]
    WindowsResume,
    #[cfg(target_os = "windows")]
    WindowsPeek,
    #[cfg(target_os = "windows")]
    WindowsRead,
    #[cfg(target_os = "windows")]
    WindowsUnassigned,
    #[cfg(target_os = "windows")]
    WindowsTerminateProcess,
    #[cfg(target_os = "windows")]
    WindowsWaitUnassigned,
    #[cfg(target_os = "windows")]
    WindowsJob,
    #[cfg(target_os = "windows")]
    WindowsTerminateJob,
    #[cfg(target_os = "windows")]
    WindowsQueryJob,
}

#[cfg(test)]
static TEST_SETTLEMENT_FAILURES: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0);

#[cfg(test)]
#[allow(dead_code)]
fn set_test_settlement_failures(points: &[TestSettlementFailure]) {
    let mut mask = 0_u64;
    for point in points {
        mask |= 1_u64 << (*point as u8);
    }
    TEST_SETTLEMENT_FAILURES.store(mask, std::sync::atomic::Ordering::SeqCst);
}

#[cfg(test)]
fn test_settlement_failure(point: TestSettlementFailure) -> bool {
    TEST_SETTLEMENT_FAILURES.load(std::sync::atomic::Ordering::SeqCst) & (1_u64 << (point as u8))
        != 0
}

macro_rules! injected_settlement_failure {
    ($point:ident) => {{
        #[cfg(test)]
        {
            test_settlement_failure(TestSettlementFailure::$point)
        }
        #[cfg(not(test))]
        {
            false
        }
    }};
}

#[cfg(all(test, unix))]
fn trace_error(context: &str, error: Error) -> Error {
    eprintln!("platform {context}: {error:?}");
    error
}

#[cfg(all(not(test), unix))]
fn trace_error(_: &str, error: Error) -> Error {
    error
}

// Physical authority stays isolated by operating system so audits never mix trust models.
#[cfg(unix)]
#[path = "unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "windows.rs"]
mod platform;
pub use platform::*;
#[cfg(test)]
mod tests;
