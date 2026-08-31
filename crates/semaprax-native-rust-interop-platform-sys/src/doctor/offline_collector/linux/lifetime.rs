//! Pinned worker ownership begins only after a child-specific non-reaping wait.
use super::{
    capture::{Exit, Operations},
    errno, stop,
};
use std::time::{Duration, Instant};

pub(super) struct Lifetime {
    origin: Instant,
    authenticated: bool,
    reaped: bool,
    armed: bool,
}
impl Lifetime {
    pub(super) fn new(origin: Instant) -> Self {
        Self {
            origin,
            authenticated: false,
            reaped: false,
            armed: true,
        }
    }
    pub(super) fn authenticate(&mut self) -> Result<(), ()> {
        if self.authenticated || self.reaped {
            return Err(());
        }
        if let Some(exit) = wait(false)? {
            if exit.pid <= 0
                || exit.signo != libc::SIGCHLD
                || !matches!(
                    exit.code,
                    libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
                )
            {
                return Err(());
            }
        }
        self.authenticated = true;
        Ok(())
    }
    pub(super) fn require_time(&self) -> Result<(), ()> {
        if self.origin.elapsed() >= Duration::from_secs(60) {
            Err(())
        } else {
            Ok(())
        }
    }
    pub(super) fn disarm(&mut self) {
        if !self.authenticated || !self.reaped {
            stop();
        }
        self.armed = false;
    }
    pub(super) fn abort(&mut self) -> ! {
        // Invalid/nonchild descriptors never confer signal authority. An
        // already reaped worker never receives a second wait or signal.
        if self.authenticated && !self.reaped {
            let origin = Instant::now();
            if unsafe {
                libc::syscall(
                    libc::SYS_pidfd_send_signal,
                    5_i32,
                    libc::SIGKILL,
                    std::ptr::null::<libc::siginfo_t>(),
                    0_u32,
                )
            } != 0
                && errno() != libc::ESRCH
            {
                stop();
            }
            loop {
                match wait(true) {
                    Ok(Some(_)) => {
                        self.reaped = true;
                        break;
                    }
                    Ok(None) => {}
                    Err(()) => stop(),
                }
                if origin.elapsed() >= Duration::from_secs(5) {
                    stop();
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        // This does not prove tool namespace closure after forced worker death.
        // Provisioner-owned aggregate cleanup remains required; no report.
        stop()
    }
}
impl Drop for Lifetime {
    fn drop(&mut self) {
        if self.armed {
            self.abort();
        }
    }
}

impl Operations for Lifetime {
    fn read(&mut self, stream: usize, bytes: &mut [u8; 8192]) -> Result<Option<usize>, ()> {
        let fd = match stream {
            0 => 6,
            1 => 7,
            _ => return Err(()),
        };
        let count = unsafe { libc::read(fd, bytes.as_mut_ptr().cast(), bytes.len()) };
        if count < 0 {
            if errno() == libc::EAGAIN {
                Ok(None)
            } else {
                Err(())
            }
        } else {
            Ok(Some(count as usize))
        }
    }
    fn observe(&mut self) -> Result<Option<Exit>, ()> {
        if !self.authenticated || self.reaped {
            return Err(());
        }
        wait(false)
    }
    fn reap(&mut self) -> Result<Option<Exit>, ()> {
        if !self.authenticated || self.reaped {
            return Err(());
        }
        let exit = wait(true)?;
        if exit.is_some() {
            self.reaped = true;
        }
        Ok(exit)
    }
    fn elapsed(&mut self) -> Duration {
        self.origin.elapsed()
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn wait(reap: bool) -> Result<Option<Exit>, ()> {
    let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
    let flags = libc::WEXITED | libc::WNOHANG | if reap { 0 } else { libc::WNOWAIT };
    if unsafe { libc::waitid(libc::P_PIDFD, 5, &mut info, flags) } != 0 {
        return Err(());
    }
    let pid = unsafe { info.si_pid() };
    if pid == 0 {
        return Ok(None);
    }
    Ok(Some(Exit {
        pid,
        signo: info.si_signo,
        code: info.si_code,
        status: unsafe { info.si_status() },
    }))
}
