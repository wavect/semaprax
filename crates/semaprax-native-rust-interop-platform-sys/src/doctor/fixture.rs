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
            if scenario == "closed-descendant" {
                command.stdout(Stdio::null()).stderr(Stdio::null());
            }
            let child = command.spawn().unwrap();
            std::fs::write(invoked.with_extension("pid"), child.id().to_string()).unwrap();
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
    use std::path::Path;
    unsafe extern "C" {
        fn syscall(number: c_long, ...) -> c_long;
        fn close(fd: c_int) -> c_int;
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
    const ZERO: c_long = 0;
    const ONE: c_long = 1;
    const NEGATIVE_ONE: c_long = -1;

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
        // Create and close only: no bind, connect, send, DNS or external peer.
        for family in [ONE, 2, 10] {
            let fd = unsafe { syscall(SOCKET, family, ONE, ZERO) };
            assert!(
                fd >= 0,
                "unguarded socket control: {}",
                std::io::Error::last_os_error()
            );
            assert_eq!(unsafe { close(c_int::try_from(fd).unwrap()) }, 0);
        }
        let mut pair: [c_int; 2] = [-1; 2];
        assert_eq!(
            unsafe { syscall(SOCKETPAIR, ONE, ONE, ZERO, pair.as_mut_ptr()) },
            0
        );
        for fd in pair {
            assert_eq!(unsafe { close(fd) }, 0);
        }
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
        let mut pair: [c_int; 2] = [-1; 2];
        eperm(
            unsafe { syscall(SOCKETPAIR, ONE, ONE, ZERO, pair.as_mut_ptr()) },
            "socketpair",
        );
        assert_eq!(pair, [-1, -1]);
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
