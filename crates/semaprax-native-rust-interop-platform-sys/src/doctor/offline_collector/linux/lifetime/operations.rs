//! Fixed native operations. No request/script chooses a descriptor or PID.
use super::super::{errno, stop};
use super::Exit;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Wait {
    Observe,
    Reap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Descriptor {
    Request,
    Bundle,
    Reply,
    Stderr,
    Pidfd,
}

pub(super) trait Operations {
    fn wait(&mut self, mode: Wait) -> Result<Option<Exit>, i32>;
    fn kill(&mut self) -> Result<(), i32>;
    fn close(&mut self, descriptor: Descriptor) -> Result<(), i32>;
    fn elapsed(&mut self) -> Duration;
    fn pause(&mut self);
    fn stop(&mut self) -> !;
}

pub(super) struct Native {
    origin: Instant,
}
impl Native {
    pub(super) fn new(origin: Instant) -> Self {
        Self { origin }
    }
    pub(super) fn read(
        &mut self,
        stream: usize,
        bytes: &mut [u8; 8192],
    ) -> Result<Option<usize>, ()> {
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
}
impl Operations for Native {
    fn wait(&mut self, mode: Wait) -> Result<Option<Exit>, i32> {
        let mut info = unsafe { std::mem::zeroed::<libc::siginfo_t>() };
        let flags = libc::WEXITED
            | libc::WNOHANG
            | if mode == Wait::Observe {
                libc::WNOWAIT
            } else {
                0
            };
        if unsafe { libc::waitid(libc::P_PIDFD, 5, &mut info, flags) } != 0 {
            return Err(errno());
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
    fn kill(&mut self) -> Result<(), i32> {
        if unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                5_i32,
                libc::SIGKILL,
                std::ptr::null::<libc::siginfo_t>(),
                0_u32,
            )
        } != 0
        {
            Err(errno())
        } else {
            Ok(())
        }
    }
    fn close(&mut self, descriptor: Descriptor) -> Result<(), i32> {
        let fd = match descriptor {
            Descriptor::Request => 3,
            Descriptor::Bundle => 4,
            Descriptor::Reply => 6,
            Descriptor::Stderr => 7,
            Descriptor::Pidfd => 5,
        };
        if unsafe { libc::close(fd) } != 0 {
            Err(errno())
        } else {
            Ok(())
        }
    }
    fn elapsed(&mut self) -> Duration {
        self.origin.elapsed()
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(1));
    }
    fn stop(&mut self) -> ! {
        stop()
    }
}
