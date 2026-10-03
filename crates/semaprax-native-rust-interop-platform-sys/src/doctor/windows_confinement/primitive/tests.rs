//! These tests exercise the pure helpers and live Win32 spawn/settle seam.
//! The live cases require the gate's explicit
//! `SEMAPRAX_WINDOWS_CONFINEMENT_TEST_PARENT`; absent or unusable provisioning
//! is a test failure. Their capsule is signed with a deterministic
//! test-only key and parsed by the shared Ed25519 verifier. That key is not a
//! release trust anchor and proves no production/release trust.
use super::super::capsule::CapsuleError;
use super::*;
use std::ffi::OsString;

mod binding;

const TEST_PARENT_ENV: &str = "SEMAPRAX_WINDOWS_CONFINEMENT_TEST_PARENT";
const TEST_MARKER: &str = "runtime-child-started.bin";
const DESCENDANT_PERMIT: &str = "runtime-descendant-permit.bin";
const HANDLE_INVENTORY_PERMIT: &str = "runtime-handle-inventory-permit.bin";
const INHERITABLE_SENTINEL: &str = "runtime-inheritable-sentinel.bin";

fn runtime_parent() -> PathBuf {
    let parent = std::env::var_os(TEST_PARENT_ENV)
        .unwrap_or_else(|| panic!("{TEST_PARENT_ENV} must be provisioned by the Windows gate"));
    let parent = PathBuf::from(parent);
    assert!(parent.is_absolute(), "runtime parent must be absolute");
    let metadata = std::fs::symlink_metadata(&parent).expect("provisioned runtime parent exists");
    assert!(metadata.is_dir(), "runtime parent is a directory");
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        use windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT;
        assert_eq!(
            metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT,
            0,
            "runtime parent must not be a reparse point"
        );
    }
    assert_parent_empty(&parent);
    parent
}

fn assert_parent_empty(parent: &Path) {
    assert!(
        std::fs::read_dir(parent)
            .expect("provisioned runtime parent can be enumerated")
            .next()
            .is_none(),
        "owned runtime scratch was removed from the explicit parent"
    );
}

struct TestCapsule {
    bytes: Vec<u8>,
    public_key_hex: String,
}

fn test_capsule_body() -> TestCapsule {
    use sha2::{Digest as _, Sha256};
    let architecture = super::super::capsule::windows_architecture_code()
        .expect("the selected runtime gate only admits Windows x86-64 or AArch64");
    let executable = std::fs::read(std::env::current_exe().unwrap()).unwrap();
    let mut artifacts = std::array::from_fn(|index| semaprax_doctor_capsule::Artifact {
        length: index as u64 + 1,
        digest: [0x42; 32],
    });
    artifacts[0] = semaprax_doctor_capsule::Artifact {
        length: TEST_REQUEST_BYTES.len() as u64,
        digest: Sha256::digest(TEST_REQUEST_BYTES).into(),
    };
    artifacts[1] = semaprax_doctor_capsule::Artifact {
        length: TEST_BUNDLE_BYTES.len() as u64,
        digest: Sha256::digest(TEST_BUNDLE_BYTES).into(),
    };
    artifacts[3] = semaprax_doctor_capsule::Artifact {
        length: executable.len() as u64,
        digest: Sha256::digest(&executable).into(),
    };
    let (bytes, public_key_hex) =
        super::super::capsule::signed_test_fixture_with_artifacts(architecture, artifacts);
    TestCapsule {
        bytes,
        public_key_hex,
    }
}

fn child_test_args(name: &str) -> Vec<OsString> {
    let child_filter = format!("doctor::windows_confinement::primitive::tests::{name}");
    ["--ignored", "--exact", child_filter.as_str(), "--nocapture"]
        .into_iter()
        .map(OsString::from)
        .collect()
}

fn wait_for_marker(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if path.is_file() {
            return;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "confined child did not create its start marker: {}",
        path.display()
    );
}

fn publish_child_marker(contents: &[u8]) {
    let directory = std::env::current_dir().expect("confined current directory");
    let marker = directory.join(TEST_MARKER);
    let temporary = directory.join(format!("{TEST_MARKER}.tmp"));
    let publish = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        std::io::Write::write_all(&mut file, contents)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, &marker)
    })();
    if publish.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    publish.expect("child atomically publishes a complete start marker");
}

#[test]
#[ignore = "spawned only by the live confinement runtime tests"]
fn runtime_child_checks_descendant_job_limit() {
    let current_exe = std::env::current_exe().expect("confined executable path");
    let descendant = std::process::Command::new(current_exe)
        .arg("--list")
        .output();
    let quota_status = windows_sys::Win32::Foundation::ERROR_NOT_ENOUGH_QUOTA as i32;
    let active_process_limit_refused = match descendant {
        Err(error) => error.raw_os_error() == Some(quota_status),
        Ok(output) => {
            output.status.code() == Some(quota_status)
                && output.stdout.is_empty()
                && output.stderr.is_empty()
        }
    };
    let observation: &[u8] = if active_process_limit_refused {
        b"active-process-limit-refused"
    } else {
        b"descendant-created-or-refused-for-another-reason"
    };
    publish_child_marker(observation);
}

#[test]
#[ignore = "spawned only by the live confinement runtime tests"]
fn runtime_child_marks_start_then_waits_for_job_termination() {
    publish_child_marker(b"started");
    loop {
        std::thread::sleep(Duration::from_secs(30));
    }
}

#[test]
#[ignore = "spawned only by the live confinement runtime tests"]
fn runtime_child_exhausts_cpu_time_limit() {
    publish_child_marker(b"cpu-limit-started");
    let mut state = 0x9e37_79b9_7f4a_7c15_u64;
    loop {
        state = std::hint::black_box(
            state
                .wrapping_mul(0xbf58_476d_1ce4_e5b9)
                .rotate_left(17)
                .wrapping_add(0x94d0_49bb_1331_11eb),
        );
    }
}

#[test]
#[ignore = "spawned only by the live confinement runtime tests"]
fn runtime_child_exhausts_committed_memory_limit() {
    const CHUNK_BYTES: usize = 8 * 1024 * 1024;
    let required_chunks = PROCESS_MEMORY_LIMIT_BYTES.div_ceil(CHUNK_BYTES) + 8;
    let mut chunks = Vec::with_capacity(required_chunks);
    for _ in 0..required_chunks {
        let mut chunk = Vec::new();
        if chunk.try_reserve_exact(CHUNK_BYTES).is_err() {
            publish_child_marker(b"committed-memory-limit-refused");
            return;
        }
        // Commit every page rather than merely reserving virtual address space.
        // With the configured job bound this must fail before the hostile
        // control can retain its requested `required_chunks` allocation.
        chunk.resize(CHUNK_BYTES, 0xa5);
        chunks.push(chunk);
    }
    publish_child_marker(b"committed-memory-limit-not-enforced");
}

#[test]
#[ignore = "spawned only by the live confinement runtime tests"]
fn runtime_child_exceeds_combined_output_limit() {
    use std::io::Write as _;

    const CHUNK_BYTES: usize = 8 * 1024;
    const CHUNKS_PER_STREAM: usize = 5;
    const STREAM_BYTES: usize = CHUNK_BYTES * CHUNKS_PER_STREAM;
    const { assert!(STREAM_BYTES < OUTPUT_LIMIT_BYTES) };
    const { assert!(STREAM_BYTES * 2 > OUTPUT_LIMIT_BYTES) };

    publish_child_marker(b"output-limit-started");
    let stdout_chunk = [b'o'; CHUNK_BYTES];
    let stderr_chunk = [b'e'; CHUNK_BYTES];
    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    let stderr = std::io::stderr();
    let mut stderr = stderr.lock();
    for _ in 0..CHUNKS_PER_STREAM {
        stdout
            .write_all(&stdout_chunk)
            .expect("write combined-limit stdout stream");
        stdout.flush().expect("flush confined stdout flood");
        stderr
            .write_all(&stderr_chunk)
            .expect("write combined-limit stderr stream");
        stderr.flush().expect("flush confined stderr flood");
    }
}

#[test]
#[ignore = "spawned only by the live confinement runtime tests"]
fn runtime_child_exits_with_nonzero_status() {
    publish_child_marker(b"exit-37");
    std::process::exit(37);
}

#[test]
#[ignore = "spawned only by the live confinement runtime tests"]
fn runtime_child_launches_descendant_after_parent_permit() {
    let directory = std::env::current_dir().expect("confined current directory");
    publish_child_marker(b"ready-for-descendant");
    let permit = directory.join(DESCENDANT_PERMIT);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !permit.is_file() {
        assert!(
            Instant::now() < deadline,
            "parent did not permit descendant launch"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let executable = std::env::current_exe().expect("confined executable path");
    let args = child_test_args("runtime_child_waits_for_job_termination");
    match std::process::Command::new(executable).args(args).spawn() {
        Ok(descendant) => {
            drop(descendant);
            publish_child_marker(b"descendant-started");
            loop {
                std::thread::sleep(Duration::from_secs(30));
            }
        }
        Err(_) => publish_child_marker(b"descendant-spawn-refused"),
    }
}

#[test]
#[ignore = "spawned only by the live confinement runtime tests"]
fn runtime_child_waits_for_job_termination() {
    loop {
        std::thread::sleep(Duration::from_secs(30));
    }
}

#[test]
#[ignore = "spawned only by the live confinement runtime tests"]
fn runtime_child_checks_unrelated_inheritable_handle_is_absent() {
    let directory = std::env::current_dir().expect("confined current directory");
    publish_child_marker(b"ready-for-handle-inventory");
    let permit = directory.join(HANDLE_INVENTORY_PERMIT);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !permit.is_file() {
        assert!(
            Instant::now() < deadline,
            "parent did not permit inherited-handle observation"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    let sentinel = directory
        .parent()
        .expect("confined scratch has the provisioned parent")
        .join(INHERITABLE_SENTINEL);
    publish_child_marker(if sentinel.exists() {
        b"unrelated-inheritable-handle-present"
    } else {
        b"unrelated-inheritable-handle-absent"
    });
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn runtime_child_reads_authenticated_request_bundle_carriers() {
    use windows_sys::Win32::Foundation::{
        DuplicateHandle, GetLastError, ERROR_ACCESS_DENIED, HANDLE,
    };
    use windows_sys::Win32::Storage::FileSystem::{WRITE_DAC, WRITE_OWNER};
    use windows_sys::Win32::System::Memory::{
        MapViewOfFile, UnmapViewOfFile, FILE_MAP_READ, FILE_MAP_WRITE,
    };

    fn inherited_handle(name: &str) -> HANDLE {
        std::env::var(name)
            .expect("parent supplied an authenticated carrier handle")
            .parse::<usize>()
            .expect("carrier handle is an unsigned process-local value") as HANDLE
    }

    fn assert_carrier(handle: HANDLE, expected: &[u8]) {
        // SAFETY: the parent placed this exact inheritable mapping handle in
        // the startup handle list and the expected length is the signed size.
        let view = unsafe { MapViewOfFile(handle, FILE_MAP_READ, 0, 0, expected.len()) };
        assert!(
            !view.Value.is_null(),
            "declared inherited carrier maps read-only in the child"
        );
        // SAFETY: the successful view covers exactly `expected.len()` bytes.
        assert_eq!(
            unsafe { std::slice::from_raw_parts(view.Value.cast::<u8>(), expected.len()) },
            expected,
            "child receives the exact signed carrier bytes"
        );
        // SAFETY: the child tests the inherited handle itself; a writable map
        // must fail because the parent duplicated it with SECTION_MAP_READ.
        let writable = unsafe { MapViewOfFile(handle, FILE_MAP_WRITE, 0, 0, expected.len()) };
        assert!(
            writable.Value.is_null(),
            "child inherited a writable request/bundle carrier"
        );
        // A read-only handle is insufficient if the section's DACL lets this
        // same-user child obtain a more powerful duplicate. Owner rights must
        // not let it rewrite that DACL and then obtain a writable duplicate.
        for access in [FILE_MAP_WRITE, WRITE_DAC, WRITE_OWNER] {
            let mut duplicate = std::ptr::null_mut();
            // SAFETY: this process owns the live inherited section handle;
            // the duplicate output is closed even if the hostile attempt wins.
            let duplicated = unsafe {
                DuplicateHandle(
                    GetCurrentProcess(),
                    handle,
                    GetCurrentProcess(),
                    &mut duplicate,
                    access,
                    0,
                    0,
                )
            };
            // SAFETY: capture the error before any other Win32 call.
            let error = unsafe { GetLastError() };
            if !duplicate.is_null() {
                drop(Handle::new(duplicate));
            }
            assert_eq!(duplicated, 0, "carrier duplicate gained access {access:#x}");
            assert_eq!(
                error, ERROR_ACCESS_DENIED,
                "carrier access escalation must fail through access control"
            );
        }
        // SAFETY: unmap the exact successful read view before returning.
        assert_ne!(unsafe { UnmapViewOfFile(view) }, 0);
    }

    assert_eq!(
        std::env::var(CARRIER_ROLE_ENV).expect("parent supplied the signed image role"),
        "worker",
        "child role is bound to the selected signed image role"
    );
    assert_eq!(
        std::env::var(CARRIER_SELECTOR_ENV).expect("parent supplied the signed selector"),
        "runtime-test",
        "child selector is bound to the signed capsule selector"
    );
    assert_carrier(
        inherited_handle(REQUEST_CARRIER_HANDLE_ENV),
        TEST_REQUEST_BYTES,
    );
    assert_carrier(
        inherited_handle(BUNDLE_CARRIER_HANDLE_ENV),
        TEST_BUNDLE_BYTES,
    );
    publish_child_marker(b"authenticated-request-bundle-carriers-observed");
}

fn current_process_handle_count() -> u32 {
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

    let mut count = 0;
    assert_ne!(
        unsafe { GetProcessHandleCount(GetCurrentProcess(), &mut count) },
        0,
        "query the current process's live handle count"
    );
    count
}

struct RuntimeChildCleanupGuard {
    process: windows_sys::Win32::Foundation::HANDLE,
    job: windows_sys::Win32::Foundation::HANDLE,
    scratch_dir: PathBuf,
    armed: bool,
}

impl RuntimeChildCleanupGuard {
    fn new(child: &ConfinedProcess) -> Self {
        Self {
            process: child.process.raw(),
            job: child.job.raw(),
            scratch_dir: child._scratch.dir.clone(),
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RuntimeChildCleanupGuard {
    fn drop(&mut self) {
        use windows_sys::Win32::Foundation::WAIT_OBJECT_0;
        use windows_sys::Win32::System::JobObjects::{
            JobObjectBasicAccountingInformation, QueryInformationJobObject, TerminateJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
        };
        use windows_sys::Win32::System::Threading::WaitForSingleObject;

        if !self.armed {
            return;
        }

        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        let mut queried = unsafe {
            QueryInformationJobObject(
                self.job,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                std::mem::size_of_val(&accounting) as u32,
                std::ptr::null_mut(),
            )
        } != 0;
        if queried && accounting.ActiveProcesses != 0 {
            if unsafe { TerminateJobObject(self.job, 126) } == 0 {
                eprintln!("runtime-test cleanup could not terminate its exact confined job");
                return;
            }
            if unsafe { WaitForSingleObject(self.process, 5_000) } != WAIT_OBJECT_0 {
                eprintln!("runtime-test cleanup could not reap its confined leader");
                return;
            }
            let deadline = Instant::now() + Duration::from_secs(5);
            loop {
                accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
                queried = unsafe {
                    QueryInformationJobObject(
                        self.job,
                        JobObjectBasicAccountingInformation,
                        (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                        std::mem::size_of_val(&accounting) as u32,
                        std::ptr::null_mut(),
                    )
                } != 0;
                if !queried {
                    eprintln!("runtime-test cleanup could not query its confined job");
                    return;
                }
                if accounting.ActiveProcesses == 0 {
                    break;
                }
                if Instant::now() >= deadline {
                    eprintln!("runtime-test cleanup could not prove its job empty");
                    return;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        if !queried
            || accounting.ActiveProcesses != 0
            || unsafe { WaitForSingleObject(self.process, 0) } != WAIT_OBJECT_0
        {
            eprintln!("runtime-test cleanup will not remove files before process settlement");
            return;
        }

        for name in [
            TEST_MARKER,
            "runtime-child-started.bin.tmp",
            DESCENDANT_PERMIT,
            HANDLE_INVENTORY_PERMIT,
        ] {
            match std::fs::remove_file(self.scratch_dir.join(name)) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => eprintln!("runtime-test cleanup could not remove {name}: {error}"),
            }
        }
    }
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_launches_restricted_child_inside_acl_scratch_and_settles_it() {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        EqualSid, GetSecurityDescriptorControl, GetTokenInformation, LookupPrivilegeValueW,
        TokenPrivileges, ACCESS_ALLOWED_ACE, DACL_SECURITY_INFORMATION, SE_CHANGE_NOTIFY_NAME,
        SE_DACL_PROTECTED, SE_PRIVILEGE_ENABLED, TOKEN_PRIVILEGES, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::{DELETE, FILE_GENERIC_READ, FILE_GENERIC_WRITE};
    use windows_sys::Win32::System::JobObjects::{
        IsProcessInJob, JobObjectBasicUIRestrictions, JobObjectExtendedLimitInformation,
        QueryInformationJobObject, JOBOBJECT_BASIC_UI_RESTRICTIONS,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOB_OBJECT_LIMIT_PROCESS_MEMORY, JOB_OBJECT_LIMIT_PROCESS_TIME,
    };
    use windows_sys::Win32::System::Threading::OpenProcessToken;

    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_checks_descendant_job_limit");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();
    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &parent,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("restricted-token child launch, job assignment, and ACL scratch setup succeed");
    let mut cleanup_guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    let scratch_dir = child._scratch.dir.clone();
    wait_for_marker(&marker);

    let mut restricted_token = std::ptr::null_mut();
    assert_ne!(
        unsafe { OpenProcessToken(child.process.raw(), TOKEN_QUERY, &mut restricted_token) },
        0,
        "the launched process exposes a queryable token"
    );
    let restricted_token = Handle::new(restricted_token);
    let mut privileges_length = 0u32;
    unsafe {
        GetTokenInformation(
            restricted_token.raw(),
            TokenPrivileges,
            std::ptr::null_mut(),
            0,
            &mut privileges_length,
        );
    }
    assert!(privileges_length >= std::mem::size_of::<TOKEN_PRIVILEGES>() as u32);
    let mut privilege_storage = vec![0u64; privileges_length.div_ceil(8) as usize];
    assert_ne!(
        unsafe {
            GetTokenInformation(
                restricted_token.raw(),
                TokenPrivileges,
                privilege_storage.as_mut_ptr().cast(),
                privileges_length,
                &mut privileges_length,
            )
        },
        0,
        "read the launched process's effective privilege set"
    );
    let privileges = unsafe { &*privilege_storage.as_ptr().cast::<TOKEN_PRIVILEGES>() };
    let privilege_bytes = privileges_length as usize;
    let privilege_offset = std::mem::offset_of!(TOKEN_PRIVILEGES, Privileges);
    let privilege_capacity = privilege_bytes.saturating_sub(privilege_offset)
        / std::mem::size_of::<windows_sys::Win32::Security::LUID_AND_ATTRIBUTES>();
    assert!(privileges.PrivilegeCount as usize <= privilege_capacity);
    let privilege_entries = unsafe {
        std::slice::from_raw_parts(
            privileges.Privileges.as_ptr(),
            privileges.PrivilegeCount as usize,
        )
    };
    let mut change_notify = windows_sys::Win32::Foundation::LUID::default();
    assert_ne!(
        unsafe {
            LookupPrivilegeValueW(std::ptr::null(), SE_CHANGE_NOTIFY_NAME, &mut change_notify)
        },
        0
    );
    assert!(
        privilege_entries.iter().all(|entry| {
            entry.Attributes & SE_PRIVILEGE_ENABLED == 0
                || (entry.Luid.LowPart == change_notify.LowPart
                    && entry.Luid.HighPart == change_notify.HighPart)
        }),
        "CreateRestrictedToken disabled every privilege except SeChangeNotifyPrivilege"
    );

    let mut in_job = 0;
    assert_ne!(
        unsafe { IsProcessInJob(child.process.raw(), child.job.raw(), &mut in_job) },
        0
    );
    assert_ne!(in_job, 0, "the child is assigned to its owned job object");

    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    assert_ne!(
        unsafe {
            QueryInformationJobObject(
                child.job.raw(),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
                std::ptr::null_mut(),
            )
        },
        0
    );
    let required_limits = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        | JOB_OBJECT_LIMIT_ACTIVE_PROCESS
        | JOB_OBJECT_LIMIT_PROCESS_TIME
        | JOB_OBJECT_LIMIT_PROCESS_MEMORY
        | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
    assert_eq!(
        limits.BasicLimitInformation.LimitFlags & required_limits,
        required_limits
    );
    assert_eq!(limits.BasicLimitInformation.ActiveProcessLimit, 1);
    assert_eq!(
        limits.BasicLimitInformation.PerProcessUserTimeLimit,
        CPU_TIME_LIMIT_100NS
    );
    assert_eq!(limits.ProcessMemoryLimit, PROCESS_MEMORY_LIMIT_BYTES);
    let mut ui = JOBOBJECT_BASIC_UI_RESTRICTIONS::default();
    assert_ne!(
        unsafe {
            QueryInformationJobObject(
                child.job.raw(),
                JobObjectBasicUIRestrictions,
                (&mut ui as *mut JOBOBJECT_BASIC_UI_RESTRICTIONS).cast(),
                std::mem::size_of_val(&ui) as u32,
                std::ptr::null_mut(),
            )
        },
        0
    );
    assert_eq!(ui.UIRestrictionsClass, DENIED_UI_LIMITS);

    let mut security_descriptor = std::ptr::null_mut();
    let mut dacl = std::ptr::null_mut();
    let scratch_wide = wide(child._scratch.dir.as_os_str()).unwrap();
    let security_result = unsafe {
        GetNamedSecurityInfoW(
            scratch_wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut security_descriptor,
        )
    };
    assert_eq!(security_result, 0, "read back the created scratch DACL");
    assert!(!dacl.is_null());
    let mut descriptor_control = 0u16;
    let mut descriptor_revision = 0u32;
    assert_ne!(
        unsafe {
            GetSecurityDescriptorControl(
                security_descriptor,
                &mut descriptor_control,
                &mut descriptor_revision,
            )
        },
        0
    );
    assert_ne!(
        descriptor_control & SE_DACL_PROTECTED,
        0,
        "scratch DACL protection blocks inherited ACEs"
    );
    let acl = unsafe { &*dacl };
    assert_eq!(acl.AceCount, 1, "scratch DACL contains one explicit ACE");
    let mut ace = std::ptr::null_mut();
    assert_ne!(
        unsafe { windows_sys::Win32::Security::GetAce(dacl, 0, &mut ace) },
        0
    );
    let ace = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
    assert_eq!(
        ace.Header.AceType, 0,
        "the sole ACE is an access-allowed ACE"
    );
    assert_eq!(
        ace.Mask,
        FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE,
        "scratch grants only the primitive's explicit read/write/delete mask"
    );
    let mut token_user = [0u8; 256];
    read_token_user_sid(&child._token, &mut token_user).unwrap();
    let expected_sid = unsafe { (*token_user.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let ace_sid = (&ace.SidStart as *const u32).cast_mut().cast();
    assert_ne!(unsafe { EqualSid(expected_sid, ace_sid) }, 0);
    assert_eq!(
        unsafe { LocalFree(security_descriptor.cast()) },
        std::ptr::null_mut()
    );

    let marker_contents = std::fs::read(&marker).unwrap();
    std::fs::remove_file(&marker).expect("remove exact child marker before scratch settlement");
    cleanup_guard.disarm();
    let settled = settle(child, Duration::from_secs(30));
    assert_eq!(settled.status, Settlement::Completed);
    assert_eq!(
        marker_contents, b"active-process-limit-refused",
        "the one-process job limit refuses a launched child's descendant"
    );
    assert!(
        !scratch_dir.exists(),
        "settlement removes the now-empty per-child scratch directory"
    );
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_timeout_terminates_the_confined_job_and_settles_cancellation() {
    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_marks_start_then_waits_for_job_termination");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();
    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &parent,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("restricted-token child launch, job assignment, and ACL scratch setup succeed");
    let mut cleanup_guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    let scratch_dir = child._scratch.dir.clone();
    wait_for_marker(&marker);
    assert_eq!(std::fs::read(&marker).unwrap(), b"started");
    std::fs::remove_file(&marker).expect("remove exact child marker before scratch settlement");
    cleanup_guard.disarm();
    let settled = settle(child, Duration::from_millis(100));
    assert_eq!(settled.status, Settlement::Cancelled);
    assert!(
        !scratch_dir.exists(),
        "settlement removes the now-empty per-child scratch directory"
    );
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_cpu_time_limit_terminates_and_settles_the_confined_job() {
    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_exhausts_cpu_time_limit");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();
    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &parent,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("restricted child enters the CPU-limited job");
    let mut cleanup_guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    let scratch_dir = child._scratch.dir.clone();
    wait_for_marker(&marker);
    assert_eq!(std::fs::read(&marker).unwrap(), b"cpu-limit-started");
    std::fs::remove_file(&marker).expect("remove exact child marker before scratch settlement");
    cleanup_guard.disarm();
    assert!(
        matches!(
            settle(child, Duration::from_secs(12)).status,
            Settlement::Failed(FailureReason::ExitCode(_))
        ),
        "without a leader-correlated process-time notification, preserve the nonzero exit code"
    );
    assert!(
        !scratch_dir.exists(),
        "CPU-limit settlement removes the now-empty per-child scratch directory"
    );
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_committed_memory_limit_refuses_the_hostile_allocation() {
    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_exhausts_committed_memory_limit");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();
    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &parent,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("restricted child enters the committed-memory-limited job");
    let mut cleanup_guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    let scratch_dir = child._scratch.dir.clone();
    wait_for_marker(&marker);
    assert_eq!(
        std::fs::read(&marker).unwrap(),
        b"committed-memory-limit-refused",
        "the hostile allocation must fail under the configured committed-memory cap"
    );
    std::fs::remove_file(&marker).expect("remove exact child marker before scratch settlement");
    cleanup_guard.disarm();
    assert_eq!(
        settle(child, Duration::from_secs(30)).status,
        Settlement::Completed
    );
    assert!(
        !scratch_dir.exists(),
        "memory-limit settlement removes the now-empty per-child scratch directory"
    );
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_combined_output_limit_terminates_and_settles_the_confined_job() {
    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_exceeds_combined_output_limit");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();
    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &parent,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("restricted child enters the output-accounted job");
    let mut cleanup_guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    let scratch_dir = child._scratch.dir.clone();
    wait_for_marker(&marker);
    assert_eq!(std::fs::read(&marker).unwrap(), b"output-limit-started");
    std::fs::remove_file(&marker).expect("remove exact child marker before scratch settlement");
    cleanup_guard.disarm();
    assert_eq!(
        settle(child, Duration::from_secs(12)).status,
        Settlement::Failed(FailureReason::OutputLimit)
    );
    assert!(
        !scratch_dir.exists(),
        "output-limit settlement removes the now-empty per-child scratch directory"
    );
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_signed_test_key_capsule_refusals_and_launch_settle() {
    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_marks_start_then_waits_for_job_termination");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();

    let handles_before_refusals = current_process_handle_count();
    let missing_anchor = confined_spawn_using(
        &executable,
        ImageRole::Worker,
        &borrowed_args,
        &parent,
        TEST_REQUEST_BYTES,
        TEST_BUNDLE_BYTES,
        || super::super::capsule::parse_with_anchor(&capsule.bytes, None),
    );
    assert_eq!(
        missing_anchor.err(),
        Some(Refusal::Capsule(CapsuleError::MissingTrustAnchor))
    );

    let mut tampered = capsule.bytes.clone();
    *tampered
        .last_mut()
        .expect("signed test capsule is nonempty") ^= 1;
    assert_eq!(
        confined_spawn_with_test_key(
            &executable,
            &borrowed_args,
            &parent,
            &tampered,
            &capsule.public_key_hex,
        )
        .err(),
        Some(Refusal::Capsule(CapsuleError::Signature))
    );
    assert_eq!(
        current_process_handle_count(),
        handles_before_refusals,
        "missing-anchor and invalid-signature refusals precede Win32 setup"
    );
    assert_parent_empty(&parent);

    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &parent,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("the test-only signed capsule verifies and reaches the Win32 seam");
    let mut cleanup_guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    let scratch_dir = child._scratch.dir.clone();
    wait_for_marker(&marker);
    assert_eq!(std::fs::read(&marker).unwrap(), b"started");
    std::fs::remove_file(&marker).expect("remove exact child marker before scratch settlement");
    cleanup_guard.disarm();
    let settled = settle(child, Duration::from_millis(100));
    assert_eq!(settled.status, Settlement::Cancelled);
    assert!(!scratch_dir.exists());
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_signed_request_bundle_substitution_refuses_before_process_effects() {
    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let capsule = test_capsule_body();
    let mut substitute_request = TEST_REQUEST_BYTES.to_vec();
    let mut substitute_bundle = TEST_BUNDLE_BYTES.to_vec();
    *substitute_request.last_mut().unwrap() ^= 0xa5;
    *substitute_bundle.last_mut().unwrap() ^= 0x5a;

    let refuse = |request: &[u8], bundle: &[u8]| {
        assert_eq!(
            confined_spawn_with_test_key_carriers(
                &executable,
                &[],
                &parent,
                &capsule.bytes,
                &capsule.public_key_hex,
                request,
                bundle,
            )
            .err(),
            Some(Refusal::Capsule(CapsuleError::ArtifactBinding)),
            "same-length substituted request or bundle must refuse before process creation"
        );
        assert_parent_empty(&parent);
    };

    // Warm the image admission path before requiring exact live-handle
    // settlement across repeated hostile substitutions.
    refuse(&substitute_request, TEST_BUNDLE_BYTES);
    let baseline = current_process_handle_count();
    for _ in 0..4 {
        refuse(&substitute_request, TEST_BUNDLE_BYTES);
        refuse(TEST_REQUEST_BYTES, &substitute_bundle);
        assert_eq!(
            current_process_handle_count(),
            baseline,
            "request/bundle substitution refusal settles every held image and carrier handle"
        );
    }
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_signed_request_bundle_carriers_reach_child_with_fixed_bindings() {
    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_reads_authenticated_request_bundle_carriers");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();
    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &parent,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect(
        "authenticated request/bundle carriers reach the child only through the fixed inventory",
    );
    let mut guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    wait_for_marker(&marker);
    assert_eq!(
        std::fs::read(&marker).unwrap(),
        b"authenticated-request-bundle-carriers-observed"
    );
    std::fs::remove_file(&marker).unwrap();
    guard.disarm();
    assert_eq!(
        settle(child, Duration::from_secs(30)).status,
        Settlement::Completed
    );
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_timeout_terminates_an_actual_job_descendant() {
    use windows_sys::Win32::System::JobObjects::{
        JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
    };

    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_launches_descendant_after_parent_permit");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();
    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &parent,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("restricted-token child launch, job assignment, and ACL scratch setup succeed");
    let mut cleanup_guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    let scratch_dir = child._scratch.dir.clone();
    wait_for_marker(&marker);
    assert_eq!(std::fs::read(&marker).unwrap(), b"ready-for-descendant");

    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    assert_ne!(
        unsafe {
            QueryInformationJobObject(
                child.job.raw(),
                JobObjectExtendedLimitInformation,
                (&mut limits as *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
                std::ptr::null_mut(),
            )
        },
        0
    );
    assert_ne!(
        limits.BasicLimitInformation.LimitFlags & JOB_OBJECT_LIMIT_ACTIVE_PROCESS,
        0
    );
    assert_eq!(limits.BasicLimitInformation.ActiveProcessLimit, 1);
    // This isolated test relaxes only its own job after confirming production's
    // configured one-process cap; the separate success test proves that cap.
    limits.BasicLimitInformation.ActiveProcessLimit = 2;
    assert_ne!(
        unsafe {
            SetInformationJobObject(
                child.job.raw(),
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        },
        0,
        "test-owned job admits one descendant solely to exercise tree timeout"
    );

    std::fs::remove_file(&marker).expect("remove readiness marker before signaling child");
    std::fs::write(child._scratch.dir.join(DESCENDANT_PERMIT), b"go")
        .expect("permit exact confined child to launch one descendant");
    wait_for_marker(&marker);
    assert_eq!(std::fs::read(&marker).unwrap(), b"descendant-started");

    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let mut accounting =
            windows_sys::Win32::System::JobObjects::JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default(
            );
        assert_ne!(
            unsafe {
                windows_sys::Win32::System::JobObjects::QueryInformationJobObject(
                    child.job.raw(),
                    windows_sys::Win32::System::JobObjects::JobObjectBasicAccountingInformation,
                    (&mut accounting as *mut windows_sys::Win32::System::JobObjects::JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                    std::mem::size_of_val(&accounting) as u32,
                    std::ptr::null_mut(),
                )
            },
            0,
            "observe live job membership before timed cancellation"
        );
        if accounting.ActiveProcesses == 2 {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "job never contained leader and descendant"
        );
        std::thread::sleep(Duration::from_millis(10));
    }

    std::fs::remove_file(&marker).expect("remove descendant marker before job settlement");
    std::fs::remove_file(child._scratch.dir.join(DESCENDANT_PERMIT))
        .expect("remove exact descendant permit before job settlement");
    cleanup_guard.disarm();
    let settled = settle(child, Duration::from_millis(100));
    assert_eq!(settled.status, Settlement::Cancelled);
    assert!(
        !scratch_dir.exists(),
        "empty-job settlement closes both processes before scratch cleanup"
    );
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_nonzero_exit_settles_failed_and_cleans_resources() {
    let parent = runtime_parent();
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_exits_with_nonzero_status");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();
    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &parent,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("restricted-token child launch, job assignment, and ACL scratch setup succeed");
    let mut cleanup_guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    let scratch_dir = child._scratch.dir.clone();
    wait_for_marker(&marker);
    assert_eq!(std::fs::read(&marker).unwrap(), b"exit-37");
    std::fs::remove_file(&marker).expect("remove exact child marker before scratch settlement");
    cleanup_guard.disarm();
    let settled = settle(child, Duration::from_secs(30));
    assert_eq!(
        settled.status,
        Settlement::Failed(FailureReason::ExitCode(37))
    );
    assert!(
        !scratch_dir.exists(),
        "settlement removes the now-empty per-child scratch directory"
    );
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_scratch_refusal_closes_setup_handles() {
    let parent = runtime_parent();
    let missing_parent = parent.join(format!("missing-scratch-parent-{}", std::process::id()));
    assert!(!missing_parent.exists());
    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_marks_start_then_waits_for_job_termination");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();

    // Warm dynamic Windows call paths before taking the handle baseline, then
    // require repeated real token/job/ACL-stage refusal to settle all handles.
    for _ in 0..2 {
        assert_eq!(
            confined_spawn_with_test_key(
                &executable,
                &borrowed_args,
                &missing_parent,
                &capsule.bytes,
                &capsule.public_key_hex,
            )
            .err(),
            Some(Refusal::FilesystemConfinement)
        );
    }
    let baseline = current_process_handle_count();
    for _ in 0..4 {
        assert_eq!(
            confined_spawn_with_test_key(
                &executable,
                &borrowed_args,
                &missing_parent,
                &capsule.bytes,
                &capsule.public_key_hex,
            )
            .err(),
            Some(Refusal::FilesystemConfinement)
        );
        assert_eq!(
            current_process_handle_count(),
            baseline,
            "token, job, and filesystem-stage failure handles are settled"
        );
    }
    assert!(
        !missing_parent.exists(),
        "refusal does not create the absent parent"
    );
    assert_parent_empty(&parent);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_protected_scratch_dacl_blocks_inherited_parent_ace() {
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{
        GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        AddAccessAllowedAceEx, EqualSid, GetAce, InitializeAcl, ACCESS_ALLOWED_ACE, ACE_HEADER,
        ACL, ACL_REVISION, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, OBJECT_INHERIT_ACE,
        PROTECTED_DACL_SECURITY_INFORMATION, SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::Storage::FileSystem::{DELETE, FILE_ALL_ACCESS};
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    let provisioned_parent = runtime_parent();
    let hostile_parent_path = provisioned_parent.join(format!(
        "r24-inherited-ace-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time is after epoch")
            .as_nanos()
    ));
    std::fs::create_dir(&hostile_parent_path).expect("create exact private test parent");
    let hostile_parent = InheritedAceParent(hostile_parent_path.clone());

    let mut process_token_raw = std::ptr::null_mut();
    assert_ne!(
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut process_token_raw,) },
        0,
        "open current process token for the controlled parent ACE"
    );
    let process_token = Handle::new(process_token_raw);
    let mut user_storage = [0u8; 256];
    read_token_user_sid(&process_token, &mut user_storage)
        .expect("read current user's SID for the inherited ACE fixture");
    let user_sid = unsafe { (*user_storage.as_ptr().cast::<TOKEN_USER>()).User.Sid };

    let mut acl_storage = [0u8; 512];
    let acl = acl_storage.as_mut_ptr().cast::<ACL>();
    assert_ne!(
        unsafe { InitializeAcl(acl, acl_storage.len() as u32, ACL_REVISION) },
        0,
        "initialize exact fixture DACL"
    );
    let inherited_flags = OBJECT_INHERIT_ACE | CONTAINER_INHERIT_ACE;
    assert_ne!(
        unsafe {
            AddAccessAllowedAceEx(
                acl,
                ACL_REVISION,
                inherited_flags,
                FILE_ALL_ACCESS,
                user_sid,
            )
        },
        0,
        "add a broad ACE inheritable by files and directories"
    );
    let hostile_parent_wide = wide(hostile_parent_path.as_os_str()).unwrap();
    assert_eq!(
        unsafe {
            SetNamedSecurityInfoW(
                hostile_parent_wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                acl,
                std::ptr::null_mut(),
            )
        },
        0,
        "install the exact known inheritable parent DACL"
    );

    let mut parent_dacl = std::ptr::null_mut();
    let mut parent_descriptor = std::ptr::null_mut();
    assert_eq!(
        unsafe {
            GetNamedSecurityInfoW(
                hostile_parent_wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut parent_dacl,
                std::ptr::null_mut(),
                &mut parent_descriptor,
            )
        },
        0,
        "read back the hostile parent DACL"
    );
    assert!(!parent_dacl.is_null());
    let parent_acl = unsafe { &*parent_dacl };
    assert_eq!(parent_acl.AceCount, 1, "fixture parent has exactly one ACE");
    let mut parent_ace = std::ptr::null_mut();
    assert_ne!(unsafe { GetAce(parent_dacl, 0, &mut parent_ace) }, 0);
    let parent_header = unsafe { &*parent_ace.cast::<ACE_HEADER>() };
    assert_eq!(
        parent_header.AceFlags, inherited_flags as u8,
        "fixture ACE is inheritable by both files and child directories"
    );
    let parent_allowed = unsafe { &*parent_ace.cast::<ACCESS_ALLOWED_ACE>() };
    assert_eq!(parent_allowed.Mask, FILE_ALL_ACCESS);
    let parent_ace_sid = (&parent_allowed.SidStart as *const u32).cast_mut().cast();
    assert_ne!(unsafe { EqualSid(user_sid, parent_ace_sid) }, 0);
    assert_eq!(
        unsafe { LocalFree(parent_descriptor.cast()) },
        std::ptr::null_mut()
    );

    let executable = std::env::current_exe().expect("current test executable exists");
    let args = child_test_args("runtime_child_marks_start_then_waits_for_job_termination");
    let borrowed_args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let capsule = test_capsule_body();
    let child = confined_spawn_with_test_key(
        &executable,
        &borrowed_args,
        &hostile_parent_path,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("protected scratch creation succeeds under an inheritable parent ACE");
    let mut cleanup_guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    let scratch_path = child._scratch.dir.clone();
    wait_for_marker(&marker);

    let mut scratch_dacl = std::ptr::null_mut();
    let mut scratch_descriptor = std::ptr::null_mut();
    let scratch_wide = wide(scratch_path.as_os_str()).unwrap();
    assert_eq!(
        unsafe {
            GetNamedSecurityInfoW(
                scratch_wide.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut scratch_dacl,
                std::ptr::null_mut(),
                &mut scratch_descriptor,
            )
        },
        0,
        "read back scratch DACL beneath the hostile parent"
    );
    assert!(!scratch_dacl.is_null());
    let mut descriptor_control = 0u16;
    let mut descriptor_revision = 0u32;
    assert_ne!(
        unsafe {
            windows_sys::Win32::Security::GetSecurityDescriptorControl(
                scratch_descriptor,
                &mut descriptor_control,
                &mut descriptor_revision,
            )
        },
        0
    );
    assert_ne!(descriptor_control & SE_DACL_PROTECTED, 0);
    let scratch_acl = unsafe { &*scratch_dacl };
    assert_eq!(
        scratch_acl.AceCount, 1,
        "inheritable broad parent ACE did not enter protected scratch DACL"
    );
    let mut scratch_ace = std::ptr::null_mut();
    assert_ne!(unsafe { GetAce(scratch_dacl, 0, &mut scratch_ace) }, 0);
    let scratch_header = unsafe { &*scratch_ace.cast::<ACE_HEADER>() };
    assert_eq!(
        scratch_header.AceFlags, 0,
        "scratch ACE is explicit, not inherited"
    );
    let scratch_allowed = unsafe { &*scratch_ace.cast::<ACCESS_ALLOWED_ACE>() };
    assert_eq!(
        scratch_allowed.Mask,
        windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_READ
            | windows_sys::Win32::Storage::FileSystem::FILE_GENERIC_WRITE
            | DELETE
    );
    let mut child_user_storage = [0u8; 256];
    read_token_user_sid(&child._token, &mut child_user_storage).unwrap();
    let child_user_sid = unsafe { (*child_user_storage.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let scratch_ace_sid = (&scratch_allowed.SidStart as *const u32).cast_mut().cast();
    assert_ne!(unsafe { EqualSid(child_user_sid, scratch_ace_sid) }, 0);
    assert_eq!(
        unsafe { LocalFree(scratch_descriptor.cast()) },
        std::ptr::null_mut()
    );

    assert_eq!(std::fs::read(&marker).unwrap(), b"started");
    std::fs::remove_file(&marker).expect("remove exact marker before scratch settlement");
    cleanup_guard.disarm();
    assert_eq!(
        settle(child, Duration::from_millis(100)).status,
        Settlement::Cancelled
    );
    assert!(!scratch_path.exists());
    std::fs::remove_dir(&hostile_parent.0).expect("remove exact hostile parent after settlement");
    assert_parent_empty(&provisioned_parent);
}

struct InheritedAceParent(PathBuf);

impl Drop for InheritedAceParent {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn wide_rejects_empty_oversized_and_interior_nul() {
    assert_eq!(wide(OsStr::new("")), Err(()));
    assert!(wide(OsStr::new("plain")).is_ok());
    let with_nul: std::ffi::OsString = {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&[u16::from(b'a'), 0, u16::from(b'b')])
    };
    assert_eq!(wide(&with_nul), Err(()));
    let oversized: std::ffi::OsString = {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&vec![u16::from(b'x'); MAX_WIDE])
    };
    assert_eq!(wide(&oversized), Err(()));
    let at_the_edge: std::ffi::OsString = {
        use std::os::windows::ffi::OsStringExt;
        std::ffi::OsString::from_wide(&vec![u16::from(b'x'); MAX_WIDE - 1])
    };
    assert!(wide(&at_the_edge).is_ok());
}

#[test]
fn wide_appends_exactly_one_terminating_nul() {
    let encoded = wide(OsStr::new("ok")).unwrap();
    assert_eq!(encoded.last(), Some(&0));
    assert_eq!(encoded.iter().filter(|unit| **unit == 0).count(), 1);
}

#[test]
fn forced_environment_pins_temp_and_tmp_to_the_scratch_dir_sorted_and_double_nul_terminated() {
    let scratch = Path::new(r"C:\scratch\dir");
    let block = forced_environment(scratch).unwrap();
    let text = String::from_utf16(&block[..block.len() - 1]).unwrap();
    let rows: Vec<&str> = text.trim_end_matches('\0').split('\0').collect();
    assert_eq!(rows, vec![r"TEMP=C:\scratch\dir", r"TMP=C:\scratch\dir"]);
    assert_eq!(&block[block.len() - 2..], &[0, 0]);
}

#[test]
fn denied_ui_limits_covers_every_documented_flag_exactly_once() {
    let expected = JOB_OBJECT_UILIMIT_HANDLES
        | JOB_OBJECT_UILIMIT_READCLIPBOARD
        | JOB_OBJECT_UILIMIT_WRITECLIPBOARD
        | JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS
        | JOB_OBJECT_UILIMIT_DESKTOP
        | JOB_OBJECT_UILIMIT_DISPLAYSETTINGS
        | JOB_OBJECT_UILIMIT_GLOBALATOMS
        | JOB_OBJECT_UILIMIT_EXITWINDOWS;
    assert_eq!(DENIED_UI_LIMITS, expected);
    // Each flag is a distinct bit: OR-ing them all must not lose any bit to
    // an accidental duplicate value.
    let bits = [
        JOB_OBJECT_UILIMIT_HANDLES,
        JOB_OBJECT_UILIMIT_READCLIPBOARD,
        JOB_OBJECT_UILIMIT_WRITECLIPBOARD,
        JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS,
        JOB_OBJECT_UILIMIT_DESKTOP,
        JOB_OBJECT_UILIMIT_DISPLAYSETTINGS,
        JOB_OBJECT_UILIMIT_GLOBALATOMS,
        JOB_OBJECT_UILIMIT_EXITWINDOWS,
    ];
    assert_eq!(
        bits.iter().fold(0u32, |acc, bit| acc | bit).count_ones() as usize,
        bits.len()
    );
}
