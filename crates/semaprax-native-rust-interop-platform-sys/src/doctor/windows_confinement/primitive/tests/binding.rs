//! Native-only image binding regressions. Every case is explicitly selected
//! by the Windows gate; no capability absence becomes a passing skip.
use super::*;
use std::fs::OpenOptions;
use std::io::{Seek as _, SeekFrom, Write as _};
use std::os::windows::fs::OpenOptionsExt as _;
use std::os::windows::io::AsRawHandle as _;

struct Fixture {
    parent: PathBuf,
    images: PathBuf,
    executable: PathBuf,
    scratch: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let parent = runtime_parent();
        let images = parent.join("images");
        std::fs::create_dir(&images).unwrap();
        let executable = images.join("tool.exe");
        std::fs::copy(std::env::current_exe().unwrap(), &executable).unwrap();
        let scratch = parent.join("scratch");
        std::fs::create_dir(&scratch).unwrap();
        Self {
            parent,
            images,
            executable,
            scratch,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Remove only these exact owned files/directories. Foreign entries or
        // leaked handles remain visible as a fixture cleanup failure.
        std::fs::remove_file(&self.executable).unwrap();
        std::fs::remove_dir(&self.images).unwrap();
        std::fs::remove_dir(&self.scratch).unwrap();
        assert_parent_empty(&self.parent);
    }
}

fn verified(capsule: &TestCapsule) -> super::super::super::capsule::VerifiedCapsule {
    super::super::super::capsule::parse_windows_signed_with_key(
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .unwrap()
}

fn assert_refused(fixture: &Fixture, capsule: &TestCapsule, role: ImageRole) {
    let called = std::cell::Cell::new(false);
    let result = confined_spawn_after_binding(
        &fixture.executable,
        role,
        &[],
        &fixture.scratch,
        || Ok(verified(capsule)),
        || {
            called.set(true);
            panic!("rejected image reached the pre-spawn hook");
        },
    );
    assert_eq!(
        result.err(),
        Some(Refusal::Capsule(CapsuleError::ArtifactBinding))
    );
    assert!(!called.get(), "mismatch must not reach the launch boundary");
    assert_parent_empty(&fixture.scratch);
}

fn retained_writable_section(executable: &Path) -> Handle {
    use windows_sys::Win32::System::Memory::{CreateFileMappingW, PAGE_READWRITE};

    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(executable)
        .unwrap();
    // SAFETY: the owned fixture is a nonempty regular file and this test
    // retains only the hostile writable section after dropping its file handle.
    let mapping = unsafe {
        CreateFileMappingW(
            file.as_raw_handle(),
            std::ptr::null(),
            PAGE_READWRITE,
            0,
            0,
            std::ptr::null(),
        )
    };
    assert!(!mapping.is_null(), "create retained writable section");
    drop(file);
    Handle::new(mapping)
}

fn assert_refused_before_file_open(fixture: &Fixture, capsule: &TestCapsule) {
    let mut reached = Vec::with_capacity(5);
    let result = confined_spawn_observing(
        &fixture.executable,
        ImageRole::Worker,
        &[],
        &fixture.scratch,
        TEST_REQUEST_BYTES,
        TEST_BUNDLE_BYTES,
        || Ok(verified(capsule)),
        |boundary| {
            reached.push(boundary);
            assert_ne!(
                boundary,
                BindingBoundary::BeforeProcessCreation,
                "hostile image must not reach process creation"
            );
        },
    );
    assert_eq!(
        result.err(),
        Some(Refusal::Capsule(CapsuleError::ArtifactBinding))
    );
    assert!(
        reached.is_empty(),
        "initial no-write-sharing open must refuse before metadata or oplock acquisition; reached {reached:?}"
    );
    assert_parent_empty(&fixture.scratch);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_signed_image_mismatch_refuses_before_process_effects() {
    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    // The same authenticated capsule cannot authorize a different image slot.
    assert_refused(&fixture, &capsule, ImageRole::Collector);
    let baseline = current_process_handle_count();
    for _ in 0..4 {
        assert_refused(&fixture, &capsule, ImageRole::Launcher);
        assert_eq!(current_process_handle_count(), baseline);
    }
    // Preserve size, change content: a length-only check cannot pass this case.
    let mut file = OpenOptions::new()
        .write(true)
        .open(&fixture.executable)
        .unwrap();
    file.seek(SeekFrom::Start(0)).unwrap();
    file.write_all(b"NO").unwrap();
    drop(file);
    assert_refused(&fixture, &capsule, ImageRole::Worker);
    assert_eq!(current_process_handle_count(), baseline);
    // Independently exercise the signed length boundary.
    OpenOptions::new()
        .append(true)
        .open(&fixture.executable)
        .unwrap()
        .write_all(b"x")
        .unwrap();
    assert_refused(&fixture, &capsule, ImageRole::Worker);
    assert_eq!(current_process_handle_count(), baseline);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_signed_image_pins_leaf_and_ancestors_through_launch() {
    use windows_sys::Win32::System::Memory::{CreateFileMappingW, PAGE_READWRITE};

    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    let admitted =
        image::HeldImage::acquire(&fixture.executable, verified(&capsule).worker()).unwrap();
    // Same bytes at a different native pathname do not authenticate a process.
    assert!(!admitted.matches_process(unsafe { GetCurrentProcess() }));
    drop(admitted);
    let args = child_test_args("runtime_child_exits_with_nonzero_status");
    let args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let attempted = std::cell::Cell::new(false);
    let child = confined_spawn_after_binding(
        &fixture.executable,
        ImageRole::Worker,
        &args,
        &fixture.scratch,
        || Ok(verified(&capsule)),
        || {
            attempted.set(true);
            let late_alias = fixture.images.join("late-alias.exe");
            if std::fs::hard_link(&fixture.executable, &late_alias).is_ok() {
                // Keep the owned fixture removable before reporting the failed
                // exclusion. A successful post-binding hard link would make
                // the authenticated leaf multiply linked after admission.
                std::fs::remove_file(&late_alias).unwrap();
                panic!("held image admitted a post-binding hard-link substitution");
            }
            assert!(
                std::fs::rename(&fixture.executable, fixture.images.join("moved.exe")).is_err()
            );
            assert!(std::fs::rename(&fixture.images, fixture.parent.join("moved-images")).is_err());
            assert!(std::fs::remove_file(&fixture.executable).is_err());
            assert!(
                OpenOptions::new()
                    .write(true)
                    .open(&fixture.executable)
                    .is_err()
            );
            let reader = OpenOptions::new()
                .read(true)
                .open(&fixture.executable)
                .unwrap();
            // SAFETY: `reader` is a live read-only handle. The requested
            // writable section must fail: the held-image sharing guard denied
            // a writer, and this new handle never held write access.
            let mapping = unsafe {
                CreateFileMappingW(
                    reader.as_raw_handle(),
                    std::ptr::null(),
                    PAGE_READWRITE,
                    0,
                    0,
                    std::ptr::null(),
                )
            };
            if !mapping.is_null() {
                drop(Handle::new(mapping));
                panic!("post-binding read handle unexpectedly created a writable section");
            }
        },
    )
    .expect("authenticated held image launches after every substitution is denied");
    assert!(attempted.get());
    let mut guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    wait_for_marker(&marker);
    assert_eq!(std::fs::read(&marker).unwrap(), b"exit-37");
    std::fs::remove_file(&marker).unwrap();
    guard.disarm();
    assert_eq!(
        settle(child, Duration::from_secs(30)).status,
        Settlement::Failed(FailureReason::ExitCode(37))
    );
    assert_parent_empty(&fixture.scratch);
    // Settlement releases the namespace guards, not just the process handle.
    let moved = fixture.parent.join("moved-images");
    std::fs::rename(&fixture.images, &moved).unwrap();
    std::fs::rename(&moved, &fixture.images).unwrap();
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_signed_image_refuses_preexisting_writer_and_hardlink() {
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_DELETE, FILE_SHARE_WRITE};
    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    let writer = OpenOptions::new()
        .write(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(&fixture.executable)
        .unwrap();
    assert_refused(&fixture, &capsule, ImageRole::Worker);
    drop(writer);
    let alias = fixture.images.join("alias.exe");
    std::fs::hard_link(&fixture.executable, &alias).unwrap();
    assert_refused(&fixture, &capsule, ImageRole::Worker);
    std::fs::remove_file(alias).unwrap();
    // Prove the fixture and NTFS/oplock prerequisites can actually succeed.
    drop(image::HeldImage::acquire(&fixture.executable, verified(&capsule).worker()).unwrap());
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_signed_image_refuses_writable_mapping_after_writer_closes() {
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
        PAGE_READWRITE, UnmapViewOfFile,
    };
    struct View(MEMORY_MAPPED_VIEW_ADDRESS);
    impl Drop for View {
        fn drop(&mut self) {
            // SAFETY: this view is exclusively owned and is unmapped once.
            assert_ne!(unsafe { UnmapViewOfFile(self.0) }, 0);
        }
    }
    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fixture.executable)
        .unwrap();
    // SAFETY: live writable file, anonymous mapping, existing file size.
    let mapping = unsafe {
        CreateFileMappingW(
            file.as_raw_handle(),
            std::ptr::null(),
            PAGE_READWRITE,
            0,
            0,
            std::ptr::null(),
        )
    };
    assert!(!mapping.is_null());
    let mapping = Handle::new(mapping);
    // SAFETY: map a writable view of the owned mapping.
    let view = unsafe { MapViewOfFile(mapping.raw(), FILE_MAP_WRITE, 0, 0, 0) };
    assert!(!view.Value.is_null());
    let view = View(view);
    drop(file);
    drop(mapping);
    // A view survives closure of both handles. Require the initial open itself
    // to refuse: a later oplock refusal cannot establish sharing exclusion.
    let baseline = current_process_handle_count();
    for _ in 0..4 {
        assert_refused_before_file_open(&fixture, &capsule);
        assert_eq!(current_process_handle_count(), baseline);
    }
    drop(view);
    drop(image::HeldImage::acquire(&fixture.executable, verified(&capsule).worker()).unwrap());
    assert_eq!(current_process_handle_count(), baseline);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_dropped_child_releases_image_and_process_handles() {
    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    let args = child_test_args("runtime_child_marks_start_then_waits_for_job_termination");
    let args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    // Warm the image/oplock path before exact handle accounting.
    drop(image::HeldImage::acquire(&fixture.executable, verified(&capsule).worker()).unwrap());
    let baseline = current_process_handle_count();
    let child = confined_spawn_with_test_key(
        &fixture.executable,
        &args,
        &fixture.scratch,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .unwrap();
    let mut guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    wait_for_marker(&marker);
    std::fs::remove_file(marker).unwrap();
    guard.disarm();
    drop(child);
    assert_eq!(current_process_handle_count(), baseline);
    assert_parent_empty(&fixture.scratch);
    OpenOptions::new()
        .write(true)
        .open(&fixture.executable)
        .unwrap();
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_signed_image_refuses_retained_writable_section_without_view() {
    use std::io::Read as _;
    use windows_sys::Win32::System::Memory::{
        CreateFileMappingW, FILE_MAP_WRITE, FlushViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS,
        MapViewOfFile, PAGE_READWRITE, UnmapViewOfFile,
    };
    struct View(MEMORY_MAPPED_VIEW_ADDRESS);
    impl Drop for View {
        fn drop(&mut self) {
            // SAFETY: this wrapper owns one successfully mapped view.
            assert_ne!(unsafe { UnmapViewOfFile(self.0) }, 0);
        }
    }
    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fixture.executable)
        .unwrap();
    // SAFETY: live writable file, unnamed section, existing file size. Do not
    // create a view: the hostile capability is a retained section handle that
    // could acquire a writable view after file-handle admission.
    let mapping = unsafe {
        CreateFileMappingW(
            file.as_raw_handle(),
            std::ptr::null(),
            PAGE_READWRITE,
            0,
            0,
            std::ptr::null(),
        )
    };
    assert!(!mapping.is_null());
    let mapping = Handle::new(mapping);
    drop(file);
    let baseline = current_process_handle_count();
    assert_refused_before_file_open(&fixture, &capsule);
    assert_eq!(current_process_handle_count(), baseline);
    // Prove the no-view capability was real: after the guard refuses, the
    // retained section must be able to create a writable view and change the
    // exact file bytes. Otherwise this case could pass without exercising the
    // section-mutation gap it is intended to witness.
    let view = unsafe { MapViewOfFile(mapping.raw(), FILE_MAP_WRITE, 0, 0, 0) };
    assert!(
        !view.Value.is_null(),
        "retained section cannot map writable"
    );
    let view = View(view);
    let last = usize::try_from(std::fs::metadata(&fixture.executable).unwrap().len()).unwrap() - 1;
    // SAFETY: a zero-length MapViewOfFile request maps the entire existing
    // file, and `last` is its final valid byte. The view is exclusively owned
    // by this test while the hostile byte is changed.
    let byte = unsafe { view.0.Value.cast::<u8>().add(last) };
    let original = unsafe { byte.read() };
    unsafe { byte.write(original ^ 1) };
    assert_ne!(unsafe { FlushViewOfFile(view.0.Value, 0) }, 0);
    drop(view);
    drop(mapping);
    // ReadFile and mapped views are not guaranteed coherent while the view
    // is live; observe the changed file only after closing the hostile view.
    let mut reader = OpenOptions::new()
        .read(true)
        .open(&fixture.executable)
        .unwrap();
    reader.seek(SeekFrom::Start(last as u64)).unwrap();
    let mut observed = [0];
    reader.read_exact(&mut observed).unwrap();
    assert_eq!(
        observed[0],
        original ^ 1,
        "hostile mapped write did not change the signed file"
    );
    drop(reader);
    let mut writer = OpenOptions::new()
        .write(true)
        .open(&fixture.executable)
        .unwrap();
    writer.seek(SeekFrom::Start(last as u64)).unwrap();
    writer.write_all(&[original]).unwrap();
    writer.sync_all().unwrap();
    drop(writer);
    let mut reader = OpenOptions::new()
        .read(true)
        .open(&fixture.executable)
        .unwrap();
    reader.seek(SeekFrom::Start(last as u64)).unwrap();
    reader.read_exact(&mut observed).unwrap();
    assert_eq!(observed[0], original);
    drop(reader);
    // This success control rejects an unavailable-oplock fixture as a failure.
    drop(image::HeldImage::acquire(&fixture.executable, verified(&capsule).worker()).unwrap());
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_retained_writable_section_refuses_before_every_launch_boundary() {
    use std::io::Read as _;
    use windows_sys::Win32::System::Memory::{
        FILE_MAP_WRITE, FlushViewOfFile, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile, UnmapViewOfFile,
    };
    struct View(MEMORY_MAPPED_VIEW_ADDRESS);
    impl Drop for View {
        fn drop(&mut self) {
            // SAFETY: this wrapper owns one successfully mapped view.
            assert_ne!(unsafe { UnmapViewOfFile(self.0) }, 0);
        }
    }

    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    let mapping = retained_writable_section(&fixture.executable);
    let mut reached = Vec::with_capacity(5);
    let result = confined_spawn_observing(
        &fixture.executable,
        ImageRole::Worker,
        &[],
        &fixture.scratch,
        TEST_REQUEST_BYTES,
        TEST_BUNDLE_BYTES,
        || Ok(verified(&capsule)),
        |boundary| reached.push(boundary),
    );
    assert_eq!(
        result.err(),
        Some(Refusal::Capsule(CapsuleError::ArtifactBinding))
    );
    assert!(
        reached.is_empty(),
        "a retained writable section must refuse at the initial file open before every launch boundary; reached {reached:?}"
    );
    assert_parent_empty(&fixture.scratch);

    // The retained no-view section remains a real hostile capability after
    // refusal. Map it, change one signed byte, and restore the fixture before
    // requiring the normal admission success control.
    let view = unsafe { MapViewOfFile(mapping.raw(), FILE_MAP_WRITE, 0, 0, 0) };
    assert!(
        !view.Value.is_null(),
        "retained section maps writable after refusal"
    );
    let view = View(view);
    let last = usize::try_from(std::fs::metadata(&fixture.executable).unwrap().len()).unwrap() - 1;
    // SAFETY: the complete nonempty file is mapped and this is its final byte.
    let byte = unsafe { view.0.Value.cast::<u8>().add(last) };
    let original = unsafe { byte.read() };
    unsafe { byte.write(original ^ 1) };
    assert_ne!(unsafe { FlushViewOfFile(view.0.Value, 0) }, 0);
    drop(view);
    drop(mapping);
    let mut reader = OpenOptions::new()
        .read(true)
        .open(&fixture.executable)
        .unwrap();
    reader.seek(SeekFrom::Start(last as u64)).unwrap();
    let mut observed = [0];
    reader.read_exact(&mut observed).unwrap();
    assert_eq!(
        observed,
        [original ^ 1],
        "hostile mapped write changed the image"
    );
    drop(reader);
    let mut writer = OpenOptions::new()
        .write(true)
        .open(&fixture.executable)
        .unwrap();
    writer.seek(SeekFrom::Start(last as u64)).unwrap();
    writer.write_all(&[original]).unwrap();
    writer.sync_all().unwrap();
    drop(writer);
    drop(image::HeldImage::acquire(&fixture.executable, verified(&capsule).worker()).unwrap());
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_signed_image_reaches_every_launch_boundary() {
    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    let args = child_test_args("runtime_child_exits_with_nonzero_status");
    let args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let mut reached = Vec::with_capacity(5);
    let child = confined_spawn_observing(
        &fixture.executable,
        ImageRole::Worker,
        &args,
        &fixture.scratch,
        TEST_REQUEST_BYTES,
        TEST_BUNDLE_BYTES,
        || Ok(verified(&capsule)),
        |boundary| reached.push(boundary),
    )
    .expect("clean signed image reaches every bounded launch checkpoint");
    assert_eq!(
        reached,
        [
            BindingBoundary::Image(image::ImageBindingBoundary::FileOpened),
            BindingBoundary::Image(image::ImageBindingBoundary::GuardAcquired),
            BindingBoundary::Image(image::ImageBindingBoundary::DigestVerified),
            BindingBoundary::BeforeProcessCreation,
            BindingBoundary::SuspendedLeader,
        ]
    );
    let mut guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    wait_for_marker(&marker);
    assert_eq!(std::fs::read(&marker).unwrap(), b"exit-37");
    std::fs::remove_file(marker).unwrap();
    guard.disarm();
    assert_eq!(
        settle(child, Duration::from_secs(30)).status,
        Settlement::Failed(FailureReason::ExitCode(37))
    );
    assert_parent_empty(&fixture.scratch);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_retained_writable_section_refusals_settle_handles_and_scratch() {
    use windows_sys::Win32::System::Memory::{CreateFileMappingW, PAGE_READWRITE};

    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    let before_mapping = current_process_handle_count();
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&fixture.executable)
        .unwrap();
    // Keep only the hostile section capability. This deliberately does not
    // claim to exclude a later writable view or mutation; it checks that each
    // refusal settles the primitive's own temporary handles and scratch state.
    let mapping = unsafe {
        CreateFileMappingW(
            file.as_raw_handle(),
            std::ptr::null(),
            PAGE_READWRITE,
            0,
            0,
            std::ptr::null(),
        )
    };
    assert!(!mapping.is_null());
    let mapping = Handle::new(mapping);
    drop(file);

    let baseline = current_process_handle_count();
    for _ in 0..4 {
        assert_refused(&fixture, &capsule, ImageRole::Worker);
        assert_eq!(
            current_process_handle_count(),
            baseline,
            "retained hostile section leaves no confinement setup handles behind"
        );
        assert_parent_empty(&fixture.scratch);
    }
    drop(mapping);
    drop(image::HeldImage::acquire(&fixture.executable, verified(&capsule).worker()).unwrap());
    assert_eq!(current_process_handle_count(), before_mapping);
}

#[test]
#[ignore = "requires the explicitly provisioned Windows runtime gate"]
fn windows_runtime_child_inherits_only_declared_standard_handles() {
    use windows_sys::Win32::Foundation::{HANDLE_FLAG_INHERIT, SetHandleInformation};
    use windows_sys::Win32::Storage::FileSystem::FILE_FLAG_DELETE_ON_CLOSE;

    let fixture = Fixture::new();
    let capsule = test_capsule_body();
    // The confined child starts in a new directory beneath `fixture.scratch`.
    // Its parent-directory probe must name this exact sentinel.
    let sentinel_path = fixture.scratch.join(INHERITABLE_SENTINEL);
    let sentinel = OpenOptions::new()
        .write(true)
        .create_new(true)
        .custom_flags(FILE_FLAG_DELETE_ON_CLOSE)
        .open(&sentinel_path)
        .unwrap();
    assert_ne!(
        unsafe {
            SetHandleInformation(
                sentinel.as_raw_handle(),
                HANDLE_FLAG_INHERIT,
                HANDLE_FLAG_INHERIT,
            )
        },
        0,
        "make the unrelated delete-on-close sentinel inheritable in the parent"
    );
    let args = child_test_args("runtime_child_checks_unrelated_inheritable_handle_is_absent");
    let args: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
    let child = confined_spawn_with_test_key(
        &fixture.executable,
        &args,
        &fixture.scratch,
        &capsule.bytes,
        &capsule.public_key_hex,
    )
    .expect("authenticated child starts with the exact standard-handle inventory");
    let mut guard = RuntimeChildCleanupGuard::new(&child);
    let marker = child._scratch.dir.join(TEST_MARKER);
    wait_for_marker(&marker);
    assert_eq!(
        std::fs::read(&marker).unwrap(),
        b"ready-for-handle-inventory"
    );
    std::fs::remove_file(&marker).unwrap();
    drop(sentinel);
    assert!(
        !sentinel_path.exists(),
        "parent sentinel closes before the child is allowed to observe it"
    );
    std::fs::File::create_new(child._scratch.dir.join(HANDLE_INVENTORY_PERMIT)).unwrap();
    wait_for_marker(&marker);
    assert_eq!(
        std::fs::read(&marker).unwrap(),
        b"unrelated-inheritable-handle-absent"
    );
    std::fs::remove_file(&marker).unwrap();
    std::fs::remove_file(child._scratch.dir.join(HANDLE_INVENTORY_PERMIT)).unwrap();
    guard.disarm();
    assert_eq!(
        settle(child, Duration::from_secs(30)).status,
        Settlement::Completed
    );
    assert_parent_empty(&fixture.scratch);
}
