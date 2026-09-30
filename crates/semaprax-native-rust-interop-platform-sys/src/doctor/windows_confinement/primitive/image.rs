//! Bind the signed image to CreateProcess's mandatory pathname interface.
//!
//! Require local NTFS, hold a read-only/no-write/no-delete file open, and pin
//! every component of its normalized volume-GUID name without following
//! reparse points. A read-oplock grant excludes pre-existing writable mapped
//! sections; the retained sharing denial prevents any new writer thereafter.
//! This is an image binding primitive, not Windows request/bundle transport,
//! DLL closure validation, or protection from kernel/administrator mutation.
use super::{wide, Handle, MAX_WIDE};
use semaprax_doctor_capsule::{Artifact, Capsule, MAX_ARTIFACT_BYTES};
use sha2::{Digest as _, Sha256};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Read as _;
use std::os::windows::ffi::OsStringExt as _;
use std::os::windows::fs::OpenOptionsExt as _;
use std::os::windows::io::AsRawHandle as _;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::{
    GetLastError, ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, HANDLE,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileInformationByHandle, GetFileType, GetFinalPathNameByHandleW,
    GetVolumeInformationByHandleW, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_TYPE_DISK, VOLUME_NAME_GUID, VOLUME_NAME_NT,
};
use windows_sys::Win32::System::Ioctl::{
    FSCTL_REQUEST_OPLOCK, OPLOCK_LEVEL_CACHE_READ, REQUEST_OPLOCK_CURRENT_VERSION,
    REQUEST_OPLOCK_INPUT_BUFFER, REQUEST_OPLOCK_INPUT_FLAG_REQUEST, REQUEST_OPLOCK_OUTPUT_BUFFER,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, QueryFullProcessImageNameW, PROCESS_NAME_NATIVE,
};
use windows_sys::Win32::System::IO::{
    CancelIoEx, DeviceIoControl, GetOverlappedResult, OVERLAPPED,
};

/// The capsule's executable slot must be selected explicitly by the caller.
#[derive(Clone, Copy)]
pub enum ImageRole {
    Launcher,
    Worker,
    Collector,
}

impl ImageRole {
    pub(super) fn artifact(self, capsule: &Capsule) -> Artifact {
        match self {
            Self::Launcher => capsule.launcher(),
            Self::Worker => capsule.worker(),
            Self::Collector => capsule.collector(),
        }
    }
}

pub(super) struct HeldImage {
    application: Vec<u16>,
    native_name: Vec<u16>,
    _file: File,
    _ancestors: Vec<File>,
}

impl HeldImage {
    pub(super) fn acquire(path: &Path, artifact: Artifact) -> Result<Self, ()> {
        if !path.is_absolute() || artifact.length == 0 || artifact.length > MAX_ARTIFACT_BYTES {
            return Err(());
        }
        let original = open(path, FILE_FLAG_OVERLAPPED)?;
        let identity = information(&original, false)?;
        require_ntfs(&original)?;
        let application = final_name(&original, VOLUME_NAME_GUID)?;
        let ancestors = pin_ancestors(&application)?;
        let mut file = open(&PathBuf::from(OsString::from_wide(&application)), 0)?;
        if file_identity(&information(&file, false)?) != file_identity(&identity) {
            return Err(());
        }
        let native_name = final_name(&file, VOLUME_NAME_NT)?;
        // The original and synchronous reader both deny write/delete sharing.
        // A successful read-oplock grant excludes a writable mapping whose
        // creator has already closed its original write handle. Cancel and
        // observe completion before letting the borrowed FFI buffers die.
        exclude_writable_mapping(&original)?;
        let length = (u64::from(identity.nFileSizeHigh) << 32) | u64::from(identity.nFileSizeLow);
        if length != artifact.length {
            return Err(());
        }
        let mut digest = Sha256::new();
        let mut remaining = length;
        let mut buffer = [0u8; 64 * 1024];
        while remaining > 0 {
            let count = remaining.min(buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..count]).map_err(|_| ())?;
            digest.update(&buffer[..count]);
            remaining -= count as u64;
        }
        if file.read(&mut buffer[..1]).map_err(|_| ())? != 0
            || <[u8; 32]>::from(digest.finalize()) != artifact.digest
        {
            return Err(());
        }
        let mut terminated = application;
        terminated.push(0);
        Ok(Self {
            application: terminated,
            native_name,
            _file: file,
            _ancestors: ancestors,
        })
    }

    pub(super) fn application(&self) -> &[u16] {
        &self.application
    }

    pub(super) fn matches_process(&self, process: HANDLE) -> bool {
        let mut name = vec![0; MAX_WIDE];
        let mut length = name.len() as u32;
        // SAFETY: caller retains its live process handle; name is writable for
        // length units. No process image path is reopened or accepted on trust.
        if unsafe {
            QueryFullProcessImageNameW(process, PROCESS_NAME_NATIVE, name.as_mut_ptr(), &mut length)
        } == 0
            || length as usize >= name.len()
        {
            return false;
        }
        name[..length as usize] == self.native_name
    }
}

fn open(path: &Path, flags: u32) -> Result<File, ()> {
    // All handles are non-inheritable. Retaining FILE_SHARE_READ alone blocks
    // file mutation, unlink and replacement; OPEN_REPARSE_POINT authenticates
    // the opened component itself rather than a substituted link target.
    let _ = wide(path.as_os_str())?;
    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS | flags)
        .open(path)
        .map_err(|_| ())
}

fn information(file: &File, directory: bool) -> Result<BY_HANDLE_FILE_INFORMATION, ()> {
    let mut value = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: file owns a live handle; output points to a complete writable struct.
    if unsafe { GetFileType(file.as_raw_handle()) } != FILE_TYPE_DISK
        || unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut value) } == 0
        || value.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || (value.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0) != directory
        || (!directory && value.nNumberOfLinks != 1)
    {
        return Err(());
    }
    Ok(value)
}

fn file_identity(value: &BY_HANDLE_FILE_INFORMATION) -> (u32, u32, u32) {
    (
        value.dwVolumeSerialNumber,
        value.nFileIndexHigh,
        value.nFileIndexLow,
    )
}

fn require_ntfs(file: &File) -> Result<(), ()> {
    let mut name = [0u16; 16];
    // SAFETY: file is held; unused output fields may be NULL; name is bounded.
    if unsafe {
        GetVolumeInformationByHandleW(
            file.as_raw_handle(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            name.as_mut_ptr(),
            name.len() as u32,
        )
    } == 0
        || name[..5] != [b'N' as u16, b'T' as u16, b'F' as u16, b'S' as u16, 0]
    {
        return Err(());
    }
    Ok(())
}

fn final_name(file: &File, volume_format: u32) -> Result<Vec<u16>, ()> {
    let mut path = vec![0; MAX_WIDE];
    // SAFETY: file is held and path is a writable buffer of the declared size.
    let length = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            path.as_mut_ptr(),
            path.len() as u32,
            volume_format,
        )
    } as usize;
    if length == 0 || length >= path.len() {
        return Err(());
    }
    path.truncate(length);
    Ok(path)
}

fn pin_ancestors(path: &[u16]) -> Result<Vec<File>, ()> {
    // Only the documented normalized GUID form, never drive aliases, UNC,
    // alternate streams, dot components or arbitrary device namespaces.
    let prefix: Vec<u16> = r"\\?\Volume{".encode_utf16().collect();
    const ROOT_END: usize = 49; // \\?\Volume{8-4-4-4-12}\
    if !path.starts_with(&prefix)
        || path.len() <= ROOT_END
        || path[47..49] != [b'}' as u16, b'\\' as u16]
    {
        return Err(());
    }
    for (index, &unit) in path[11..47].iter().enumerate() {
        let valid = if [8, 13, 18, 23].contains(&index) {
            unit == b'-' as u16
        } else {
            u8::try_from(unit).is_ok_and(|byte| byte.is_ascii_hexdigit())
        };
        if !valid {
            return Err(());
        }
    }
    let mut ends = vec![ROOT_END];
    let mut start = ROOT_END;
    for end in ROOT_END..=path.len() {
        if end == path.len() || path[end] == b'\\' as u16 {
            let component = &path[start..end];
            if component.is_empty()
                || component == [b'.' as u16]
                || component == [b'.' as u16, b'.' as u16]
                || component
                    .iter()
                    .any(|unit| [0, b':' as u16, b'/' as u16].contains(unit))
            {
                return Err(());
            }
            if end < path.len() {
                ends.push(end);
            }
            start = end + 1;
        }
    }
    let mut held = Vec::with_capacity(ends.len());
    for end in ends {
        let directory = open(&PathBuf::from(OsString::from_wide(&path[..end])), 0)?;
        information(&directory, true)?;
        held.push(directory);
    }
    Ok(held)
}

fn exclude_writable_mapping(file: &File) -> Result<(), ()> {
    let input = REQUEST_OPLOCK_INPUT_BUFFER {
        StructureVersion: REQUEST_OPLOCK_CURRENT_VERSION as u16,
        StructureLength: std::mem::size_of::<REQUEST_OPLOCK_INPUT_BUFFER>() as u16,
        RequestedOplockLevel: OPLOCK_LEVEL_CACHE_READ,
        Flags: REQUEST_OPLOCK_INPUT_FLAG_REQUEST,
    };
    let mut output = REQUEST_OPLOCK_OUTPUT_BUFFER::default();
    // SAFETY: unnamed, non-inheritable, manual-reset event with no attributes.
    let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
    if event.is_null() {
        return Err(());
    }
    let event = Handle::new(event);
    let mut overlap = OVERLAPPED {
        hEvent: event.raw(),
        ..Default::default()
    };
    // SAFETY: every buffer lives until the pending request has been drained.
    let granted = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_REQUEST_OPLOCK,
            (&input as *const REQUEST_OPLOCK_INPUT_BUFFER).cast(),
            std::mem::size_of_val(&input) as u32,
            (&mut output as *mut REQUEST_OPLOCK_OUTPUT_BUFFER).cast(),
            std::mem::size_of_val(&output) as u32,
            std::ptr::null_mut(),
            &mut overlap,
        )
    } == 0
        && unsafe { GetLastError() } == ERROR_IO_PENDING;
    if !granted {
        return Err(());
    }
    // SAFETY: cancel only this owned request, then wait for its completion.
    // Cancellation is not completion: no return may drop the buffers first.
    unsafe { CancelIoEx(file.as_raw_handle(), &overlap) };
    let mut transferred = 0;
    let completed =
        unsafe { GetOverlappedResult(file.as_raw_handle(), &overlap, &mut transferred, 1) };
    if completed != 0 {
        // A real oplock break is completed, but is not an admission.
        return Err(());
    }
    if unsafe { GetLastError() } != ERROR_OPERATION_ABORTED {
        // An unexpected wait/handle failure gives no completion proof. Never
        // unwind or return while the kernel may still own these stack buffers.
        std::process::abort();
    }
    Ok(())
}
