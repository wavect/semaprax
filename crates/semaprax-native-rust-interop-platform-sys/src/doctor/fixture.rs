// Standalone trusted test executable; never linked into the library or CLI.
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

fn main() {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    #[cfg(all(
        target_os = "linux",
        target_pointer_width = "64",
        target_endian = "little",
        any(target_arch = "x86_64", target_arch = "aarch64")
    ))]
    if arguments
        .get(1)
        .is_some_and(|value| value == "--socket-child")
    {
        assert_eq!(arguments.len(), 2);
        linux_sockets::denied();
        println!("socket-child-ok");
        return;
    }
    if arguments
        .get(1)
        .is_some_and(|value| value == "--fixture-child")
    {
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt as _;

            assert_eq!(arguments.len(), 4);
            let lease = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .share_mode(0)
                .open(&arguments[2])
                .unwrap();
            std::fs::write(&arguments[3], std::process::id().to_string()).unwrap();
            std::hint::black_box(&lease);
            std::thread::sleep(Duration::from_secs(3));
            return;
        }
        #[cfg(not(windows))]
        assert_eq!(arguments.len(), 2);
        std::thread::sleep(Duration::from_secs(3));
        return;
    }
    assert_eq!(arguments.len(), 2);
    assert_eq!(arguments[1], "--version");
    let invoked = PathBuf::from(&arguments[0]);
    let scenario = invoked.file_stem().unwrap().to_str().unwrap();
    match scenario {
        "exact" => std::io::stdout().write_all(&vec![b'x'; 65_536]).unwrap(),
        "overflow" => {
            let _ = std::io::stdout().write_all(&vec![b'x'; 65_537]);
        }
        "stderr-overflow" => {
            let _ = std::io::stderr().write_all(&vec![b'x'; 65_537]);
        }
        "combined-overflow" => {
            let _ = std::io::stdout().write_all(&vec![b'x'; 32_768]);
            let _ = std::io::stderr().write_all(&vec![b'y'; 32_769]);
        }
        "sleep" => std::thread::sleep(Duration::from_secs(3)),
        "closed-sleep" => {
            close_output();
            std::thread::sleep(Duration::from_secs(3));
        }
        "nonzero" => std::process::exit(7),
        "invalid-utf8" => std::io::stdout().write_all(&[0xff]).unwrap(),
        "stdin-eof" => {
            let mut byte = [0_u8; 1];
            assert_eq!(std::io::stdin().read(&mut byte).unwrap(), 0);
            println!("rustc 1.88.0 (fixture)");
        }
        "descendant"
        | "closed-descendant"
        | "fault-kill-descendant"
        | "fault-settle-descendant"
        | "fault-close-descendant" => {
            let mut command = Command::new(&invoked);
            command.arg("--fixture-child").stdin(Stdio::null());
            #[cfg(windows)]
            command
                .arg(invoked.with_extension("lease"))
                .arg(invoked.with_extension("pid"));
            if scenario == "closed-descendant" {
                command.stdout(Stdio::null()).stderr(Stdio::null());
            }
            let mut child = command.spawn().unwrap();
            #[cfg(not(windows))]
            std::fs::write(invoked.with_extension("pid"), child.id().to_string()).unwrap();
            #[cfg(windows)]
            {
                let pid = invoked.with_extension("pid");
                let deadline = std::time::Instant::now() + Duration::from_secs(1);
                while !pid.exists() {
                    assert!(
                        child.try_wait().unwrap().is_none(),
                        "descendant exited before acquiring its lease"
                    );
                    assert!(
                        std::time::Instant::now() < deadline,
                        "descendant did not acquire its lease"
                    );
                    std::thread::yield_now();
                }
            }
            // Deliberately do not wait: the doctor owns descendant settlement.
            drop(child);
            println!("rustc 1.88.0 (fixture)");
        }
        "normal" => println!("rustc 1.88.0 (fixture)"),
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        "socket-control" => {
            linux_sockets::control();
            println!("socket-control-ok");
        }
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        "socket-guard" => {
            linux_sockets::denied();
            println!("socket-guard-ok");
        }
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        "socket-descendant" => {
            linux_sockets::denied();
            linux_sockets::exec_child(&invoked);
            println!("socket-parent-ok");
        }
        #[cfg(all(
            target_os = "linux",
            target_pointer_width = "64",
            target_endian = "little",
            any(target_arch = "x86_64", target_arch = "aarch64")
        ))]
        "socket-command-descendant" => {
            linux_sockets::denied();
            linux_sockets::command_child(&invoked);
            println!("socket-command-parent-ok");
        }
        #[cfg(target_os = "linux")]
        "socket-setup-sentinel" => {
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(invoked.with_extension("entered"))
                .unwrap()
                .write_all(b"executed\n")
                .unwrap();
            println!("socket-setup-executed");
        }
        #[cfg(target_os = "macos")]
        "closed-high-fd" => {
            unsafe extern "C" {
                fn fcntl(fd: i32, command: i32, ...) -> i32;
                fn __error() -> *mut i32;
            }
            unsafe {
                assert_eq!(fcntl(512, 1), -1); // F_GETFD
                assert_eq!(*__error(), 9); // EBADF
            }
            println!("rustc 1.88.0 (fixture)");
        }
        _ => panic!("unknown fixture scenario"),
    }
}

#[cfg(all(
    target_os = "linux",
    target_pointer_width = "64",
    target_endian = "little",
    any(target_arch = "x86_64", target_arch = "aarch64")
))]
mod linux_sockets {
    // Independent native syscall literals, not imported production policy.
    use std::ffi::{c_char, c_int, c_long, CString};
    use std::os::unix::ffi::OsStrExt as _;
    use std::os::unix::process::CommandExt as _;
    use std::path::Path;
    use std::process::Command;
    unsafe extern "C" {
        fn syscall(number: c_long, ...) -> c_long;
        fn close(fd: c_int) -> c_int;
        fn read(fd: c_int, buffer: *mut u8, count: usize) -> isize;
        fn write(fd: c_int, buffer: *const u8, count: usize) -> isize;
        fn prctl(option: c_int, ...) -> c_int;
        fn fork() -> c_int;
        fn execv(path: *const c_char, argv: *const *const c_char) -> c_int;
        fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
        fn _exit(status: c_int) -> !;
    }
    #[cfg(target_arch = "x86_64")]
    const SOCKET: c_long = 41;
    #[cfg(target_arch = "aarch64")]
    const SOCKET: c_long = 198;
    #[cfg(target_arch = "x86_64")]
    const SOCKETPAIR: c_long = 53;
    #[cfg(target_arch = "aarch64")]
    const SOCKETPAIR: c_long = 199;
    #[cfg(target_arch = "x86_64")]
    const PTRACE: c_long = 101;
    #[cfg(target_arch = "aarch64")]
    const PTRACE: c_long = 117;
    #[cfg(target_arch = "x86_64")]
    const PROCESS_VM_WRITEV: c_long = 311;
    #[cfg(target_arch = "aarch64")]
    const PROCESS_VM_WRITEV: c_long = 271;
    #[cfg(target_arch = "x86_64")]
    const NAMED_OPERATIONS: [c_long; 5] = [42, 49, 50, 43, 288];
    #[cfg(target_arch = "aarch64")]
    const NAMED_OPERATIONS: [c_long; 5] = [203, 200, 201, 202, 242];
    const ZERO: c_long = 0;
    const ONE: c_long = 1;
    const NEGATIVE_ONE: c_long = -1;

    pub fn command_child(path: &Path) {
        // A pre_exec callback disables the posix_spawn fast path. The callback
        // itself performs no allocation, locking, I/O or other operation.
        let mut command = Command::new(path);
        command.arg("--socket-child");
        unsafe {
            command.pre_exec(|| Ok(()));
        }
        let output = command.output().unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout, b"socket-child-ok\n");
        assert!(output.stderr.is_empty());

        // A failed exec must report ENOENT through the same real error channel,
        // not succeed merely because the child's successful-exec pipe reached EOF.
        let missing = path.with_extension("missing-child");
        assert_eq!(
            std::fs::symlink_metadata(&missing).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        let mut command = Command::new(missing);
        command.arg("--socket-child");
        unsafe {
            command.pre_exec(|| Ok(()));
        }
        assert_eq!(
            command.spawn().unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }

    fn pairs() {
        for kind in [ONE, 5] {
            for flags in [ZERO, 0x800, 0x80000, 0x80800] {
                let mut pair: [c_int; 2] = [-1; 2];
                assert_eq!(
                    unsafe { syscall(SOCKETPAIR, ONE, kind | flags, ZERO, pair.as_mut_ptr()) },
                    0
                );
                assert!(pair[0] >= 0 && pair[1] >= 0 && pair[0] != pair[1]);
                // Write before reading: the single byte fits even for NONBLOCK.
                for (sender, receiver, byte) in
                    [(pair[0], pair[1], 0x35_u8), (pair[1], pair[0], 0xa7_u8)]
                {
                    assert_eq!(unsafe { write(sender, &byte, 1) }, 1);
                    let mut received = 0_u8;
                    assert_eq!(unsafe { read(receiver, &mut received, 1) }, 1);
                    assert_eq!(received, byte);
                }
                for fd in pair {
                    assert_eq!(unsafe { close(fd) }, 0);
                }
            }
        }
    }

    pub fn exec_child(path: &Path) {
        // No Command fallback socketpair in this dedicated inheritance witness.
        // Existing Command-based descendant fixtures remain unchanged.
        let path = CString::new(path.as_os_str().as_bytes()).unwrap();
        let argument = CString::new("--socket-child").unwrap();
        let argv = [path.as_ptr(), argument.as_ptr(), std::ptr::null()];
        let pid = unsafe { fork() };
        assert!(pid >= 0);
        if pid == 0 {
            unsafe {
                execv(path.as_ptr(), argv.as_ptr());
                _exit(127);
            }
        }
        let mut status = 0;
        loop {
            let waited = unsafe { waitpid(pid, &mut status, 0) };
            if waited == pid {
                break;
            }
            assert_eq!(waited, -1);
            assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(4));
        }
        assert_eq!(status, 0);
    }

    pub fn control() {
        // Network sockets are created/closed only. Pair traffic below remains
        // between the two anonymous endpoints: no named or external peer.
        for family in [ONE, 2, 10] {
            let fd = unsafe { syscall(SOCKET, family, ONE, ZERO) };
            assert!(
                fd >= 0,
                "unguarded socket control: {}",
                std::io::Error::last_os_error()
            );
            assert_eq!(unsafe { close(c_int::try_from(fd).unwrap()) }, 0);
        }
        pairs();
    }

    fn eperm(result: c_long, operation: &str) {
        let error = std::io::Error::last_os_error().raw_os_error();
        assert_eq!(result, -1, "{operation}");
        assert_eq!(error, Some(1), "{operation}: expected EPERM");
    }

    pub fn denied() {
        assert_eq!(unsafe { prctl(39, ZERO, ZERO, ZERO, ZERO) }, 1); // PR_GET_NO_NEW_PRIVS
        assert_eq!(unsafe { prctl(21, ZERO, ZERO, ZERO, ZERO) }, 2); // PR_GET_SECCOMP, FILTER
        for family in [ONE, 2, 10, 16, 17] {
            eperm(unsafe { syscall(SOCKET, family, ONE, ZERO) }, "socket");
        }
        pairs();
        for (family, kind, protocol) in [
            (ONE, 2, ZERO), // DGRAM must not acquire named-message routing.
            (2, ONE, ZERO),
            (10, ONE, ZERO),
            (ONE, ONE, ONE),
            (ONE, ONE | 0x1000, ZERO),
            (ONE | (ONE << 32), ONE, ZERO),
            (ONE, ONE | (ONE << 32), ZERO),
            (ONE, ONE, ONE << 32),
        ] {
            let mut pair: [c_int; 2] = [-1; 2];
            eperm(
                unsafe { syscall(SOCKETPAIR, family, kind, protocol, pair.as_mut_ptr()) },
                "socketpair policy",
            );
            assert_eq!(pair, [-1, -1]);
        }
        // connect/bind/listen/accept/accept4 cannot turn an anonymous pair into
        // a named endpoint. Invalid FDs/pointers cause no connection if allowed.
        for operation in NAMED_OPERATIONS {
            eperm(
                unsafe { syscall(operation, NEGATIVE_ONE, ZERO, ZERO, ZERO) },
                "named socket operation",
            );
        }
        // Invalid descriptors/PIDs and zero lengths avoid authority over any
        // real process even if a regression accidentally allows these calls.
        eperm(unsafe { syscall(425, ZERO, ZERO) }, "io_uring_setup");
        eperm(
            unsafe { syscall(426, NEGATIVE_ONE, ZERO, ZERO, ZERO, ZERO, ZERO) },
            "io_uring_enter",
        );
        eperm(
            unsafe { syscall(427, NEGATIVE_ONE, ZERO, ZERO, ZERO) },
            "io_uring_register",
        );
        eperm(
            unsafe { syscall(438, NEGATIVE_ONE, NEGATIVE_ONE, ZERO) },
            "pidfd_getfd",
        );
        eperm(
            unsafe { syscall(PTRACE, 2 as c_long, NEGATIVE_ONE, ZERO, ZERO) },
            "ptrace",
        );
        eperm(
            unsafe {
                syscall(
                    PROCESS_VM_WRITEV,
                    NEGATIVE_ONE,
                    ZERO,
                    ZERO,
                    ZERO,
                    ZERO,
                    ZERO,
                )
            },
            "process_vm_writev",
        );
    }
}

#[cfg(unix)]
fn close_output() {
    unsafe extern "C" {
        fn close(fd: i32) -> i32;
    }
    unsafe {
        assert_eq!(close(1), 0);
        assert_eq!(close(2), 0);
    }
}

#[cfg(windows)]
fn close_output() {
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> *mut std::ffi::c_void;
        fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
    }
    unsafe {
        assert_ne!(CloseHandle(GetStdHandle(-11_i32 as u32)), 0);
        assert_ne!(CloseHandle(GetStdHandle(-12_i32 as u32)), 0);
    }
}
