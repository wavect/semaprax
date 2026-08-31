//! Private syscall boundary. Only Native owns OS resources; scripts own none.
use super::super::{errno, fail_stop, Fd};
use std::time::{Duration, Instant};

pub(super) trait Operations {
    /// None means would-block; Some(0) means EOF. Bytes initialize the buffer.
    fn read(&mut self, stream: usize, buffer: &mut [u8; 8192]) -> Result<Option<usize>, ()>;
    fn observe_exit(&mut self) -> Result<bool, ()>;
    fn kill_owned(&mut self) -> Result<(), ()>;
    fn reap_owned(&mut self) -> Result<Option<bool>, ()>;
    fn now(&mut self) -> Duration;
    fn pause(&mut self);
    fn fail_stop(&mut self) -> !;
}

pub(super) struct Native {
    // Drop the child before its capture descriptors during emergency unwinding.
    child: Child,
    streams: [Fd; 2],
    origin: Instant,
}

impl Native {
    pub(super) fn new(pid: i32, pidfd: i32, stdout: Fd, stderr: Fd, origin: Instant) -> Self {
        Self {
            child: Child {
                pid,
                pidfd: Fd(pidfd),
                reaped: false,
            },
            streams: [stdout, stderr],
            origin,
        }
    }
}

impl Operations for Native {
    fn read(&mut self, stream: usize, buffer: &mut [u8; 8192]) -> Result<Option<usize>, ()> {
        let fd = self.streams.get(stream).ok_or(())?.0;
        let count = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
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

    fn observe_exit(&mut self) -> Result<bool, ()> {
        if self.child.reaped {
            return Err(());
        }
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        if unsafe {
            libc::waitid(
                libc::P_PIDFD,
                self.child.pidfd.0 as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        } != 0
        {
            return Err(());
        }
        Ok(unsafe { info.si_pid() } != 0)
    }

    fn kill_owned(&mut self) -> Result<(), ()> {
        self.child.kill()
    }
    fn reap_owned(&mut self) -> Result<Option<bool>, ()> {
        self.child.reap()
    }
    fn now(&mut self) -> Duration {
        self.origin.elapsed()
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(1));
    }
    fn fail_stop(&mut self) -> ! {
        fail_stop()
    }
}

struct Child {
    pid: i32,
    pidfd: Fd,
    reaped: bool,
}
impl Child {
    fn kill(&mut self) -> Result<(), ()> {
        if self.reaped {
            return Err(());
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
            return Err(());
        }
        Ok(())
    }

    fn reap(&mut self) -> Result<Option<bool>, ()> {
        if self.reaped {
            return Err(());
        }
        let mut status = 0;
        let waited = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
        if waited == self.pid {
            self.reaped = true;
            Ok(Some(
                libc::WIFEXITED(status) && libc::WEXITSTATUS(status) == 0,
            ))
        } else if waited == 0 {
            Ok(None)
        } else {
            Err(())
        }
    }

    // Emergency no-report path retains the original bounded cleanup on Rust
    // unwinding. Ordinary control flow uses the shared capture::settle loop.
    fn settle(&mut self, deadline: Instant) {
        if self.kill().is_err() {
            fail_stop();
        }
        loop {
            match self.reap() {
                Ok(Some(_)) => return,
                Ok(None) => {}
                Err(()) => fail_stop(),
            }
            if Instant::now() >= deadline {
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
