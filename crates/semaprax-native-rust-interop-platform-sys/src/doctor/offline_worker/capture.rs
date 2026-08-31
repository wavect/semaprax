//! PID-namespace ownership and fair, bounded capture for one prepared tool.
use super::{
    child, errno, fail_stop, guard::Guard, nonblocking, offline_root, pipe, Fd, ProbeError,
};
use std::ffi::CStr;
use std::time::{Duration, Instant};

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

pub(super) fn run(
    plan: &offline_root::Plan<'_>,
    guard: &Guard,
    path: &CStr,
    output: &mut Vec<u8>,
) -> Result<(), ProbeError> {
    let (stdin, empty) = pipe()?;
    drop(empty);
    let (stdout, stdout_writer) = pipe()?;
    let (stderr, stderr_writer) = pipe()?;
    nonblocking(stdout.0).map_err(|_| ProbeError::Io)?;
    nonblocking(stderr.0).map_err(|_| ProbeError::Io)?;
    let supervisor = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::getpid(), 0_u32) };
    if supervisor < 0 {
        return Err(ProbeError::Spawn);
    }
    let supervisor = Fd(supervisor as i32);
    let mut pidfd = -1_i32;
    let arguments = CloneArgs {
        flags: (libc::CLONE_NEWPID | libc::CLONE_PIDFD) as u64,
        pidfd: (&mut pidfd as *mut i32) as u64,
        exit_signal: libc::SIGCHLD as u64,
        ..CloneArgs::default()
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    // No CLONE_VM/FILES/THREAD: ordinary private fork-like state, but a fresh
    // PID namespace for each tool, not an irreversible supervisor unshare.
    let pid = unsafe {
        libc::syscall(
            libc::SYS_clone3,
            &arguments as *const CloneArgs,
            std::mem::size_of::<CloneArgs>(),
        )
    };
    if pid == 0 {
        unsafe {
            child::enter(
                plan,
                guard,
                path,
                [stdin.0, stdout_writer.0, stderr_writer.0],
                supervisor.0,
            )
        }
    }
    if pid < 0 {
        return Err(ProbeError::Spawn);
    }
    if pidfd < 0 || pid > i32::MAX as libc::c_long {
        fail_stop();
    }
    let mut owned = Child {
        pid: pid as i32,
        pidfd: Fd(pidfd),
        reaped: false,
    };
    drop((stdin, stdout_writer, stderr_writer, supervisor));
    let mut selected = None;
    let mut total = 0;
    let mut ended = [false; 2];
    loop {
        for (index, fd) in [stdout.0, stderr.0].into_iter().enumerate() {
            if !ended[index] {
                match read(fd, &mut total, (index == 0).then_some(&mut *output)) {
                    Ok(eof) => ended[index] = eof,
                    Err(error) => {
                        selected.get_or_insert(error);
                    }
                }
            }
        }
        if selected.is_some() {
            break;
        }
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        if unsafe {
            libc::waitid(
                libc::P_PIDFD,
                pidfd as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } != 0
        {
            selected = Some(ProbeError::Io);
            break;
        }
        if unsafe { info.si_pid() } != 0 {
            break;
        }
        if Instant::now() >= deadline {
            selected = Some(ProbeError::Timeout);
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    // Signal only the pinned, unreaped identity. Reaping namespace PID 1 also
    // waits for the kernel's namespace-descendant teardown before publication.
    let settle_deadline = Instant::now() + Duration::from_secs(5);
    let status = owned.settle(settle_deadline);
    while !ended.iter().all(|eof| *eof) {
        if Instant::now() >= settle_deadline {
            fail_stop();
        }
        for (index, fd) in [stdout.0, stderr.0].into_iter().enumerate() {
            if !ended[index] {
                match read(
                    fd,
                    &mut total,
                    (index == 0 && selected.is_none()).then_some(&mut *output),
                ) {
                    Ok(eof) => ended[index] = eof,
                    Err(error) => {
                        selected.get_or_insert(error);
                        // An I/O error cannot establish EOF. No successful or
                        // ordinary-error reply may hide uncertain drainage.
                        if error == ProbeError::Io {
                            fail_stop();
                        }
                    }
                }
            }
        }
    }
    if !libc::WIFEXITED(status) || libc::WEXITSTATUS(status) != 0 {
        selected.get_or_insert(ProbeError::Exit);
    }
    if let Some(error) = selected {
        output.clear();
        Err(error)
    } else {
        Ok(())
    }
}

fn read(fd: i32, total: &mut usize, output: Option<&mut Vec<u8>>) -> Result<bool, ProbeError> {
    let mut bytes = [0_u8; 8192];
    let count = unsafe { libc::read(fd, bytes.as_mut_ptr().cast(), bytes.len()) };
    if count == 0 {
        return Ok(true);
    }
    if count < 0 {
        return if errno() == libc::EAGAIN {
            Ok(false)
        } else {
            Err(ProbeError::Io)
        };
    }
    let count = count as usize;
    *total = total.checked_add(count).ok_or(ProbeError::OutputLimit)?;
    if *total > 65_536 {
        return Err(ProbeError::OutputLimit);
    }
    if let Some(output) = output {
        if output.capacity() - output.len() < count {
            return Err(ProbeError::OutputLimit);
        }
        output.extend_from_slice(&bytes[..count]);
    }
    Ok(false)
}

struct Child {
    pid: i32,
    pidfd: Fd,
    reaped: bool,
}
impl Child {
    fn settle(&mut self, deadline: Instant) -> i32 {
        if self.reaped {
            fail_stop();
        }
        if unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                self.pidfd.0,
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0_u32,
            )
        } != 0
            && errno() != libc::ESRCH
        {
            fail_stop();
        }
        loop {
            let mut status = 0;
            let waited = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
            if waited == self.pid {
                self.reaped = true;
                return status;
            }
            if waited != 0 || Instant::now() >= deadline {
                fail_stop();
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        if !self.reaped {
            self.settle(Instant::now() + Duration::from_secs(5));
        }
    }
}
