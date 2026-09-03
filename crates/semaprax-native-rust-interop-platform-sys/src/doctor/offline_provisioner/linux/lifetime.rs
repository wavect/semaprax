//! Pidfd and cgroup ownership; no ordinary return while either scope is live.
use super::super::cgroup;
use std::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Exit {
    pub(super) pid: libc::pid_t,
    pub(super) signo: i32,
    pub(super) code: i32,
    pub(super) status: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Owned,
    Observed(Exit),
    Reaped,
    PidfdClosed,
    Closed,
}

pub(super) struct Lifetime {
    pidfd: i32,
    pid: libc::pid_t,
    phase: Phase,
}

impl Lifetime {
    pub(super) fn new(pidfd: i32, pid: libc::pid_t) -> Self {
        if pidfd < 0 || pid <= 0 {
            stop();
        }
        Self {
            pidfd,
            pid,
            phase: Phase::Owned,
        }
    }

    pub(super) fn observe(&mut self) -> Result<Option<Exit>, ()> {
        match self.phase {
            Phase::Owned => {}
            Phase::Observed(exit) => return Ok(Some(exit)),
            Phase::Reaped | Phase::PidfdClosed | Phase::Closed => return Err(()),
        }
        wait(self.pidfd, true)
    }

    pub(super) fn accept(&mut self, exit: Exit) -> Result<(), ()> {
        if self.phase != Phase::Owned || !valid_identity(exit, self.pid) {
            return Err(());
        }
        self.phase = Phase::Observed(exit);
        Ok(())
    }

    pub(super) fn complete(&mut self) -> Result<(), ()> {
        let Phase::Observed(expected) = self.phase else {
            return Err(());
        };
        let actual = wait(self.pidfd, false)?.ok_or(())?;
        // Reap is irreversible; latch before comparing the returned event.
        self.phase = Phase::Reaped;
        if actual != expected || !valid_identity(actual, self.pid) {
            return Err(());
        }
        if unsafe { libc::close(self.pidfd) } != 0 {
            // Close uncertainty cannot safely retry this identity handle.
            stop();
        }
        self.phase = Phase::PidfdClosed;
        let deadline = cgroup::settlement_deadline().ok_or(())?;
        cgroup::wait_empty(deadline)?;
        self.phase = Phase::Closed;
        Ok(())
    }

    pub(super) fn abort(&mut self) -> ! {
        if self.phase != Phase::Closed {
            let deadline = cgroup::settlement_deadline().unwrap_or_else(|| stop());
            if cgroup::require_empty().is_err() && cgroup::kill().is_err() {
                stop();
            }
            if matches!(self.phase, Phase::Owned | Phase::Observed(_)) {
                if unsafe {
                    libc::syscall(
                        libc::SYS_pidfd_send_signal,
                        self.pidfd,
                        libc::SIGKILL,
                        std::ptr::null::<libc::siginfo_t>(),
                        0u32,
                    )
                } != 0
                    && super::super::errno() != libc::ESRCH
                {
                    stop();
                }
                loop {
                    match wait(self.pidfd, false) {
                        Ok(Some(exit)) => {
                            self.phase = Phase::Reaped;
                            if !valid_identity(exit, self.pid) {
                                stop();
                            }
                            break;
                        }
                        Ok(None) => {}
                        Err(()) => stop(),
                    }
                    if Instant::now() >= deadline {
                        stop();
                    }
                    pause();
                }
            }
            if self.phase == Phase::Reaped {
                if unsafe { libc::close(self.pidfd) } != 0 {
                    stop();
                }
                self.phase = Phase::PidfdClosed;
            }
            if cgroup::wait_empty(deadline).is_err() {
                stop();
            }
            self.phase = Phase::Closed;
        }
        stop()
    }
}

impl Drop for Lifetime {
    fn drop(&mut self) {
        if self.phase != Phase::Closed {
            self.abort();
        }
    }
}

fn wait(pidfd: i32, observe: bool) -> Result<Option<Exit>, ()> {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    let options = libc::WEXITED | libc::WNOHANG | if observe { libc::WNOWAIT } else { 0 };
    if unsafe {
        libc::waitid(
            libc::P_PIDFD,
            pidfd as libc::id_t,
            info.as_mut_ptr(),
            options,
        )
    } != 0
    {
        return Err(());
    }
    let info = unsafe { info.assume_init() };
    let pid = unsafe { info.si_pid() };
    if pid == 0 {
        return if info.si_signo == 0 && info.si_code == 0 {
            Ok(None)
        } else {
            Err(())
        };
    }
    Ok(Some(Exit {
        pid,
        signo: info.si_signo,
        code: info.si_code,
        status: unsafe { info.si_status() },
    }))
}

fn valid_identity(exit: Exit, pid: libc::pid_t) -> bool {
    exit.pid == pid
        && exit.signo == libc::SIGCHLD
        && match exit.code {
            libc::CLD_EXITED => (0..=255).contains(&exit.status),
            libc::CLD_KILLED | libc::CLD_DUMPED => (1..=64).contains(&exit.status),
            _ => false,
        }
}

fn pause() {
    let value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 1_000_000,
    };
    if unsafe { libc::nanosleep(&value, std::ptr::null_mut()) } != 0
        && super::super::errno() != libc::EINTR
    {
        stop();
    }
}

fn stop() -> ! {
    unsafe { libc::_exit(126) }
}

#[cfg(test)]
mod tests {
    use super::{valid_identity, Exit};

    #[test]
    fn only_exact_terminal_child_events_are_owned() {
        let valid = Exit {
            pid: 42,
            signo: libc::SIGCHLD,
            code: libc::CLD_EXITED,
            status: 1,
        };
        assert!(valid_identity(valid, 42));
        for hostile in [
            Exit { pid: 41, ..valid },
            Exit {
                signo: libc::SIGTERM,
                ..valid
            },
            Exit {
                code: libc::CLD_STOPPED,
                ..valid
            },
            Exit {
                status: 256,
                ..valid
            },
        ] {
            assert!(!valid_identity(hostile, 42));
        }
    }
}
