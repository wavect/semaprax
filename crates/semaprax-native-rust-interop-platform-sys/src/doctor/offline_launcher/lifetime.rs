//! Launcher-owned child settlement. Scripts carry no pidfd or cleanup authority.

const SETTLEMENT_NS: u64 = 5_000_000_000;
const PAUSE_NS: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Owned,
    Reaped,
    Closed,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Exit {
    pid: libc::pid_t,
    signo: i32,
    code: i32,
    status: i32,
}

trait Operations {
    fn now(&mut self) -> Result<u64, ()>;
    fn kill(&mut self, fd: i32) -> Result<(), i32>;
    fn wait(&mut self, fd: i32) -> Result<Option<Exit>, ()>;
    fn pause(&mut self, nanos: u64) -> Result<(), ()>;
    fn close(&mut self, fd: i32) -> Result<(), ()>;
    fn stop(&mut self) -> !;
}

// This resource-free state can decide operations but never owns a descriptor.
// Native Lifetime is armed only by the parent's successful clone3 handoff.
struct State {
    fd: i32,
    pid: libc::pid_t,
    phase: Phase,
}

impl State {
    fn new(fd: i32, pid: libc::pid_t) -> Self {
        Self {
            fd,
            pid,
            phase: Phase::Owned,
        }
    }

    fn redirect(&mut self, fd: i32) -> bool {
        if self.phase != Phase::Owned || fd < 0 || fd == self.fd {
            return false;
        }
        self.fd = fd;
        true
    }

    fn fail(&mut self, operations: &mut impl Operations) -> ! {
        self.phase = Phase::Failed;
        operations.stop()
    }

    fn abort(&mut self, operations: &mut impl Operations) -> ! {
        if self.phase != Phase::Owned {
            operations.stop();
        }
        if self.fd < 0 || self.pid <= 0 {
            self.fail(operations);
        }
        let origin = match operations.now() {
            Ok(now) => now,
            Err(()) => self.fail(operations),
        };
        let deadline = match origin.checked_add(SETTLEMENT_NS) {
            Some(deadline) => deadline,
            None => self.fail(operations),
        };
        if let Err(error) = operations.kill(self.fd) {
            if error != libc::ESRCH {
                self.fail(operations);
            }
        }
        let mut previous = origin;
        loop {
            let now = match operations.now() {
                Ok(now) if now >= previous && now < deadline => now,
                _ => self.fail(operations),
            };
            previous = now;
            match operations.wait(self.fd) {
                Ok(Some(exit)) => {
                    // waitid without WNOWAIT consumed the event. Latch that
                    // irreversible fact BEFORE inspecting identity or status.
                    self.phase = Phase::Reaped;
                    let status_valid = match exit.code {
                        libc::CLD_EXITED => (0..=255).contains(&exit.status),
                        libc::CLD_KILLED | libc::CLD_DUMPED => (1..=64).contains(&exit.status),
                        _ => false,
                    };
                    if exit.pid != self.pid || exit.signo != libc::SIGCHLD || !status_valid {
                        self.fail(operations);
                    }
                    break;
                }
                Ok(None) => {}
                Err(()) => self.fail(operations),
            }
            if operations.pause(PAUSE_NS.min(deadline - now)).is_err() {
                self.fail(operations);
            }
        }
        // Disarm before close: uncertainty cannot cause a retry from Drop.
        self.phase = Phase::Closed;
        if operations.close(self.fd).is_err() {
            self.fail(operations);
        }
        // Forced worker reap is never a successful report or tool-closure proof.
        operations.stop()
    }
}

pub(super) struct Lifetime {
    state: State,
}

impl Lifetime {
    /// Arm immediately after clone3, before any fallible parent-side operation.
    pub(super) fn new(fd: i32, pid: libc::pid_t) -> Self {
        Self {
            state: State::new(fd, pid),
        }
    }

    /// The caller has already obtained an exact duplicate of this owned pidfd.
    /// Switch first, then let the caller check closure of the old descriptor.
    pub(super) fn redirect(&mut self, fd: i32) {
        if !self.state.redirect(fd) {
            self.abort();
        }
    }

    pub(super) fn abort(&mut self) -> ! {
        self.state.abort(&mut Native)
    }
}

impl Drop for Lifetime {
    fn drop(&mut self) {
        if self.state.phase != Phase::Closed {
            self.abort();
        }
    }
}

struct Native;

impl Operations for Native {
    fn now(&mut self) -> Result<u64, ()> {
        let mut time = std::mem::MaybeUninit::<libc::timespec>::zeroed();
        if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, time.as_mut_ptr()) } != 0 {
            return Err(());
        }
        let time = unsafe { time.assume_init() };
        let seconds = u64::try_from(time.tv_sec).map_err(|_| ())?;
        let nanos = u64::try_from(time.tv_nsec).map_err(|_| ())?;
        if nanos >= 1_000_000_000 {
            return Err(());
        }
        seconds
            .checked_mul(1_000_000_000)
            .and_then(|value| value.checked_add(nanos))
            .ok_or(())
    }

    fn kill(&mut self, fd: i32) -> Result<(), i32> {
        if unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                fd,
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0_u32,
            )
        } != 0
        {
            Err(std::io::Error::last_os_error()
                .raw_os_error()
                .unwrap_or(libc::EIO))
        } else {
            Ok(())
        }
    }

    fn wait(&mut self, fd: i32) -> Result<Option<Exit>, ()> {
        let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
        if unsafe {
            libc::waitid(
                libc::P_PIDFD,
                fd as libc::id_t,
                info.as_mut_ptr(),
                libc::WEXITED | libc::WNOHANG,
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

    fn pause(&mut self, nanos: u64) -> Result<(), ()> {
        if nanos == 0 || nanos > PAUSE_NS {
            return Err(());
        }
        let time = libc::timespec {
            tv_sec: 0,
            tv_nsec: nanos as libc::c_long,
        };
        if unsafe { libc::nanosleep(&time, std::ptr::null_mut()) } != 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    fn close(&mut self, fd: i32) -> Result<(), ()> {
        if unsafe { libc::close(fd) } != 0 {
            Err(())
        } else {
            Ok(())
        }
    }

    fn stop(&mut self) -> ! {
        unsafe { libc::_exit(126) }
    }
}

#[cfg(test)]
mod tests;
