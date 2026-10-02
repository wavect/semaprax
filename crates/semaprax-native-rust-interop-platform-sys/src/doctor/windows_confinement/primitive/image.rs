//! Bind the signed image to CreateProcess's mandatory pathname interface.
//!
//! Require local NTFS, hold a read-only/no-write/no-delete file open, and pin
//! every component of its normalized volume-GUID name without following
//! reparse points. A retained read oplock detects observed section changes.
//! Oplock breaks for writable sections are advisory: this does not establish
//! atomic exclusion of mutation through every retained writable section.
//! This image binding module supplies one part of the primitive's separate
//! request/bundle handoff; it does not validate DLL closure or protect against
//! kernel/administrator mutation.
use super::{Handle, MAX_WIDE, wide};
use semaprax_doctor_capsule::{Artifact, Capsule, MAX_ARTIFACT_BYTES};
use sha2::{Digest as _, Sha256};
use std::cell::UnsafeCell;
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Read as _;
use std::os::windows::ffi::OsStringExt as _;
use std::os::windows::fs::OpenOptionsExt as _;
use std::os::windows::io::AsRawHandle as _;
use std::path::{Path, PathBuf};
use windows_sys::Win32::Foundation::{
    ERROR_IO_PENDING, ERROR_OPERATION_ABORTED, GetLastError, HANDLE, WAIT_TIMEOUT,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_FLAG_OVERLAPPED,
    FILE_SHARE_READ, FILE_TYPE_DISK, GetFileInformationByHandle, GetFileType,
    GetFinalPathNameByHandleW, GetVolumeInformationByHandleW, VOLUME_NAME_GUID, VOLUME_NAME_NT,
};
use windows_sys::Win32::System::IO::{
    CancelIoEx, DeviceIoControl, GetOverlappedResult, OVERLAPPED,
};
use windows_sys::Win32::System::Ioctl::{
    FSCTL_REQUEST_OPLOCK, OPLOCK_LEVEL_CACHE_READ, REQUEST_OPLOCK_CURRENT_VERSION,
    REQUEST_OPLOCK_INPUT_BUFFER, REQUEST_OPLOCK_INPUT_FLAG_REQUEST, REQUEST_OPLOCK_OUTPUT_BUFFER,
};
use windows_sys::Win32::System::Threading::{
    CreateEventW, PROCESS_NAME_NATIVE, QueryFullProcessImageNameW, WaitForSingleObject,
};

/// Test-visible checkpoints in the image admission half of a launch. These
/// are observations only: callers cannot waive a failed image check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ImageBindingBoundary {
    FileOpened,
    GuardAcquired,
    DigestVerified,
}

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

    pub(super) fn wire(self) -> &'static str {
        match self {
            Self::Launcher => "launcher",
            Self::Worker => "worker",
            Self::Collector => "collector",
        }
    }
}

pub(super) struct HeldImage {
    application: Vec<u16>,
    native_name: Vec<u16>,
    oplock: ImageOplock,
    _file: File,
    _ancestors: Vec<File>,
}

impl HeldImage {
    pub(super) fn acquire(path: &Path, artifact: Artifact) -> Result<Self, ()> {
        Self::acquire_observing(path, artifact, |_| {})
    }

    pub(super) fn acquire_observing(
        path: &Path,
        artifact: Artifact,
        mut observe: impl FnMut(ImageBindingBoundary),
    ) -> Result<Self, ()> {
        if !path.is_absolute() || artifact.length == 0 || artifact.length > MAX_ARTIFACT_BYTES {
            return Err(());
        }
        let original = open(path, FILE_FLAG_OVERLAPPED)?;
        // Distinguish sharing admission from later metadata/oplock refusal.
        // Reaching this observation grants no authenticated-image authority.
        observe(ImageBindingBoundary::FileOpened);
        let identity = information(&original, false)?;
        require_ntfs(&original)?;
        let application = final_name(&original, VOLUME_NAME_GUID)?;
        let ancestors = pin_ancestors(&application)?;
        let mut file = open(&PathBuf::from(OsString::from_wide(&application)), 0)?;
        if file_identity(&information(&file, false)?) != file_identity(&identity) {
            return Err(());
        }
        let native_name = final_name(&file, VOLUME_NAME_NT)?;
        // Keep the request and its stable buffers alive through process
        // creation and settlement. An observed break refuses; an unobserved
        // advisory break is not proof that concurrent writes are excluded.
        let oplock = ImageOplock::acquire(original)?;
        observe(ImageBindingBoundary::GuardAcquired);
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
            || !oplock.intact()
        {
            return Err(());
        }
        observe(ImageBindingBoundary::DigestVerified);
        let mut terminated = application;
        terminated.push(0);
        Ok(Self {
            application: terminated,
            native_name,
            oplock,
            _file: file,
            _ancestors: ancestors,
        })
    }

    pub(super) fn application(&self) -> &[u16] {
        &self.application
    }

    pub(super) fn intact(&self) -> bool {
        self.oplock.intact()
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
        name[..length as usize] == self.native_name && self.intact()
    }
}

fn open(path: &Path, flags: u32) -> Result<File, ()> {
    // All handles are non-inheritable. FILE_SHARE_READ denies new write/delete
    // opens; it does not by itself exclude retained writable sections.
    // OPEN_REPARSE_POINT authenticates
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

// OS-owned asynchronous buffers need stable addresses and interior mutability
// until completion. No Rust code reads output/overlap while the I/O is pending.
struct OplockBuffers {
    input: REQUEST_OPLOCK_INPUT_BUFFER,
    output: UnsafeCell<REQUEST_OPLOCK_OUTPUT_BUFFER>,
    overlap: UnsafeCell<OVERLAPPED>,
}

struct ImageOplock {
    file: File,
    event: Handle,
    buffers: Box<OplockBuffers>,
    pending: bool,
}

impl ImageOplock {
    fn acquire(file: File) -> Result<Self, ()> {
        // SAFETY: unnamed, non-inheritable, manual-reset event.
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        if event.is_null() {
            return Err(());
        }
        let event = Handle::new(event);
        let buffers = Box::new(OplockBuffers {
            input: REQUEST_OPLOCK_INPUT_BUFFER {
                StructureVersion: REQUEST_OPLOCK_CURRENT_VERSION as u16,
                StructureLength: std::mem::size_of::<REQUEST_OPLOCK_INPUT_BUFFER>() as u16,
                RequestedOplockLevel: OPLOCK_LEVEL_CACHE_READ,
                Flags: REQUEST_OPLOCK_INPUT_FLAG_REQUEST,
            },
            output: UnsafeCell::new(REQUEST_OPLOCK_OUTPUT_BUFFER::default()),
            overlap: UnsafeCell::new(OVERLAPPED {
                hEvent: event.raw(),
                ..Default::default()
            }),
        });
        let mut guard = Self {
            file,
            event,
            buffers,
            pending: false,
        };
        // SAFETY: boxed buffers do not move with the guard. The exclusive
        // owner drains this exact request in Drop before freeing any buffer.
        guard.pending = unsafe {
            DeviceIoControl(
                guard.file.as_raw_handle(),
                FSCTL_REQUEST_OPLOCK,
                (&guard.buffers.input as *const REQUEST_OPLOCK_INPUT_BUFFER).cast(),
                std::mem::size_of::<REQUEST_OPLOCK_INPUT_BUFFER>() as u32,
                guard.buffers.output.get().cast(),
                std::mem::size_of::<REQUEST_OPLOCK_OUTPUT_BUFFER>() as u32,
                std::ptr::null_mut(),
                guard.buffers.overlap.get(),
            )
        } == 0
            && unsafe { GetLastError() } == ERROR_IO_PENDING;
        if !guard.pending || !guard.intact() {
            return Err(());
        }
        Ok(guard)
    }

    fn intact(&self) -> bool {
        // SAFETY: event stays live until the request is drained. A break or
        // wait error refuses. This observation is not an atomic write barrier.
        (unsafe { WaitForSingleObject(self.event.raw(), 0) }) == WAIT_TIMEOUT
    }
}

impl Drop for ImageOplock {
    fn drop(&mut self) {
        if !self.pending {
            return;
        }
        // SAFETY: cancel/drain this sole owned request while its file, event
        // and boxed buffers remain live. CancelIoEx alone is not completion.
        unsafe { CancelIoEx(self.file.as_raw_handle(), self.buffers.overlap.get()) };
        let mut transferred = 0;
        let completed = unsafe {
            GetOverlappedResult(
                self.file.as_raw_handle(),
                self.buffers.overlap.get(),
                &mut transferred,
                1,
            )
        };
        if completed == 0 && unsafe { GetLastError() } != ERROR_OPERATION_ABORTED {
            // No completion proof: never free a buffer still owned by the OS.
            std::process::abort();
        }
        self.pending = false;
    }
}
