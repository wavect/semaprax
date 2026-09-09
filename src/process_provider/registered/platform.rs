//! Held-executable Unix adapter. This module owns raw process and pipe authority.
//! The registry supplies every executable, cwd, argument and environment byte.
#![allow(unsafe_code)]
use super::HeldProcessTool;
use crate::process_provider::{ProcessFailure, ProcessOutput, ProcessRequest, ProcessTermination};
use libc::{c_char, c_int};
use std::ffi::CString;
use std::fs::File;
#[cfg(target_os = "macos")]
use std::fs::Metadata;
use std::io;
use std::os::fd::AsRawFd;
#[cfg(target_os = "macos")]
use std::os::unix::fs::MetadataExt;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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
            // An uncertain close cannot relinquish descriptor authority.
            std::process::abort();
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
    #[cfg(target_os = "linux")]
    launch: &'a Pipe,
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
                launch_failed(child_io.launch.write.raw(), 126);
            }
            let mut empty_mask = std::mem::MaybeUninit::<libc::sigset_t>::uninit();
            if libc::sigemptyset(empty_mask.as_mut_ptr()) != 0
                || libc::sigprocmask(libc::SIG_SETMASK, empty_mask.as_ptr(), std::ptr::null_mut())
                    != 0
            {
                launch_failed(child_io.launch.write.raw(), 126);
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
                    launch_failed(child_io.launch.write.raw(), 126);
                }
            }
            libc::umask(0o077);
            let executable_fd = libc::fcntl(executable.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 3);
            let launch_fd = libc::fcntl(
                child_io.launch.write.raw(),
                libc::F_DUPFD_CLOEXEC,
                executable_fd + 1,
            );
            if executable_fd < 3
                || launch_fd < 0
                || libc::fchdir(repository.as_raw_fd()) != 0
                || libc::dup2(child_io.stdin.read.raw(), libc::STDIN_FILENO) < 0
                || libc::dup2(child_io.stdout.write.raw(), libc::STDOUT_FILENO) < 0
                || libc::dup2(child_io.stderr.write.raw(), libc::STDERR_FILENO) < 0
                || libc::fcntl(libc::STDIN_FILENO, libc::F_SETFD, 0) != 0
                || libc::fcntl(libc::STDOUT_FILENO, libc::F_SETFD, 0) != 0
                || libc::fcntl(libc::STDERR_FILENO, libc::F_SETFD, 0) != 0
            {
                launch_failed(child_io.launch.write.raw(), 126);
            }
            for descriptor in [libc::STDIN_FILENO, libc::STDOUT_FILENO, libc::STDERR_FILENO] {
                let flags = libc::fcntl(descriptor, libc::F_GETFL);
                if flags < 0
                    || libc::fcntl(descriptor, libc::F_SETFL, flags & !libc::O_NONBLOCK) != 0
                {
                    launch_failed(launch_fd, 126);
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
                launch_failed(launch_fd, 126);
            }
            if launch_fd > executable_fd + 1
                && libc::syscall(
                    libc::SYS_close_range,
                    (executable_fd + 1) as u32,
                    (launch_fd - 1) as u32,
                    0_u32,
                ) != 0
            {
                launch_failed(launch_fd, 126);
            }
            if launch_fd < c_int::MAX
                && libc::syscall(
                    libc::SYS_close_range,
                    (launch_fd + 1) as u32,
                    u32::MAX,
                    0_u32,
                ) != 0
            {
                launch_failed(launch_fd, 126);
            }
            unsafe extern "C" {
                fn fexecve(
                    fd: c_int,
                    argv: *const *const c_char,
                    envp: *const *const c_char,
                ) -> c_int;
            }
            fexecve(executable_fd, argv.as_ptr(), env.as_ptr());
            launch_failed(launch_fd, 127);
        }
    }
    // Either side can win the setpgid race. EACCES means the child already
    // crossed exec after installing its own group and is therefore acceptable.
    if unsafe { libc::setpgid(pid, pid) } != 0 {
        match io::Error::last_os_error().raw_os_error() {
            Some(libc::EACCES) | Some(libc::ESRCH) => {}
            _ => {
                must_settle(pid);
                return Err(io::Error::other(
                    "cannot establish registered tool process group",
                ));
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
        return Err(io::Error::other(
            "cannot resume attested registered tool process",
        ));
    }
    Ok(pid)
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
        return Err(io::Error::other(
            "cannot inspect suspended registered tool cwd",
        ));
    }
    let cwd = unsafe { cwd.assume_init() };
    if u64::from(cwd.cwd.info.stat.dev) != repository.dev()
        || cwd.cwd.info.stat.ino != repository.ino()
        || cwd.cwd.info.kind != 2
    {
        return Err(io::Error::other(
            "suspended registered tool cwd differs from held repository",
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
            return Err(io::Error::other(
                "cannot inspect suspended registered tool executable",
            ));
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
            "suspended child did not map the held registered tool executable exactly once",
        ));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
unsafe fn launch_failed(fd: c_int, code: u8) -> ! {
    unsafe {
        let _ = libc::write(fd, (&code as *const u8).cast(), 1);
        libc::_exit(i32::from(code));
    }
}

// Failed settlement keeps the unreaped child identity here. A provider cannot
// start another child while this quarantine is nonempty. Reaped leaders are
// never signalled again, avoiding a signal to a recycled PID/process group.
static QUARANTINE: Mutex<Vec<Pending>> = Mutex::new(Vec::new());
struct Pending {
    pid: libc::pid_t,
    reaped: bool,
    lost: bool,
}
impl Pending {
    fn attempt(&mut self, deadline: Instant) -> bool {
        if self.lost {
            return false;
        }
        if !self.reaped {
            // WNOWAIT observes without releasing our child identity. Hosts
            // must not independently reap children owned by this provider.
            let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
            loop {
                let checked = unsafe {
                    libc::waitid(
                        libc::P_PID,
                        self.pid as libc::id_t,
                        info.as_mut_ptr(),
                        libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                    )
                };
                if checked == 0 {
                    break;
                }
                if io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                    self.lost = true;
                    return false;
                }
                if Instant::now() >= deadline {
                    return false;
                }
            }
            unsafe {
                libc::kill(-self.pid, libc::SIGKILL);
                libc::kill(self.pid, libc::SIGKILL);
            }
        }
        loop {
            if !self.reaped {
                let mut status = 0;
                let waited = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
                if waited == self.pid {
                    self.reaped = true;
                } else if waited < 0
                    && io::Error::last_os_error().raw_os_error() != Some(libc::EINTR)
                {
                    // Never signal a PID after exclusive child ownership is lost.
                    self.lost = true;
                    return false;
                }
            }
            if self.reaped
                && unsafe { libc::kill(-self.pid, 0) } != 0
                && io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
fn quarantine(pending: Pending) {
    QUARANTINE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
        .push(pending);
}
fn must_settle(pid: libc::pid_t) {
    let mut pending = Pending {
        pid,
        reaped: false,
        lost: false,
    };
    if !pending.attempt(Instant::now() + Duration::from_millis(250)) {
        quarantine(pending);
    }
}
pub(super) fn settle() -> Result<(), ProcessFailure> {
    let mut children = QUARANTINE
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    let deadline = Instant::now() + Duration::from_millis(250);
    children.retain_mut(|child| !child.attempt(deadline));
    if children.is_empty() {
        Ok(())
    } else {
        Err(ProcessFailure::SettlementFailed)
    }
}
struct ChildGuard(Option<Pending>);
impl ChildGuard {
    fn finish(&mut self) -> Result<(), ProcessFailure> {
        let Some(mut child) = self.0.take() else {
            return Ok(());
        };
        if child.attempt(Instant::now() + Duration::from_millis(250)) {
            Ok(())
        } else {
            quarantine(child);
            Err(ProcessFailure::SettlementFailed)
        }
    }
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

fn set_nonblocking(fd: &Fd) -> io::Result<()> {
    let flags = unsafe { libc::fcntl(fd.raw(), libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd.raw(), libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(super) fn run(
    tool: &HeldProcessTool,
    request: &ProcessRequest,
) -> Result<ProcessOutput, ProcessFailure> {
    settle()?;
    let mut signal_policy = std::mem::MaybeUninit::<libc::sigaction>::zeroed();
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), signal_policy.as_mut_ptr()) } != 0
    {
        return Err(ProcessFailure::IoFailure);
    }
    let signal_policy = unsafe { signal_policy.assume_init() };
    if signal_policy.sa_sigaction != libc::SIG_DFL
        || signal_policy.sa_flags & libc::SA_NOCLDWAIT != 0
    {
        return Err(ProcessFailure::AuthorityDenied);
    }
    let deadline = Instant::now() + Duration::from_millis(request.timeout_ms());
    let mut arguments = Vec::with_capacity(request.arguments().len() + 1);
    arguments.push(tool.argv0.clone());
    for value in request.arguments() {
        arguments.push(CString::new(value.as_slice()).map_err(|_| ProcessFailure::InvalidInput)?);
    }
    let stdin = pipe().map_err(|_| ProcessFailure::LaunchFailed)?;
    let stdout = pipe().map_err(|_| ProcessFailure::LaunchFailed)?;
    let stderr = pipe().map_err(|_| ProcessFailure::LaunchFailed)?;
    #[cfg(target_os = "linux")]
    let launch = pipe().map_err(|_| ProcessFailure::LaunchFailed)?;
    let child_io = ChildIo {
        stdin: &stdin,
        stdout: &stdout,
        stderr: &stderr,
        #[cfg(target_os = "linux")]
        launch: &launch,
    };
    #[cfg(target_os = "linux")]
    let spawned = spawn_linux(
        &tool.executable,
        &tool.cwd,
        &arguments,
        &tool.environment,
        &child_io,
    );
    #[cfg(target_os = "macos")]
    let spawned = spawn_macos(
        &tool.executable,
        &tool.executable_metadata,
        &tool.cwd,
        &arguments,
        &tool.environment,
        &child_io,
    );
    let pid = spawned.map_err(|_| ProcessFailure::LaunchFailed)?;
    let mut guard = ChildGuard(Some(Pending {
        pid,
        reaped: false,
        lost: false,
    }));
    // Close all child-side pipe references in the parent before polling EOF.
    stdin.read.close().map_err(|_| ProcessFailure::IoFailure)?;
    stdout
        .write
        .close()
        .map_err(|_| ProcessFailure::IoFailure)?;
    stderr
        .write
        .close()
        .map_err(|_| ProcessFailure::IoFailure)?;
    #[cfg(target_os = "linux")]
    launch
        .write
        .close()
        .map_err(|_| ProcessFailure::IoFailure)?;
    let mut pipes = [Some(stdin.write), Some(stdout.read), Some(stderr.read)];
    for pipe in pipes.iter().flatten() {
        set_nonblocking(pipe).map_err(|_| ProcessFailure::IoFailure)?;
    }
    #[cfg(target_os = "linux")]
    set_nonblocking(&launch.read).map_err(|_| ProcessFailure::IoFailure)?;
    let result = exchange(
        pid,
        request,
        deadline,
        &mut pipes,
        #[cfg(target_os = "linux")]
        &launch.read,
    );
    // Drop parent pipe authority before settlement and before publishing data.
    drop(pipes);
    #[cfg(target_os = "linux")]
    drop(launch.read);
    let settled = guard.finish();
    match result {
        Err(selected) => Err(selected),
        Ok(output) => settled.map(|()| output),
    }
}

fn observe_exit(pid: libc::pid_t) -> Result<Option<ProcessTermination>, ProcessFailure> {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    let waited = unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            info.as_mut_ptr(),
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    };
    if waited < 0 {
        return if io::Error::last_os_error().raw_os_error() == Some(libc::EINTR) {
            Ok(None)
        } else {
            Err(ProcessFailure::IoFailure)
        };
    }
    let info = unsafe { info.assume_init() };
    if unsafe { info.si_pid() } == 0 {
        return Ok(None);
    }
    let status = unsafe { info.si_status() };
    match info.si_code {
        libc::CLD_EXITED => Ok(Some(ProcessTermination::Exited(status as u32))),
        libc::CLD_KILLED | libc::CLD_DUMPED if (1..=255).contains(&status) => {
            Ok(Some(ProcessTermination::Signalled(status as u8)))
        }
        _ => Err(ProcessFailure::IoFailure),
    }
}

fn exchange(
    pid: libc::pid_t,
    request: &ProcessRequest,
    deadline: Instant,
    pipes: &mut [Option<Fd>; 3],
    #[cfg(target_os = "linux")] launch: &Fd,
) -> Result<ProcessOutput, ProcessFailure> {
    let mut streams = [Vec::new(), Vec::new()];
    let limits = [request.stdout_max(), request.stderr_max()];
    let mut input_offset = 0;
    let mut termination = None;
    #[cfg(target_os = "linux")]
    let mut launched = false;
    loop {
        if Instant::now() >= deadline {
            return Err(ProcessFailure::TimedOut);
        }
        #[cfg(target_os = "linux")]
        {
            let mut byte = 0_u8;
            let read = unsafe { libc::read(launch.raw(), (&mut byte as *mut u8).cast(), 1) };
            if read > 0 {
                return Err(ProcessFailure::LaunchFailed);
            }
            if read == 0 {
                launched = true;
            }
            if read < 0
                && !matches!(
                    io::Error::last_os_error().raw_os_error(),
                    Some(libc::EAGAIN) | Some(libc::EINTR)
                )
            {
                return Err(ProcessFailure::IoFailure);
            }
        }
        if termination.is_none() {
            termination = observe_exit(pid)?;
            if termination.is_some() {
                // The unreaped leader pins the identity while we stop any
                // descendants that could otherwise keep pipe handles alive.
                unsafe {
                    libc::kill(-pid, libc::SIGKILL);
                }
                pipes[0].take();
            }
        }
        if input_offset == request.stdin().len() {
            pipes[0].take();
        }
        let mut descriptors = std::array::from_fn::<_, 3, _>(|index| libc::pollfd {
            fd: pipes[index].as_ref().map_or(-1, Fd::raw),
            events: if index == 0 {
                libc::POLLOUT
            } else {
                libc::POLLIN
            },
            revents: 0,
        });
        let wait_ms = deadline
            .saturating_duration_since(Instant::now())
            .as_millis()
            .min(10) as c_int;
        if unsafe { libc::poll(descriptors.as_mut_ptr(), 3, wait_ms) } < 0
            && io::Error::last_os_error().raw_os_error() != Some(libc::EINTR)
        {
            return Err(ProcessFailure::IoFailure);
        }
        if descriptors[0].revents & (libc::POLLERR | libc::POLLHUP) != 0 {
            pipes[0].take();
        }
        if let Some(pipe) = pipes[0].as_ref() {
            if descriptors[0].revents & libc::POLLOUT != 0 {
                input_offset += write_input(pipe, &request.stdin()[input_offset..])?;
            }
        }
        for index in 0..2 {
            if let Some(pipe) = pipes[index + 1].as_ref() {
                if drain(pipe, &mut streams[index], limits[index])? {
                    pipes[index + 1].take();
                }
            }
        }
        if let Some(termination) = termination {
            #[cfg(target_os = "linux")]
            if !launched {
                continue;
            }
            if pipes.iter().all(Option::is_none) {
                let [stdout, stderr] = streams;
                return Ok(ProcessOutput {
                    termination,
                    stdout,
                    stderr,
                });
            }
        }
    }
}

fn drain(pipe: &Fd, output: &mut Vec<u8>, maximum: usize) -> Result<bool, ProcessFailure> {
    // One read per poll gives stdin and the other stream equal service.
    let mut bytes = [0_u8; 4096];
    let read = unsafe { libc::read(pipe.raw(), bytes.as_mut_ptr().cast(), bytes.len()) };
    if read > 0 {
        let count = read as usize;
        if count > maximum.saturating_sub(output.len()) {
            return Err(ProcessFailure::CapacityExceeded);
        }
        output.extend_from_slice(&bytes[..count]);
        Ok(false)
    } else if read == 0 {
        Ok(true)
    } else if matches!(
        io::Error::last_os_error().raw_os_error(),
        Some(libc::EAGAIN) | Some(libc::EINTR)
    ) {
        Ok(false)
    } else {
        Err(ProcessFailure::IoFailure)
    }
}

fn write_input(pipe: &Fd, input: &[u8]) -> Result<usize, ProcessFailure> {
    // Block SIGPIPE on this thread only. Consume a newly generated pending
    // SIGPIPE before restoring the caller's mask; never change process policy.
    unsafe {
        let mut blocked: libc::sigset_t = std::mem::zeroed();
        let mut previous: libc::sigset_t = std::mem::zeroed();
        if libc::sigemptyset(&mut blocked) != 0
            || libc::sigaddset(&mut blocked, libc::SIGPIPE) != 0
            || libc::pthread_sigmask(libc::SIG_BLOCK, &blocked, &mut previous) != 0
        {
            return Err(ProcessFailure::IoFailure);
        }
        let mut pending: libc::sigset_t = std::mem::zeroed();
        let pending_ok = libc::sigpending(&mut pending) == 0;
        let already_pending = pending_ok && libc::sigismember(&pending, libc::SIGPIPE) == 1;
        let written = if pending_ok {
            libc::write(pipe.raw(), input.as_ptr().cast(), input.len())
        } else {
            -1
        };
        let error = io::Error::last_os_error().raw_os_error();
        if written < 0 && error == Some(libc::EPIPE) && !already_pending {
            let mut signal = 0;
            // SIGPIPE generated by this failed write is pending while blocked.
            if libc::sigpending(&mut pending) == 0
                && libc::sigismember(&pending, libc::SIGPIPE) == 1
            {
                let _ = libc::sigwait(&blocked, &mut signal);
            }
        }
        if libc::pthread_sigmask(libc::SIG_SETMASK, &previous, std::ptr::null_mut()) != 0 {
            std::process::abort();
        }
        if !pending_ok {
            return Err(ProcessFailure::IoFailure);
        }
        if written >= 0 {
            Ok(written as usize)
        } else {
            match error {
                Some(libc::EAGAIN) | Some(libc::EINTR) => Ok(0),
                Some(libc::EPIPE) => Ok(input.len()),
                _ => Err(ProcessFailure::IoFailure),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_settlement_lost_child_identity_is_never_signalled_again() {
        // Our own process is not our child. WNOWAIT must establish ECHILD
        // before any signal, then permanently retire signalling authority.
        let mut pending = Pending {
            pid: std::process::id() as libc::pid_t,
            reaped: false,
            lost: false,
        };
        assert!(!pending.attempt(Instant::now() + Duration::from_millis(10)));
        assert!(pending.lost);
        assert!(!pending.attempt(Instant::now() + Duration::from_millis(10)));
    }
}
