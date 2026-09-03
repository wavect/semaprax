//! Quarantined Unix process boundary for held Git execution.
//!
//! Every caller and all repository policy remain safe Rust. This module is the
//! single exception because Linux `fexecve` and Darwin's suspended
//! `posix_spawn`/vnode inspection have no safe standard-library interface.
#![allow(unsafe_code)]

use libc::{c_char, c_int};
use std::ffi::CString;
use std::fs::{File, Metadata};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
#[cfg(target_os = "macos")]
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::time::{Duration, Instant};

const POLL_SLICE_MS: c_int = 25;
const SETTLEMENT_LIMIT: Duration = Duration::from_secs(30);
pub(super) const SUPPORTED: bool = true;

#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub(super) stdout: usize,
    pub(super) stderr: usize,
    pub(super) deadline: Instant,
}

struct Fd(Option<c_int>);

impl Fd {
    fn new(raw: c_int) -> Self {
        Self(Some(raw))
    }

    fn raw(&self) -> c_int {
        self.0.expect("owned descriptor")
    }

    fn close(mut self) -> io::Result<()> {
        let Some(raw) = self.0.take() else {
            return Ok(());
        };
        if unsafe { libc::close(raw) } == 0 {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }
}

impl Drop for Fd {
    fn drop(&mut self) {
        if let Some(raw) = self.0.take() {
            if unsafe { libc::close(raw) } != 0 {
                // A close error leaves descriptor ownership uncertain. There
                // is no safe ordinary continuation at this authority boundary.
                std::process::abort();
            }
        }
    }
}

struct Pipe {
    read: Fd,
    write: Fd,
}

struct ChildIo<'a> {
    stdin: &'a Pipe,
    stdout: &'a Pipe,
    stderr: &'a Pipe,
}

fn pipe() -> io::Result<Pipe> {
    let mut descriptors = [-1; 2];
    #[cfg(target_os = "linux")]
    let opened = unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) };
    #[cfg(target_os = "macos")]
    let opened = unsafe { libc::pipe(descriptors.as_mut_ptr()) };
    if opened != 0 {
        return Err(io::Error::last_os_error());
    }
    let pipe = Pipe {
        read: Fd::new(descriptors[0]),
        write: Fd::new(descriptors[1]),
    };
    #[cfg(target_os = "macos")]
    for descriptor in [pipe.read.raw(), pipe.write.raw()] {
        if unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) } != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(pipe)
}

fn arguments(executable: &Path, command: &[&str]) -> io::Result<Vec<CString>> {
    let mut result = Vec::with_capacity(command.len() + 16);
    result.push(
        CString::new(executable.as_os_str().as_bytes())
            .map_err(|_| io::Error::other("Git executable path contains NUL"))?,
    );
    for argument in [
        "--git-dir=.",
        "--no-replace-objects",
        "--no-optional-locks",
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "core.fsmonitor=false",
        "-c",
        "core.commitGraph=false",
        "-c",
        "core.attributesFile=/dev/null",
        "-c",
        "credential.helper=",
        "-c",
        "protocol.allow=never",
    ] {
        result.push(CString::new(argument).expect("fixed Git argument has no NUL"));
    }
    for argument in command {
        result.push(
            CString::new(*argument)
                .map_err(|_| io::Error::other("Git command argument contains NUL"))?,
        );
    }
    Ok(result)
}

fn environment() -> Vec<CString> {
    [
        "GIT_ATTR_NOSYSTEM=1",
        "GIT_CONFIG_GLOBAL=/dev/null",
        "GIT_CONFIG_NOSYSTEM=1",
        "GIT_CONFIG_SYSTEM=/dev/null",
        "GIT_NO_LAZY_FETCH=1",
        "GIT_NO_REPLACE_OBJECTS=1",
        "GIT_OPTIONAL_LOCKS=0",
        "GIT_TERMINAL_PROMPT=0",
        "LC_ALL=C",
    ]
    .into_iter()
    .map(|value| CString::new(value).expect("fixed Git environment has no NUL"))
    .collect()
}

pub(super) fn run(
    executable_path: &Path,
    executable: &File,
    executable_metadata: &Metadata,
    repository: &File,
    command: &[&str],
    input: &[u8],
    limits: Limits,
) -> io::Result<(i32, Vec<u8>)> {
    if Instant::now() >= limits.deadline {
        return Err(io::Error::other("Git host deadline exceeded"));
    }
    let arguments = arguments(executable_path, command)?;
    run_arguments(
        executable,
        executable_metadata,
        repository,
        &arguments,
        input,
        limits,
    )
}

#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(super) fn run_for_test(
    executable_path: &Path,
    executable: &File,
    executable_metadata: &Metadata,
    repository: &File,
    raw_arguments: &[&str],
    input: &[u8],
    stdout_limit: usize,
    stderr_limit: usize,
    deadline: Instant,
) -> io::Result<(i32, Vec<u8>)> {
    let mut arguments = Vec::with_capacity(raw_arguments.len() + 1);
    arguments.push(
        CString::new(executable_path.as_os_str().as_bytes())
            .map_err(|_| io::Error::other("test executable path contains NUL"))?,
    );
    for argument in raw_arguments {
        arguments.push(
            CString::new(*argument)
                .map_err(|_| io::Error::other("test process argument contains NUL"))?,
        );
    }
    run_arguments(
        executable,
        executable_metadata,
        repository,
        &arguments,
        input,
        Limits {
            stdout: stdout_limit,
            stderr: stderr_limit,
            deadline,
        },
    )
}

fn run_arguments(
    executable: &File,
    executable_metadata: &Metadata,
    repository: &File,
    arguments: &[CString],
    input: &[u8],
    limits: Limits,
) -> io::Result<(i32, Vec<u8>)> {
    #[cfg(target_os = "linux")]
    let _ = executable_metadata;
    let stdin = pipe()?;
    let stdout = pipe()?;
    let stderr = pipe()?;
    let environment = environment();
    let output = Vec::with_capacity(limits.stdout);
    let child_io = ChildIo {
        stdin: &stdin,
        stdout: &stdout,
        stderr: &stderr,
    };
    if Instant::now() >= limits.deadline {
        return Err(io::Error::other("Git host deadline exceeded"));
    }

    #[cfg(target_os = "linux")]
    let pid = spawn_linux(executable, repository, arguments, &environment, &child_io)?;
    #[cfg(target_os = "macos")]
    let pid = spawn_macos(
        executable,
        executable_metadata,
        repository,
        arguments,
        &environment,
        &child_io,
    )?;

    close_parent_child_ends(pid, stdin.read, stdout.write, stderr.write)?;
    if set_nonblocking(&stdin.write).is_err()
        || set_nonblocking(&stdout.read).is_err()
        || set_nonblocking(&stderr.read).is_err()
    {
        must_settle_with_pipes(pid, stdin.write, stdout.read, stderr.read, false);
        return Err(io::Error::other("cannot configure Git pipes"));
    }
    drain_and_settle(
        pid,
        stdin.write,
        stdout.read,
        stderr.read,
        input,
        output,
        limits,
    )
}

fn set_nonblocking(descriptor: &Fd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(descriptor.raw(), libc::F_GETFL) };
    if flags < 0
        || unsafe { libc::fcntl(descriptor.raw(), libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0
    {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn close_parent_child_ends(
    pid: libc::pid_t,
    stdin_read: Fd,
    stdout_write: Fd,
    stderr_write: Fd,
) -> io::Result<()> {
    if stdin_read.close().is_err() || stdout_write.close().is_err() || stderr_write.close().is_err()
    {
        must_settle(pid);
        std::process::abort();
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn spawn_linux(
    executable: &File,
    repository: &File,
    arguments: &[CString],
    environment: &[CString],
    child_io: &ChildIo<'_>,
) -> io::Result<libc::pid_t> {
    let mut argv = arguments
        .iter()
        .map(|argument| argument.as_ptr())
        .collect::<Vec<_>>();
    argv.push(std::ptr::null());
    let mut env = environment
        .iter()
        .map(|value| value.as_ptr())
        .collect::<Vec<_>>();
    env.push(std::ptr::null::<c_char>());
    let signal_limit = libc::SIGRTMAX();
    let pid = unsafe { libc::fork() };
    if pid < 0 {
        return Err(io::Error::last_os_error());
    }
    if pid == 0 {
        unsafe {
            if libc::setpgid(0, 0) != 0 {
                libc::_exit(126);
            }
            let mut empty_mask = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
            if libc::sigemptyset(empty_mask.as_mut_ptr()) != 0
                || libc::sigprocmask(libc::SIG_SETMASK, empty_mask.as_ptr(), std::ptr::null_mut())
                    != 0
            {
                libc::_exit(126);
            }
            let mut default_action: libc::sigaction = std::mem::zeroed();
            default_action.sa_sigaction = libc::SIG_DFL;
            default_action.sa_mask = empty_mask.assume_init();
            for signal in 1..=signal_limit {
                if signal != libc::SIGKILL
                    && signal != libc::SIGSTOP
                    && libc::sigaction(signal, &default_action, std::ptr::null_mut()) != 0
                    && *libc::__errno_location() != libc::EINVAL
                {
                    libc::_exit(126);
                }
            }
            libc::umask(0o077);
            let executable_fd = libc::fcntl(executable.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3);
            if executable_fd < 3
                || libc::fchdir(repository.as_raw_fd()) != 0
                || libc::dup2(child_io.stdin.read.raw(), libc::STDIN_FILENO) < 0
                || libc::dup2(child_io.stdout.write.raw(), libc::STDOUT_FILENO) < 0
                || libc::dup2(child_io.stderr.write.raw(), libc::STDERR_FILENO) < 0
                || libc::fcntl(libc::STDIN_FILENO, libc::F_SETFD, 0) != 0
                || libc::fcntl(libc::STDOUT_FILENO, libc::F_SETFD, 0) != 0
                || libc::fcntl(libc::STDERR_FILENO, libc::F_SETFD, 0) != 0
            {
                libc::_exit(126);
            }
            for descriptor in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
                let flags = libc::fcntl(descriptor, libc::F_GETFL);
                if flags < 0
                    || libc::fcntl(descriptor, libc::F_SETFL, flags & !libc::O_NONBLOCK) != 0
                {
                    libc::_exit(126);
                }
            }
            if executable_fd > 3
                && libc::syscall(
                    libc::SYS_close_range,
                    3_u32,
                    executable_fd.saturating_sub(1) as u32,
                    0_u32,
                ) != 0
            {
                libc::_exit(126);
            }
            if executable_fd < c_int::MAX
                && libc::syscall(
                    libc::SYS_close_range,
                    (executable_fd + 1) as u32,
                    u32::MAX,
                    0_u32,
                ) != 0
            {
                libc::_exit(126);
            }
            unsafe extern "C" {
                fn fexecve(
                    fd: c_int,
                    argv: *const *const c_char,
                    envp: *const *const c_char,
                ) -> c_int;
            }
            fexecve(executable_fd, argv.as_ptr(), env.as_ptr());
            libc::_exit(127);
        }
    }
    // Either side can win the setpgid race. EACCES means the child already
    // crossed exec after installing its own group and is therefore acceptable.
    if unsafe { libc::setpgid(pid, pid) } != 0 {
        match io::Error::last_os_error().raw_os_error() {
            Some(libc::EACCES) | Some(libc::ESRCH) => {}
            _ => {
                must_settle(pid);
                return Err(io::Error::other("cannot establish Git process group"));
            }
        }
    }
    Ok(pid)
}

#[cfg(target_os = "macos")]
fn spawn_macos(
    executable: &File,
    executable_metadata: &Metadata,
    repository: &File,
    arguments: &[CString],
    environment: &[CString],
    child_io: &ChildIo<'_>,
) -> io::Result<libc::pid_t> {
    unsafe extern "C" {
        fn posix_spawn_file_actions_addfchdir_np(
            actions: *mut libc::posix_spawn_file_actions_t,
            fd: c_int,
        ) -> c_int;
    }
    let mut held_path = [0_u8; 1024];
    if unsafe { libc::fcntl(executable.as_raw_fd(), 50, held_path.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    let executable_path = unsafe { std::ffi::CStr::from_ptr(held_path.as_ptr().cast()) };
    let mut argv = arguments
        .iter()
        .map(|argument| argument.as_ptr().cast_mut())
        .collect::<Vec<_>>();
    argv.push(std::ptr::null_mut());
    argv[0] = executable_path.as_ptr().cast_mut();
    let mut env = environment
        .iter()
        .map(|value| value.as_ptr().cast_mut())
        .collect::<Vec<_>>();
    env.push(std::ptr::null_mut::<c_char>());
    let mut actions = std::ptr::null_mut();
    let mut attributes = std::ptr::null_mut();
    let mut pid = 0;
    let flags = libc::c_short::try_from(
        libc::POSIX_SPAWN_CLOEXEC_DEFAULT
            | libc::POSIX_SPAWN_SETPGROUP
            | libc::POSIX_SPAWN_START_SUSPENDED
            | libc::POSIX_SPAWN_SETSIGMASK
            | libc::POSIX_SPAWN_SETSIGDEF,
    )
    .map_err(|_| io::Error::other("invalid Darwin spawn flags"))?;
    let (spawn, destroyed) = unsafe {
        let actions_result = libc::posix_spawn_file_actions_init(&mut actions);
        let attributes_result = libc::posix_spawnattr_init(&mut attributes);
        let actions_ready = actions_result == 0;
        let attributes_ready = attributes_result == 0;
        let mut empty_mask = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        let mut default_signals = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
        let signals_ready = libc::sigemptyset(empty_mask.as_mut_ptr()) == 0
            && libc::sigfillset(default_signals.as_mut_ptr()) == 0
            && libc::sigdelset(default_signals.as_mut_ptr(), libc::SIGKILL) == 0
            && libc::sigdelset(default_signals.as_mut_ptr(), libc::SIGSTOP) == 0;
        let configured = actions_ready
            && attributes_ready
            && signals_ready
            && libc::posix_spawnattr_setflags(&mut attributes, flags) == 0
            && libc::posix_spawnattr_setpgroup(&mut attributes, 0) == 0
            && libc::posix_spawnattr_setsigmask(&mut attributes, empty_mask.as_ptr()) == 0
            && libc::posix_spawnattr_setsigdefault(&mut attributes, default_signals.as_ptr()) == 0
            && posix_spawn_file_actions_addfchdir_np(&mut actions, repository.as_raw_fd()) == 0
            && libc::posix_spawn_file_actions_adddup2(
                &mut actions,
                child_io.stdin.read.raw(),
                libc::STDIN_FILENO,
            ) == 0
            && libc::posix_spawn_file_actions_adddup2(
                &mut actions,
                child_io.stdout.write.raw(),
                libc::STDOUT_FILENO,
            ) == 0
            && libc::posix_spawn_file_actions_adddup2(
                &mut actions,
                child_io.stderr.write.raw(),
                libc::STDERR_FILENO,
            ) == 0;
        let result = if configured {
            libc::posix_spawn(
                &mut pid,
                executable_path.as_ptr(),
                &actions,
                &attributes,
                argv.as_ptr(),
                env.as_ptr(),
            )
        } else {
            libc::EINVAL
        };
        let actions_destroyed =
            !actions_ready || libc::posix_spawn_file_actions_destroy(&mut actions) == 0;
        let attributes_destroyed =
            !attributes_ready || libc::posix_spawnattr_destroy(&mut attributes) == 0;
        (result, actions_destroyed && attributes_destroyed)
    };
    if !destroyed {
        if spawn == 0 {
            must_settle(pid);
        }
        std::process::abort();
    }
    if spawn != 0 {
        return Err(io::Error::from_raw_os_error(spawn));
    }
    if let Err(error) = attest_macos(pid, executable_metadata, repository) {
        must_settle(pid);
        return Err(error);
    }
    if unsafe { libc::kill(pid, libc::SIGCONT) } != 0 {
        must_settle(pid);
        return Err(io::Error::other("cannot resume attested Git process"));
    }
    Ok(pid)
}

fn drain_and_settle(
    pid: libc::pid_t,
    mut stdin: Fd,
    mut stdout: Fd,
    mut stderr: Fd,
    input: &[u8],
    mut output: Vec<u8>,
    limits: Limits,
) -> io::Result<(i32, Vec<u8>)> {
    let mut settlement = ChildSettlement::new(pid);
    let mut input_offset = 0usize;
    let mut stderr_bytes = 0usize;
    let mut stdin_open = true;
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut status = None;
    if input.is_empty() {
        stdin = close_child_pipe(stdin, &mut settlement);
        stdin_open = false;
    }
    loop {
        if Instant::now() >= limits.deadline {
            return Err(io::Error::other("Git host deadline exceeded"));
        }
        if status.is_none() {
            let mut child_status = 0;
            let waited = unsafe { libc::waitpid(pid, &mut child_status, libc::WNOHANG) };
            if waited == pid {
                settlement.leader_reaped = true;
                status = Some(child_status);
                // Descendants can neither contribute to nor delay the admitted
                // result once the designated leader has exited.
                if quiesce_group(pid).is_err() {
                    std::process::abort();
                }
            } else if waited < 0 && io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                return Err(io::Error::other("cannot reap Git process"));
            }
        }

        let mut descriptors = [
            libc::pollfd {
                fd: if stdin_open { stdin.raw() } else { -1 },
                events: libc::POLLOUT,
                revents: 0,
            },
            libc::pollfd {
                fd: if stdout_open { stdout.raw() } else { -1 },
                events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
                revents: 0,
            },
            libc::pollfd {
                fd: if stderr_open { stderr.raw() } else { -1 },
                events: libc::POLLIN | libc::POLLHUP | libc::POLLERR,
                revents: 0,
            },
        ];
        let remaining = limits.deadline.saturating_duration_since(Instant::now());
        let wait_ms = c_int::try_from(remaining.as_millis().min(POLL_SLICE_MS as u128))
            .unwrap_or(POLL_SLICE_MS);
        let polled = unsafe {
            libc::poll(
                descriptors.as_mut_ptr(),
                descriptors.len() as libc::nfds_t,
                wait_ms,
            )
        };
        if polled < 0 && io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            return Err(io::Error::other("Git pipe poll failed"));
        }

        if stdin_open && descriptors[0].revents != 0 {
            if descriptors[0].revents & libc::POLLOUT != 0 {
                let written = unsafe {
                    libc::write(
                        stdin.raw(),
                        input[input_offset..].as_ptr().cast(),
                        input.len() - input_offset,
                    )
                };
                if written > 0 {
                    input_offset = input_offset
                        .checked_add(usize::try_from(written).map_err(|_| {
                            io::Error::other("Git stdin byte count conversion failed")
                        })?)
                        .ok_or_else(|| io::Error::other("Git stdin byte count overflow"))?;
                } else if written < 0 {
                    match io::Error::last_os_error().raw_os_error() {
                        Some(libc::EAGAIN) | Some(libc::EINTR) => {}
                        Some(libc::EPIPE) => input_offset = input.len(),
                        _ => return Err(io::Error::other("Git stdin write failed")),
                    }
                }
            }
            if input_offset == input.len()
                || descriptors[0].revents & (libc::POLLHUP | libc::POLLERR) != 0
            {
                stdin = close_child_pipe(stdin, &mut settlement);
                stdin_open = false;
            }
        }

        if stdout_open {
            match drain_pipe(&stdout, &mut output, limits.stdout) {
                Ok(true) => {
                    stdout = close_child_pipe(stdout, &mut settlement);
                    stdout_open = false;
                }
                Ok(false) => {}
                Err(error) => return Err(error),
            }
        }
        if stderr_open {
            match discard_pipe(&stderr, &mut stderr_bytes, limits.stderr) {
                Ok(true) => {
                    stderr = close_child_pipe(stderr, &mut settlement);
                    stderr_open = false;
                }
                Ok(false) => {}
                Err(error) => return Err(error),
            }
        }
        if let Some(status) = status {
            if !stdin_open && !stdout_open && !stderr_open {
                if quiesce_group(pid).is_err() {
                    std::process::abort();
                }
                let code = if libc::WIFEXITED(status) {
                    libc::WEXITSTATUS(status)
                } else {
                    -1
                };
                settlement.armed = false;
                return Ok((code, output));
            }
        }
    }
}

struct ChildSettlement {
    pid: libc::pid_t,
    leader_reaped: bool,
    armed: bool,
}

impl ChildSettlement {
    fn new(pid: libc::pid_t) -> Self {
        Self {
            pid,
            leader_reaped: false,
            armed: true,
        }
    }

    fn settle_now(&mut self) {
        if self.armed {
            if settle(self.pid, self.leader_reaped).is_err() {
                std::process::abort();
            }
            self.armed = false;
        }
    }
}

impl Drop for ChildSettlement {
    fn drop(&mut self) {
        if self.armed && settle(self.pid, self.leader_reaped).is_err() {
            std::process::abort();
        }
    }
}

fn close_child_pipe(descriptor: Fd, settlement: &mut ChildSettlement) -> Fd {
    if descriptor.close().is_err() {
        settlement.settle_now();
        std::process::abort();
    }
    Fd(None)
}

fn drain_pipe(pipe: &Fd, output: &mut Vec<u8>, limit: usize) -> io::Result<bool> {
    loop {
        let mut buffer = [0_u8; 8192];
        let read = unsafe { libc::read(pipe.raw(), buffer.as_mut_ptr().cast(), buffer.len()) };
        if read > 0 {
            let count = usize::try_from(read)
                .map_err(|_| io::Error::other("Git stdout byte count conversion failed"))?;
            if count > limit.saturating_sub(output.len()) {
                return Err(io::Error::other("Git stdout exceeded byte bound"));
            }
            output.extend_from_slice(&buffer[..count]);
        } else if read == 0 {
            return Ok(true);
        } else {
            return match io::Error::last_os_error().raw_os_error() {
                Some(libc::EAGAIN) => Ok(false),
                Some(libc::EINTR) => continue,
                _ => Err(io::Error::other("Git stdout read failed")),
            };
        }
    }
}

fn discard_pipe(pipe: &Fd, total: &mut usize, limit: usize) -> io::Result<bool> {
    loop {
        let mut buffer = [0_u8; 8192];
        let read = unsafe { libc::read(pipe.raw(), buffer.as_mut_ptr().cast(), buffer.len()) };
        if read > 0 {
            let count = usize::try_from(read)
                .map_err(|_| io::Error::other("Git stderr byte count conversion failed"))?;
            *total = total
                .checked_add(count)
                .ok_or_else(|| io::Error::other("Git stderr byte count overflow"))?;
            if *total > limit {
                return Err(io::Error::other("Git stderr exceeded byte bound"));
            }
        } else if read == 0 {
            return Ok(true);
        } else {
            return match io::Error::last_os_error().raw_os_error() {
                Some(libc::EAGAIN) => Ok(false),
                Some(libc::EINTR) => continue,
                _ => Err(io::Error::other("Git stderr read failed")),
            };
        }
    }
}

fn must_settle_with_pipes(
    pid: libc::pid_t,
    stdin: Fd,
    stdout: Fd,
    stderr: Fd,
    leader_reaped: bool,
) {
    let close_failed = stdin.close().is_err() || stdout.close().is_err() || stderr.close().is_err();
    let settled = settle(pid, leader_reaped);
    if close_failed || settled.is_err() {
        std::process::abort();
    }
}

fn must_settle(pid: libc::pid_t) {
    if settle(pid, false).is_err() {
        std::process::abort();
    }
}

fn settle(pid: libc::pid_t, leader_reaped: bool) -> io::Result<()> {
    let deadline = Instant::now() + SETTLEMENT_LIMIT;
    let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
    if !leader_reaped {
        wait_leader(pid, deadline)?;
    }
    quiesce_group_until(pid, deadline)
}

fn wait_leader(pid: libc::pid_t, deadline: Instant) -> io::Result<()> {
    loop {
        let mut status = 0;
        let waited = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
        if waited == pid {
            return Ok(());
        }
        if waited == 0 {
            if Instant::now() >= deadline {
                return Err(io::Error::other("Git process leader did not settle"));
            }
            std::thread::sleep(Duration::from_millis(1));
            continue;
        }
        if waited < 0 && io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            continue;
        }
        return Err(io::Error::other("cannot reap Git process leader"));
    }
}

fn quiesce_group(pid: libc::pid_t) -> io::Result<()> {
    quiesce_group_until(pid, Instant::now() + SETTLEMENT_LIMIT)
}

fn quiesce_group_until(pid: libc::pid_t, deadline: Instant) -> io::Result<()> {
    let _ = unsafe { libc::kill(-pid, libc::SIGKILL) };
    loop {
        if unsafe { libc::kill(-pid, 0) } != 0 {
            return match io::Error::last_os_error().raw_os_error() {
                Some(libc::ESRCH) => Ok(()),
                Some(libc::EINTR) => continue,
                _ => Err(io::Error::other("cannot inspect Git process group")),
            };
        }
        if Instant::now() >= deadline {
            return Err(io::Error::other("Git process group did not quiesce"));
        }
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[cfg(target_os = "macos")]
fn attest_macos(pid: libc::pid_t, executable: &Metadata, repository: &File) -> io::Result<()> {
    #[repr(C)]
    struct RegionInfo {
        protection: u32,
        max_protection: u32,
        inheritance: u32,
        flags: u32,
        offset: u64,
        behavior: u32,
        user_wired: u32,
        tag: u32,
        resident: u32,
        shared_private: u32,
        swapped: u32,
        dirtied: u32,
        refs: u32,
        shadow: u32,
        share_mode: u32,
        private_resident: u32,
        shared_resident: u32,
        object: u32,
        depth: u32,
        address: u64,
        size: u64,
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VnodeStat {
        dev: u32,
        mode: u16,
        nlink: u16,
        ino: u64,
        uid: u32,
        gid: u32,
        atime: i64,
        atime_ns: i64,
        mtime: i64,
        mtime_ns: i64,
        ctime: i64,
        ctime_ns: i64,
        birth: i64,
        birth_ns: i64,
        size: i64,
        blocks: i64,
        block_size: i32,
        flags: u32,
        generation: u32,
        rdev: u32,
        spare: [i64; 2],
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VnodeInfo {
        stat: VnodeStat,
        kind: i32,
        pad: i32,
        fsid: [i32; 2],
    }
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct VnodePath {
        info: VnodeInfo,
        path: [c_char; 1024],
    }
    #[repr(C)]
    struct RegionPath {
        region: RegionInfo,
        vnode: VnodePath,
    }
    #[repr(C)]
    struct VnodePaths {
        cwd: VnodePath,
        root: VnodePath,
    }
    #[link(name = "proc")]
    unsafe extern "C" {
        fn proc_pidinfo(
            pid: c_int,
            flavor: c_int,
            arg: u64,
            buffer: *mut libc::c_void,
            size: c_int,
        ) -> c_int;
    }

    let repository = repository.metadata()?;
    let mut cwd = std::mem::MaybeUninit::<VnodePaths>::zeroed();
    let cwd_size = c_int::try_from(std::mem::size_of::<VnodePaths>())
        .map_err(|_| io::Error::other("Darwin cwd record is too large"))?;
    if unsafe { proc_pidinfo(pid, 9, 0, cwd.as_mut_ptr().cast(), cwd_size) } != cwd_size {
        return Err(io::Error::other("cannot inspect suspended Git cwd"));
    }
    let cwd = unsafe { cwd.assume_init() };
    if u64::from(cwd.cwd.info.stat.dev) != repository.dev()
        || cwd.cwd.info.stat.ino != repository.ino()
        || cwd.cwd.info.kind != 2
    {
        return Err(io::Error::other(
            "suspended Git cwd differs from held repository",
        ));
    }

    let mut address = 0_u64;
    let mut saw_region = false;
    let mut executable_regions = 0_u32;
    let mut terminal = false;
    for _ in 0..4096 {
        let mut region = std::mem::MaybeUninit::<RegionPath>::zeroed();
        let size = c_int::try_from(std::mem::size_of::<RegionPath>())
            .map_err(|_| io::Error::other("Darwin region record is too large"))?;
        unsafe { *libc::__error() = 0 };
        let returned = unsafe { proc_pidinfo(pid, 8, address, region.as_mut_ptr().cast(), size) };
        let errno = unsafe { *libc::__error() };
        if returned == 0 && matches!(errno, 0 | libc::EINVAL) {
            terminal = saw_region;
            break;
        }
        if returned != size || errno != 0 {
            return Err(io::Error::other("cannot inspect suspended Git executable"));
        }
        let region = unsafe { region.assume_init() };
        if region.region.size == 0 || region.region.address < address {
            return Err(io::Error::other("invalid Darwin executable region"));
        }
        if u64::from(region.vnode.info.stat.dev) == executable.dev()
            && region.vnode.info.stat.ino == executable.ino()
            && region.vnode.info.stat.size >= 0
            && u64::try_from(region.vnode.info.stat.size).ok() == Some(executable.len())
            && region.vnode.info.kind == 1
            && region.region.protection & libc::VM_PROT_EXECUTE as u32 != 0
        {
            executable_regions = executable_regions
                .checked_add(1)
                .ok_or_else(|| io::Error::other("Darwin executable region count overflow"))?;
        }
        address = region
            .region
            .address
            .checked_add(region.region.size)
            .ok_or_else(|| io::Error::other("Darwin region address overflow"))?;
        saw_region = true;
        if address == 0 {
            return Err(io::Error::other("Darwin region address wrapped"));
        }
    }
    if !terminal || executable_regions != 1 {
        return Err(io::Error::other(
            "suspended child did not map the held Git executable exactly once",
        ));
    }
    Ok(())
}
