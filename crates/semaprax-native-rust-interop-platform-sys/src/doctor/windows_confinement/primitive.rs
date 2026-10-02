//! `#[cfg(windows)]` Windows confinement primitive for
//! [`DOCTOR-PRODUCTION-PROVISIONER-WINDOWS-V1`][doc].
//!
//! # Authoring-host and hosted evidence boundaries
//!
//! The historical ten-case native Windows selector passed on `f4d3291f`;
//! the owning specification retains the earlier exact compilation/runtime
//! receipts. The signed-image binding continuation and its seven additional
//! cases have no native Windows execution receipt on this macOS authoring
//! host. A cross-target type-check is not runtime acceptance. Production
//! release trust, ordinary Windows CLI transport and broader confinement
//! remain separate requirements. Retained writable-section mutation can still
//! race advisory oplock checks; exact image-binding acceptance remains open.
//!
//! # Scope
//!
//! This is a standalone confinement primitive, analogous to
//! `doctor::darwin_confinement` and tested only in isolation: it is not the
//! ordinary `--version` probe in `doctor::windows`, and it is not wired into
//! any ordinary CLI route or into `provisioned_doctor_*`. It reuses the
//! existing `doctor::windows` job-object plumbing's *shape* (suspended
//! leader, non-breakaway job assigned before resume, settlement observed via
//! `JobObjectBasicAccountingInformation`) without importing its private
//! types, exactly as `darwin_confinement` does not import
//! `doctor::unix::launch::darwin`'s private types.
//!
//! Three deliberate simplifications versus a full production primitive,
//! recorded here rather than left implicit:
//!
//! 1. **Output capture is bounded anonymous-pipe accounting.** The confined
//!    leader inherits only the write ends of two anonymous pipes through the
//!    existing explicit handle list. Settlement drains a fixed amount from the
//!    parent-only readers and terminates the owned job when the combined
//!    stdout/stderr ceiling is exceeded. This prevents scratch-file growth and
//!    keeps a flooding child from retaining unbounded kernel or filesystem
//!    storage. It remains a primitive-local capture contract, not ordinary
//!    Windows CLI transport.
//! 2. **The restricted token disables maximum privilege only**
//!    (`CreateRestrictedToken` with `DISABLE_MAX_PRIVILEGE` and empty
//!    disable/delete/restrict lists), not the fuller "disable the caller's
//!    own logon SID" refinement `DOCTOR-PRODUCTION-PROVISIONER-WINDOWS-V1.md`
//!    proposes. That refinement requires walking the calling token's
//!    `TokenGroups` to find the `SE_GROUP_LOGON_ID` entry, a variable-length,
//!    harder-to-verify-blind structure; this file keeps the FFI surface
//!    reviewable and defers that specific tightening.
//! 3. **Filesystem confinement is a restricted-ACL scratch root**, the
//!    design the owning spec identifies as needing no `Cargo.toml` feature
//!    change, not an AppContainer profile (which needs
//!    `Win32_Security_Isolation`, not enabled today and outside this
//!    session's file lease).
//!
//! [doc]: https://github.com/wavect/semaprax/blob/main/docs/DOCTOR-PRODUCTION-PROVISIONER-WINDOWS-V1.md
use super::carrier::AuthenticatedRequestBundle;
use super::refusal::{admit, Refusal};
use super::settlement::{FailureReason, Settlement, StickySettlement, UncertainReason};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{
    CloseHandle, GetLastError, SetHandleInformation, ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED,
    HANDLE, HANDLE_FLAG_INHERIT, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Security::{
    AddAccessAllowedAceEx, CreateRestrictedToken, GetTokenInformation, InitializeAcl,
    InitializeSecurityDescriptor, SetSecurityDescriptorControl, SetSecurityDescriptorDacl,
    TokenUser, ACL, ACL_REVISION, DISABLE_MAX_PRIVILEGE, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
    SE_DACL_PROTECTED, TOKEN_ASSIGN_PRIMARY, TOKEN_DUPLICATE, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::Storage::FileSystem::{
    CreateDirectoryW, CreateFileW, ReadFile, DELETE, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
    JobObjectBasicUIRestrictions, JobObjectExtendedLimitInformation, QueryInformationJobObject,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_BASIC_UI_RESTRICTIONS, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_ACTIVE_PROCESS, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PROCESS_MEMORY,
    JOB_OBJECT_LIMIT_PROCESS_TIME, JOB_OBJECT_UILIMIT_DESKTOP, JOB_OBJECT_UILIMIT_DISPLAYSETTINGS,
    JOB_OBJECT_UILIMIT_EXITWINDOWS, JOB_OBJECT_UILIMIT_GLOBALATOMS, JOB_OBJECT_UILIMIT_HANDLES,
    JOB_OBJECT_UILIMIT_READCLIPBOARD, JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS,
    JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
};
use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows_sys::Win32::System::Threading::{
    CreateProcessAsUserW, DeleteProcThreadAttributeList, GetCurrentProcess, GetExitCodeProcess,
    InitializeProcThreadAttributeList, OpenProcessToken, ResumeThread, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

const MAX_WIDE: usize = 32767;
/// Each confined leader receives at most two seconds of user-mode CPU time.
/// `JOBOBJECT_BASIC_LIMIT_INFORMATION` records this value in 100 ns units.
const CPU_TIME_LIMIT_100NS: i64 = 2 * 10_000_000;
/// Bound committed address-space usage for one restricted leader. The hostile
/// fixture grows in 8 MiB chunks and must receive an allocation refusal before
/// it can retain more than this cap.
const PROCESS_MEMORY_LIMIT_BYTES: usize = 256 * 1024 * 1024;
/// The total stdout and stderr bytes settlement may drain from a confined job.
/// Anonymous pipes bound bytes pending in the kernel; this independent counter
/// bounds cumulative output from a child that the parent continues to drain.
const OUTPUT_LIMIT_BYTES: usize = 64 * 1024;
const OUTPUT_PIPE_BYTES: u32 = 4096;
const REQUEST_CARRIER_HANDLE_ENV: &str = "SEMAPRAX_DOCTOR_REQUEST_CARRIER_HANDLE";
const BUNDLE_CARRIER_HANDLE_ENV: &str = "SEMAPRAX_DOCTOR_BUNDLE_CARRIER_HANDLE";
const CARRIER_ROLE_ENV: &str = "SEMAPRAX_DOCTOR_CARRIER_ROLE";
const CARRIER_SELECTOR_ENV: &str = "SEMAPRAX_DOCTOR_CARRIER_SELECTOR";

#[cfg(test)]
const TEST_REQUEST_BYTES: &[u8] = b"SPXDWK1\0windows-request-carrier";
#[cfg(test)]
const TEST_BUNDLE_BYTES: &[u8] = b"windows-bundle-carrier";

mod image;

/// Test-visible checkpoints from image admission through the suspended-child
/// verification. They do not grant authority or alter a failed binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BindingBoundary {
    Image(image::ImageBindingBoundary),
    BeforeProcessCreation,
    SuspendedLeader,
}
pub use image::ImageRole;
/// Denies every UI-affecting capability a confined batch tool has no
/// legitimate use for, per this contract's job-limit tightening.
const DENIED_UI_LIMITS: u32 = JOB_OBJECT_UILIMIT_HANDLES
    | JOB_OBJECT_UILIMIT_READCLIPBOARD
    | JOB_OBJECT_UILIMIT_WRITECLIPBOARD
    | JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS
    | JOB_OBJECT_UILIMIT_DESKTOP
    | JOB_OBJECT_UILIMIT_DISPLAYSETTINGS
    | JOB_OBJECT_UILIMIT_GLOBALATOMS
    | JOB_OBJECT_UILIMIT_EXITWINDOWS;

struct Handle(Option<HANDLE>);

impl Handle {
    fn new(raw: HANDLE) -> Self {
        Self(Some(raw))
    }

    fn raw(&self) -> HANDLE {
        self.0.expect("primitive handle is owned")
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        if let Some(raw) = self.0.take() {
            // SAFETY: sole remaining owner of this handle.
            unsafe { CloseHandle(raw) };
        }
    }
}

/// Owns the Win32 attribute-list initialization until process creation has
/// copied the explicit inherited-handle inventory.
struct AttributeList(LPPROC_THREAD_ATTRIBUTE_LIST);

impl Drop for AttributeList {
    fn drop(&mut self) {
        // SAFETY: the list was initialized once over backing storage that
        // outlives this guard.
        unsafe { DeleteProcThreadAttributeList(self.0) };
    }
}

fn wide(value: &OsStr) -> Result<Vec<u16>, ()> {
    let length = value.encode_wide().count();
    if length == 0 || length >= MAX_WIDE || value.encode_wide().any(|unit| unit == 0) {
        return Err(());
    }
    Ok(value.encode_wide().chain(Some(0)).collect())
}

/// A per-invocation restricted-ACL scratch root. It owns only the directory it
/// created, never an arbitrary recursive walk.
struct ScratchRoot {
    dir: PathBuf,
}

impl Drop for ScratchRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir(&self.dir);
    }
}

/// Build a restricted token with `DISABLE_MAX_PRIVILEGE` from the calling
/// process's own token. See the module documentation for why this does not
/// also disable the caller's logon SID.
fn restricted_token() -> Result<Handle, ()> {
    let mut process_token = std::ptr::null_mut();
    // SAFETY: `GetCurrentProcess` returns a pseudo-handle that need not be
    // closed; `process_token` is a live, exclusively-owned output pointer.
    if unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_DUPLICATE | TOKEN_QUERY | TOKEN_ASSIGN_PRIMARY,
            &mut process_token,
        )
    } == 0
    {
        return Err(());
    }
    let process_token = Handle::new(process_token);
    let mut restricted = std::ptr::null_mut();
    // SAFETY: `process_token` is live and has `TOKEN_DUPLICATE`; every
    // disable/delete/restrict list is empty (count 0, pointer null), which
    // `CreateRestrictedToken` documents as valid; `restricted` is a live,
    // exclusively-owned output pointer.
    if unsafe {
        CreateRestrictedToken(
            process_token.raw(),
            DISABLE_MAX_PRIVILEGE,
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            0,
            std::ptr::null(),
            &mut restricted,
        )
    } == 0
    {
        return Err(());
    }
    Ok(Handle::new(restricted))
}

/// Read the restricted token's own user SID via `GetTokenInformation` into
/// `buffer` in place. `GetTokenInformation(TokenUser)` returns a `TOKEN_USER`
/// whose `Sid` field is a pointer *into this same buffer* (the SID bytes are
/// appended after the struct), not a separately allocated region -- so the
/// buffer must never move between this call and any later dereference of
/// that pointer. Taking `buffer` by mutable reference rather than returning
/// an owned array is deliberate: an owned return would let the caller move
/// it, which would relocate the bytes without updating the pointer embedded
/// inside them.
fn read_token_user_sid(token: &Handle, buffer: &mut [u8; 256]) -> Result<(), ()> {
    let mut returned = 0u32;
    // SAFETY: `token` is live; `buffer` is a live, exclusively-owned output
    // region of the declared length; `returned` is a live output pointer.
    let ok = unsafe {
        GetTokenInformation(
            token.raw(),
            TokenUser,
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut returned,
        )
    } != 0;
    if !ok
        || (returned as usize) > buffer.len()
        || (returned as usize) < std::mem::size_of::<TOKEN_USER>()
    {
        return Err(());
    }
    Ok(())
}

/// Create a fresh scratch subdirectory of `scratch_root` whose DACL grants
/// only `sid_buffer`'s `TOKEN_USER.User.Sid` read/write/delete access. No
/// other principal is listed, which is an implicit deny under Windows DACL
/// evaluation. The protected DACL prevents inheritable ACEs from the supplied
/// parent's own parent from adding principals to the created directory.
fn confined_scratch_root(scratch_root: &Path, sid_buffer: &[u8; 256]) -> Result<ScratchRoot, ()> {
    // SAFETY: `sid_buffer` was populated by `GetTokenInformation(TokenUser)`
    // and is large enough for a `TOKEN_USER`; the resulting `Sid` pointer
    // stays valid for `sid_buffer`'s lifetime, which outlives this call.
    let sid = unsafe { (*sid_buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };

    let mut acl_buffer = [0u8; 512];
    let acl_ptr = acl_buffer.as_mut_ptr().cast::<ACL>();
    // SAFETY: `acl_ptr` points at a live, exclusively-owned 512-byte buffer;
    // `ACL_REVISION` is the fixed revision this crate targets.
    if unsafe { InitializeAcl(acl_ptr, 512, ACL_REVISION) } == 0 {
        return Err(());
    }
    // SAFETY: `acl_ptr` was just initialized above and has spare capacity
    // for one ACE with a real-world SID; `sid` is a live pointer into
    // `sid_buffer`, which outlives this call.
    if unsafe {
        AddAccessAllowedAceEx(
            acl_ptr,
            ACL_REVISION,
            0,
            FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE,
            sid,
        )
    } == 0
    {
        return Err(());
    }

    let mut descriptor: SECURITY_DESCRIPTOR = unsafe { std::mem::zeroed() };
    let descriptor_ptr = (&mut descriptor as *mut SECURITY_DESCRIPTOR).cast();
    // `SECURITY_DESCRIPTOR_REVISION` (documented Win32 value `1`) lives in
    // `Win32_System_SystemServices`, a feature this crate does not enable
    // (`Cargo.toml`, outside this session's lease); the literal is Microsoft's
    // own fixed, never-changed ABI constant, not a value this file invents.
    const SECURITY_DESCRIPTOR_REVISION: u32 = 1;
    // SAFETY: `descriptor_ptr` points at a live, exclusively-owned
    // `SECURITY_DESCRIPTOR`.
    if unsafe { InitializeSecurityDescriptor(descriptor_ptr, SECURITY_DESCRIPTOR_REVISION) } == 0 {
        return Err(());
    }
    // SAFETY: `descriptor_ptr` was just initialized; `acl_ptr` outlives this
    // call and this descriptor's use in `CreateDirectoryW` below.
    if unsafe { SetSecurityDescriptorDacl(descriptor_ptr, 1, acl_ptr, 0) } == 0 {
        return Err(());
    }
    // SAFETY: `descriptor_ptr` is a live initialized descriptor. Prevent
    // inheritable ACEs on the supplied parent from broadening this explicit
    // user-only DACL.
    if unsafe { SetSecurityDescriptorControl(descriptor_ptr, SE_DACL_PROTECTED, SE_DACL_PROTECTED) }
        == 0
    {
        return Err(());
    }

    let dir = fresh_child_dir(scratch_root)?;
    let dir_wide = wide(dir.as_os_str())?;
    let security = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: descriptor_ptr,
        bInheritHandle: 0,
    };
    // SAFETY: `dir_wide` is a live NUL-terminated wide string; `security`'s
    // descriptor outlives this call.
    if unsafe { CreateDirectoryW(dir_wide.as_ptr(), &security) } == 0 {
        return Err(());
    }
    Ok(ScratchRoot { dir })
}

fn fresh_child_dir(scratch_root: &Path) -> Result<PathBuf, ()> {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| ())?
        .as_nanos();
    Ok(scratch_root.join(format!(
        "semaprax-doctor-confinement-{}-{}-{}",
        std::process::id(),
        nanos,
        COUNTER.fetch_add(1, Ordering::Relaxed)
    )))
}

/// Create one bounded anonymous output pipe. The child receives only its
/// inheritable writer; the parent reader is explicitly stripped of inheritance
/// before it is placed outside the startup handle list.
fn create_inheritable_output_pipe() -> Result<(Handle, Handle), ()> {
    let security = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let mut reader = std::ptr::null_mut();
    let mut writer = std::ptr::null_mut();
    // SAFETY: both result pointers and the inherited security attributes are
    // live. `OUTPUT_PIPE_BYTES` bounds pending output even before settlement
    // begins draining the parent-only reader.
    if unsafe { CreatePipe(&mut reader, &mut writer, &security, OUTPUT_PIPE_BYTES) } == 0 {
        return Err(());
    }
    let reader = Handle::new(reader);
    let writer = Handle::new(writer);
    // SAFETY: the reader is owned by this parent and must never enter the
    // child inventory. The writer remains inheritable for the fixed list.
    if unsafe { SetHandleInformation(reader.raw(), HANDLE_FLAG_INHERIT, 0) } == 0 {
        return Err(());
    }
    Ok((reader, writer))
}

fn open_inheritable_null() -> Result<Handle, ()> {
    let security = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: 1,
    };
    let null = [u16::from(b'N'), u16::from(b'U'), u16::from(b'L'), 0];
    // SAFETY: `null` is a live NUL-terminated wide string naming the fixed
    // device path `NUL`; `security` is a live, stack-owned value.
    let raw = unsafe {
        CreateFileW(
            null.as_ptr(),
            FILE_GENERIC_READ,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            &security,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            std::ptr::null_mut(),
        )
    };
    if raw == INVALID_HANDLE_VALUE {
        return Err(());
    }
    Ok(Handle::new(raw))
}

/// Create a job object and tighten its limits per this contract's
/// "job-object limits, tightened" section: `KILL_ON_JOB_CLOSE` (already the
/// ordinary probe's behavior), `ACTIVE_PROCESS` capped at one, a bounded
/// user-mode CPU time and committed memory, and `DIE_ON_UNHANDLED_EXCEPTION`,
/// plus a `JOBOBJECT_BASIC_UI_RESTRICTIONS` call denying every listed UI
/// capability.
fn tightened_job() -> Result<Handle, ()> {
    // SAFETY: both name arguments are null, requesting an unnamed job.
    let raw_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw_job.is_null() {
        return Err(());
    }
    let job = Handle::new(raw_job);
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_PROCESS_TIME
        | JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    limits.BasicLimitInformation.ActiveProcessLimit = 1;
    limits.BasicLimitInformation.PerProcessUserTimeLimit = CPU_TIME_LIMIT_100NS;
    limits.ProcessMemoryLimit = PROCESS_MEMORY_LIMIT_BYTES;
    // SAFETY: `job` is live; `limits` is a live, exclusively-owned local of
    // the exact size passed.
    if unsafe {
        SetInformationJobObject(
            job.raw(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
    } == 0
    {
        return Err(());
    }
    let restrictions = JOBOBJECT_BASIC_UI_RESTRICTIONS {
        UIRestrictionsClass: DENIED_UI_LIMITS,
    };
    // SAFETY: `job` is live; `restrictions` is a live, exclusively-owned
    // local of the exact size passed.
    if unsafe {
        SetInformationJobObject(
            job.raw(),
            JobObjectBasicUIRestrictions,
            (&restrictions as *const JOBOBJECT_BASIC_UI_RESTRICTIONS).cast(),
            std::mem::size_of_val(&restrictions) as u32,
        )
    } == 0
    {
        return Err(());
    }
    Ok(job)
}

/// A minimal, forced-only environment block: `TEMP` and `TMP` are pinned to
/// the confined scratch directory, and nothing else is inherited. Windows
/// requires case-insensitive sorted names and a trailing empty row; `TEMP` <
/// `TMP` case-insensitively, so this fixed pair is already sorted.
fn forced_environment(scratch_dir: &Path) -> Result<Vec<u16>, ()> {
    let value = wide(scratch_dir.as_os_str())?;
    let mut output = Vec::new();
    for name in ["TEMP", "TMP"] {
        output.extend(name.encode_utf16());
        output.push(u16::from(b'='));
        output.extend_from_slice(&value[..value.len() - 1]);
        output.push(0);
    }
    output.push(0);
    Ok(output)
}

/// Build the closed child environment for an authenticated request/bundle
/// handoff. The handle values are process-local labels for the exact mapping
/// handles in the startup list; the signed selector and image role make their
/// request-then-bundle ordering explicit to the child protocol.
fn carrier_environment(
    scratch_dir: &Path,
    role: ImageRole,
    selector: &str,
    carriers: &AuthenticatedRequestBundle,
) -> Result<Vec<u16>, ()> {
    if selector.is_empty() || selector.len() > 64 || selector.as_bytes().contains(&0) {
        return Err(());
    }
    let handles = carriers.child_handles();
    // Windows requires case-insensitive ordering. The `SEMAPRAX_*` names sort
    // before `TEMP` and `TMP`, and the four handoff names are already ordered.
    let values = [
        format!("{BUNDLE_CARRIER_HANDLE_ENV}={}", handles[1] as usize),
        format!("{CARRIER_ROLE_ENV}={}", role.wire()),
        format!("{CARRIER_SELECTOR_ENV}={selector}"),
        format!("{REQUEST_CARRIER_HANDLE_ENV}={}", handles[0] as usize),
    ];
    let scratch = wide(scratch_dir.as_os_str())?;
    let mut environment = Vec::new();
    for value in values {
        if value.len() >= MAX_WIDE || value.as_bytes().contains(&0) {
            return Err(());
        }
        environment.extend(value.encode_utf16());
        environment.push(0);
    }
    for name in ["TEMP", "TMP"] {
        environment.extend(name.encode_utf16());
        environment.push(u16::from(b'='));
        environment.extend_from_slice(&scratch[..scratch.len() - 1]);
        environment.push(0);
    }
    environment.push(0);
    (environment.len() < MAX_WIDE)
        .then_some(environment)
        .ok_or(())
}

pub struct ConfinedProcess {
    process: Handle,
    thread: Handle,
    job: Handle,
    _token: Handle,
    // Rust drops fields in declaration order. Close the parent-held stdio
    // handles before ScratchRoot removes its directory on Windows.
    _stdin: Handle,
    stdout: Handle,
    stderr: Handle,
    // Retain both parent handles until the child has settled. The child sees
    // only these two mappings, bound in the explicit startup handle list.
    _carriers: AuthenticatedRequestBundle,
    _scratch: ScratchRoot,
    // Retain the authenticated file and its namespace guards until settlement.
    _image: image::HeldImage,
    settled: bool,
}

impl Drop for ConfinedProcess {
    fn drop(&mut self) {
        if !self.settled {
            // A caller that drops a `ConfinedProcess` without calling
            // `settle` gets a short, fixed grace period rather than the
            // caller's own deadline (which is not available here); this
            // mirrors `doctor::windows::Child::drop` treating an unsettled
            // drop as an urgent cleanup, not an ordinary wait.
            let _ = settle_confined(self, Instant::now() + Duration::from_secs(5));
            self.settled = true;
        }
    }
}

/// Verify the release-signed capsule using the compile-time release trust
/// anchor, check `exe` against the selected signed image role with held NTFS
/// file/namespace guards, then spawn suspended under a restricted token, inside a
/// fresh ACL-confined scratch root, assigned to a tightened job object,
/// before any target code runs. Missing or malformed trust input refuses; it
/// never falls back to structural-only parsing.
pub fn confined_spawn(
    exe: &Path,
    role: ImageRole,
    args: &[&OsStr],
    scratch_root: &Path,
    capsule_bytes: &[u8],
    request: &[u8],
    bundle: &[u8],
) -> Result<ConfinedProcess, Refusal> {
    confined_spawn_using(exe, role, args, scratch_root, request, bundle, || {
        super::capsule::parse_with_release_anchor(capsule_bytes)
    })
}

/// Test-only seam for exercising the same signed parser and Win32 launch
/// stages with a deterministic test key. It is not reachable in production
/// builds and must not be treated as release-trust evidence.
#[cfg(test)]
fn confined_spawn_with_test_key(
    exe: &Path,
    args: &[&OsStr],
    scratch_root: &Path,
    capsule_bytes: &[u8],
    public_key_hex: &str,
) -> Result<ConfinedProcess, Refusal> {
    confined_spawn_using(
        exe,
        ImageRole::Worker,
        args,
        scratch_root,
        TEST_REQUEST_BYTES,
        TEST_BUNDLE_BYTES,
        || super::capsule::parse_windows_signed_with_key(capsule_bytes, public_key_hex),
    )
}

#[cfg(test)]
fn confined_spawn_with_test_key_carriers(
    exe: &Path,
    args: &[&OsStr],
    scratch_root: &Path,
    capsule_bytes: &[u8],
    public_key_hex: &str,
    request: &[u8],
    bundle: &[u8],
) -> Result<ConfinedProcess, Refusal> {
    confined_spawn_using(
        exe,
        ImageRole::Worker,
        args,
        scratch_root,
        request,
        bundle,
        || super::capsule::parse_windows_signed_with_key(capsule_bytes, public_key_hex),
    )
}

fn confined_spawn_using(
    exe: &Path,
    role: ImageRole,
    args: &[&OsStr],
    scratch_root: &Path,
    request: &[u8],
    bundle: &[u8],
    parse_capsule: impl FnOnce()
        -> Result<super::capsule::VerifiedCapsule, super::capsule::CapsuleError>,
) -> Result<ConfinedProcess, Refusal> {
    confined_spawn_observing_inner(
        exe,
        role,
        args,
        scratch_root,
        request,
        bundle,
        parse_capsule,
        |_| {},
    )
}

#[cfg(test)]
fn confined_spawn_after_binding(
    exe: &Path,
    role: ImageRole,
    args: &[&OsStr],
    scratch_root: &Path,
    parse_capsule: impl FnOnce()
        -> Result<super::capsule::VerifiedCapsule, super::capsule::CapsuleError>,
    after_binding: impl FnOnce(),
) -> Result<ConfinedProcess, Refusal> {
    let mut after_binding = Some(after_binding);
    confined_spawn_observing(
        exe,
        role,
        args,
        scratch_root,
        TEST_REQUEST_BYTES,
        TEST_BUNDLE_BYTES,
        parse_capsule,
        |boundary| {
            if boundary == BindingBoundary::BeforeProcessCreation {
                after_binding.take().expect("binding callback runs once")();
            }
        },
    )
}

#[cfg(test)]
fn confined_spawn_observing(
    exe: &Path,
    role: ImageRole,
    args: &[&OsStr],
    scratch_root: &Path,
    request: &[u8],
    bundle: &[u8],
    parse_capsule: impl FnOnce()
        -> Result<super::capsule::VerifiedCapsule, super::capsule::CapsuleError>,
    observe: impl FnMut(BindingBoundary),
) -> Result<ConfinedProcess, Refusal> {
    confined_spawn_observing_inner(
        exe,
        role,
        args,
        scratch_root,
        request,
        bundle,
        parse_capsule,
        observe,
    )
}

fn confined_spawn_observing_inner(
    exe: &Path,
    role: ImageRole,
    args: &[&OsStr],
    scratch_root: &Path,
    request: &[u8],
    bundle: &[u8],
    parse_capsule: impl FnOnce()
        -> Result<super::capsule::VerifiedCapsule, super::capsule::CapsuleError>,
    mut observe: impl FnMut(BindingBoundary),
) -> Result<ConfinedProcess, Refusal> {
    let (_host, (image, carriers, selector), token, job, scratch) = admit(
        || {
            if cfg!(all(
                windows,
                target_pointer_width = "64",
                any(target_arch = "x86_64", target_arch = "aarch64")
            )) {
                Ok(())
            } else {
                Err(())
            }
        },
        || {
            let capsule = parse_capsule()?;
            image::HeldImage::acquire_observing(exe, role.artifact(&capsule), |boundary| {
                observe(BindingBoundary::Image(boundary));
            })
            .map_err(|()| super::capsule::CapsuleError::ArtifactBinding)
            .and_then(|image| {
                let carriers = AuthenticatedRequestBundle::create(request, bundle, &capsule)
                    .map_err(|()| super::capsule::CapsuleError::ArtifactBinding)?;
                Ok((image, carriers, capsule.selector))
            })
        },
        restricted_token,
        tightened_job,
        || {
            let token = restricted_token()?;
            let mut sid_buffer = [0u8; 256];
            read_token_user_sid(&token, &mut sid_buffer)?;
            confined_scratch_root(scratch_root, &sid_buffer)
        },
    )?;
    observe(BindingBoundary::BeforeProcessCreation);
    let stdin = open_inheritable_null().map_err(|()| Refusal::FilesystemConfinement)?;
    let (stdout, stdout_writer) =
        create_inheritable_output_pipe().map_err(|()| Refusal::FilesystemConfinement)?;
    let (stderr, stderr_writer) =
        create_inheritable_output_pipe().map_err(|()| Refusal::FilesystemConfinement)?;

    let application = image.application();
    let mut command: Vec<u16> = Vec::new();
    command.push(u16::from(b'"'));
    command.extend_from_slice(&application[..application.len() - 1]);
    command.push(u16::from(b'"'));
    for arg in args {
        command.push(u16::from(b' '));
        command.extend(arg.encode_wide());
    }
    command.push(0);
    let cwd = wide(scratch.dir.as_os_str()).map_err(|()| Refusal::Invalid)?;
    let environment = carrier_environment(&scratch.dir, role, &selector, &carriers)
        .map_err(|()| Refusal::Invalid)?;

    // `bInheritHandles` alone would copy every inheritable handle held by the
    // parent into the restricted child. Bind that broad Win32 switch to the
    // three standard handles plus the exact request-then-bundle carrier pair
    // authenticated from this capsule.
    let [request_carrier, bundle_carrier] = carriers.child_handles();
    let inherited = [
        stdin.raw(),
        stdout_writer.raw(),
        stderr_writer.raw(),
        request_carrier,
        bundle_carrier,
    ];
    let mut attribute_bytes = 0usize;
    // SAFETY: this sizing invocation has no output list and only reports the
    // required bounded allocation through `attribute_bytes`.
    unsafe {
        InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut attribute_bytes);
    }
    if attribute_bytes == 0 || attribute_bytes > 65_536 {
        return Err(Refusal::Spawn);
    }
    let attribute_words = attribute_bytes.div_ceil(std::mem::size_of::<usize>());
    let mut attribute_backing = vec![0usize; attribute_words];
    let attribute_pointer = attribute_backing.as_mut_ptr().cast();
    // SAFETY: the backing allocation has the requested size and remains live
    // until `attributes` is dropped after process creation.
    if unsafe { InitializeProcThreadAttributeList(attribute_pointer, 1, 0, &mut attribute_bytes) }
        == 0
    {
        return Err(Refusal::Spawn);
    }
    let attributes = AttributeList(attribute_pointer);
    // SAFETY: every listed handle is live, inheritable, and remains live until
    // CreateProcessAsUserW returns; the fixed array is the entire intended
    // child handle inventory.
    if unsafe {
        UpdateProcThreadAttribute(
            attributes.0,
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            inherited.as_ptr().cast(),
            std::mem::size_of_val(&inherited),
            std::ptr::null_mut(),
            std::ptr::null(),
        )
    } == 0
    {
        return Err(Refusal::Spawn);
    }
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin.raw();
    startup.StartupInfo.hStdOutput = stdout_writer.raw();
    startup.StartupInfo.hStdError = stderr_writer.raw();
    startup.lpAttributeList = attributes.0;
    let mut process_information = PROCESS_INFORMATION::default();
    if !image.intact() {
        return Err(Refusal::Capsule(
            super::capsule::CapsuleError::ArtifactBinding,
        ));
    }
    // SAFETY: `token` is a live restricted token with the rights
    // `CreateProcessAsUserW` requires; `application`/`cwd`/`environment` are
    // live NUL-terminated (or double-NUL-terminated) wide buffers; `command`
    // is a live, exclusively-owned mutable buffer as this API requires;
    // `startup`'s three handles are live and inheritable. The extended-startup
    // handle-list attribute limits inheritance to exactly that inventory.
    if unsafe {
        CreateProcessAsUserW(
            token.raw(),
            application.as_ptr(),
            command.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            1,
            CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT | EXTENDED_STARTUPINFO_PRESENT,
            environment.as_ptr().cast(),
            cwd.as_ptr(),
            &startup.StartupInfo,
            &mut process_information,
        )
    } == 0
    {
        return Err(Refusal::Spawn);
    }
    let process = Handle::new(process_information.hProcess);
    let thread = Handle::new(process_information.hThread);
    // SAFETY: the leader remains suspended; both handles are exclusively
    // held by this function.
    if unsafe { AssignProcessToJobObject(job.raw(), process.raw()) } == 0 {
        // SAFETY: the leader is still suspended; terminating it now cannot
        // race with any code the leader would otherwise run.
        unsafe { TerminateProcess(process.raw(), 1) };
        // SAFETY: the owned process remains suspended until it exits. Do not
        // return an unowned live leader after a setup failure.
        if unsafe { WaitForSingleObject(process.raw(), 5_000) } != WAIT_OBJECT_0 {
            std::process::abort();
        }
        return Err(Refusal::Spawn);
    }
    observe(BindingBoundary::SuspendedLeader);
    // CreateProcess can be redirected by host policy (for example IFEO).
    // Require the suspended process's native image name to name the held
    // authenticated file before its first thread may execute any code.
    if !image.matches_process(process.raw()) {
        // SAFETY: this job owns the suspended leader; no thread was resumed.
        unsafe { TerminateJobObject(job.raw(), 1) };
        if unsafe { WaitForSingleObject(process.raw(), 5_000) } != WAIT_OBJECT_0 {
            std::process::abort();
        }
        return Err(Refusal::Capsule(
            super::capsule::CapsuleError::ArtifactBinding,
        ));
    }
    // SAFETY: this is the primary thread `CreateProcessAsUserW` returned
    // suspended.
    if unsafe { ResumeThread(thread.raw()) } == u32::MAX {
        return Err(Refusal::Spawn);
    }

    // `CreateProcessAsUserW` duplicated the two exact writers listed above.
    // Drop the parent's copies before returning so EOF means the confined job
    // has released its writers; only these parent-only readers remain for
    // bounded settlement accounting.
    drop(stdout_writer);
    drop(stderr_writer);

    Ok(ConfinedProcess {
        process,
        thread,
        job,
        _token: token,
        _scratch: scratch,
        _stdin: stdin,
        stdout,
        stderr,
        _carriers: carriers,
        _image: image,
        settled: false,
    })
}

pub struct Settled {
    pub status: Settlement,
}

pub fn settle(mut confined: ConfinedProcess, deadline: Duration) -> Settled {
    let deadline = Instant::now()
        .checked_add(deadline)
        .unwrap_or_else(Instant::now);
    let status = settle_confined(&mut confined, deadline);
    confined.settled = true;
    Settled { status }
}

enum OutputDrain {
    Pending,
    Eof,
    Limit,
    Failed,
}

/// Drain one bounded chunk from one parent-only output reader. The pipe itself
/// limits pending kernel bytes; `charged` limits the complete stream pair even
/// while a writer keeps making progress.
fn drain_output(reader: &Handle, charged: &mut usize) -> OutputDrain {
    let mut available = 0u32;
    // SAFETY: `reader` is a live parent-only pipe reader. No data buffer is
    // passed to this sizing query and `available` is exclusively writable.
    if unsafe {
        PeekNamedPipe(
            reader.raw(),
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            &mut available,
            std::ptr::null_mut(),
        )
    } == 0
    {
        // SAFETY: the failing PeekNamedPipe call set the thread-local error.
        return match unsafe { GetLastError() } {
            ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED => OutputDrain::Eof,
            _ => OutputDrain::Failed,
        };
    }
    let available = available as usize;
    if available > OUTPUT_LIMIT_BYTES.saturating_sub(*charged) {
        return OutputDrain::Limit;
    }
    if available == 0 {
        return OutputDrain::Pending;
    }
    let mut buffer = [0u8; 8192];
    let count = available.min(buffer.len());
    let mut read = 0u32;
    // SAFETY: PeekNamedPipe reported at least `count` readable bytes, the
    // fixed buffer is writable for exactly `count`, and this is the sole
    // parent read handle for the pipe.
    if unsafe {
        ReadFile(
            reader.raw(),
            buffer.as_mut_ptr().cast(),
            count as u32,
            &mut read,
            std::ptr::null_mut(),
        )
    } == 0
        || read == 0
        || read as usize > count
    {
        return OutputDrain::Failed;
    }
    *charged += read as usize;
    OutputDrain::Pending
}

fn settle_confined(confined: &mut ConfinedProcess, deadline: Instant) -> Settlement {
    let mut state = StickySettlement::default();
    let mut timed_out = false;
    let mut output_limited = false;
    let mut output_fault = false;
    let mut killed_and_reaped = false;
    let mut empty_job_deadline = None;
    let mut exit_code = None;
    let mut output_eof = [false; 2];
    let mut charged_output = 0usize;
    let readers = [&confined.stdout, &confined.stderr];

    loop {
        for (index, reader) in readers.iter().enumerate() {
            if output_eof[index] {
                continue;
            }
            match drain_output(reader, &mut charged_output) {
                OutputDrain::Pending => {}
                OutputDrain::Eof => output_eof[index] = true,
                OutputDrain::Limit => {
                    state.select(Settlement::Failed(FailureReason::OutputLimit));
                    output_limited = true;
                    break;
                }
                OutputDrain::Failed => {
                    state.select(Settlement::Uncertain(UncertainReason::OutputReadFailed));
                    output_fault = true;
                    break;
                }
            }
        }
        if output_limited || state.is_selected() {
            break;
        }
        // SAFETY: `confined.process` is a live, held process handle.
        match unsafe { WaitForSingleObject(confined.process.raw(), 0) } {
            WAIT_OBJECT_0 => {
                let mut code = u32::MAX;
                // SAFETY: `confined.process` is live and signaled.
                if unsafe { GetExitCodeProcess(confined.process.raw(), &mut code) } == 0 {
                    state.select(Settlement::Uncertain(UncertainReason::WaitFailed));
                } else {
                    exit_code = Some(code);
                }
                break;
            }
            WAIT_TIMEOUT => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            _ => {
                state.select(Settlement::Uncertain(UncertainReason::WaitFailed));
                break;
            }
        }
    }

    if timed_out || output_limited || output_fault {
        // SAFETY: `confined.job` is live and owns exclusive termination
        // authority over this confined process tree.
        let killed = unsafe { TerminateJobObject(confined.job.raw(), 126) } != 0;
        if !killed {
            state.select(Settlement::Uncertain(UncertainReason::KillAmbiguous));
        } else {
            // TerminateJobObject requests termination; it does not prove that
            // the leader has released its inherited scratch-file handles.
            // Keep one fixed leader-reap grace after the selected limit.
            match unsafe { WaitForSingleObject(confined.process.raw(), 5_000) } {
                WAIT_OBJECT_0 => {
                    killed_and_reaped = true;
                    // A signaled leader does not prove its descendants have
                    // completed termination. Give the job a bounded drain
                    // interval before choosing a sticky nonempty-job result.
                    empty_job_deadline = Some(Instant::now() + Duration::from_secs(5));
                }
                WAIT_TIMEOUT => {
                    state.select(Settlement::Uncertain(UncertainReason::KillWaitTimedOut));
                }
                _ => state.select(Settlement::Uncertain(UncertainReason::WaitFailed)),
            }
        }
    } else if exit_code.is_some() {
        // The child has closed both inherited writers once it exits. Drain the
        // remaining fixed pipe buffers before classifying a successful exit so
        // an end-of-process flood cannot bypass the combined output ceiling.
        let output_deadline = Instant::now() + Duration::from_secs(5);
        while !output_eof.iter().all(|eof| *eof) {
            for (index, reader) in readers.iter().enumerate() {
                if output_eof[index] {
                    continue;
                }
                match drain_output(reader, &mut charged_output) {
                    OutputDrain::Pending => {}
                    OutputDrain::Eof => output_eof[index] = true,
                    OutputDrain::Limit => {
                        state.select(Settlement::Failed(FailureReason::OutputLimit));
                        output_limited = true;
                        break;
                    }
                    OutputDrain::Failed => {
                        state.select(Settlement::Uncertain(UncertainReason::OutputReadFailed));
                        break;
                    }
                }
            }
            if output_limited
                || output_fault
                || state.is_selected()
                || output_eof.iter().all(|eof| *eof)
            {
                break;
            }
            if Instant::now() >= output_deadline {
                state.select(Settlement::Uncertain(UncertainReason::OutputReadFailed));
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    loop {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: `confined.job` is live; `accounting` is a live,
        // exclusively-owned local of the exact size passed.
        let queried = unsafe {
            QueryInformationJobObject(
                confined.job.raw(),
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                std::mem::size_of_val(&accounting) as u32,
                std::ptr::null_mut(),
            )
        } != 0;
        if !queried {
            state.select(Settlement::Uncertain(UncertainReason::QueryFailed));
            break;
        }
        if accounting.ActiveProcesses == 0 {
            if timed_out && killed_and_reaped {
                // Only a reaped leader and empty job prove cancellation.
                state.select(Settlement::Cancelled);
            } else if !output_limited {
                match exit_code {
                    Some(0) => state.select(Settlement::Completed),
                    Some(code) => state.select(Settlement::Failed(FailureReason::ExitCode(code))),
                    None => {}
                }
            }
            break;
        }
        match empty_job_deadline {
            Some(deadline) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(1));
            }
            _ => {
                state.select(Settlement::Uncertain(
                    UncertainReason::ActiveProcessesNonZero,
                ));
                break;
            }
        }
    }

    state
        .resolve()
        .unwrap_or(Settlement::Uncertain(UncertainReason::WaitFailed))
}

#[cfg(test)]
mod tests;
