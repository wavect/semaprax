//! Windows held-handle filesystem, process, and archive authority.
//!
//! This root owns the held-handle types, the access and flag constants, and the
//! frozen Windows link tail. The phases that operate on them live in
//! submodules: [`plans`] builds exact preflight capacity and the relative-open
//! primitives, [`handles`] holds directories, files, and tools, [`inventory`]
//! scans and publishes stage directories, [`process`] launches children and
//! settles jobs, and [`invocations`] builds prepared tool invocations and
//! admits exact archives.
use super::*;
#[cfg(test)]
#[path = "windows/publish_tests.rs"]
mod publish_tests;
use sha2::{Digest as _, Sha256};
use std::io::{Read as _, Seek as _, SeekFrom};
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::fs::FileExt as _;
use std::os::windows::io::{AsRawHandle as _, FromRawHandle as _, IntoRawHandle as _};
use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{
    FileLinkInformationEx, FileRenameInformation, FileRenameInformationEx, NtCreateFile,
    NtSetInformationFile, FILE_CREATE, FILE_DIRECTORY_FILE, FILE_NON_DIRECTORY_FILE, FILE_OPEN,
    FILE_OPEN_REPARSE_POINT, FILE_SYNCHRONOUS_IO_NONALERT,
};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, SetHandleInformation, ERROR_BROKEN_PIPE, ERROR_INSUFFICIENT_BUFFER,
    ERROR_NO_MORE_FILES, ERROR_PIPE_NOT_CONNECTED, HANDLE, HANDLE_FLAG_INHERIT,
    INVALID_HANDLE_VALUE, STATUS_ACCESS_DENIED, STATUS_DELETE_PENDING,
    STATUS_OBJECT_NAME_COLLISION, STATUS_OBJECT_NAME_NOT_FOUND, STATUS_SHARING_VIOLATION,
    UNICODE_STRING, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FileDispositionInfoEx, FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo,
    FileIdExtdDirectoryInfo, FileIdExtdDirectoryRestartInfo, FileIdInfo,
    GetFileInformationByHandle, GetFileInformationByHandleEx, GetFinalPathNameByHandleW, ReadFile,
    SetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ADD_FILE,
    FILE_ADD_SUBDIRECTORY, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD, FILE_DISPOSITION_FLAG_DELETE,
    FILE_DISPOSITION_FLAG_POSIX_SEMANTICS, FILE_DISPOSITION_INFO_EX, FILE_EXECUTE,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_ID_BOTH_DIR_INFO, FILE_ID_EXTD_DIR_INFO, FILE_ID_INFO,
    FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES, FILE_SHARE_DELETE, FILE_SHARE_READ,
    FILE_SHARE_WRITE, FILE_TRAVERSE, FILE_WRITE_ATTRIBUTES, OPEN_EXISTING, SYNCHRONIZE,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
    QueryFullProcessImageNameW, ResumeThread, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    EXTENDED_STARTUPINFO_PRESENT, PROCESS_INFORMATION, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
    STARTF_USESTDHANDLES, STARTUPINFOEXW,
};
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK;

#[path = "windows/handles.rs"]
mod handles;
#[path = "windows/inventory.rs"]
mod inventory;
#[path = "windows/invocations.rs"]
mod invocations;
#[path = "windows/plans.rs"]
mod plans;
#[path = "windows/process.rs"]
mod process;

pub use handles::*;
pub use inventory::*;
pub use invocations::*;
pub use plans::*;
use process::*;

const NORMAL_FILE_FLAGS: u32 = FILE_FLAG_OPEN_REPARSE_POINT;
const DIRECTORY_FLAGS: u32 = FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT;
const DIRECTORY_READ_ACCESS: u32 =
    FILE_LIST_DIRECTORY | FILE_TRAVERSE | FILE_READ_ATTRIBUTES | SYNCHRONIZE;
const DIRECTORY_OWNED_ACCESS: u32 = DIRECTORY_READ_ACCESS
    | FILE_ADD_FILE
    | FILE_ADD_SUBDIRECTORY
    | FILE_DELETE_CHILD
    | FILE_WRITE_ATTRIBUTES
    | DELETE;
const REGULAR_READ_ACCESS: u32 = FILE_GENERIC_READ | FILE_EXECUTE | SYNCHRONIZE;
const REGULAR_OWNED_ACCESS: u32 = FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE | SYNCHRONIZE;
const HELD_SHARE: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;
const OBJ_CASE_INSENSITIVE: u32 = 0x40;

#[cfg(test)]
thread_local! {
    static LAST_CAPTURED_STDOUT: std::cell::RefCell<Vec<u8>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
fn test_remember_overflowing_stdout(read_pipe: HANDLE, count: usize) {
    let mut buffer = vec![0_u8; count.min(4096)];
    let mut read = 0_u32;
    let ok = unsafe {
        ReadFile(
            read_pipe,
            buffer.as_mut_ptr().cast(),
            u32::try_from(buffer.len()).unwrap_or(1),
            &mut read,
            std::ptr::null_mut(),
        )
    };
    if ok != 0 && read != 0 {
        test_remember_captured_stdout(&buffer[..read as usize]);
    }
}

#[cfg(test)]
fn test_remember_captured_stdout(output: &[u8]) {
    LAST_CAPTURED_STDOUT.with(|capture| {
        let mut capture = capture.borrow_mut();
        if capture.len() < 4096 {
            let remaining = 4096 - capture.len();
            let take = remaining.min(output.len());
            capture.extend_from_slice(&output[..take]);
        }
    });
}

#[cfg(test)]
thread_local! {
    static LAST_PUBLISH_STATUSES: std::cell::RefCell<[i32; 11]> =
        const { std::cell::RefCell::new([0; 11]) };
}

#[cfg(test)]
fn test_remember_publish_statuses(statuses: &[i32; 11]) {
    LAST_PUBLISH_STATUSES.with(|slot| *slot.borrow_mut() = *statuses);
}

#[cfg(test)]
pub fn test_last_publish_statuses() -> [i32; 11] {
    LAST_PUBLISH_STATUSES.with(std::cell::RefCell::take)
}

#[cfg(test)]
pub fn test_hold_directory_owned(path: &Path) -> Result<Directory, Error> {
    if !path.is_absolute() {
        return Err(Error::Invalid);
    }
    let canonical = path.canonicalize().map_err(|_| Error::Changed)?;
    if canonical != path {
        return Err(Error::Changed);
    }
    let file = open_absolute(&canonical, DIRECTORY_OWNED_ACCESS, DIRECTORY_FLAGS)?;
    let identity = directory_information(&file)?;
    Ok(Directory { file, identity })
}

#[cfg(test)]
pub fn test_last_captured_stdout() -> Vec<u8> {
    LAST_CAPTURED_STDOUT.with(std::cell::RefCell::take)
}

#[cfg(test)]
pub fn test_publish_stage_identity_probe(
    parent: &Directory,
    stage: &Directory,
    stage_name: &PreparedRelativeNameArena,
) -> String {
    match observe_publish_rebound(parent, stage_name, false, false) {
        Ok(observed) => format!(
            "fresh-open stage identity matches held handle: {}",
            observed == stage.identity
        ),
        Err(error) => format!("fresh-open stage identity probe failed: {error:?}"),
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct Identity {
    volume: u64,
    file_id: [u8; 16],
    attributes: u32,
    length: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectoryIdentity {
    volume: u64,
    file_id: [u8; 16],
    stable_attributes: u32,
}

pub struct Directory {
    file: File,
    identity: DirectoryIdentity,
}

pub struct RegularFile {
    file: File,
    identity: Identity,
    digest: [u8; 32],
}

pub struct SettledRegularFile {
    identity: Identity,
    digest: [u8; 32],
}

pub fn settle_regular_file_for_publish(file: RegularFile) -> SettledRegularFile {
    let RegularFile {
        file,
        identity,
        digest,
    } = file;
    let handle = file.into_raw_handle();
    if unsafe { CloseHandle(handle.cast()) } == 0 {
        std::process::abort();
    }
    SettledRegularFile { identity, digest }
}

pub struct Executable {
    file: RegularFile,
}

pub struct RustcDiscovery {
    executable: Executable,
    resolver: PreparedToolResolver,
}

pub struct DirectRustc {
    executable: Executable,
    sysroot: Directory,
    recheck_resolver: Option<PreparedToolResolver>,
}

pub struct PreparedRelativeName(Vec<u16>);

pub struct PreparedRelativeNameArena {
    units: Vec<u16>,
    maximum: usize,
}

pub struct PreparedVersionInvocation {
    command_line: Vec<u16>,
    output: Vec<u8>,
}

pub struct PreparedSysrootInvocation(PreparedVersionInvocation);
pub struct PreparedRustcVersionInvocation(PreparedVersionInvocation);

const PROCESS_PATH_UNITS: usize = 32_769;
const MAX_TOOL_PATH_UNITS: usize = 32_768;
const MAX_COMMAND_LINE_UNITS: usize = 32_767;
const MAX_PROCESS_ENVIRONMENT_UNITS: usize = 32_768;
const MAX_PROCESS_ATTRIBUTE_BYTES: usize = 1_048_576;

pub struct PreparedProcessArena {
    application: Vec<u16>,
    cwd: Vec<u16>,
    environment: Vec<u16>,
    attributes: Vec<u64>,
    attribute_bytes: usize,
    remaining: usize,
}

pub struct PreparedProcessArenaPlan {
    uses: usize,
    attribute_bytes: usize,
    attribute_words: usize,
    environment_units: usize,
    owned_capacity: usize,
}

pub struct PreparedToolResolver {
    candidate: Vec<u16>,
    canonical: Vec<u16>,
    display: String,
    fallback: Vec<u16>,
    maximum: usize,
}

struct PreparedCommand {
    arguments: Vec<String>,
    command_line: Vec<u16>,
    output: Vec<u8>,
}

pub struct PreparedCCompileInvocation(PreparedCommand);
pub struct PreparedRustCompileInvocation {
    command: PreparedCommand,
    output_name: PreparedRelativeName,
}
pub struct PreparedLinkInvocation {
    command: PreparedCommand,
    output_name: PreparedRelativeName,
}
pub struct PreparedArchiveInvocation {
    command: PreparedCommand,
    input_name: PreparedRelativeName,
    output_name: PreparedRelativeName,
}
pub struct PreparedRunInvocation(PreparedCommand);

const WINDOWS_DYNAMIC_CRT_LINK_ARGS: [&str; 2] = ["-Xlinker", "/NODEFAULTLIB:libcmt"];

const WINDOWS_RUST_STATICLIB_NATIVE_LIBS: [&str; 7] = [
    "-lkernel32",
    "-ladvapi32",
    "-ldbghelp",
    "-lntdll",
    "-luserenv",
    "-lws2_32",
    "-lmsvcrt",
];

pub struct PreparedDiscardNames<const N: usize> {
    names: [Option<PreparedRelativeName>; N],
}

pub struct PreparedLinkOrCopy {
    destination_index: usize,
    storage: Box<[usize]>,
    total: usize,
    #[cfg(debug_assertions)]
    fail_before_authentication: bool,
}

const INVENTORY_EXACT_ARENA_WORDS: usize = 8192;

pub struct PreparedInventoryExact<const N: usize> {
    names: [Option<PreparedRelativeName>; N],
    bindings: [(usize, usize); N],
    storage: Box<[u64]>,
    directory_identity: Option<DirectoryIdentity>,
    remaining: u8,
}

pub struct PreparedInventoryEntriesExact<const N: usize> {
    names: PreparedDiscardNames<N>,
    file_count: usize,
    storage: Box<[u64]>,
    remaining: u8,
}

pub struct PreparedPublishDirectory {
    storage: Box<[usize]>,
    total: usize,
    name_units: usize,
    exact_capacity: usize,
    remaining: u8,
    #[cfg(test)]
    force_extended_rejection: bool,
    #[cfg(test)]
    observed_extended_flags: Option<u32>,
    #[cfg(test)]
    observed_legacy_flags: Option<(u32, u8)>,
    #[cfg(debug_assertions)]
    fail_before_open: bool,
    #[cfg(debug_assertions)]
    fail_information: bool,
    #[cfg(debug_assertions)]
    fail_close: bool,
    #[cfg(debug_assertions)]
    fail_rename: bool,
}

#[repr(C)]
struct NamedInformation {
    flags: u32,
    root_directory: HANDLE,
    file_name_length: u32,
    file_name: [u16; 1],
}
