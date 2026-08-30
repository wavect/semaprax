//! Owned private-group lifecycle, with no destructive PID operation after reap.
use super::{Fault, Prepared, ProbeError};
use std::time::{Duration, Instant};

mod launch;

struct Fd(Option<libc::c_int>);
impl Fd {
    fn raw(&self) -> libc::c_int {
        self.0.expect("owned descriptor")
    }
    fn close(mut self) -> Result<(), ProbeError> {
        let fd = self.0.take().expect("owned descriptor");
        if unsafe { libc::close(fd) } == 0 {
            Ok(())
        } else {
            Err(ProbeError::Io)
        }
    }
}
impl Drop for Fd {
    fn drop(&mut self) {
        if let Some(fd) = self.0.take() {
            if unsafe { libc::close(fd) } != 0 {
                std::process::abort()
            }
        }
    }
}

struct Group<'a> {
    pid: libc::pid_t,
    probe: &'a Prepared,
    settled: bool,
}
impl Group<'_> {
    fn must_settle(&mut self) {
        if !self.settled {
            self.must_settle_at(Instant::now() + self.probe.limits.settle);
        }
    }
    fn must_settle_at(&mut self, deadline: Instant) {
        if !self.settled {
            if self.settle(deadline).is_err() {
                std::process::abort()
            }
            self.settled = true;
        }
    }
    fn settle(&mut self, deadline: Instant) -> Result<(), ProbeError> {
        // Under the checked default SIGCHLD policy and documented no-foreign-
        // reaper host condition, the unreaped leader pins this numeric group ID.
        // Also kill the leader if it had not reached setpgid before the deadline.
        for target in [-self.pid, self.pid] {
            if unsafe { libc::kill(target, libc::SIGKILL) } != 0 {
                let error = errno();
                // XNU's group kill filters zombies, then returns EPERM when
                // no signalable member remains. This is not permission to
                // ignore a live group's denial: independently prove leader
                // exit (without reap) and absence of every other member.
                #[cfg(target_os = "macos")]
                if target == -self.pid && error == libc::EPERM && observe(self.pid)?.is_some() {
                    self.darwin_group_before_reap(deadline)?;
                    continue;
                }
                if error != libc::ESRCH {
                    return Err(ProbeError::Io);
                }
            }
        }
        #[cfg(target_os = "macos")]
        self.darwin_group_before_reap(deadline)?;
        loop {
            let mut status = 0;
            let result = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
            if result == self.pid {
                break;
            }
            if result < 0 && errno() != libc::EINTR {
                return Err(ProbeError::Io);
            }
            if Instant::now() >= deadline {
                return Err(ProbeError::Io);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        // There are deliberately NO signals except signal-zero probes after
        // this point. PID reuse may cause rejection, never an unrelated kill.
        #[cfg(target_os = "linux")]
        loop {
            if unsafe { libc::kill(-self.pid, 0) } != 0 {
                match errno() {
                    libc::ESRCH => break,
                    libc::EINTR => {}
                    _ => return Err(ProbeError::Io),
                }
            }
            if Instant::now() >= deadline {
                return Err(ProbeError::Io);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        if self.probe.injected(Fault::Kill) || self.probe.injected(Fault::Settle) {
            return Err(ProbeError::Io);
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    fn darwin_group_before_reap(&self, deadline: Instant) -> Result<(), ProbeError> {
        #[link(name = "proc")]
        unsafe extern "C" {
            fn proc_listpgrppids(
                group: libc::pid_t,
                buffer: *mut libc::c_void,
                size: libc::c_int,
            ) -> libc::c_int;
        }
        let mut members = [0 as libc::pid_t; 4096];
        loop {
            unsafe {
                *libc::__error() = 0;
            }
            let required = unsafe { proc_listpgrppids(self.pid, std::ptr::null_mut(), 0) };
            if required < 0 || (required == 0 && errno() != 0) || required as usize > members.len()
            {
                return Err(ProbeError::Io);
            }
            members.fill(0);
            unsafe {
                *libc::__error() = 0;
            }
            let count = unsafe {
                proc_listpgrppids(
                    self.pid,
                    members.as_mut_ptr().cast(),
                    std::mem::size_of_val(&members) as libc::c_int,
                )
            };
            if count < 0 || (count == 0 && errno() != 0) || count as usize >= members.len() {
                return Err(ProbeError::Io);
            }
            if members[..count as usize].iter().all(|pid| *pid == self.pid) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(ProbeError::Io);
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for Group<'_> {
    fn drop(&mut self) {
        self.must_settle()
    }
}

fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn nonblocking(fd: &Fd) -> Result<(), ProbeError> {
    let flags = unsafe { libc::fcntl(fd.raw(), libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd.raw(), libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
        Err(ProbeError::Io)
    } else {
        Ok(())
    }
}

fn observe(pid: libc::pid_t) -> Result<Option<bool>, ProbeError> {
    let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
    if unsafe {
        libc::waitid(
            libc::P_PID,
            pid as libc::id_t,
            &mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    } != 0
    {
        return if errno() == libc::EINTR {
            Ok(None)
        } else {
            Err(ProbeError::Io)
        };
    }
    if unsafe { info.si_pid() } == 0 {
        Ok(None)
    } else {
        Ok(Some(
            info.si_code == libc::CLD_EXITED && unsafe { info.si_status() } == 0,
        ))
    }
}

fn read_once(
    fd: &Fd,
    stdout: bool,
    output: &mut Vec<u8>,
    total: &mut usize,
    limit: usize,
) -> Result<bool, ProbeError> {
    let mut buffer = [0_u8; 8192];
    let count = unsafe { libc::read(fd.raw(), buffer.as_mut_ptr().cast(), buffer.len()) };
    if count == 0 {
        return Ok(false);
    }
    // Interrupted reads are retryable, not evidence that the pipe is drained.
    if count < 0 {
        return match errno() {
            libc::EAGAIN => Ok(false),
            libc::EINTR => Ok(true),
            _ => Err(ProbeError::Io),
        };
    }
    let count = count as usize;
    if count > limit.saturating_sub(*total) {
        return Err(ProbeError::OutputLimit);
    }
    *total += count;
    if stdout {
        output.extend_from_slice(&buffer[..count])
    }
    Ok(true)
}

pub(super) fn run(probe: &Prepared) -> Result<Vec<u8>, ProbeError> {
    require_owned_wait_policy()?;
    let launch = launch::Launch::prepare(probe)?;
    let (stdout, stdout_write) = launch::pipe()?;
    let (stderr, stderr_write) = launch::pipe()?;
    let null = launch::null()?;
    nonblocking(&stdout)?;
    nonblocking(&stderr)?;
    let mut output = Vec::with_capacity(probe.limits.output);
    if probe.injected(Fault::Spawn) {
        return Err(ProbeError::Spawn);
    }
    let deadline = Instant::now() + probe.limits.run;
    let (pid, launch_close_failed) = launch.spawn(&stdout_write, &stderr_write, &null)?;
    let mut group = Group {
        pid,
        probe,
        settled: false,
    };
    let close_failed =
        stdout_write.close().is_err() | stderr_write.close().is_err() | null.close().is_err();
    if launch_close_failed || close_failed {
        group.must_settle();
        std::process::abort()
    }
    let mut total = 0;
    let mut selected = None;
    loop {
        if probe.injected(Fault::Deadline) || Instant::now() >= deadline {
            selected = Some(ProbeError::Timeout);
            break;
        }
        if probe.injected(Fault::Read) {
            selected = Some(ProbeError::Io);
            break;
        }
        // One chunk per stream per turn: neither writer can starve the other
        // stream, process observation, or the deadline.
        let read = read_once(&stdout, true, &mut output, &mut total, probe.limits.output)
            .and_then(|_| read_once(&stderr, false, &mut output, &mut total, probe.limits.output));
        if let Err(error) = read {
            selected = Some(error);
            break;
        }
        if probe.injected(Fault::Wait) {
            selected = Some(ProbeError::Io);
            break;
        }
        match observe(pid) {
            Ok(Some(success)) => {
                if !success {
                    selected = Some(ProbeError::Exit)
                }
                break;
            }
            Ok(None) => {}
            Err(error) => {
                selected = Some(error);
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let settlement_deadline = Instant::now() + probe.limits.settle;
    group.must_settle_at(settlement_deadline);
    // Only already-buffered bytes can remain after owned group settlement.
    // Do not wait on EOF from an out-of-contract escaped descriptor holder.
    if selected.is_none() {
        loop {
            if Instant::now() >= settlement_deadline {
                selected = Some(ProbeError::Io);
                break;
            }
            let read = read_once(&stdout, true, &mut output, &mut total, probe.limits.output)
                .and_then(|first| {
                    read_once(&stderr, false, &mut output, &mut total, probe.limits.output)
                        .map(|second| first || second)
                });
            match read {
                Ok(false) => break,
                Err(error) => {
                    selected = Some(error);
                    break;
                }
                Ok(true) => {}
            }
        }
    }
    let close_failed = stdout.close().is_err() | stderr.close().is_err();
    if close_failed || probe.injected(Fault::Close) {
        std::process::abort()
    }
    if let Some(error) = selected {
        Err(error)
    } else {
        Ok(output)
    }
}

fn require_owned_wait_policy() -> Result<(), ProbeError> {
    let mut action = std::mem::MaybeUninit::<libc::sigaction>::zeroed();
    if unsafe { libc::sigaction(libc::SIGCHLD, std::ptr::null(), action.as_mut_ptr()) } != 0 {
        return Err(ProbeError::Invalid);
    }
    let action = unsafe { action.assume_init() };
    // Default disposition keeps the exited leader waitable until this owner
    // reaps it. A caller must also exclude foreign waiters and concurrent
    // signal-policy changes: this query cannot establish process-wide locking.
    if action.sa_sigaction != libc::SIG_DFL || action.sa_flags & libc::SA_NOCLDWAIT != 0 {
        return Err(ProbeError::Invalid);
    }
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::os::unix::process::CommandExt;
    use std::process::Command;

    #[test]
    fn exited_only_group_settles_without_treating_zombie_eperm_as_live_permission() {
        require_owned_wait_policy().unwrap();
        let probe = super::super::prepare(std::path::Path::new("/usr/bin/true")).unwrap();
        let mut child = Command::new("/usr/bin/true")
            .process_group(0)
            .spawn()
            .unwrap();
        let mut group = Group {
            pid: child.id() as libc::pid_t,
            probe: &probe,
            settled: false,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while observe(group.pid).unwrap().is_none() {
            assert!(Instant::now() < deadline);
            std::thread::yield_now();
        }
        // Observation leaves the zombie unreaped, pinning the group identity.
        group.must_settle_at(deadline);
        assert!(group.settled);
        assert_eq!(child.wait().unwrap_err().raw_os_error(), Some(libc::ECHILD));
    }
}
