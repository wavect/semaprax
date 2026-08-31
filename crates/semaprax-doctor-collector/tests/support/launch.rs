//! Trusted external provisioner. Never call an unsafe worker/collector entry in
//! the multithreaded test process. Raw clone3 + exec preserves exact parenthood.
use std::ffi::CString;
use std::fs::File;
use std::io::{self, Write};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub(super) fn provisioned_path(variable: &str) -> PathBuf {
    let path = PathBuf::from(std::env::var_os(variable).expect(variable));
    assert!(path.is_absolute(), "{variable} must be absolute");
    assert!(std::fs::metadata(&path).unwrap().is_file());
    path
}

pub(super) fn high(fd: i32) -> File {
    let duplicate = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 64) };
    assert!(duplicate >= 64, "{}", io::Error::last_os_error());
    unsafe { File::from_raw_fd(duplicate) }
}

pub(super) fn sealed(bytes: &[u8], executable: bool) -> File {
    // Executable-memfd permission is an explicit surrogate prerequisite. Never
    // retry without MFD_EXEC, change host configuration, or use a disk fallback.
    let flags =
        libc::MFD_CLOEXEC | libc::MFD_ALLOW_SEALING | if executable { libc::MFD_EXEC } else { 0 };
    let fd = unsafe { libc::memfd_create(c"collector-fixture".as_ptr(), flags) };
    assert!(fd >= 0, "{}", io::Error::last_os_error());
    let mut file = unsafe { File::from_raw_fd(fd) };
    file.write_all(bytes).unwrap();
    let seals = libc::F_SEAL_SEAL | libc::F_SEAL_SHRINK | libc::F_SEAL_GROW | libc::F_SEAL_WRITE;
    assert_eq!(unsafe { libc::fcntl(fd, libc::F_ADD_SEALS, seals) }, 0);
    high(fd)
}

// Before Command allocates its private exec-error pipe, occupy every unused
// fixed destination. The provisioner excludes competing descriptor mutators.
// Never overwrite a live descriptor; keep reservations until spawn completes.
pub(super) fn reserve_destinations(source: i32) -> Vec<File> {
    let mut reservations = Vec::with_capacity(6);
    loop {
        let fd = unsafe { libc::fcntl(source, libc::F_DUPFD_CLOEXEC, 3) };
        assert!(fd >= 3, "{}", io::Error::last_os_error());
        reservations.push(unsafe { File::from_raw_fd(fd) });
        if fd >= 8 {
            return reservations;
        }
    }
}

// This runs ONLY in pre_exec. The outer test process must never retain worker
// capture endpoints, even briefly across collector entry. Before clone, errors
// simply fail startup and std exits the provisioner child, closing its table.
pub(super) unsafe fn high_pipe() -> io::Result<[i32; 2]> {
    let mut descriptors = [-1; 2];
    unsafe {
        if libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) != 0 {
            return Err(io::Error::last_os_error());
        }
        let read = libc::fcntl(descriptors[0], libc::F_DUPFD_CLOEXEC, 64);
        let write = libc::fcntl(descriptors[1], libc::F_DUPFD_CLOEXEC, 64);
        if read < 64 || write < 64 {
            return Err(io::Error::last_os_error());
        }
        if libc::close(descriptors[0]) != 0 || libc::close(descriptors[1]) != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok([read, write])
    }
}

#[repr(C)]
#[derive(Default)]
struct CloneArgs {
    flags: u64,
    pidfd: u64,
    child_tid: u64,
    parent_tid: u64,
    exit_signal: u64,
    stack: u64,
    stack_size: u64,
    tls: u64,
    set_tid: u64,
    set_tid_size: u64,
    cgroup: u64,
}

/// `surrogate` is a trusted literal test program, NOT an approved worker image.
/// Its cases test collector mechanics, never executable provenance admission.
pub fn spawn(request: &[u8], bundle: &[u8], surrogate: Option<&[u8]>) -> Child {
    spawn_with_capacity(request, bundle, surrogate, None)
}

pub(super) fn spawn_with_capacity(
    request: &[u8],
    bundle: &[u8],
    surrogate: Option<&[u8]>,
    capacity: Option<i32>,
) -> Child {
    assert_eq!(
        std::env::var("SEMAPRAX_DOCTOR_WORKER_TEST_CONTEXT").as_deref(),
        Ok("private-mapped-user-mount-clean-worker-cgroup-v1")
    );
    let collector = provisioned_path("SEMAPRAX_DOCTOR_COLLECTOR");
    let worker = provisioned_path("SEMAPRAX_DOCTOR_WORKER");
    let worker = CString::new(worker.as_os_str().as_bytes()).unwrap();
    let executable = surrogate.map(|bytes| sealed(bytes, true));
    let executable_fd = executable.as_ref().map(AsRawFd::as_raw_fd);
    let request = sealed(request, false);
    let bundle = sealed(bundle, false);
    let request_fd = request.as_raw_fd();
    let bundle_fd = bundle.as_raw_fd();
    let _reservations = reserve_destinations(request_fd);
    let mut command = Command::new(collector);
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // SAFETY: after std's fork, only preallocated data and async-signal-safe
    // syscalls run. clone3 has private memory/table and invokes no atfork hooks.
    // All sources are >=64, including the newly created pidfd before remapping.
    // CLOEXEC preserves std Command's internal exec-error handshake until exec.
    // Startup, descriptor flushes and uncertainty remain provisioner-owned.
    unsafe {
        command.pre_exec(move || {
            // Configure the report pipe before any report bytes can exist;
            // shrinking after spawn could race a full pipe and yield EBUSY.
            if let Some(capacity) = capacity {
                if libc::fcntl(1, libc::F_SETPIPE_SZ, capacity) < 0 {
                    return Err(io::Error::last_os_error());
                }
            }
            let mut action: libc::sigaction = std::mem::zeroed();
            if libc::sigaction(libc::SIGCHLD, std::ptr::null(), &mut action) != 0
                || action.sa_sigaction != libc::SIG_DFL
                || action.sa_flags & libc::SA_NOCLDWAIT != 0
            {
                return Err(io::Error::from_raw_os_error(libc::EINVAL));
            }
            let input = high_pipe()?;
            let reply = high_pipe()?;
            let error = high_pipe()?;
            if libc::close(input[1]) != 0 {
                return Err(io::Error::last_os_error());
            }
            let worker_fds = [input[0], reply[1], error[1], request_fd, bundle_fd];
            let collector_fds = [request_fd, bundle_fd, reply[0], error[0]];
            let parent = libc::getpid();
            let mut pidfd = -1i32;
            let arguments = CloneArgs {
                flags: libc::CLONE_PIDFD as u64,
                pidfd: (&mut pidfd as *mut i32) as u64,
                exit_signal: libc::SIGCHLD as u64,
                ..CloneArgs::default()
            };
            let child = libc::syscall(
                libc::SYS_clone3,
                &arguments as *const CloneArgs,
                std::mem::size_of::<CloneArgs>(),
            );
            if child < 0 {
                return Err(io::Error::last_os_error());
            }
            if child == 0 {
                if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL, 0, 0, 0) != 0
                    || libc::getppid() != parent
                {
                    libc::_exit(126);
                }
                for (destination, source) in worker_fds.iter().enumerate() {
                    if libc::dup2(*source, destination as i32) != destination as i32 {
                        libc::_exit(126);
                    }
                }
                if libc::syscall(
                    libc::SYS_close_range,
                    5u32,
                    u32::MAX,
                    libc::CLOSE_RANGE_CLOEXEC,
                ) != 0
                {
                    libc::_exit(126);
                }
                let argv = [worker.as_ptr(), std::ptr::null()];
                let environment: [*const libc::c_char; 1] = [std::ptr::null()];
                if let Some(fd) = executable_fd {
                    // Sealed literal ET_EXEC has no interpreter; its CLOEXEC
                    // source remains live until successful executable entry.
                    libc::syscall(
                        libc::SYS_execveat,
                        fd,
                        c"".as_ptr(),
                        argv.as_ptr(),
                        environment.as_ptr(),
                        libc::AT_EMPTY_PATH,
                    );
                } else {
                    libc::execve(worker.as_ptr(), argv.as_ptr(), environment.as_ptr());
                }
                libc::_exit(126);
            }
            // clone3 may allocate pidfd 3/4; pin a high source before dup2.
            let high_pidfd = libc::fcntl(pidfd, libc::F_DUPFD_CLOEXEC, 64);
            if high_pidfd < 64 {
                let error = io::Error::last_os_error();
                settle_or_exit(child as libc::pid_t, pidfd);
                return Err(error);
            }
            let mappings = [
                (3, collector_fds[0]),
                (4, collector_fds[1]),
                (5, high_pidfd),
                (6, collector_fds[2]),
                (7, collector_fds[3]),
            ];
            for (destination, source) in mappings {
                if libc::dup2(source, destination) != destination {
                    let error = io::Error::last_os_error();
                    settle_or_exit(child as libc::pid_t, high_pidfd);
                    return Err(error);
                }
            }
            if libc::syscall(
                libc::SYS_close_range,
                8u32,
                u32::MAX,
                libc::CLOSE_RANGE_CLOEXEC,
            ) != 0
            {
                let error = io::Error::last_os_error();
                settle_or_exit(child as libc::pid_t, high_pidfd);
                return Err(error);
            }
            Ok(())
        });
    }
    // A failure of Command's final exec kills the worker through PDEATHSIG;
    // orphan reconciliation is still mandatory in the provisioner's cgroup.
    command
        .spawn()
        .expect("start provisioned collector; on startup uncertainty reconcile cgroup")
}

/// Called only in the provisioner pre_exec with exclusive ownership of a still
/// unreaped clone child. On uncertainty terminate; never return or retry fork.
unsafe fn settle_or_exit(child: libc::pid_t, pidfd: i32) {
    unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd,
            libc::SIGKILL,
            std::ptr::null::<libc::siginfo_t>(),
            0u32,
        );
        let mut start: libc::timespec = std::mem::zeroed();
        if libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut start) != 0 {
            libc::_exit(126);
        }
        loop {
            let result = libc::waitpid(child, std::ptr::null_mut(), libc::WNOHANG);
            if result == child {
                return;
            }
            if result < 0 {
                libc::_exit(126);
            }
            let mut now: libc::timespec = std::mem::zeroed();
            if libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut now) != 0
                || now.tv_sec - start.tv_sec > 5
                || (now.tv_sec - start.tv_sec == 5 && now.tv_nsec >= start.tv_nsec)
            {
                libc::_exit(126);
            }
            let pause = libc::timespec {
                tv_sec: 0,
                tv_nsec: 1_000_000,
            };
            libc::nanosleep(&pause, std::ptr::null_mut());
        }
    }
}
