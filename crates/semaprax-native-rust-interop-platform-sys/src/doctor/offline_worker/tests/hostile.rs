//! Executed syscall oracles require the real provisioned worker. Denial alone
//! cannot distinguish its filter from a stricter outer policy or prove cap state.
use super::native::{Arg, Expected, Program, Syscall};
use super::{bundle, request, run, wire, SELECTOR};
use std::fs::{self, File};
use std::io::Write;
use std::os::unix::{ffi::OsStrExt, fs::MetadataExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

const ZERO: Arg = Arg::Constant(0);
const DIR: Arg = Arg::Constant((-100_i64) as u64); // AT_FDCWD
const DENIED: Expected = Expected::Return(-1); // raw -EPERM

fn observe(program: Program, label: &str) {
    let marker = format!("worker-hostile:{label}\n").into_bytes();
    let bundle = bundle(&program.finish(&marker));
    let request = request(&bundle, 1, SELECTOR);
    let (status, output, errors) = run(&request, &bundle);
    assert!(status.success(), "{label}: {status:?}");
    assert!(errors.is_empty(), "{label}: {errors:?}");
    assert_eq!(
        wire::validate_reply(&wire::Request::parse(&request).unwrap(), &output).unwrap(),
        vec![(1, Ok(marker))],
        "exact executed oracle: {label}"
    );
}

#[test]
#[ignore = "requires provisioned worker/private mapped namespaces/cgroup; run serially"]
fn provisioned_capability_operations_and_process_creation_are_denied() {
    observe(Program::new(), "marker-control");
    for (call, label) in [
        (Syscall::Capget, "capget-denied"),
        (Syscall::Capset, "capset-denied"),
    ] {
        let mut program = Program::new();
        // A valid v3 header and a full 24-byte capability array. capget would
        // write valid stack storage if allowed; capset requests zero rights.
        let header = program.data(&[0x22, 0x05, 0x08, 0x20, 0, 0, 0, 0]);
        let data = if matches!(call, Syscall::Capget) {
            Arg::Stack(0)
        } else {
            program.data(&[0; 24])
        };
        program.call(call, &[header, data], DENIED);
        observe(program, label);
    }
    for (label, arguments) in [
        ("securebits-change-denied", [28, 0, 0]),
        ("pdeathsig-clear-denied", [1, 0, 0]),
        ("keepcaps-change-denied", [8, 1, 0]),
        ("ambient-raise-denied", [47, 2, 21]),
        ("nnp-clear-denied", [38, 0, 0]),
    ] {
        let mut program = Program::new();
        program.call(Syscall::Prctl, &arguments.map(Arg::Constant), DENIED);
        observe(program, label);
    }
    let mut program = Program::new();
    // clone(SIGCHLD, NULL, NULL, NULL, 0): no shared VM, thread, stack or TLS.
    // If erroneously allowed, BOTH the child return 0 and parent positive PID
    // immediately fail the -EPERM comparison and exit7, before any later call.
    // No repeated spawn, uncontrolled callback or child success marker exists.
    program.call(
        Syscall::Clone,
        &[Arg::Constant(17), ZERO, ZERO, ZERO, ZERO],
        DENIED,
    );
    observe(program, "clone-denied");
}

#[test]
#[ignore = "requires provisioned worker/private mapped namespaces/cgroup; run serially"]
fn provisioned_stdin_is_eof_and_nonstandard_descriptors_are_closed() {
    let mut program = Program::new();
    program.call(
        Syscall::Read,
        &[ZERO, Arg::Stack(0), Arg::Constant(1)],
        Expected::Return(0),
    );
    // read is admitted by the guard. EBADF, unlike EPERM, observes absent FDs.
    for fd in [3, 4, 5, 63, 64, 1024] {
        program.call(
            Syscall::Read,
            &[Arg::Constant(fd), Arg::Stack(0), Arg::Constant(1)],
            Expected::Return(-9),
        );
    }
    observe(program, "stdin-eof-fds-closed");
}

struct Outside {
    directory: PathBuf,
    path: PathBuf,
    file: File,
    directory_identity: (u64, u64),
}

impl Outside {
    fn create() -> Self {
        static SERIAL: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-worker-outside-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&directory).unwrap();
        let metadata = fs::symlink_metadata(&directory).unwrap();
        let path = directory.join("sentinel");
        let mut file = File::options()
            .write(true)
            .read(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(b"outside-worker-root\n").unwrap();
        Self {
            directory,
            path,
            file,
            directory_identity: (metadata.dev(), metadata.ino()),
        }
    }

    fn verify(&self) {
        let held = self.file.metadata().unwrap();
        let path = fs::symlink_metadata(&self.path).unwrap();
        assert!(path.is_file() && !path.file_type().is_symlink());
        assert_eq!((path.dev(), path.ino()), (held.dev(), held.ino()));
        assert_eq!(fs::read(&self.path).unwrap(), b"outside-worker-root\n");
    }
}

impl Drop for Outside {
    fn drop(&mut self) {
        // Fixed inventory only, no recursive cleanup. Drift retains evidence;
        // the externally provisioned driver context excludes hostile mutators.
        let cleanup = || -> std::io::Result<()> {
            let directory = fs::symlink_metadata(&self.directory)?;
            let path = fs::symlink_metadata(&self.path)?;
            let held = self.file.metadata()?;
            let entries = fs::read_dir(&self.directory)?
                .map(|entry| entry.map(|entry| entry.file_name()))
                .collect::<std::io::Result<Vec<_>>>()?;
            if !directory.is_dir()
                || directory.file_type().is_symlink()
                || (directory.dev(), directory.ino()) != self.directory_identity
                || !path.is_file()
                || path.file_type().is_symlink()
                || (path.dev(), path.ino()) != (held.dev(), held.ino())
                || entries != [std::ffi::OsString::from("sentinel")]
            {
                return Err(std::io::Error::other(
                    "outside fixture identity/inventory drift",
                ));
            }
            fs::remove_file(&self.path)?;
            fs::remove_dir(&self.directory)
        };
        if let Err(error) = cleanup() {
            if std::thread::panicking() {
                eprintln!(
                    "outside fixture cleanup stopped at {}: {error}",
                    self.directory.display()
                );
            } else {
                panic!(
                    "outside fixture cleanup stopped at {}: {error}",
                    self.directory.display()
                );
            }
        }
    }
}

#[test]
#[ignore = "requires provisioned worker/private mapped namespaces/cgroup; run serially"]
fn provisioned_root_hides_real_outside_file_and_rejects_write_opens() {
    let _worker = super::worker();
    let outside = Outside::create();
    outside.verify();
    for (path, label) in [
        (b"/bin/clang\0".as_slice(), "bundled-read-control"),
        (b"/../../bin/clang\0", "bundled-traversal-control"),
    ] {
        let mut program = Program::new();
        let path = program.data(path);
        program.call(Syscall::Openat, &[DIR, path, ZERO], Expected::OpenedFd);
        program.call(
            Syscall::Read,
            &[Arg::SavedFd, Arg::Stack(0), Arg::Constant(4)],
            Expected::Return(4),
        );
        program.stack_bytes(b"\x7fELF");
        program.call(Syscall::Close, &[Arg::SavedFd], Expected::Return(0));
        observe(program, label);
    }
    let path_bytes = outside.path.as_os_str().as_bytes();
    assert_eq!(path_bytes.first(), Some(&b'/'));
    let mut absolute = path_bytes.to_vec();
    absolute.push(0);
    let mut traversal = b"/../../".to_vec();
    traversal.extend_from_slice(&path_bytes[1..]);
    traversal.push(0);
    for (path, label) in [
        (absolute, "outside-absolute-absent"),
        (traversal, "outside-traversal-absent"),
    ] {
        let mut program = Program::new();
        let path = program.data(&path);
        program.call(Syscall::Openat, &[DIR, path, ZERO], Expected::Return(-2)); // ENOENT
        observe(program, label);
        outside.verify();
    }
    for (path, flags, label) in [
        (b"/bin/clang\0".as_slice(), 1, "bundled-write-denied"),
        (b"/forbidden-new\0", 1 | 64 | 512, "create-truncate-denied"),
    ] {
        let mut program = Program::new();
        let path = program.data(path);
        program.call(
            Syscall::Openat,
            &[DIR, path, Arg::Constant(flags), Arg::Constant(0o600)],
            DENIED,
        );
        observe(program, label);
        outside.verify();
    }
}
