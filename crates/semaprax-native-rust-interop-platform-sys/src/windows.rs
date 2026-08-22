//! Windows held-handle filesystem, process, and archive authority.
use super::*;
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

fn prepare_command(values: &[&str], output_capacity: usize) -> Result<PreparedCommand, Error> {
    let mut arguments = Vec::with_capacity(values.len());
    if arguments.capacity() != values.len() {
        return Err(Error::OutputLimit);
    }
    for value in values {
        arguments.push((*value).to_owned());
    }
    let command_line = windows_command_line(&arguments)?;
    let output = Vec::with_capacity(output_capacity);
    if output.capacity() != output_capacity {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedCommand {
        arguments,
        command_line,
        output,
    })
}

fn prepared_command_owned_capacity(command: &PreparedCommand) -> usize {
    command
        .arguments
        .capacity()
        .saturating_mul(std::mem::size_of::<String>())
        .saturating_add(
            command
                .arguments
                .iter()
                .map(String::capacity)
                .sum::<usize>(),
        )
        .saturating_add(
            command
                .command_line
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
        .saturating_add(command.output.capacity())
}

pub fn prepare_tool_resolver(
    fallback: &str,
    maximum: usize,
) -> Result<PreparedToolResolver, Error> {
    if maximum == 0
        || maximum > 32_768
        || fallback.is_empty()
        || fallback.contains(['/', '\\', '\0'])
    {
        return Err(Error::Invalid);
    }
    let candidate = Vec::with_capacity(maximum);
    let canonical = Vec::with_capacity(maximum);
    let display_capacity = maximum.checked_mul(3).ok_or(Error::OutputLimit)?;
    let display = String::with_capacity(display_capacity);
    let fallback = fallback.encode_utf16().collect::<Vec<_>>();
    if candidate.capacity() != maximum
        || canonical.capacity() != maximum
        || display.capacity() != display_capacity
    {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedToolResolver {
        candidate,
        canonical,
        display,
        fallback,
        maximum,
    })
}

pub fn prepared_tool_resolver_owned_capacity(prepared: &PreparedToolResolver) -> usize {
    prepared
        .candidate
        .capacity()
        .saturating_add(prepared.canonical.capacity())
        .saturating_add(prepared.fallback.capacity())
        .saturating_mul(std::mem::size_of::<u16>())
        .saturating_add(prepared.display.capacity())
}

pub fn prepare_version_invocation(
    argument: &str,
    maximum: usize,
) -> Result<PreparedVersionInvocation, Error> {
    if maximum > 65_536 {
        return Err(Error::OutputLimit);
    }
    let command_line = windows_command_line(&[argument.to_owned()])?;
    let output = Vec::with_capacity(maximum);
    if output.capacity() != maximum {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedVersionInvocation {
        command_line,
        output,
    })
}

pub fn prepared_version_owned_capacity(prepared: &PreparedVersionInvocation) -> usize {
    prepared
        .command_line
        .capacity()
        .saturating_mul(std::mem::size_of::<u16>())
        .saturating_add(prepared.output.capacity())
}

pub fn prepare_sysroot_invocation(maximum: usize) -> Result<PreparedSysrootInvocation, Error> {
    prepare_version_invocation("--print=sysroot", maximum).map(PreparedSysrootInvocation)
}

pub fn prepare_rustc_version_invocation(
    maximum: usize,
) -> Result<PreparedRustcVersionInvocation, Error> {
    prepare_version_invocation("-vV", maximum).map(PreparedRustcVersionInvocation)
}

pub fn prepared_sysroot_owned_capacity(prepared: &PreparedSysrootInvocation) -> usize {
    prepared_version_owned_capacity(&prepared.0)
}

pub fn prepared_rustc_version_owned_capacity(prepared: &PreparedRustcVersionInvocation) -> usize {
    prepared_version_owned_capacity(&prepared.0)
}

pub(super) fn process_arena_plan(
    uses: usize,
    attribute_bytes: usize,
    environment_units: usize,
) -> Result<PreparedProcessArenaPlan, Error> {
    if uses == 0 || uses > 32 {
        return Err(Error::Invalid);
    }
    if attribute_bytes == 0 {
        return Err(Error::Unsupported);
    }
    if attribute_bytes > MAX_PROCESS_ATTRIBUTE_BYTES {
        return Err(Error::OutputLimit);
    }
    if !(2..=MAX_PROCESS_ENVIRONMENT_UNITS).contains(&environment_units) {
        return Err(Error::OutputLimit);
    }
    let attribute_words = attribute_bytes
        .checked_add(std::mem::size_of::<u64>() - 1)
        .and_then(|bytes| bytes.checked_div(std::mem::size_of::<u64>()))
        .ok_or(Error::OutputLimit)?;
    let path_capacity = PROCESS_PATH_UNITS
        .checked_mul(std::mem::size_of::<u16>())
        .and_then(|bytes| bytes.checked_mul(2))
        .ok_or(Error::OutputLimit)?;
    let owned_capacity = attribute_words
        .checked_mul(std::mem::size_of::<u64>())
        .and_then(|bytes| path_capacity.checked_add(bytes))
        .and_then(|bytes| {
            environment_units
                .checked_mul(std::mem::size_of::<u16>())
                .and_then(|environment| bytes.checked_add(environment))
        })
        .ok_or(Error::OutputLimit)?;
    Ok(PreparedProcessArenaPlan {
        uses,
        attribute_bytes,
        attribute_words,
        environment_units,
        owned_capacity,
    })
}

fn process_environment_units(
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<usize, Error> {
    match (include, libraries) {
        (None, None) => Ok(2),
        (Some(include), Some(libraries)) => {
            let include_units = include.encode_wide().try_fold(0usize, |count, unit| {
                if unit == 0 {
                    Err(Error::Invalid)
                } else {
                    count.checked_add(1).ok_or(Error::OutputLimit)
                }
            })?;
            let library_units = libraries.encode_wide().try_fold(0usize, |count, unit| {
                if unit == 0 {
                    Err(Error::Invalid)
                } else {
                    count.checked_add(1).ok_or(Error::OutputLimit)
                }
            })?;
            8usize
                .checked_add(include_units)
                .and_then(|units| units.checked_add(1))
                .and_then(|units| units.checked_add(4))
                .and_then(|units| units.checked_add(library_units))
                .and_then(|units| units.checked_add(2))
                .filter(|units| *units <= MAX_PROCESS_ENVIRONMENT_UNITS)
                .ok_or(Error::OutputLimit)
        }
        _ => Err(Error::Invalid),
    }
}

pub fn prepare_process_arena_plan(uses: usize) -> Result<PreparedProcessArenaPlan, Error> {
    if uses == 0 || uses > 32 {
        return Err(Error::Invalid);
    }
    let mut attribute_bytes = 0_usize;
    let initialized = unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes)
    };
    if initialized != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(Error::Unsupported);
    }
    process_arena_plan(uses, attribute_bytes, 2)
}

pub fn prepare_process_arena_plan_with_environment(
    uses: usize,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArenaPlan, Error> {
    let environment_units = process_environment_units(include, libraries)?;
    let mut attribute_bytes = 0_usize;
    let initialized = unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes)
    };
    if initialized != 0 || unsafe { GetLastError() } != ERROR_INSUFFICIENT_BUFFER {
        return Err(Error::Unsupported);
    }
    process_arena_plan(uses, attribute_bytes, environment_units)
}

pub fn prepared_process_arena_plan_capacity(plan: &PreparedProcessArenaPlan) -> usize {
    plan.owned_capacity
}

pub fn materialize_process_arena(
    plan: PreparedProcessArenaPlan,
) -> Result<PreparedProcessArena, Error> {
    materialize_process_arena_with_environment(plan, None, None)
}

pub fn materialize_process_arena_with_environment(
    plan: PreparedProcessArenaPlan,
    include: Option<&OsStr>,
    libraries: Option<&OsStr>,
) -> Result<PreparedProcessArena, Error> {
    if process_environment_units(include, libraries)? != plan.environment_units {
        return Err(Error::Invalid);
    }
    let application = Vec::with_capacity(PROCESS_PATH_UNITS);
    let cwd = Vec::with_capacity(PROCESS_PATH_UNITS);
    let mut environment = Vec::with_capacity(plan.environment_units);
    match (include, libraries) {
        (None, None) => environment.extend([0, 0]),
        (Some(include), Some(libraries)) => {
            environment.extend("INCLUDE=".encode_utf16());
            environment.extend(include.encode_wide());
            environment.push(0);
            environment.extend("LIB=".encode_utf16());
            environment.extend(libraries.encode_wide());
            environment.extend([0, 0]);
        }
        _ => return Err(Error::Invalid),
    }
    let attributes = Vec::with_capacity(plan.attribute_words);
    if application.capacity() != PROCESS_PATH_UNITS
        || cwd.capacity() != PROCESS_PATH_UNITS
        || environment.capacity() != plan.environment_units
        || environment.len() != plan.environment_units
        || attributes.capacity() != plan.attribute_words
    {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedProcessArena {
        application,
        cwd,
        environment,
        attributes,
        attribute_bytes: plan.attribute_bytes,
        remaining: plan.uses,
    })
}

pub fn prepare_process_arena(uses: usize) -> Result<PreparedProcessArena, Error> {
    materialize_process_arena(prepare_process_arena_plan(uses)?)
}

pub fn prepared_process_arena_owned_capacity(prepared: &PreparedProcessArena) -> usize {
    prepared
        .application
        .capacity()
        .saturating_mul(std::mem::size_of::<u16>())
        .saturating_add(
            prepared
                .environment
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
        .saturating_add(
            prepared
                .cwd
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
        .saturating_add(
            prepared
                .attributes
                .capacity()
                .saturating_mul(std::mem::size_of::<u64>()),
        )
}

pub fn prepared_process_arena_remaining(prepared: &PreparedProcessArena) -> usize {
    prepared.remaining
}

pub(super) fn consume_process_arena(prepared: &mut PreparedProcessArena) -> Result<(), Error> {
    let attribute_words = prepared
        .attribute_bytes
        .checked_add(std::mem::size_of::<u64>() - 1)
        .and_then(|bytes| bytes.checked_div(std::mem::size_of::<u64>()))
        .ok_or(Error::OutputLimit)?;
    if prepared.application.capacity() != PROCESS_PATH_UNITS
        || prepared.cwd.capacity() != PROCESS_PATH_UNITS
        || !(2..=MAX_PROCESS_ENVIRONMENT_UNITS).contains(&prepared.environment.capacity())
        || prepared.environment.len() != prepared.environment.capacity()
        || !prepared.environment.ends_with(&[0, 0])
        || prepared.attributes.capacity() != attribute_words
    {
        return Err(Error::OutputLimit);
    }
    prepared.remaining = prepared
        .remaining
        .checked_sub(1)
        .ok_or(Error::OutputLimit)?;
    prepared.application.clear();
    prepared.cwd.clear();
    prepared.attributes.clear();
    Ok(())
}

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
    #[cfg(debug_assertions)]
    fail_before_open: bool,
    #[cfg(debug_assertions)]
    fail_information: bool,
    #[cfg(debug_assertions)]
    fail_close: bool,
    #[cfg(debug_assertions)]
    fail_rename: bool,
}

fn prepared_name_bindings<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<[(usize, usize); N], Error> {
    let mut bindings = [(0, 0); N];
    for (index, binding) in bindings.iter_mut().enumerate() {
        let name = prepared_discard_name(names, index)?;
        *binding = (name.0.as_ptr() as usize, name.0.len());
    }
    Ok(bindings)
}

pub fn inventory_exact_required_capacity<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<usize, Error> {
    let names = (0..N).try_fold(0usize, |total, index| {
        total
            .checked_add(
                prepared_discard_name(names, index)?
                    .0
                    .len()
                    .checked_mul(std::mem::size_of::<u16>())
                    .ok_or(Error::OutputLimit)?,
            )
            .ok_or(Error::OutputLimit)
    })?;
    names
        .checked_add(
            INVENTORY_EXACT_ARENA_WORDS
                .checked_mul(std::mem::size_of::<u64>())
                .ok_or(Error::OutputLimit)?,
        )
        .ok_or(Error::OutputLimit)
}

pub fn prepare_inventory_exact<const N: usize>(
    names: &PreparedDiscardNames<N>,
) -> Result<PreparedInventoryExact<N>, Error> {
    let bindings = prepared_name_bindings(names)?;
    let mut copied = [const { None }; N];
    for (index, slot) in copied.iter_mut().enumerate() {
        let source = prepared_discard_name(names, index)?;
        let mut units = Vec::with_capacity(source.0.len());
        units.extend_from_slice(&source.0);
        if units.capacity() != source.0.len() {
            return Err(Error::OutputLimit);
        }
        *slot = Some(PreparedRelativeName(units));
    }
    let storage = vec![0_u64; INVENTORY_EXACT_ARENA_WORDS].into_boxed_slice();
    Ok(PreparedInventoryExact {
        names: copied,
        bindings,
        storage,
        directory_identity: None,
        remaining: 2,
    })
}

pub fn prepared_inventory_exact_owned_capacity<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> usize {
    prepared
        .names
        .iter()
        .filter_map(Option::as_ref)
        .map(|name| name.0.capacity().saturating_mul(std::mem::size_of::<u16>()))
        .sum::<usize>()
        .saturating_add(
            prepared
                .storage
                .len()
                .saturating_mul(std::mem::size_of::<u64>()),
        )
}

pub fn prepared_inventory_exact_remaining<const N: usize>(
    prepared: &PreparedInventoryExact<N>,
) -> u8 {
    prepared.remaining
}

pub fn prepare_inventory_entries_exact<const N: usize>(
    names: [&OsStr; N],
    file_count: usize,
) -> Result<PreparedInventoryEntriesExact<N>, Error> {
    if N == 0 || file_count > N {
        return Err(Error::Invalid);
    }
    Ok(PreparedInventoryEntriesExact {
        names: prepare_discard_names(names)?,
        file_count,
        storage: vec![0_u64; INVENTORY_EXACT_ARENA_WORDS].into_boxed_slice(),
        remaining: 1,
    })
}

pub fn prepared_inventory_entries_exact_owned_capacity<const N: usize>(
    prepared: &PreparedInventoryEntriesExact<N>,
) -> usize {
    prepared_discard_names_owned_capacity(&prepared.names).saturating_add(
        prepared
            .storage
            .len()
            .saturating_mul(std::mem::size_of::<u64>()),
    )
}

fn publish_information_layout(name_units: usize) -> Result<(usize, usize), Error> {
    let name_bytes = name_units.checked_mul(2).ok_or(Error::OutputLimit)?;
    let total = std::mem::size_of::<NamedInformation>()
        .checked_add(name_bytes)
        .ok_or(Error::OutputLimit)?;
    let words = total
        .checked_add(std::mem::size_of::<usize>() - 1)
        .ok_or(Error::OutputLimit)?
        / std::mem::size_of::<usize>();
    Ok((total, words))
}

pub fn publish_directory_required_capacity(name: &OsStr) -> Result<usize, Error> {
    let text = prepared_normal_name(name)?;
    let (_, words) = publish_information_layout(text.encode_utf16().count())?;
    words
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(Error::OutputLimit)
}

pub fn prepare_publish_directory(name: &OsStr) -> Result<PreparedPublishDirectory, Error> {
    let text = prepared_normal_name(name)?;
    let name_units = text.encode_utf16().count();
    let (total, words) = publish_information_layout(name_units)?;
    let exact_capacity = words
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(Error::OutputLimit)?;
    let mut storage = vec![0_usize; words].into_boxed_slice();
    let information = storage.as_mut_ptr().cast::<NamedInformation>();
    unsafe {
        (*information).flags = 0;
        (*information).root_directory = std::ptr::null_mut();
        (*information).file_name_length =
            u32::try_from(name_units.checked_mul(2).ok_or(Error::OutputLimit)?)
                .map_err(|_| Error::OutputLimit)?;
        let destination = std::ptr::addr_of_mut!((*information).file_name).cast::<u16>();
        for (index, unit) in text.encode_utf16().enumerate() {
            destination.add(index).write(unit);
        }
    }
    Ok(PreparedPublishDirectory {
        storage,
        total,
        name_units,
        exact_capacity,
        remaining: 1,
        #[cfg(debug_assertions)]
        fail_before_open: false,
        #[cfg(debug_assertions)]
        fail_information: false,
        #[cfg(debug_assertions)]
        fail_close: false,
        #[cfg(debug_assertions)]
        fail_rename: false,
    })
}

pub fn prepared_publish_directory_owned_capacity(prepared: &PreparedPublishDirectory) -> usize {
    prepared
        .storage
        .len()
        .saturating_mul(std::mem::size_of::<usize>())
}

pub fn prepared_publish_directory_remaining(prepared: &PreparedPublishDirectory) -> u8 {
    prepared.remaining
}

#[cfg(debug_assertions)]
pub fn inject_publish_directory_failure(
    prepared: &mut PreparedPublishDirectory,
    point: u8,
) -> Result<(), Error> {
    match point {
        1 => prepared.fail_before_open = true,
        2 => prepared.fail_information = true,
        3 => prepared.fail_close = true,
        4 => prepared.fail_rename = true,
        _ => return Err(Error::Invalid),
    }
    Ok(())
}

pub fn prepare_link_or_copy<const N: usize>(
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
) -> Result<PreparedLinkOrCopy, Error> {
    let name = prepared_discard_name(names, destination_index)?;
    let (total, words) = link_information_layout(name)?;
    let name_bytes = name.0.len().checked_mul(2).ok_or(Error::OutputLimit)?;
    let mut storage = vec![0_usize; words].into_boxed_slice();
    let information = storage.as_mut_ptr().cast::<NamedInformation>();
    unsafe {
        (*information).flags = 0;
        (*information).root_directory = std::ptr::null_mut();
        (*information).file_name_length =
            u32::try_from(name_bytes).map_err(|_| Error::OutputLimit)?;
        std::ptr::copy_nonoverlapping(
            name.0.as_ptr(),
            std::ptr::addr_of_mut!((*information).file_name).cast::<u16>(),
            name.0.len(),
        );
    }
    Ok(PreparedLinkOrCopy {
        destination_index,
        storage,
        total,
        #[cfg(debug_assertions)]
        fail_before_authentication: false,
    })
}

fn link_information_layout(name: &PreparedRelativeName) -> Result<(usize, usize), Error> {
    let name_bytes = name.0.len().checked_mul(2).ok_or(Error::OutputLimit)?;
    let fixed = std::mem::offset_of!(NamedInformation, file_name);
    let total = fixed
        .checked_add(name_bytes)
        .ok_or(Error::OutputLimit)?
        .max(std::mem::size_of::<NamedInformation>());
    let words = total
        .checked_add(std::mem::size_of::<usize>() - 1)
        .ok_or(Error::OutputLimit)?
        / std::mem::size_of::<usize>();
    Ok((total, words))
}

pub fn link_or_copy_required_capacity<const N: usize>(
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
) -> Result<usize, Error> {
    let name = prepared_discard_name(names, destination_index)?;
    let (_, words) = link_information_layout(name)?;
    words
        .checked_mul(std::mem::size_of::<usize>())
        .ok_or(Error::OutputLimit)
}

pub fn prepared_link_or_copy_owned_capacity(prepared: &PreparedLinkOrCopy) -> usize {
    prepared
        .storage
        .len()
        .saturating_mul(std::mem::size_of::<usize>())
}

#[cfg(debug_assertions)]
pub fn inject_link_or_copy_failure_before_authentication(prepared: &mut PreparedLinkOrCopy) {
    prepared.fail_before_authentication = true;
}

fn ascii_fold(value: u16) -> u16 {
    if value >= u16::from(b'a') && value <= u16::from(b'z') {
        value - u16::from(b'a' - b'A')
    } else {
        value
    }
}

fn prepared_names_equal(left: &PreparedRelativeName, right: &PreparedRelativeName) -> bool {
    left.0.len() == right.0.len()
        && left
            .0
            .iter()
            .zip(&right.0)
            .all(|(left, right)| ascii_fold(*left) == ascii_fold(*right))
}

fn prepared_discard_name<const N: usize>(
    prepared: &PreparedDiscardNames<N>,
    index: usize,
) -> Result<&PreparedRelativeName, Error> {
    prepared
        .names
        .get(index)
        .and_then(Option::as_ref)
        .ok_or(Error::Invalid)
}

fn prepared_matches_slice(expected: &PreparedRelativeName, actual: &[u16]) -> bool {
    expected.0.len() == actual.len()
        && expected
            .0
            .iter()
            .zip(actual)
            .all(|(expected, actual)| ascii_fold(*expected) == ascii_fold(*actual))
}

fn prepared_normal_name(name: &OsStr) -> Result<&str, Error> {
    let text = name.to_str().ok_or(Error::Invalid)?;
    if text.is_empty()
        || !text.is_ascii()
        || matches!(text, "." | "..")
        || text.contains(['/', '\\', '\0'])
        || text.ends_with([' ', '.'])
        || text.contains(':')
    {
        return Err(Error::Invalid);
    }
    let stem = text.split('.').next().ok_or(Error::Invalid)?;
    if ["CON", "PRN", "AUX", "NUL", "CLOCK$"]
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
        || (stem.len() == 4
            && (stem[..3].eq_ignore_ascii_case("COM") || stem[..3].eq_ignore_ascii_case("LPT"))
            && matches!(stem.as_bytes()[3], b'1'..=b'9'))
    {
        return Err(Error::Invalid);
    }
    Ok(text)
}

pub fn prepare_relative_name(name: &OsStr) -> Result<PreparedRelativeName, Error> {
    let text = prepared_normal_name(name)?;
    let exact = text.encode_utf16().count();
    let mut encoded = Vec::with_capacity(exact);
    encoded.extend(text.encode_utf16());
    if encoded.len() != exact || encoded.capacity() != exact {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedRelativeName(encoded))
}

pub fn prepare_relative_name_arena(maximum: usize) -> Result<PreparedRelativeNameArena, Error> {
    let units = Vec::with_capacity(maximum);
    if units.capacity() != maximum {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedRelativeNameArena { units, maximum })
}

pub fn set_relative_name_arena(
    arena: &mut PreparedRelativeNameArena,
    name: &OsStr,
) -> Result<(), Error> {
    let text = prepared_normal_name(name)?;
    if text.encode_utf16().count() > arena.maximum {
        return Err(Error::OutputLimit);
    }
    arena.units.clear();
    arena.units.extend(text.encode_utf16());
    if arena.units.capacity() != arena.maximum {
        return Err(Error::OutputLimit);
    }
    Ok(())
}

pub fn relative_name_arena_capacity(arena: &PreparedRelativeNameArena) -> usize {
    arena.units.capacity()
}

pub fn prepare_discard_names<const N: usize>(
    names: [&OsStr; N],
) -> Result<PreparedDiscardNames<N>, Error> {
    let names = names.map(|name| prepare_relative_name(name).ok());
    if names.iter().any(Option::is_none) {
        return Err(Error::Invalid);
    }
    for left in 0..N {
        for right in 0..left {
            if prepared_names_equal(
                names[left].as_ref().expect("validated"),
                names[right].as_ref().expect("validated"),
            ) {
                return Err(Error::Invalid);
            }
        }
    }
    Ok(PreparedDiscardNames { names })
}

pub fn prepared_discard_names_owned_capacity<const N: usize>(
    prepared: &PreparedDiscardNames<N>,
) -> usize {
    prepared
        .names
        .iter()
        .filter_map(Option::as_ref)
        .map(|name| name.0.capacity().saturating_mul(std::mem::size_of::<u16>()))
        .sum()
}

fn normal_name(name: &OsStr) -> Result<(), Error> {
    let text = name.to_str().ok_or(Error::Invalid)?;
    if text.is_empty()
        || !text.is_ascii()
        || matches!(text, "." | "..")
        || text.contains(['/', '\\', '\0'])
        || text.ends_with([' ', '.'])
        || text.contains(':')
    {
        return Err(Error::Invalid);
    }
    let stem = text
        .split('.')
        .next()
        .ok_or(Error::Invalid)?
        .to_ascii_uppercase();
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CLOCK$")
        || stem.strip_prefix("COM").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
        || stem.strip_prefix("LPT").is_some_and(|suffix| {
            matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9")
        })
    {
        return Err(Error::Invalid);
    }
    Ok(())
}

fn open_directory(path: &Path) -> Result<File, Error> {
    open_absolute(path, DIRECTORY_READ_ACCESS, DIRECTORY_FLAGS)
}

fn open_absolute(path: &Path, access: u32, flags: u32) -> Result<File, Error> {
    let path = wide_null(path.as_os_str())?;
    let handle = unsafe {
        CreateFileW(
            path.as_ptr(),
            access,
            HELD_SHARE,
            std::ptr::null(),
            OPEN_EXISTING,
            flags,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(Error::Changed);
    }
    Ok(unsafe { File::from_raw_handle(handle.cast()) })
}

fn relative_file(
    parent: &File,
    name: &OsStr,
    access: u32,
    disposition: u32,
    create_options: u32,
) -> Result<File, Error> {
    let name = prepare_relative_name(name)?;
    relative_file_prepared(parent, &name, access, disposition, create_options)
}

fn relative_file_prepared(
    parent: &File,
    name: &PreparedRelativeName,
    access: u32,
    disposition: u32,
    create_options: u32,
) -> Result<File, Error> {
    relative_file_units(parent, &name.0, access, disposition, create_options)
}

fn relative_file_arena(
    parent: &File,
    name: &PreparedRelativeNameArena,
    access: u32,
    disposition: u32,
    create_options: u32,
) -> Result<File, Error> {
    relative_file_units(parent, &name.units, access, disposition, create_options)
}

fn relative_file_units(
    parent: &File,
    name: &[u16],
    access: u32,
    disposition: u32,
    create_options: u32,
) -> Result<File, Error> {
    let byte_length = name.len().checked_mul(2).ok_or(Error::Invalid)?;
    let length = u16::try_from(byte_length).map_err(|_| Error::Invalid)?;
    let unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: name.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>())
            .map_err(|_| Error::Changed)?,
        RootDirectory: parent.as_raw_handle().cast(),
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut io = IO_STATUS_BLOCK::default();
    let mut handle = std::ptr::null_mut();
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            access,
            &attributes,
            &mut io,
            std::ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            HELD_SHARE,
            disposition,
            create_options | FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if status < 0 {
        return Err(if status == STATUS_OBJECT_NAME_COLLISION {
            Error::Exists
        } else {
            Error::Changed
        });
    }
    if handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(Error::Changed);
    }
    Ok(unsafe { File::from_raw_handle(handle.cast()) })
}

fn open_relative_regular_read(parent: &Directory, name: &OsStr) -> Result<File, Error> {
    relative_file(
        &parent.file,
        name,
        REGULAR_READ_ACCESS,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )
}

fn information(file: &File) -> Result<Identity, Error> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe {
        GetFileInformationByHandle(
            file.as_raw_handle().cast::<core::ffi::c_void>(),
            &mut information,
        )
    } == 0
    {
        return Err(Error::Changed);
    }
    let mut file_id = FILE_ID_INFO::default();
    if unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle().cast::<core::ffi::c_void>(),
            FileIdInfo,
            (&mut file_id as *mut FILE_ID_INFO).cast(),
            u32::try_from(std::mem::size_of::<FILE_ID_INFO>()).map_err(|_| Error::Changed)?,
        )
    } == 0
    {
        return Err(Error::Changed);
    }
    Ok(Identity {
        volume: file_id.VolumeSerialNumber,
        file_id: file_id.FileId.Identifier,
        attributes: information.dwFileAttributes,
        length: (u64::from(information.nFileSizeHigh) << 32) | u64::from(information.nFileSizeLow),
    })
}

fn stable_directory_identity(identity: Identity) -> Result<DirectoryIdentity, Error> {
    let stable_attributes =
        identity.attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT);
    if stable_attributes != FILE_ATTRIBUTE_DIRECTORY {
        return Err(Error::Changed);
    }
    Ok(DirectoryIdentity {
        volume: identity.volume,
        file_id: identity.file_id,
        stable_attributes,
    })
}

fn directory_information(file: &File) -> Result<DirectoryIdentity, Error> {
    let identity = stable_directory_identity(information(file)?)?;
    if !file.metadata().map_err(|_| Error::Changed)?.is_dir() {
        return Err(Error::Changed);
    }
    Ok(identity)
}

fn digest(file: &File, length: u64) -> Result<[u8; 32], Error> {
    let mut file = file.try_clone().map_err(|_| Error::Changed)?;
    file.seek(SeekFrom::Start(0)).map_err(|_| Error::Changed)?;
    let mut hasher = Sha256::new();
    let mut remaining = length;
    let mut buffer = [0_u8; 8192];
    while remaining != 0 {
        let maximum =
            usize::try_from(remaining.min(buffer.len() as u64)).map_err(|_| Error::OutputLimit)?;
        let count = file
            .read(&mut buffer[..maximum])
            .map_err(|_| Error::Changed)?;
        if count == 0 {
            return Err(Error::Changed);
        }
        hasher.update(&buffer[..count]);
        remaining -= u64::try_from(count).map_err(|_| Error::OutputLimit)?;
    }
    Ok(hasher.finalize().into())
}

fn digest_bytes(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

fn final_path_prepared(file: &File, output: &mut Vec<u16>) -> Result<(), Error> {
    let maximum = output.capacity();
    if maximum != PROCESS_PATH_UNITS {
        return Err(Error::OutputLimit);
    }
    output.clear();
    output.resize(maximum, 0);
    let written = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle().cast(),
            output.as_mut_ptr(),
            u32::try_from(maximum).map_err(|_| Error::OutputLimit)?,
            0,
        )
    };
    let written = usize::try_from(written).map_err(|_| Error::Changed)?;
    if written == 0 || written >= maximum {
        return Err(Error::OutputLimit);
    }
    output.truncate(written);
    let prefix = [
        u16::from(b'\\'),
        u16::from(b'\\'),
        u16::from(b'?'),
        u16::from(b'\\'),
    ];
    if output.starts_with(&prefix) {
        output.copy_within(prefix.len().., 0);
        output.truncate(written - prefix.len());
    }
    output.push(0);
    if output.capacity() != maximum {
        return Err(Error::OutputLimit);
    }
    Ok(())
}

pub fn hold_directory(path: &Path) -> Result<Directory, Error> {
    if !path.is_absolute() {
        return Err(Error::Invalid);
    }
    let canonical = path.canonicalize().map_err(|_| Error::Changed)?;
    if canonical != path {
        return Err(Error::Changed);
    }
    let file = open_directory(path)?;
    let identity = directory_information(&file)?;
    Ok(Directory { file, identity })
}

pub fn recheck_directory(directory: &Directory) -> Result<(), Error> {
    if directory_information(&directory.file)? != directory.identity {
        return Err(Error::Changed);
    }
    Ok(())
}

pub fn same_directory_path(directory: &Directory, path: &Path) -> Result<bool, Error> {
    if !path.is_absolute() {
        return Err(Error::Invalid);
    }
    let rebound = open_directory(path)?;
    Ok(directory_information(&rebound)? == directory.identity)
}

pub fn create_directory_new(
    parent: &Directory,
    name: &OsStr,
    _mode: u32,
) -> Result<Directory, Error> {
    recheck_directory(parent)?;
    normal_name(name)?;
    let file = relative_file(
        &parent.file,
        name,
        DIRECTORY_OWNED_ACCESS,
        FILE_CREATE,
        FILE_DIRECTORY_FILE,
    )?;
    let identity = directory_information(&file)?;
    Ok(Directory { file, identity })
}

pub fn create_directory_new_prepared(
    parent: &Directory,
    name: &PreparedRelativeNameArena,
    _mode: u32,
) -> Result<Directory, Error> {
    recheck_directory(parent)?;
    let file = relative_file_arena(
        &parent.file,
        name,
        DIRECTORY_OWNED_ACCESS,
        FILE_CREATE,
        FILE_DIRECTORY_FILE,
    )?;
    let identity = directory_information(&file)?;
    Ok(Directory { file, identity })
}

pub fn write_file_new(
    directory: &Directory,
    name: &OsStr,
    bytes: &[u8],
    _mode: u32,
) -> Result<RegularFile, Error> {
    recheck_directory(directory)?;
    normal_name(name)?;
    let mut file = relative_file(
        &directory.file,
        name,
        REGULAR_OWNED_ACCESS,
        FILE_CREATE,
        FILE_NON_DIRECTORY_FILE,
    )?;
    file.write_all(bytes).map_err(|_| Error::Changed)?;
    file.sync_all().map_err(|_| Error::Changed)?;
    let identity = information(&file)?;
    if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !file.metadata().map_err(|_| Error::Changed)?.is_file()
    {
        return Err(Error::Changed);
    }
    let digest = digest(&file, identity.length)?;
    Ok(RegularFile {
        file,
        identity,
        digest,
    })
}

pub fn write_file_new_prepared<const N: usize>(
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    index: usize,
    bytes: &[u8],
    _mode: u32,
) -> Result<RegularFile, Error> {
    let name = enter_prepared_file_syscalls(prepared_discard_name(names, index))?;
    recheck_directory(directory)?;
    let mut file = relative_file_prepared(
        &directory.file,
        name,
        REGULAR_OWNED_ACCESS,
        FILE_CREATE,
        FILE_NON_DIRECTORY_FILE,
    )?;
    file.write_all(bytes).map_err(|_| Error::Changed)?;
    file.sync_all().map_err(|_| Error::Changed)?;
    authenticate_regular_file(file)
}

fn hold_regular_file_name_external_read_prepared(
    directory: &Directory,
    name: &PreparedRelativeName,
) -> Result<RegularFile, Error> {
    let file = relative_file_prepared(
        &directory.file,
        name,
        REGULAR_READ_ACCESS,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )?;
    authenticate_regular_file(file)
}

fn hold_regular_file_name_external_read_bounded_prepared(
    directory: &Directory,
    name: &PreparedRelativeName,
    maximum: u64,
) -> Result<RegularFile, Error> {
    let file = relative_file_prepared(
        &directory.file,
        name,
        REGULAR_READ_ACCESS,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )?;
    authenticate_regular_file_bounded(file, maximum)
}

#[cfg(test)]
pub(crate) fn test_hold_regular_file_name_bounded(
    directory: &Directory,
    name: &OsStr,
    maximum: u64,
) -> Result<RegularFile, Error> {
    let name = prepare_relative_name(name)?;
    hold_regular_file_name_external_read_bounded_prepared(directory, &name, maximum)
}

pub fn hold_regular_file(directory: &Directory, name: &OsStr) -> Result<RegularFile, Error> {
    recheck_directory(directory)?;
    let name = prepare_relative_name(name)?;
    hold_regular_file_name_prepared(directory, &name)
}

fn authenticate_regular_file(file: File) -> Result<RegularFile, Error> {
    authenticate_regular_file_bounded(file, u64::MAX)
}

fn authenticate_regular_file_bounded(file: File, maximum: u64) -> Result<RegularFile, Error> {
    let identity = information(&file)?;
    if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !file.metadata().map_err(|_| Error::Changed)?.is_file()
    {
        return Err(Error::Changed);
    }
    if identity.length > maximum {
        return Err(Error::OutputLimit);
    }
    let digest = digest(&file, identity.length)?;
    Ok(RegularFile {
        file,
        identity,
        digest,
    })
}

fn hold_regular_file_name_prepared(
    directory: &Directory,
    name: &PreparedRelativeName,
) -> Result<RegularFile, Error> {
    let file = relative_file_prepared(
        &directory.file,
        name,
        REGULAR_OWNED_ACCESS,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )?;
    authenticate_regular_file(file)
}

pub fn hold_regular_file_prepared<const N: usize>(
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    index: usize,
    tracked: &RegularFile,
) -> Result<RegularFile, Error> {
    let name = enter_prepared_file_syscalls(prepared_discard_name(names, index))?;
    recheck_directory(directory)?;
    let rebound = hold_regular_file_name_prepared(directory, name)?;
    if rebound.identity != tracked.identity || rebound.digest != tracked.digest {
        return Err(Error::Changed);
    }
    Ok(rebound)
}

pub fn transition_regular_file_to_external_read_prepared<const N: usize>(
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    index: usize,
    tracked: &RegularFile,
) -> Result<RegularFile, Error> {
    let name = enter_prepared_file_syscalls(prepared_discard_name(names, index))?;
    recheck_directory(directory)?;
    recheck_held_regular(tracked)?;
    let rebound = hold_regular_file_name_external_read_prepared(directory, name)?;
    if rebound.identity != tracked.identity || rebound.digest != tracked.digest {
        return Err(Error::Changed);
    }
    Ok(rebound)
}

pub fn recheck_regular(file: &RegularFile) -> Result<(), Error> {
    recheck_held_regular(file)
}

fn recheck_held_regular(file: &RegularFile) -> Result<(), Error> {
    let held = information(&file.file)?;
    if held != file.identity || digest(&file.file, file.identity.length)? != file.digest {
        return Err(Error::Changed);
    }
    Ok(())
}

pub fn hold_external_executable(path: &Path) -> Result<Executable, Error> {
    let parent_path = path
        .parent()
        .ok_or(Error::Invalid)?
        .canonicalize()
        .map_err(|_| Error::Changed)?;
    let directory = hold_directory(&parent_path)?;
    let name = path.file_name().ok_or(Error::Invalid)?;
    normal_name(name)?;
    let file = open_relative_regular_read(&directory, name)?;
    let identity = information(&file)?;
    if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !file.metadata().map_err(|_| Error::Changed)?.is_file()
    {
        return Err(Error::Changed);
    }
    let digest = digest(&file, identity.length)?;
    let regular = RegularFile {
        file,
        identity,
        digest,
    };
    let mut prefix = [0_u8; 2];
    let mut duplicate = regular.file.try_clone().map_err(|_| Error::Changed)?;
    duplicate
        .seek(SeekFrom::Start(0))
        .map_err(|_| Error::Changed)?;
    duplicate
        .read_exact(&mut prefix)
        .map_err(|_| Error::Invalid)?;
    if prefix != *b"MZ" {
        return Err(Error::Invalid);
    }
    Ok(Executable { file: regular })
}

fn append_tool_fallback(prepared: &mut PreparedToolResolver) -> Result<(), Error> {
    if prepared.candidate.first() == Some(&u16::from(b'"'))
        && prepared.candidate.last() == Some(&u16::from(b'"'))
        && prepared.candidate.len() >= 2
    {
        prepared.candidate.remove(0);
        prepared.candidate.pop();
    }
    if prepared.candidate.is_empty() {
        prepared.candidate.push(u16::from(b'.'));
    }
    if !prepared.candidate.ends_with(&[u16::from(b'/')])
        && !prepared.candidate.ends_with(&[u16::from(b'\\')])
    {
        prepared.candidate.push(u16::from(b'\\'));
    }
    if prepared
        .candidate
        .len()
        .checked_add(prepared.fallback.len())
        .and_then(|length| length.checked_add(1))
        .is_none_or(|length| length > prepared.maximum)
    {
        return Err(Error::OutputLimit);
    }
    prepared.candidate.extend_from_slice(&prepared.fallback);
    Ok(())
}

fn hold_tool_candidate(
    prepared: &mut PreparedToolResolver,
    record_display: bool,
) -> Result<Option<Executable>, Error> {
    if prepared.candidate.len().saturating_add(1) > prepared.maximum {
        return Err(Error::OutputLimit);
    }
    prepared.candidate.push(0);
    let handle = unsafe {
        CreateFileW(
            prepared.candidate.as_ptr(),
            REGULAR_READ_ACCESS,
            HELD_SHARE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    prepared.candidate.pop();
    if handle == INVALID_HANDLE_VALUE {
        return Ok(None);
    }
    let file = unsafe { File::from_raw_handle(handle.cast()) };
    let identity = information(&file)?;
    if !file.metadata().map_err(|_| Error::Changed)?.is_file() {
        return Ok(None);
    }
    if record_display {
        prepared.canonical.clear();
        prepared.canonical.resize(prepared.maximum, 0);
        let written = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle().cast(),
                prepared.canonical.as_mut_ptr(),
                u32::try_from(prepared.canonical.len()).map_err(|_| Error::OutputLimit)?,
                0,
            )
        };
        let written = usize::try_from(written).map_err(|_| Error::Changed)?;
        if written == 0 || written >= prepared.maximum {
            return Err(Error::OutputLimit);
        }
        prepared.canonical.truncate(written);
        let prefix = [
            u16::from(b'\\'),
            u16::from(b'\\'),
            u16::from(b'?'),
            u16::from(b'\\'),
        ];
        if prepared.canonical.starts_with(&prefix) {
            prepared.canonical.copy_within(prefix.len().., 0);
            prepared.canonical.truncate(written - prefix.len());
        }
        prepared.display.clear();
        for character in char::decode_utf16(prepared.canonical.iter().copied()) {
            prepared
                .display
                .push(character.map_err(|_| Error::Invalid)?);
        }
        if prepared.display.capacity() != prepared.maximum.saturating_mul(3) {
            return Err(Error::OutputLimit);
        }
    }
    let digest = digest(&file, identity.length)?;
    let regular = RegularFile {
        file,
        identity,
        digest,
    };
    let mut prefix = [0_u8; 2];
    let mut duplicate = regular.file.try_clone().map_err(|_| Error::Changed)?;
    duplicate
        .seek(SeekFrom::Start(0))
        .map_err(|_| Error::Changed)?;
    duplicate
        .read_exact(&mut prefix)
        .map_err(|_| Error::Invalid)?;
    if prefix != *b"MZ" {
        return Err(Error::Invalid);
    }
    Ok(Some(Executable { file: regular }))
}

pub fn resolve_and_hold_tool_prepared(
    prepared: PreparedToolResolver,
    configured: Option<&OsStr>,
    paths: Option<&OsStr>,
) -> Result<(Executable, String), Error> {
    let (executable, path, _) =
        resolve_and_hold_tool_reusing_prepared(prepared, configured, paths)?;
    Ok((executable, path))
}

pub fn resolve_and_hold_tool_reusing_prepared(
    mut prepared: PreparedToolResolver,
    configured: Option<&OsStr>,
    paths: Option<&OsStr>,
) -> Result<(Executable, String, PreparedToolResolver), Error> {
    if let Some(configured) = configured {
        prepared.candidate.clear();
        for unit in configured.encode_wide() {
            if prepared.candidate.len().saturating_add(1) > prepared.maximum {
                return Err(Error::OutputLimit);
            }
            prepared.candidate.push(unit);
        }
        if prepared.candidate.is_empty() {
            return Err(Error::Invalid);
        }
        let executable = hold_tool_candidate(&mut prepared, true)?.ok_or(Error::Changed)?;
        let path = std::mem::take(&mut prepared.display);
        return Ok((executable, path, prepared));
    }
    let paths = paths.ok_or(Error::Invalid)?;
    prepared.candidate.clear();
    for unit in paths.encode_wide().chain(std::iter::once(u16::from(b';'))) {
        if unit == u16::from(b';') {
            append_tool_fallback(&mut prepared)?;
            if let Some(executable) = hold_tool_candidate(&mut prepared, true)? {
                let path = std::mem::take(&mut prepared.display);
                return Ok((executable, path, prepared));
            }
            prepared.candidate.clear();
        } else {
            if prepared.candidate.len().saturating_add(2) > prepared.maximum {
                return Err(Error::OutputLimit);
            }
            prepared.candidate.push(unit);
        }
    }
    Err(Error::Changed)
}

fn windows_sysroot_line_actual(output: &[u8]) -> Result<&str, Error> {
    let line = output.strip_suffix(b"\n").ok_or(Error::Invalid)?;
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    if line.is_empty() || line.contains(&0) || line.contains(&b'\n') || line.contains(&b'\r') {
        return Err(Error::Invalid);
    }
    std::str::from_utf8(line).map_err(|_| Error::Invalid)
}

fn windows_sysroot_directory_actual(
    prepared: &mut PreparedToolResolver,
    output: &[u8],
) -> Result<Directory, Error> {
    let line = windows_sysroot_line_actual(output)?;
    prepared.candidate.clear();
    for unit in line.encode_utf16() {
        if prepared.candidate.len().saturating_add(1) > prepared.maximum {
            return Err(Error::OutputLimit);
        }
        prepared.candidate.push(unit);
    }
    let drive = prepared.candidate.first().copied().ok_or(Error::Invalid)?;
    let drive_is_ascii_alphabetic =
        u8::try_from(drive).is_ok_and(|unit| unit.is_ascii_alphabetic());
    if prepared.candidate.capacity() != prepared.maximum
        || prepared.candidate.len() < 4
        || !drive_is_ascii_alphabetic
        || prepared.candidate[1] != u16::from(b':')
        || !matches!(prepared.candidate[2], 47 | 92)
        || matches!(prepared.candidate.last(), Some(47 | 92))
    {
        return Err(Error::Invalid);
    }
    let root = [prepared.candidate[0], u16::from(b':'), u16::from(b'\\'), 0];
    let handle = unsafe {
        CreateFileW(
            root.as_ptr(),
            DIRECTORY_READ_ACCESS,
            HELD_SHARE,
            std::ptr::null(),
            OPEN_EXISTING,
            DIRECTORY_FLAGS,
            std::ptr::null_mut(),
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(Error::Changed);
    }
    let file = unsafe { File::from_raw_handle(handle.cast()) };
    let identity = directory_information(&file)?;
    let mut current = Directory { file, identity };
    let mut start = 3usize;
    while start < prepared.candidate.len() {
        let end = prepared.candidate[start..]
            .iter()
            .position(|unit| matches!(*unit, 47 | 92))
            .map_or(prepared.candidate.len(), |offset| start + offset);
        let component = &prepared.candidate[start..end];
        if component.is_empty()
            || component == [u16::from(b'.')]
            || component == [u16::from(b'.'), u16::from(b'.')]
        {
            return Err(Error::Invalid);
        }
        let file = relative_file_units(
            &current.file,
            component,
            DIRECTORY_READ_ACCESS,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )?;
        let identity = directory_information(&file)?;
        current = Directory { file, identity };
        start = end.saturating_add(1);
    }
    Ok(current)
}

pub fn hold_rustc_discovery_prepared(
    mut prepared: PreparedToolResolver,
    configured: &OsStr,
) -> Result<RustcDiscovery, Error> {
    if !Path::new(configured).is_absolute() {
        return Err(Error::Invalid);
    }
    prepared.candidate.clear();
    for unit in configured.encode_wide() {
        if prepared.candidate.len().saturating_add(1) >= prepared.maximum {
            return Err(Error::OutputLimit);
        }
        prepared.candidate.push(unit);
    }
    if prepared.candidate.is_empty() {
        return Err(Error::Invalid);
    }
    let executable = hold_tool_candidate(&mut prepared, false)?.ok_or(Error::Changed)?;
    Ok(RustcDiscovery {
        executable,
        resolver: prepared,
    })
}

pub fn rustc_discovery_output_prepared(
    discovery: &RustcDiscovery,
    cwd: &Directory,
    prepared: PreparedSysrootInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    version_prepared(&discovery.executable, cwd, prepared.0, process_arena)
}

pub fn hold_direct_rustc_prepared(
    discovery: RustcDiscovery,
    output: &[u8],
) -> Result<DirectRustc, Error> {
    let RustcDiscovery {
        executable,
        mut resolver,
    } = discovery;
    drop(executable);
    let sysroot = windows_sysroot_directory_actual(&mut resolver, output)?;
    let bin = relative_file_units(
        &sysroot.file,
        &[98, 105, 110],
        DIRECTORY_READ_ACCESS,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
    )?;
    let bin_identity = information(&bin)?;
    if bin_identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !bin.metadata().map_err(|_| Error::Changed)?.is_dir()
    {
        return Err(Error::Changed);
    }
    let file = relative_file_units(
        &bin,
        &[114, 117, 115, 116, 99, 46, 101, 120, 101],
        REGULAR_READ_ACCESS,
        FILE_OPEN,
        FILE_NON_DIRECTORY_FILE,
    )?;
    let identity = information(&file)?;
    if identity.attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
        || !file.metadata().map_err(|_| Error::Changed)?.is_file()
    {
        return Err(Error::Changed);
    }
    let digest = digest(&file, identity.length)?;
    let regular = RegularFile {
        file,
        identity,
        digest,
    };
    let mut prefix = [0_u8; 2];
    let mut duplicate = regular.file.try_clone().map_err(|_| Error::Changed)?;
    duplicate
        .seek(SeekFrom::Start(0))
        .map_err(|_| Error::Changed)?;
    duplicate
        .read_exact(&mut prefix)
        .map_err(|_| Error::Invalid)?;
    if prefix != *b"MZ" {
        return Err(Error::Invalid);
    }
    Ok(DirectRustc {
        executable: Executable { file: regular },
        sysroot,
        recheck_resolver: Some(resolver),
    })
}

pub fn direct_rustc_output_prepared(
    direct: &DirectRustc,
    cwd: &Directory,
    prepared: PreparedSysrootInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    version_prepared(&direct.executable, cwd, prepared.0, process_arena)
}

pub fn direct_rustc_version_prepared(
    direct: &DirectRustc,
    cwd: &Directory,
    prepared: PreparedRustcVersionInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    recheck_directory(&direct.sysroot)?;
    version_prepared(&direct.executable, cwd, prepared.0, process_arena)
}

pub fn direct_rustc_reproduces_sysroot(
    direct: &mut DirectRustc,
    output: &[u8],
) -> Result<(), Error> {
    let mut prepared = direct.recheck_resolver.take().ok_or(Error::Invalid)?;
    let rebound = windows_sysroot_directory_actual(&mut prepared, output)?;
    if rebound.identity != direct.sysroot.identity {
        return Err(Error::Changed);
    }
    recheck_held_regular(&direct.executable.file)?;
    recheck_directory(&direct.sysroot)
}

pub fn hold_executable(directory: &Directory, name: &OsStr) -> Result<Executable, Error> {
    recheck_directory(directory)?;
    let name = prepare_relative_name(name)?;
    let file = hold_regular_file_name_external_read_prepared(directory, &name)?;
    let mut prefix = [0_u8; 2];
    let mut duplicate = file.file.try_clone().map_err(|_| Error::Changed)?;
    duplicate
        .seek(SeekFrom::Start(0))
        .map_err(|_| Error::Changed)?;
    duplicate
        .read_exact(&mut prefix)
        .map_err(|_| Error::Invalid)?;
    if prefix != *b"MZ" {
        return Err(Error::Invalid);
    }
    Ok(Executable { file })
}

pub fn executable_regular_file(executable: &Executable) -> Result<RegularFile, Error> {
    recheck_held_regular(&executable.file)?;
    Ok(RegularFile {
        file: executable
            .file
            .file
            .try_clone()
            .map_err(|_| Error::Changed)?,
        identity: executable.file.identity,
        digest: executable.file.digest,
    })
}

pub fn read_exact(file: &RegularFile, maximum: usize) -> Result<Vec<u8>, Error> {
    recheck_regular(file)?;
    let length = usize::try_from(file.identity.length).map_err(|_| Error::OutputLimit)?;
    if length > maximum {
        return Err(Error::OutputLimit);
    }
    let mut bytes = vec![0_u8; length];
    let mut duplicate = file.file.try_clone().map_err(|_| Error::Changed)?;
    duplicate
        .seek(SeekFrom::Start(0))
        .map_err(|_| Error::Changed)?;
    duplicate
        .read_exact(&mut bytes)
        .map_err(|_| Error::Changed)?;
    recheck_regular(file)?;
    Ok(bytes)
}

pub fn compare_exact(
    file: &RegularFile,
    expected: &[u8],
    scratch: &mut [u8; 8192],
) -> Result<bool, Error> {
    recheck_regular(file)?;
    if usize::try_from(file.identity.length).map_err(|_| Error::OutputLimit)? != expected.len() {
        return Ok(false);
    }
    let mut offset = 0usize;
    while offset < expected.len() {
        let chunk = (expected.len() - offset).min(scratch.len());
        let count = file
            .file
            .seek_read(
                &mut scratch[..chunk],
                u64::try_from(offset).map_err(|_| Error::OutputLimit)?,
            )
            .map_err(|_| Error::Changed)?;
        if count == 0 || scratch[..count] != expected[offset..offset + count] {
            return Ok(false);
        }
        offset = offset.checked_add(count).ok_or(Error::OutputLimit)?;
    }
    recheck_regular(file)?;
    Ok(true)
}

pub fn link_or_copy_new_prepared<const N: usize>(
    mut prepared: PreparedLinkOrCopy,
    source: &RegularFile,
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    destination_index: usize,
    source_bytes: &[u8],
) -> Result<RegularFile, Error> {
    if prepared.destination_index != destination_index {
        return Err(Error::Invalid);
    }
    let name = prepared_discard_name(names, destination_index)?;
    if usize::try_from(source.identity.length).map_err(|_| Error::OutputLimit)?
        != source_bytes.len()
        || digest_bytes(source_bytes) != source.digest
    {
        return Err(Error::Changed);
    }
    recheck_held_regular(source)?;
    recheck_directory(directory)?;
    let information = prepared.storage.as_mut_ptr().cast::<NamedInformation>();
    unsafe {
        (*information).root_directory = directory.file.as_raw_handle().cast();
    }
    let mut io = IO_STATUS_BLOCK::default();
    let status = unsafe {
        NtSetInformationFile(
            source.file.as_raw_handle().cast(),
            &mut io,
            prepared.storage.as_mut_ptr().cast(),
            u32::try_from(prepared.total).map_err(|_| Error::Invalid)?,
            FileLinkInformationEx,
        )
    };
    if status < 0 {
        return Err(if status == STATUS_OBJECT_NAME_COLLISION {
            Error::Exists
        } else {
            Error::Changed
        });
    }
    #[cfg(debug_assertions)]
    if prepared.fail_before_authentication {
        return Err(Error::Changed);
    }
    let destination = hold_regular_file_name_prepared(directory, name)?;
    if destination.identity != source.identity || destination.digest != source.digest {
        return Err(Error::Changed);
    }
    Ok(destination)
}

pub fn inventory_entries_exact_prepared<const N: usize, const F: usize, const D: usize>(
    prepared: &mut PreparedInventoryEntriesExact<N>,
    directory: &Directory,
    files: [&RegularFile; F],
    directories: [&Directory; D],
) -> Result<(), Error> {
    if prepared.remaining != 1 || prepared.file_count != F || F.checked_add(D) != Some(N) {
        return Err(Error::Invalid);
    }
    prepared.remaining = 0;
    recheck_directory(directory)?;
    files
        .iter()
        .try_for_each(|file| recheck_held_regular(file))?;
    directories
        .iter()
        .try_for_each(|child| recheck_directory(child))?;
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut records = 0usize;
    let mut queries = 0usize;
    let mut restart = true;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > N.checked_add(3).ok_or(Error::OutputLimit)? {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let class = if restart {
            FileIdExtdDirectoryRestartInfo
        } else {
            FileIdExtdDirectoryInfo
        };
        restart = false;
        let ok = unsafe {
            GetFileInformationByHandleEx(
                directory.file.as_raw_handle().cast(),
                class,
                prepared.storage.as_mut_ptr().cast(),
                u32::try_from(prepared.storage.len() * std::mem::size_of::<u64>())
                    .map_err(|_| Error::Changed)?,
            )
        };
        if ok == 0 {
            if unsafe { GetLastError() } == ERROR_NO_MORE_FILES {
                break;
            }
            return Err(Error::Changed);
        }
        let byte_length = prepared.storage.len() * std::mem::size_of::<u64>();
        let mut offset = 0usize;
        loop {
            let header_end = offset
                .checked_add(std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileName))
                .ok_or(Error::Changed)?;
            if offset
                .checked_add(std::mem::size_of::<FILE_ID_EXTD_DIR_INFO>())
                .ok_or(Error::Changed)?
                > byte_length
                || header_end > byte_length
            {
                return Err(Error::Changed);
            }
            let entry = unsafe {
                &*prepared
                    .storage
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset)
                    .cast::<FILE_ID_EXTD_DIR_INFO>()
            };
            let name_bytes = usize::try_from(entry.FileNameLength).map_err(|_| Error::Changed)?;
            if name_bytes % 2 != 0 {
                return Err(Error::Changed);
            }
            let name_end = header_end.checked_add(name_bytes).ok_or(Error::Changed)?;
            if name_end > byte_length {
                return Err(Error::Changed);
            }
            let name = unsafe {
                std::slice::from_raw_parts(
                    prepared
                        .storage
                        .as_ptr()
                        .cast::<u8>()
                        .add(header_end)
                        .cast::<u16>(),
                    name_bytes / 2,
                )
            };
            let dot = name == [u16::from(b'.')];
            let dot_dot = name == [u16::from(b'.'), u16::from(b'.')];
            records = records.checked_add(1).ok_or(Error::OutputLimit)?;
            if records > N.checked_add(2).ok_or(Error::OutputLimit)? {
                return Err(Error::Changed);
            }
            if !dot && !dot_dot {
                let index = prepared
                    .names
                    .names
                    .iter()
                    .position(|expected| {
                        prepared_matches_slice(expected.as_ref().expect("prepared name"), name)
                    })
                    .ok_or(Error::Changed)?;
                let expected = if index < F {
                    files[index].identity.file_id
                } else {
                    directories[index.checked_sub(F).ok_or(Error::Changed)?]
                        .identity
                        .file_id
                };
                if seen[index] || entry.FileId.Identifier != expected {
                    return Err(Error::Changed);
                }
                seen[index] = true;
                count = count.checked_add(1).ok_or(Error::OutputLimit)?;
            }
            if entry.NextEntryOffset == 0 {
                break;
            }
            let next = usize::try_from(entry.NextEntryOffset).map_err(|_| Error::Changed)?;
            let minimum = name_end.checked_sub(offset).ok_or(Error::Changed)?;
            let next_end = offset.checked_add(next).ok_or(Error::Changed)?;
            if next < minimum
                || next % std::mem::align_of::<FILE_ID_EXTD_DIR_INFO>() != 0
                || next_end > byte_length
            {
                return Err(Error::Changed);
            }
            offset = next_end;
        }
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    for (index, file) in files.iter().enumerate() {
        recheck_named_regular(
            directory,
            prepared_discard_name(&prepared.names, index)?,
            file,
        )?;
    }
    for (index, child) in directories.iter().enumerate() {
        let name = prepared_discard_name(
            &prepared.names,
            F.checked_add(index).ok_or(Error::OutputLimit)?,
        )?;
        let rebound = relative_file_prepared(
            &directory.file,
            name,
            DIRECTORY_READ_ACCESS,
            FILE_OPEN,
            FILE_DIRECTORY_FILE,
        )?;
        if directory_information(&rebound)? != child.identity {
            return Err(Error::Changed);
        }
    }
    recheck_directory(directory)?;
    files
        .iter()
        .try_for_each(|file| recheck_held_regular(file))?;
    directories
        .iter()
        .try_for_each(|child| recheck_directory(child))?;
    Ok(())
}

pub fn inventory_exact_prepared<const N: usize>(
    prepared: &mut PreparedInventoryExact<N>,
    directory: &Directory,
    names: &PreparedDiscardNames<N>,
    files: [Option<&RegularFile>; N],
) -> Result<(), Error> {
    if prepared.remaining == 0
        || prepared.bindings != prepared_name_bindings(names)?
        || files.iter().any(Option::is_none)
    {
        return Err(Error::Invalid);
    }
    match prepared.directory_identity {
        Some(first) if first != directory.identity => return Err(Error::Changed),
        None => prepared.directory_identity = Some(directory.identity),
        Some(_) => {}
    }
    prepared.remaining -= 1;
    recheck_directory(directory)?;
    for file in files.iter().flatten() {
        recheck_held_regular(file)?;
    }
    let mut seen = [false; N];
    let mut count = 0usize;
    let mut raw_records = 0usize;
    let mut queries = 0usize;
    let mut saw_dot = false;
    let mut saw_dot_dot = false;
    let maximum_records = N.checked_add(2).ok_or(Error::OutputLimit)?;
    let maximum_queries = N.checked_add(3).ok_or(Error::OutputLimit)?;
    let mut restart = true;
    loop {
        queries = queries.checked_add(1).ok_or(Error::OutputLimit)?;
        if queries > maximum_queries {
            return Err(Error::Changed);
        }
        prepared.storage.fill(u64::MAX);
        let class = if restart {
            FileIdExtdDirectoryRestartInfo
        } else {
            FileIdExtdDirectoryInfo
        };
        restart = false;
        let ok = unsafe {
            GetFileInformationByHandleEx(
                directory.file.as_raw_handle().cast(),
                class,
                prepared.storage.as_mut_ptr().cast(),
                u32::try_from(prepared.storage.len() * std::mem::size_of::<u64>())
                    .map_err(|_| Error::Changed)?,
            )
        };
        if ok == 0 {
            if unsafe { GetLastError() } == ERROR_NO_MORE_FILES {
                break;
            }
            return Err(Error::Changed);
        }
        let byte_length = prepared.storage.len() * std::mem::size_of::<u64>();
        let mut offset = 0_usize;
        loop {
            let record_header_end = offset
                .checked_add(std::mem::size_of::<FILE_ID_EXTD_DIR_INFO>())
                .ok_or(Error::Changed)?;
            let header_end = offset
                .checked_add(std::mem::offset_of!(FILE_ID_EXTD_DIR_INFO, FileName))
                .ok_or(Error::Changed)?;
            if record_header_end > byte_length || header_end > byte_length {
                return Err(Error::Changed);
            }
            let entry = unsafe {
                &*prepared
                    .storage
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset)
                    .cast::<FILE_ID_EXTD_DIR_INFO>()
            };
            let name_bytes = usize::try_from(entry.FileNameLength).map_err(|_| Error::Changed)?;
            if name_bytes % 2 != 0 {
                return Err(Error::Changed);
            }
            let name_end = header_end.checked_add(name_bytes).ok_or(Error::Changed)?;
            if name_end > byte_length {
                return Err(Error::Changed);
            }
            let name = unsafe {
                std::slice::from_raw_parts(
                    prepared
                        .storage
                        .as_ptr()
                        .cast::<u8>()
                        .add(header_end)
                        .cast::<u16>(),
                    name_bytes / 2,
                )
            };
            let dot = name == [u16::from(b'.')];
            let dot_dot = name == [u16::from(b'.'), u16::from(b'.')];
            raw_records = raw_records.checked_add(1).ok_or(Error::OutputLimit)?;
            if raw_records > maximum_records {
                return Err(Error::Changed);
            }
            if dot {
                if saw_dot {
                    return Err(Error::Changed);
                }
                saw_dot = true;
            } else if dot_dot {
                if saw_dot_dot {
                    return Err(Error::Changed);
                }
                saw_dot_dot = true;
            }
            if !dot && !dot_dot {
                let Some(index) = prepared.names.iter().position(|expected| {
                    prepared_matches_slice(expected.as_ref().expect("prepared name"), name)
                }) else {
                    return Err(Error::Changed);
                };
                let tracked = files[index].expect("attached");
                if seen[index] || entry.FileId.Identifier != tracked.identity.file_id {
                    return Err(Error::Changed);
                }
                seen[index] = true;
                count = count.checked_add(1).ok_or(Error::OutputLimit)?;
                if count > N {
                    return Err(Error::Changed);
                }
            }
            if entry.NextEntryOffset == 0 {
                break;
            }
            let next = usize::try_from(entry.NextEntryOffset).map_err(|_| Error::Changed)?;
            let minimum = name_end.checked_sub(offset).ok_or(Error::Changed)?;
            let next_end = offset.checked_add(next).ok_or(Error::Changed)?;
            if next < minimum
                || next % std::mem::align_of::<FILE_ID_EXTD_DIR_INFO>() != 0
                || next_end > byte_length
            {
                return Err(Error::Changed);
            }
            offset = next_end;
        }
    }
    if count != N || seen.iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    recheck_directory(directory)?;
    for file in files.iter().flatten() {
        recheck_held_regular(file)?;
    }
    Ok(())
}

fn observe_publish_rebound(
    parent: &Directory,
    stage_name: &PreparedRelativeNameArena,
    fail_information: bool,
    fail_close: bool,
) -> Result<DirectoryIdentity, Error> {
    let file = relative_file_arena(
        &parent.file,
        stage_name,
        DIRECTORY_READ_ACCESS,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
    )?;
    let handle = file.into_raw_handle();
    let file = std::mem::ManuallyDrop::new(unsafe { File::from_raw_handle(handle) });
    let observed = if fail_information {
        Err(Error::Changed)
    } else {
        directory_information(&file)
    };
    let close_failed = unsafe { CloseHandle(handle.cast()) } == 0;
    if close_failed || fail_close {
        std::process::abort();
    }
    observed
}

pub fn publish_directory_new_prepared(
    prepared: &mut PreparedPublishDirectory,
    parent: &Directory,
    stage: &Directory,
    stage_name: &PreparedRelativeNameArena,
    output_name: &OsStr,
) -> Result<(), Error> {
    let output = prepared_normal_name(output_name)?;
    let information = prepared.storage.as_mut_ptr().cast::<NamedInformation>();
    let stored_name = unsafe {
        std::slice::from_raw_parts(
            std::ptr::addr_of!((*information).file_name).cast::<u16>(),
            prepared.name_units,
        )
    };
    let total = u32::try_from(prepared.total).map_err(|_| Error::Invalid)?;
    if prepared.remaining != 1
        || prepared.exact_capacity
            != prepared
                .storage
                .len()
                .saturating_mul(std::mem::size_of::<usize>())
        || !output.encode_utf16().eq(stored_name.iter().copied())
        || stage_name.units.is_empty()
    {
        return Err(Error::Invalid);
    }
    prepared.remaining = 0;
    recheck_directory(parent)?;
    recheck_directory(stage)?;
    #[cfg(debug_assertions)]
    if prepared.fail_before_open {
        return Err(Error::Changed);
    }
    #[cfg(debug_assertions)]
    let (fail_information, fail_close) = (prepared.fail_information, prepared.fail_close);
    #[cfg(not(debug_assertions))]
    let (fail_information, fail_close) = (false, false);
    if observe_publish_rebound(parent, stage_name, fail_information, fail_close)? != stage.identity
    {
        return Err(Error::Changed);
    }
    #[cfg(debug_assertions)]
    if prepared.fail_rename {
        return Err(Error::Changed);
    }
    let byte_length = stage_name
        .units
        .len()
        .checked_mul(2)
        .ok_or(Error::Invalid)?;
    let length = u16::try_from(byte_length).map_err(|_| Error::Invalid)?;
    let unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: stage_name.units.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>())
            .map_err(|_| Error::Changed)?,
        RootDirectory: parent.file.as_raw_handle().cast(),
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut rename_handle: HANDLE = std::ptr::null_mut();
    {
        let mut open_io = IO_STATUS_BLOCK::default();
        let status = unsafe {
            NtCreateFile(
                &mut rename_handle,
                DELETE,
                &attributes,
                &mut open_io,
                std::ptr::null(),
                FILE_ATTRIBUTE_NORMAL,
                HELD_SHARE,
                FILE_OPEN,
                FILE_DIRECTORY_FILE,
                std::ptr::null(),
                0,
            )
        };
        if status < 0 || rename_handle.is_null() || rename_handle == INVALID_HANDLE_VALUE {
            return Err(Error::Changed);
        }
    }
    unsafe {
        (*information).root_directory = parent.file.as_raw_handle().cast();
    }
    let mut io = IO_STATUS_BLOCK::default();
    #[cfg(test)]
    let mut attempted_statuses = [0_i32; 11];
    const RENAME_BACKOFF_MILLIS: [u64; 10] = [1, 2, 4, 8, 16, 32, 64, 128, 256, 512];
    let outcome = (|| {
        #[allow(unused_variables)]
        for (attempt, backoff_millis) in RENAME_BACKOFF_MILLIS.into_iter().enumerate() {
            unsafe {
                (*information).flags =
                    windows_sys::Win32::System::WindowsProgramming::FILE_RENAME_FLAG_POSIX_SEMANTICS;
            }
            let status = unsafe {
                NtSetInformationFile(
                    rename_handle,
                    &mut io,
                    information.cast(),
                    total,
                    FileRenameInformationEx,
                )
            };
            #[cfg(test)]
            {
                attempted_statuses[attempt] = status;
            }
            if status == STATUS_OBJECT_NAME_COLLISION {
                return Err(Error::Exists);
            }
            if status >= 0 {
                return Ok(());
            }
            if !matches!(
                status,
                STATUS_SHARING_VIOLATION | STATUS_ACCESS_DENIED | STATUS_DELETE_PENDING
            ) {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(backoff_millis));
        }
        let status = unsafe {
            NtSetInformationFile(
                rename_handle.cast(),
                &mut io,
                information.cast(),
                total,
                FileRenameInformation,
            )
        };
        #[cfg(test)]
        {
            attempted_statuses[10] = status;
        }
        if status == STATUS_OBJECT_NAME_COLLISION {
            return Err(Error::Exists);
        }
        if status >= 0 {
            return Ok(());
        }
        Err(Error::Changed)
    })();
    if unsafe { CloseHandle(rename_handle) } == 0 {
        std::process::abort();
    }
    #[cfg(test)]
    test_remember_publish_statuses(&attempted_statuses);
    outcome
}

pub fn discard_owned_stage_prepared<const N: usize>(
    parent: &Directory,
    stage: &Directory,
    stage_name: &PreparedRelativeNameArena,
    names: &PreparedDiscardNames<N>,
    files: &[Option<&RegularFile>; N],
    settled: &[Option<&SettledRegularFile>; N],
    #[cfg(debug_assertions)] failure_after_delete: Option<usize>,
) -> Result<(), Error> {
    recheck_directory(parent)?;
    recheck_directory(stage)?;
    let rebound = relative_file_arena(
        &parent.file,
        stage_name,
        DIRECTORY_READ_ACCESS,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
    )?;
    if directory_information(&rebound)? != stage.identity {
        return Err(Error::Changed);
    }
    let attached = files
        .iter()
        .zip(settled)
        .take_while(|(file, settled)| file.is_some() ^ settled.is_some())
        .count();
    if files[attached..]
        .iter()
        .zip(&settled[attached..])
        .any(|(file, settled)| file.is_some() || settled.is_some())
    {
        return Err(Error::Invalid);
    }

    let mut seen = [false; N];
    let mut storage = [0_u64; 8192];
    let mut restart = true;
    loop {
        let class = if restart {
            FileIdBothDirectoryRestartInfo
        } else {
            FileIdBothDirectoryInfo
        };
        restart = false;
        let ok = unsafe {
            GetFileInformationByHandleEx(
                stage.file.as_raw_handle().cast(),
                class,
                storage.as_mut_ptr().cast(),
                u32::try_from(storage.len() * std::mem::size_of::<u64>())
                    .map_err(|_| Error::Changed)?,
            )
        };
        if ok == 0 {
            if unsafe { GetLastError() } == ERROR_NO_MORE_FILES {
                break;
            }
            return Err(Error::Changed);
        }
        let byte_length = storage.len() * std::mem::size_of::<u64>();
        let mut offset = 0_usize;
        loop {
            let header_end = offset
                .checked_add(std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName))
                .ok_or(Error::Changed)?;
            if header_end > byte_length {
                return Err(Error::Changed);
            }
            let entry = unsafe {
                &*storage
                    .as_ptr()
                    .cast::<u8>()
                    .add(offset)
                    .cast::<FILE_ID_BOTH_DIR_INFO>()
            };
            let name_bytes = usize::try_from(entry.FileNameLength).map_err(|_| Error::Changed)?;
            if name_bytes % 2 != 0 {
                return Err(Error::Changed);
            }
            let name_end = header_end.checked_add(name_bytes).ok_or(Error::Changed)?;
            if name_end > byte_length {
                return Err(Error::Changed);
            }
            let actual = unsafe {
                std::slice::from_raw_parts(
                    storage.as_ptr().cast::<u8>().add(header_end).cast::<u16>(),
                    name_bytes / 2,
                )
            };
            let dot = actual == [u16::from(b'.')];
            let dot_dot = actual == [u16::from(b'.'), u16::from(b'.')];
            if !dot && !dot_dot {
                let Some(index) = names.names[..attached].iter().position(|expected| {
                    prepared_matches_slice(expected.as_ref().expect("validated"), actual)
                }) else {
                    return Err(Error::Changed);
                };
                if seen[index] {
                    return Err(Error::Changed);
                }
                seen[index] = true;
            }
            if entry.NextEntryOffset == 0 {
                break;
            }
            let next = usize::try_from(entry.NextEntryOffset).map_err(|_| Error::Changed)?;
            if next == 0 || next % std::mem::align_of::<FILE_ID_BOTH_DIR_INFO>() != 0 {
                return Err(Error::Changed);
            }
            offset = offset.checked_add(next).ok_or(Error::Changed)?;
        }
    }
    if seen[..attached].iter().any(|seen| !seen) {
        return Err(Error::Changed);
    }
    let mut deletion_handles: [Option<RegularFile>; N] = std::array::from_fn(|_| None);
    for index in 0..attached {
        let (identity, digest) = if let Some(file) = files[index] {
            recheck_held_regular(file)?;
            (file.identity, file.digest)
        } else {
            let file = settled[index].expect("attached settled prefix");
            (file.identity, file.digest)
        };
        let name = names.names[index].as_ref().expect("validated");
        let rebound = hold_regular_file_name_prepared(stage, name)?;
        if rebound.identity != identity || rebound.digest != digest {
            return Err(Error::Changed);
        }
        deletion_handles[index] = Some(rebound);
    }
    let mut disposition_error = None;
    for (deleted, file) in deletion_handles[..attached].iter().flatten().enumerate() {
        #[cfg(not(debug_assertions))]
        let _ = deleted;
        #[cfg(debug_assertions)]
        if failure_after_delete == Some(deleted) {
            disposition_error = Some(Error::Changed);
            break;
        }
        if let Err(error) = disposition_delete(&file.file) {
            disposition_error = Some(error);
            break;
        }
    }
    #[cfg(debug_assertions)]
    if disposition_error.is_none() && failure_after_delete == Some(attached) {
        disposition_error = Some(Error::Changed);
    }
    must_close_deletion_handles(&mut deletion_handles[..attached]);
    if let Some(error) = disposition_error {
        return Err(error);
    }
    let stage_deletion = relative_file_arena(
        &parent.file,
        stage_name,
        DIRECTORY_OWNED_ACCESS,
        FILE_OPEN,
        FILE_DIRECTORY_FILE,
    )?;
    let observed = directory_information(&stage_deletion);
    if observed != Ok(stage.identity) {
        must_close_file(stage_deletion);
        return Err(Error::Changed);
    }
    disposition_delete_and_close(stage_deletion)
}

#[repr(C)]
struct NamedInformation {
    flags: u32,
    root_directory: HANDLE,
    file_name_length: u32,
    file_name: [u16; 1],
}

fn disposition_delete(file: &File) -> Result<(), Error> {
    let information = FILE_DISPOSITION_INFO_EX {
        Flags: FILE_DISPOSITION_FLAG_DELETE | FILE_DISPOSITION_FLAG_POSIX_SEMANTICS,
    };
    if unsafe {
        SetFileInformationByHandle(
            file.as_raw_handle().cast(),
            FileDispositionInfoEx,
            (&information as *const FILE_DISPOSITION_INFO_EX).cast(),
            u32::try_from(std::mem::size_of_val(&information)).map_err(|_| Error::Changed)?,
        )
    } == 0
    {
        return Err(Error::Changed);
    }
    Ok(())
}

fn disposition_delete_and_close(file: File) -> Result<(), Error> {
    let result = disposition_delete(&file);
    must_close_file(file);
    result
}

fn must_close_file(file: File) {
    let handle = file.into_raw_handle();
    if unsafe { CloseHandle(handle.cast()) } == 0 {
        std::process::abort();
    }
}

fn must_close_deletion_handles(files: &mut [Option<RegularFile>]) {
    let mut close_failed = false;
    for file in files {
        let RegularFile { file, .. } = file.take().expect("authenticated deletion handle");
        let handle = file.into_raw_handle();
        close_failed |= unsafe { CloseHandle(handle.cast()) } == 0;
    }
    if close_failed {
        std::process::abort();
    }
}

fn run_argv(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[String],
    stdout_limit: usize,
    prepared_command_line: Option<Vec<u16>>,
    prepared_output: Option<Vec<u8>>,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    if arguments.len() > 32
        || prepared_command_line.as_ref().is_none_or(Vec::is_empty)
        || prepared_output
            .as_ref()
            .is_none_or(|output| output.capacity() != stdout_limit || !output.is_empty())
    {
        return Err(Error::Invalid);
    }
    consume_process_arena(process_arena)?;
    struct CheckedHandle(Option<HANDLE>);
    impl CheckedHandle {
        fn new(handle: HANDLE) -> Self {
            Self(Some(handle))
        }

        fn raw(&self) -> HANDLE {
            self.0.expect("checked handle remains owned")
        }

        fn close(mut self) -> Result<(), Error> {
            let handle = self.0.take().expect("checked handle remains owned");
            if unsafe { CloseHandle(handle) } == 0 {
                Err(Error::Spawn)
            } else {
                Ok(())
            }
        }
    }
    impl Drop for CheckedHandle {
        fn drop(&mut self) {
            if let Some(handle) = self.0.take() {
                if unsafe { CloseHandle(handle) } == 0 {
                    std::process::abort();
                }
            }
        }
    }
    fn must_close(handles: [CheckedHandle; 4]) {
        let mut failed = false;
        for handle in handles {
            failed |= handle.close().is_err();
        }
        if failed {
            std::process::abort();
        }
    }
    recheck_held_regular(&executable.file)?;
    recheck_directory(cwd)?;
    final_path_prepared(&executable.file.file, &mut process_arena.application)?;
    final_path_prepared(&cwd.file, &mut process_arena.cwd)?;
    let mut command_line = prepared_command_line.ok_or(Error::Invalid)?;

    let security = SECURITY_ATTRIBUTES {
        nLength: u32::try_from(std::mem::size_of::<SECURITY_ATTRIBUTES>())
            .map_err(|_| Error::Spawn)?,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let mut read_pipe = std::ptr::null_mut();
    let mut write_pipe = std::ptr::null_mut();
    if unsafe { CreatePipe(&mut read_pipe, &mut write_pipe, &security, 0) } == 0 {
        return Err(Error::Spawn);
    }
    let read_pipe = CheckedHandle::new(read_pipe);
    let write_pipe = CheckedHandle::new(write_pipe);
    if unsafe { SetHandleInformation(read_pipe.raw(), HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(Error::Spawn);
    }
    let null_name = [u16::from(b'N'), u16::from(b'U'), u16::from(b'L'), 0];
    let null_handle = unsafe {
        CreateFileW(
            null_name.as_ptr(),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            std::ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if null_handle == INVALID_HANDLE_VALUE {
        return Err(Error::Spawn);
    }
    let null_handle = CheckedHandle::new(null_handle);
    if unsafe { SetHandleInformation(null_handle.raw(), HANDLE_FLAG_INHERIT, HANDLE_FLAG_INHERIT) }
        == 0
    {
        return Err(Error::Spawn);
    }

    let inherited = [null_handle.raw(), write_pipe.raw()];
    let mut attribute_bytes = process_arena.attribute_bytes;
    let attribute_words = attribute_bytes
        .checked_add(std::mem::size_of::<u64>() - 1)
        .and_then(|bytes| bytes.checked_div(std::mem::size_of::<u64>()))
        .ok_or(Error::OutputLimit)?;
    process_arena.attributes.resize(attribute_words, 0);
    if attribute_bytes
        > process_arena
            .attributes
            .len()
            .saturating_mul(std::mem::size_of::<u64>())
        || process_arena.attributes.capacity() != attribute_words
    {
        return Err(Error::OutputLimit);
    }
    let attribute_list = process_arena.attributes.as_mut_ptr().cast();
    if unsafe { InitializeProcThreadAttributeList(attribute_list, 1, 0, &mut attribute_bytes) } == 0
        || attribute_bytes != process_arena.attribute_bytes
    {
        return Err(Error::Spawn);
    }
    struct AttributeList(windows_sys::Win32::System::Threading::LPPROC_THREAD_ATTRIBUTE_LIST);
    impl Drop for AttributeList {
        fn drop(&mut self) {
            unsafe { DeleteProcThreadAttributeList(self.0) };
        }
    }
    let attribute_list = AttributeList(attribute_list);
    if unsafe {
        UpdateProcThreadAttribute(
            attribute_list.0,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            inherited.as_ptr().cast(),
            std::mem::size_of_val(&inherited),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    } == 0
    {
        return Err(Error::Spawn);
    }

    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
        return Err(Error::Spawn);
    }
    let job = CheckedHandle::new(job);
    let mut job_limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    job_limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if unsafe {
        SetInformationJobObject(
            job.raw(),
            JobObjectExtendedLimitInformation,
            (&job_limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            u32::try_from(std::mem::size_of_val(&job_limits)).map_err(|_| Error::Spawn)?,
        )
    } == 0
    {
        return Err(Error::Spawn);
    }

    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb =
        u32::try_from(std::mem::size_of::<STARTUPINFOEXW>()).map_err(|_| Error::Spawn)?;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = null_handle.raw();
    startup.StartupInfo.hStdOutput = write_pipe.raw();
    startup.StartupInfo.hStdError = null_handle.raw();
    startup.lpAttributeList = attribute_list.0;
    let mut process = PROCESS_INFORMATION::default();
    let created = unsafe {
        CreateProcessW(
            process_arena.application.as_ptr(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            process_arena.environment.as_ptr().cast(),
            process_arena.cwd.as_ptr(),
            &startup.StartupInfo,
            &mut process,
        )
    };
    drop(attribute_list);
    if created == 0 {
        if write_pipe.close().is_err() {
            std::process::abort();
        }
        return Err(Error::Spawn);
    }
    let process_handle = CheckedHandle::new(process.hProcess);
    let thread_handle = CheckedHandle::new(process.hThread);
    if write_pipe.close().is_err() {
        if terminate_unassigned(process_handle.raw()).is_err() {
            std::process::abort();
        }
        let mut failed = false;
        failed |= thread_handle.close().is_err();
        failed |= read_pipe.close().is_err();
        failed |= null_handle.close().is_err();
        failed |= process_handle.close().is_err();
        failed |= job.close().is_err();
        let _ = failed;
        std::process::abort();
    }

    fn must_terminate_unassigned(process: HANDLE) {
        if terminate_unassigned(process).is_err() {
            std::process::abort();
        }
    }

    fn must_settle_job(job: HANDLE, process: HANDLE, terminate: bool) {
        if settle_job(job, process, terminate).is_err() {
            std::process::abort();
        }
    }

    let image_matches = (|| {
        process_arena.application.clear();
        process_arena.application.resize(PROCESS_PATH_UNITS, 0);
        let mut image_len =
            u32::try_from(process_arena.application.len()).map_err(|_| Error::Spawn)?;
        if unsafe {
            QueryFullProcessImageNameW(
                process_handle.raw(),
                0,
                process_arena.application.as_mut_ptr(),
                &mut image_len,
            )
        } == 0
        {
            return Err(Error::Changed);
        }
        let image_len = usize::try_from(image_len).map_err(|_| Error::Spawn)?;
        if image_len == 0 || image_len.saturating_add(1) > PROCESS_PATH_UNITS {
            return Err(Error::OutputLimit);
        }
        process_arena.application.truncate(image_len);
        process_arena.application.push(0);
        let file_handle = unsafe {
            CreateFileW(
                process_arena.application.as_ptr(),
                REGULAR_READ_ACCESS,
                HELD_SHARE,
                std::ptr::null(),
                OPEN_EXISTING,
                NORMAL_FILE_FLAGS,
                std::ptr::null_mut(),
            )
        };
        if file_handle == INVALID_HANDLE_VALUE {
            return Err(Error::Changed);
        }
        let file = unsafe { File::from_raw_handle(file_handle.cast()) };
        let identity = information(&file)?;
        let bytes = digest(&file, identity.length)?;
        recheck_held_regular(&executable.file)?;
        recheck_directory(cwd)?;
        Ok(!injected_settlement_failure!(WindowsImage)
            && identity == executable.file.identity
            && bytes == executable.file.digest)
    })();
    if image_matches != Ok(true) {
        must_terminate_unassigned(process_handle.raw());
        return Err(Error::Changed);
    }
    if injected_settlement_failure!(WindowsAssign)
        || unsafe { AssignProcessToJobObject(job.raw(), process_handle.raw()) } == 0
    {
        must_terminate_unassigned(process_handle.raw());
        return Err(Error::Changed);
    }
    if injected_settlement_failure!(WindowsResume)
        || unsafe { ResumeThread(thread_handle.raw()) } == u32::MAX
    {
        must_settle_job(job.raw(), process_handle.raw(), true);
        return Err(Error::Spawn);
    }
    if thread_handle.close().is_err() {
        must_settle_job(job.raw(), process_handle.raw(), true);
        std::process::abort();
    }
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    let output_is_prepared = prepared_output.is_some();
    let mut output = prepared_output.unwrap_or_default();
    if output_is_prepared && (output.capacity() != stdout_limit || !output.is_empty()) {
        must_settle_job(job.raw(), process_handle.raw(), true);
        return Err(Error::OutputLimit);
    }
    let mut selected_error = None;
    loop {
        let mut available = 0_u32;
        if injected_settlement_failure!(WindowsPeek) {
            selected_error = Some(Error::Spawn);
            break;
        }
        if unsafe {
            PeekNamedPipe(
                read_pipe.raw(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        } == 0
        {
            let error = unsafe { GetLastError() };
            if !matches!(error, ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) {
                selected_error = Some(Error::Spawn);
                break;
            }
            available = 0;
        }
        while available != 0 {
            let count = usize::try_from(available).unwrap_or(usize::MAX).min(8192);
            if count > stdout_limit.saturating_sub(output.len()) {
                #[cfg(test)]
                test_remember_overflowing_stdout(read_pipe.raw(), count);
                selected_error = Some(Error::OutputLimit);
                break;
            }
            let mut buffer = [0_u8; 8192];
            let mut read = 0_u32;
            if injected_settlement_failure!(WindowsRead)
                || unsafe {
                    ReadFile(
                        read_pipe.raw(),
                        buffer.as_mut_ptr().cast(),
                        u32::try_from(count).map_err(|_| Error::Spawn)?,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                } == 0
            {
                selected_error = Some(Error::Spawn);
                break;
            }
            let read = usize::try_from(read).map_err(|_| Error::Spawn)?;
            if read == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..read]);
            if output_is_prepared && output.capacity() != stdout_limit {
                selected_error = Some(Error::OutputLimit);
                break;
            }
            available = available.saturating_sub(u32::try_from(read).unwrap_or(u32::MAX));
        }
        if selected_error.is_some() {
            break;
        }
        match unsafe { WaitForSingleObject(process_handle.raw(), 0) } {
            WAIT_OBJECT_0 => break,
            WAIT_TIMEOUT => {}
            WAIT_FAILED => {
                selected_error = Some(Error::Spawn);
                break;
            }
            _ => {
                selected_error = Some(Error::Spawn);
                break;
            }
        }
        if std::time::Instant::now() >= deadline {
            selected_error = Some(Error::Spawn);
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    if selected_error.is_none() {
        let mut exit_code = u32::MAX;
        if unsafe {
            windows_sys::Win32::System::Threading::GetExitCodeProcess(
                process_handle.raw(),
                &mut exit_code,
            )
        } == 0
        {
            selected_error = Some(Error::Spawn);
        } else if exit_code != 0 {
            selected_error = Some(Error::Exit);
        }
    }
    must_settle_job(job.raw(), process_handle.raw(), true);
    if let Some(error) = selected_error {
        must_close([read_pipe, null_handle, process_handle, job]);
        return Err(error);
    }
    let result = (|| {
        loop {
            let mut available = 0_u32;
            if unsafe {
                PeekNamedPipe(
                    read_pipe.raw(),
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                let error = unsafe { GetLastError() };
                if matches!(error, ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) {
                    break;
                }
                return Err(Error::Spawn);
            }
            if available == 0 {
                break;
            }
            let count = usize::try_from(available).unwrap_or(usize::MAX).min(8192);
            if count > stdout_limit.saturating_sub(output.len()) {
                #[cfg(test)]
                test_remember_overflowing_stdout(read_pipe.raw(), count);
                return Err(Error::OutputLimit);
            }
            let mut buffer = [0_u8; 8192];
            let mut read = 0_u32;
            if unsafe {
                ReadFile(
                    read_pipe.raw(),
                    buffer.as_mut_ptr().cast(),
                    u32::try_from(count).map_err(|_| Error::Spawn)?,
                    &mut read,
                    std::ptr::null_mut(),
                )
            } == 0
            {
                return Err(Error::Spawn);
            }
            let read = usize::try_from(read).map_err(|_| Error::Spawn)?;
            if read == 0 {
                break;
            }
            output.extend_from_slice(&buffer[..read]);
        }
        recheck_regular(&executable.file)?;
        recheck_directory(cwd)?;
        Ok(output)
    })();
    must_close([read_pipe, null_handle, process_handle, job]);
    result
}

fn terminate_unassigned(process: HANDLE) -> Result<(), Error> {
    let terminate_failed = unsafe { TerminateProcess(process, 126) } == 0;
    let wait = unsafe { WaitForSingleObject(process, 30_000) };
    if terminate_failed
        || wait != WAIT_OBJECT_0
        || injected_settlement_failure!(WindowsUnassigned)
        || injected_settlement_failure!(WindowsTerminateProcess)
        || injected_settlement_failure!(WindowsWaitUnassigned)
    {
        return Err(Error::Spawn);
    }
    Ok(())
}

fn settle_job(job: HANDLE, process: HANDLE, terminate: bool) -> Result<(), Error> {
    let terminate_failed = terminate
        && (unsafe { TerminateJobObject(job, 126) } == 0
            || injected_settlement_failure!(WindowsJob)
            || injected_settlement_failure!(WindowsTerminateJob));
    let leader_wait = unsafe { WaitForSingleObject(process, 30_000) };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        if injected_settlement_failure!(WindowsQueryJob)
            || unsafe {
                QueryInformationJobObject(
                    job,
                    JobObjectBasicAccountingInformation,
                    (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    u32::try_from(std::mem::size_of_val(&accounting)).map_err(|_| Error::Spawn)?,
                    std::ptr::null_mut(),
                )
            } == 0
        {
            return Err(Error::Spawn);
        }
        if accounting.ActiveProcesses == 0 {
            return if terminate_failed || leader_wait != WAIT_OBJECT_0 {
                Err(Error::Spawn)
            } else {
                Ok(())
            };
        }
        if std::time::Instant::now() >= deadline {
            return Err(Error::Spawn);
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

fn wide_null(value: &OsStr) -> Result<Vec<u16>, Error> {
    let mut wide = value.encode_wide().collect::<Vec<_>>();
    if wide.is_empty() || wide.contains(&0) {
        return Err(Error::Invalid);
    }
    wide.push(0);
    Ok(wide)
}

fn windows_command_line(arguments: &[String]) -> Result<Vec<u16>, Error> {
    let mut line = String::from("semaprax-native-rust-interop-tool");
    for argument in arguments {
        if argument.contains(['\0', '\r', '\n']) {
            return Err(Error::Invalid);
        }
        line.push(' ');
        let needs_quotes = argument.is_empty()
            || argument
                .chars()
                .any(|character| matches!(character, ' ' | '\t' | '"'));
        if !needs_quotes {
            line.push_str(argument);
            continue;
        }
        line.push('"');
        let mut slashes = 0_usize;
        for character in argument.chars() {
            if character == '\\' {
                slashes += 1;
            } else {
                if character == '"' {
                    line.extend(std::iter::repeat_n('\\', slashes * 2 + 1));
                } else {
                    line.extend(std::iter::repeat_n('\\', slashes));
                }
                slashes = 0;
                line.push(character);
            }
        }
        line.extend(std::iter::repeat_n('\\', slashes * 2));
        line.push('"');
    }
    wide_null(OsStr::new(&line))
}

fn preflight_windows_command_line(arguments: &[&[&str]]) -> Result<(), Error> {
    let mut units = "semaprax-native-rust-interop-tool"
        .encode_utf16()
        .count()
        .checked_add(1)
        .ok_or(Error::OutputLimit)?;
    for parts in arguments {
        units = units.checked_add(3).ok_or(Error::OutputLimit)?;
        let mut slashes = 0_usize;
        for character in parts.iter().flat_map(|part| part.chars()) {
            if matches!(character, '\0' | '\r' | '\n') {
                return Err(Error::Invalid);
            }
            if character == '\\' {
                slashes = slashes.checked_add(1).ok_or(Error::OutputLimit)?;
                continue;
            }
            let escaped_slashes = if character == '"' {
                slashes
                    .checked_mul(2)
                    .and_then(|count| count.checked_add(1))
                    .ok_or(Error::OutputLimit)?
            } else {
                slashes
            };
            units = units
                .checked_add(escaped_slashes)
                .and_then(|count| count.checked_add(character.len_utf16()))
                .ok_or(Error::OutputLimit)?;
            slashes = 0;
        }
        units = units
            .checked_add(slashes.checked_mul(2).ok_or(Error::OutputLimit)?)
            .ok_or(Error::OutputLimit)?;
    }
    if units > MAX_COMMAND_LINE_UNITS {
        return Err(Error::OutputLimit);
    }
    Ok(())
}

pub fn rustc_version(
    executable: &Executable,
    cwd: &Directory,
    maximum: usize,
) -> Result<Vec<u8>, Error> {
    let prepared = prepare_version_invocation("-vV", maximum.min(65_536))?;
    let mut process_arena = prepare_process_arena(1)?;
    version_prepared(executable, cwd, prepared, &mut process_arena)
}
pub fn clang_version(
    executable: &Executable,
    cwd: &Directory,
    maximum: usize,
) -> Result<Vec<u8>, Error> {
    let prepared = prepare_version_invocation("--version", maximum.min(65_536))?;
    let mut process_arena = prepare_process_arena(1)?;
    version_prepared(executable, cwd, prepared, &mut process_arena)
}

pub fn version_prepared(
    executable: &Executable,
    cwd: &Directory,
    prepared: PreparedVersionInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    let maximum = prepared.output.capacity();
    run_argv(
        executable,
        cwd,
        &[],
        maximum,
        Some(prepared.command_line),
        Some(prepared.output),
        process_arena,
    )
}

pub fn prepare_c_compile_invocation(
    target: &str,
    input: &OsStr,
    optimization: u8,
    sanitizers: bool,
    maximum: usize,
) -> Result<PreparedCCompileInvocation, Error> {
    normal_name(input)?;
    if sanitizers
        || !matches!(optimization, 0 | 2)
        || target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(Error::Invalid);
    }
    let input = input.to_str().ok_or(Error::Invalid)?;
    Ok(PreparedCCompileInvocation(prepare_command(
        &[
            "-std=c11",
            "-target",
            target,
            "-Wall",
            "-Wextra",
            "-Werror",
            if optimization == 0 { "-O0" } else { "-O2" },
            "-c",
            input,
            "-o",
            "-",
        ],
        maximum.min(33_554_432),
    )?))
}

pub fn prepared_c_compile_owned_capacity(prepared: &PreparedCCompileInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.0)
}

pub fn compile_c_prepared(
    executable: &Executable,
    cwd: &Directory,
    prepared: PreparedCCompileInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Vec<u8>, Error> {
    let maximum = prepared.0.output.capacity();
    run_argv(
        executable,
        cwd,
        &prepared.0.arguments,
        maximum,
        Some(prepared.0.command_line),
        Some(prepared.0.output),
        process_arena,
    )
}

pub fn prepare_rust_compile_invocation(
    target: &str,
    source: &OsStr,
    output: &OsStr,
) -> Result<PreparedRustCompileInvocation, Error> {
    normal_name(source)?;
    normal_name(output)?;
    if target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(Error::Invalid);
    }
    Ok(PreparedRustCompileInvocation {
        command: prepare_command(
            &[
                "--edition=2021",
                "-Dwarnings",
                "--crate-type",
                "staticlib",
                "-C",
                "panic=unwind",
                "--target",
                target,
                source.to_str().ok_or(Error::Invalid)?,
                "-o",
                output.to_str().ok_or(Error::Invalid)?,
            ],
            0,
        )?,
        output_name: prepare_relative_name(output)?,
    })
}

pub fn prepared_rust_compile_owned_capacity(prepared: &PreparedRustCompileInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.command).saturating_add(
        prepared
            .output_name
            .0
            .capacity()
            .saturating_mul(std::mem::size_of::<u16>()),
    )
}

fn compile_rust_prepared_inner(
    rustc: &Executable,
    cwd: &Directory,
    prepared: PreparedRustCompileInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<RegularFile, Error> {
    if hold_regular_file_name_prepared(cwd, &prepared.output_name).is_ok() {
        return Err(Error::Exists);
    }
    if !run_argv(
        rustc,
        cwd,
        &prepared.command.arguments,
        0,
        Some(prepared.command.command_line),
        Some(prepared.command.output),
        process_arena,
    )?
    .is_empty()
    {
        return Err(Error::OutputLimit);
    }
    hold_regular_file_name_prepared(cwd, &prepared.output_name)
}

pub fn compile_direct_rustc_prepared(
    rustc: &DirectRustc,
    cwd: &Directory,
    prepared: PreparedRustCompileInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<RegularFile, Error> {
    recheck_directory(&rustc.sysroot)?;
    compile_rust_prepared_inner(&rustc.executable, cwd, prepared, process_arena)
}

#[allow(clippy::too_many_arguments)]
pub fn prepare_link_invocation(
    target: &str,
    linker: Option<&OsStr>,
    vctools: Option<&OsStr>,
    harness: &OsStr,
    c_object: &OsStr,
    rust_archive: &OsStr,
    output: &OsStr,
    sanitizers: bool,
) -> Result<PreparedLinkInvocation, Error> {
    for name in [harness, c_object, rust_archive, output] {
        normal_name(name)?;
    }
    let linker = linker
        .filter(|path| std::path::Path::new(path).is_absolute())
        .and_then(OsStr::to_str)
        .ok_or(Error::Invalid)?;
    let vctools = vctools
        .filter(|path| std::path::Path::new(path).is_absolute())
        .and_then(OsStr::to_str)
        .ok_or(Error::Invalid)?;
    let linker_units = linker.encode_utf16().count();
    let vctools_units = vctools.encode_utf16().count();
    if linker_units == 0
        || linker_units > MAX_TOOL_PATH_UNITS
        || vctools_units == 0
        || vctools_units > MAX_TOOL_PATH_UNITS
    {
        return Err(Error::OutputLimit);
    }
    if std::path::Path::new(linker).strip_prefix(vctools).ok()
        != Some(std::path::Path::new(r"bin\Hostx64\x64\link.exe"))
    {
        return Err(Error::Invalid);
    }
    if sanitizers
        || target.is_empty()
        || !target
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(Error::Invalid);
    }
    let harness = harness.to_str().ok_or(Error::Invalid)?;
    let c_object = c_object.to_str().ok_or(Error::Invalid)?;
    let rust_archive = rust_archive.to_str().ok_or(Error::Invalid)?;
    let output_text = output.to_str().ok_or(Error::Invalid)?;
    let argument_parts: [&[&str]; 19] = [
        &["-target"],
        &[target],
        &["-Xmicrosoft-visualc-tools-root"],
        &[vctools],
        &["-fuse-ld=link"],
        &[WINDOWS_DYNAMIC_CRT_LINK_ARGS[0]],
        &[WINDOWS_DYNAMIC_CRT_LINK_ARGS[1]],
        &[harness],
        &[c_object],
        &[rust_archive],
        &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[0]],
        &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[1]],
        &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[2]],
        &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[3]],
        &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[4]],
        &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[5]],
        &[WINDOWS_RUST_STATICLIB_NATIVE_LIBS[6]],
        &["-o"],
        &[output_text],
    ];
    preflight_windows_command_line(&argument_parts)?;
    let mut arguments = Vec::with_capacity(argument_parts.len());
    for parts in argument_parts {
        let capacity = parts.iter().try_fold(0_usize, |total, part| {
            total.checked_add(part.len()).ok_or(Error::OutputLimit)
        })?;
        let mut argument = String::with_capacity(capacity);
        for part in parts {
            argument.push_str(part);
        }
        if argument.capacity() != capacity {
            return Err(Error::OutputLimit);
        }
        arguments.push(argument);
    }
    if arguments.len() != 19 || arguments.capacity() != 19 {
        return Err(Error::OutputLimit);
    }
    let command_line = windows_command_line(&arguments)?;
    Ok(PreparedLinkInvocation {
        command: PreparedCommand {
            arguments,
            command_line,
            output: Vec::new(),
        },
        output_name: prepare_relative_name(output)?,
    })
}

pub fn prepared_link_owned_capacity(prepared: &PreparedLinkInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.command).saturating_add(
        prepared
            .output_name
            .0
            .capacity()
            .saturating_mul(std::mem::size_of::<u16>()),
    )
}

#[cfg(test)]
pub(super) fn test_prepared_link_arguments(
    prepared: &PreparedLinkInvocation,
) -> (&[String], usize) {
    (
        &prepared.command.arguments,
        prepared.command.arguments.capacity(),
    )
}

pub fn link_prepared(
    clang: &Executable,
    linker: Option<(&Executable, &str)>,
    cwd: &Directory,
    prepared: PreparedLinkInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<Executable, Error> {
    let (linker, linker_path) = linker.ok_or(Error::Invalid)?;
    let vctools = prepared.command.arguments.get(3).ok_or(Error::Invalid)?;
    if prepared.command.arguments.get(2).map(String::as_str)
        != Some("-Xmicrosoft-visualc-tools-root")
        || prepared.command.arguments.get(4).map(String::as_str) != Some("-fuse-ld=link")
        || std::path::Path::new(linker_path).strip_prefix(vctools).ok()
            != Some(std::path::Path::new(r"bin\Hostx64\x64\link.exe"))
    {
        return Err(Error::Invalid);
    }
    if hold_regular_file_name_prepared(cwd, &prepared.output_name).is_ok() {
        return Err(Error::Exists);
    }
    recheck_held_regular(&linker.file)?;
    let process_output = run_argv(
        clang,
        cwd,
        &prepared.command.arguments,
        0,
        Some(prepared.command.command_line),
        Some(prepared.command.output),
        process_arena,
    );
    let linker_recheck = recheck_held_regular(&linker.file);
    let process_output = process_output?;
    linker_recheck?;
    if !process_output.is_empty() {
        return Err(Error::OutputLimit);
    }
    let file = hold_regular_file_name_external_read_prepared(cwd, &prepared.output_name)?;
    let mut prefix = [0_u8; 2];
    let mut duplicate = file.file.try_clone().map_err(|_| Error::Changed)?;
    duplicate
        .seek(SeekFrom::Start(0))
        .map_err(|_| Error::Changed)?;
    duplicate
        .read_exact(&mut prefix)
        .map_err(|_| Error::Invalid)?;
    if prefix != *b"MZ" {
        return Err(Error::Invalid);
    }
    Ok(Executable { file })
}

pub fn prepare_archive_invocation(
    input: &OsStr,
    output: &OsStr,
) -> Result<PreparedArchiveInvocation, Error> {
    normal_name(input)?;
    normal_name(output)?;
    if input != OsStr::new("module.obj") || output != OsStr::new("semaprax_native_rust_sdk.lib") {
        return Err(Error::Invalid);
    }
    let input_text = input.to_str().ok_or(Error::Invalid)?;
    let output_text = output.to_str().ok_or(Error::Invalid)?;
    let mut output_argument = String::with_capacity(5 + output_text.len());
    output_argument.push_str("/OUT:");
    output_argument.push_str(output_text);
    if output_argument.capacity() != 5 + output_text.len() {
        return Err(Error::OutputLimit);
    }
    Ok(PreparedArchiveInvocation {
        command: prepare_command(
            &["/NOLOGO", "/BREPRO", output_argument.as_str(), input_text],
            0,
        )?,
        input_name: prepare_relative_name(input)?,
        output_name: prepare_relative_name(output)?,
    })
}

pub fn prepared_archive_owned_capacity(prepared: &PreparedArchiveInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.command)
        .saturating_add(
            prepared
                .input_name
                .0
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
        .saturating_add(
            prepared
                .output_name
                .0
                .capacity()
                .saturating_mul(std::mem::size_of::<u16>()),
        )
}

#[cfg(test)]
pub(super) fn test_prepared_archive_arguments(prepared: &PreparedArchiveInvocation) -> &[String] {
    &prepared.command.arguments
}

fn recheck_named_regular(
    cwd: &Directory,
    name: &PreparedRelativeName,
    input: &RegularFile,
) -> Result<(), Error> {
    recheck_held_regular(input)?;
    let rebound = hold_regular_file_name_external_read_prepared(cwd, name)?;
    if rebound.identity != input.identity || rebound.digest != input.digest {
        return Err(Error::Changed);
    }
    Ok(())
}

fn child_absent_impl(directory: &Directory, name: &PreparedRelativeName) -> Result<bool, Error> {
    let byte_length = name.0.len().checked_mul(2).ok_or(Error::Invalid)?;
    let length = u16::try_from(byte_length).map_err(|_| Error::Invalid)?;
    let unicode = UNICODE_STRING {
        Length: length,
        MaximumLength: length,
        Buffer: name.0.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: u32::try_from(std::mem::size_of::<OBJECT_ATTRIBUTES>())
            .map_err(|_| Error::Changed)?,
        RootDirectory: directory.file.as_raw_handle().cast(),
        ObjectName: &unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut io = IO_STATUS_BLOCK::default();
    let mut handle = std::ptr::null_mut();
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_READ_ATTRIBUTES | SYNCHRONIZE,
            &attributes,
            &mut io,
            std::ptr::null(),
            FILE_ATTRIBUTE_NORMAL,
            HELD_SHARE,
            FILE_OPEN,
            FILE_OPEN_REPARSE_POINT | FILE_SYNCHRONOUS_IO_NONALERT,
            std::ptr::null(),
            0,
        )
    };
    if status == STATUS_OBJECT_NAME_NOT_FOUND {
        return Ok(true);
    }
    if status < 0 || handle.is_null() || handle == INVALID_HANDLE_VALUE {
        return Err(Error::Changed);
    }
    if unsafe { CloseHandle(handle) } == 0 {
        return Err(Error::Changed);
    }
    Ok(false)
}

pub fn child_absent_prepared(
    directory: &Directory,
    name: &PreparedRelativeName,
) -> Result<bool, Error> {
    child_absent_impl(directory, name)
}

fn read_exact_offset(file: &File, mut bytes: &mut [u8], mut offset: u64) -> Result<(), Error> {
    while !bytes.is_empty() {
        let read = file.seek_read(bytes, offset).map_err(|_| Error::Changed)?;
        if read == 0 {
            return Err(Error::Invalid);
        }
        let (_, remaining) = bytes.split_at_mut(read);
        bytes = remaining;
        offset = offset
            .checked_add(u64::try_from(read).map_err(|_| Error::OutputLimit)?)
            .ok_or(Error::OutputLimit)?;
    }
    Ok(())
}

fn exact_archive_member(archive: &RegularFile, input: &RegularFile) -> Result<(), Error> {
    let archive_len = archive.identity.length;
    let input_len = input.identity.length;
    if archive_len < 68 || input_len == 0 {
        return Err(Error::Invalid);
    }
    let mut magic = [0_u8; 8];
    read_exact_offset(&archive.file, &mut magic, 0)?;
    if magic != *b"!<arch>\n" {
        return Err(Error::Invalid);
    }
    let mut offset = 8_u64;
    let mut input_members = 0_u8;
    let mut members = 0_u8;
    let mut empty_longnames = false;
    while offset < archive_len {
        let mut header = [0_u8; 60];
        read_exact_offset(&archive.file, &mut header, offset)?;
        if header[58..] != *b"`\n" {
            return Err(Error::Invalid);
        }
        let size = archive_member_size(&header[48..58])?;
        let data = offset.checked_add(60).ok_or(Error::OutputLimit)?;
        let end = data.checked_add(size).ok_or(Error::OutputLimit)?;
        if end > archive_len {
            return Err(Error::Invalid);
        }
        let header_kind = archive_member_kind(&header[..16], b"module.obj")?;
        exact_archive_member_metadata(&header, header_kind, 0)?;
        let (kind, member_data, member_size) = match header_kind {
            ArchiveMemberKind::Extended(length) => {
                let length = u64::try_from(length).map_err(|_| Error::OutputLimit)?;
                if length > size {
                    return Err(Error::Invalid);
                }
                let mut name = [0_u8; 255];
                let name_length = usize::try_from(length).map_err(|_| Error::OutputLimit)?;
                read_exact_offset(&archive.file, &mut name[..name_length], data)?;
                let name = archive_extended_name(&name[..name_length])?;
                let kind = archive_member_kind(name, b"module.obj")?;
                if matches!(kind, ArchiveMemberKind::Extended(_)) {
                    return Err(Error::Invalid);
                }
                (
                    kind,
                    data.checked_add(length).ok_or(Error::OutputLimit)?,
                    size - length,
                )
            }
            kind => (kind, data, size),
        };
        let ordered = matches!(
            (members, header_kind, kind),
            (
                0 | 1,
                ArchiveMemberKind::GnuLinkerIndex,
                ArchiveMemberKind::GnuLinkerIndex
            ) | (
                2,
                ArchiveMemberKind::LongNames,
                ArchiveMemberKind::LongNames
            ) | (2, ArchiveMemberKind::Input, ArchiveMemberKind::Input)
                | (3, ArchiveMemberKind::Input, ArchiveMemberKind::Input)
        );
        if !ordered
            || matches!(kind, ArchiveMemberKind::LongNames) && (size != 0 || members != 2)
            || members == 3 && !empty_longnames
        {
            return Err(Error::Invalid);
        }
        if matches!(kind, ArchiveMemberKind::LongNames) {
            empty_longnames = true;
        }
        match kind {
            ArchiveMemberKind::GnuLinkerIndex
            | ArchiveMemberKind::BsdSortedLinkerIndex
            | ArchiveMemberKind::LongNames => {}
            ArchiveMemberKind::Input => {
                input_members = input_members.checked_add(1).ok_or(Error::Invalid)?;
                if member_size != input_len {
                    return Err(Error::Invalid);
                }
                let mut compared = 0_u64;
                let mut archive_bytes = [0_u8; 8192];
                let mut input_bytes = [0_u8; 8192];
                while compared < member_size {
                    let count = usize::try_from((member_size - compared).min(8192))
                        .map_err(|_| Error::OutputLimit)?;
                    read_exact_offset(
                        &archive.file,
                        &mut archive_bytes[..count],
                        member_data + compared,
                    )?;
                    read_exact_offset(&input.file, &mut input_bytes[..count], compared)?;
                    if archive_bytes[..count] != input_bytes[..count] {
                        return Err(Error::Invalid);
                    }
                    compared = compared
                        .checked_add(u64::try_from(count).map_err(|_| Error::OutputLimit)?)
                        .ok_or(Error::OutputLimit)?;
                }
            }
            ArchiveMemberKind::Extended(_) => return Err(Error::Invalid),
        }
        if size & 1 != 0 {
            let mut padding = [0_u8; 1];
            read_exact_offset(&archive.file, &mut padding, end)?;
            if padding != *b"\n" {
                return Err(Error::Invalid);
            }
        }
        offset = end.checked_add(size & 1).ok_or(Error::OutputLimit)?;
        members = members.checked_add(1).ok_or(Error::Invalid)?;
    }
    let expected_members = if empty_longnames { 4 } else { 3 };
    if offset != archive_len || input_members != 1 || members != expected_members {
        return Err(Error::Invalid);
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn test_exact_archive_member(
    archive: &RegularFile,
    input: &RegularFile,
) -> Result<(), Error> {
    exact_archive_member(archive, input)
}

pub fn archive_prepared(
    archiver: &Executable,
    cwd: &Directory,
    input: &RegularFile,
    prepared: PreparedArchiveInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<RegularFile, Error> {
    if !child_absent_impl(cwd, &prepared.output_name)? {
        return Err(Error::Exists);
    }
    recheck_named_regular(cwd, &prepared.input_name, input)?;
    recheck_held_regular(&archiver.file)?;
    recheck_directory(cwd)?;
    let process = run_argv(
        archiver,
        cwd,
        &prepared.command.arguments,
        0,
        Some(prepared.command.command_line),
        Some(prepared.command.output),
        process_arena,
    );
    let archiver_recheck = recheck_held_regular(&archiver.file);
    let cwd_recheck = recheck_directory(cwd);
    let input_recheck = recheck_named_regular(cwd, &prepared.input_name, input);
    let output = process?;
    archiver_recheck?;
    cwd_recheck?;
    input_recheck?;
    if !output.is_empty() {
        return Err(Error::OutputLimit);
    }
    let archive = hold_regular_file_name_external_read_bounded_prepared(
        cwd,
        &prepared.output_name,
        SDK_ARCHIVE_MAX_BYTES,
    )?;
    exact_archive_member(&archive, input)?;
    recheck_held_regular(&archiver.file)?;
    recheck_directory(cwd)?;
    recheck_named_regular(cwd, &prepared.input_name, input)?;
    recheck_named_regular(cwd, &prepared.output_name, &archive)?;
    Ok(archive)
}

pub fn prepare_run_invocation() -> Result<PreparedRunInvocation, Error> {
    Ok(PreparedRunInvocation(prepare_command(&[], 0)?))
}

pub fn prepared_run_owned_capacity(prepared: &PreparedRunInvocation) -> usize {
    prepared_command_owned_capacity(&prepared.0)
}

pub fn run_prepared(
    executable: &Executable,
    cwd: &Directory,
    prepared: PreparedRunInvocation,
    process_arena: &mut PreparedProcessArena,
) -> Result<(), Error> {
    if run_argv(
        executable,
        cwd,
        &prepared.0.arguments,
        0,
        Some(prepared.0.command_line),
        Some(prepared.0.output),
        process_arena,
    )?
    .is_empty()
    {
        Ok(())
    } else {
        Err(Error::OutputLimit)
    }
}

pub fn compile_c_to_stdout(
    executable: &Executable,
    cwd: &Directory,
    target: &str,
    input: &OsStr,
    optimization: u8,
    sanitizers: bool,
    maximum: usize,
) -> Result<Vec<u8>, Error> {
    normal_name(input)?;
    if sanitizers || !matches!(optimization, 0 | 2) {
        return Err(Error::Invalid);
    }
    let arguments = vec![
        "-std=c11".to_owned(),
        "-target".to_owned(),
        target.to_owned(),
        "-Wall".to_owned(),
        "-Wextra".to_owned(),
        "-Werror".to_owned(),
        if optimization == 0 {
            "-O0".to_owned()
        } else {
            "-O2".to_owned()
        },
        "-c".to_owned(),
        input.to_string_lossy().into_owned(),
        "-o".to_owned(),
        "-".to_owned(),
    ];
    let command_line = windows_command_line(&arguments)?;
    let output = Vec::with_capacity(maximum.min(33_554_432));
    let mut process_arena = prepare_process_arena(1)?;
    run_argv(
        executable,
        cwd,
        &arguments,
        maximum.min(33_554_432),
        Some(command_line),
        Some(output),
        &mut process_arena,
    )
}

pub fn execute_harness(executable: &Executable, cwd: &Directory) -> Result<(), Error> {
    let command_line = windows_command_line(&[])?;
    let mut process_arena = prepare_process_arena(1)?;
    if run_argv(
        executable,
        cwd,
        &[],
        0,
        Some(command_line),
        Some(Vec::new()),
        &mut process_arena,
    )?
    .is_empty()
    {
        Ok(())
    } else {
        Err(Error::OutputLimit)
    }
}

#[cfg(test)]
pub(super) fn execute_harness_with_arguments(
    executable: &Executable,
    cwd: &Directory,
    arguments: &[String; 3],
) -> Result<(), Error> {
    let command_line = windows_command_line(arguments)?;
    let mut process_arena = prepare_process_arena(1)?;
    if run_argv(
        executable,
        cwd,
        arguments,
        0,
        Some(command_line),
        Some(Vec::new()),
        &mut process_arena,
    )?
    .is_empty()
    {
        Ok(())
    } else {
        Err(Error::OutputLimit)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn link_harness(
    clang: &Executable,
    linker: Option<(&Executable, &str)>,
    vctools: Option<&OsStr>,
    cwd: &Directory,
    target: &str,
    harness: &OsStr,
    c_object: &OsStr,
    rust_archive: &OsStr,
    output: &OsStr,
    sanitizers: bool,
) -> Result<Executable, Error> {
    for name in [harness, c_object, rust_archive, output] {
        normal_name(name)?;
    }
    let (linker, linker_path) = linker.ok_or(Error::Invalid)?;
    let vctools = vctools
        .filter(|path| std::path::Path::new(path).is_absolute())
        .and_then(OsStr::to_str)
        .ok_or(Error::Invalid)?;
    if !std::path::Path::new(linker_path).is_absolute()
        || std::path::Path::new(linker_path).strip_prefix(vctools).ok()
            != Some(std::path::Path::new(r"bin\Hostx64\x64\link.exe"))
        || sanitizers
    {
        return Err(Error::Invalid);
    }
    let mut arguments = vec!["-target".to_owned(), target.to_owned()];
    arguments.extend([
        "-Xmicrosoft-visualc-tools-root".to_owned(),
        vctools.to_owned(),
        "-fuse-ld=link".to_owned(),
    ]);
    arguments.extend(WINDOWS_DYNAMIC_CRT_LINK_ARGS.into_iter().map(str::to_owned));
    arguments.extend([
        harness.to_string_lossy().into_owned(),
        c_object.to_string_lossy().into_owned(),
        rust_archive.to_string_lossy().into_owned(),
    ]);
    arguments.extend(
        WINDOWS_RUST_STATICLIB_NATIVE_LIBS
            .into_iter()
            .map(str::to_owned),
    );
    arguments.extend(["-o".to_owned(), output.to_string_lossy().into_owned()]);
    let command_line = windows_command_line(&arguments)?;
    let mut process_arena = prepare_process_arena(1)?;
    recheck_held_regular(&linker.file)?;
    let process_output = run_argv(
        clang,
        cwd,
        &arguments,
        0,
        Some(command_line),
        Some(Vec::new()),
        &mut process_arena,
    );
    let linker_recheck = recheck_held_regular(&linker.file);
    let process_output = process_output?;
    linker_recheck?;
    if !process_output.is_empty() {
        return Err(Error::OutputLimit);
    }
    hold_executable(cwd, output)
}
