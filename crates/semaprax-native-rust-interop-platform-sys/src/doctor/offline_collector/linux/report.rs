//! Bounded report delivery after collection; scripted operations grant no authority.
use super::{errno, nonblocking, require_pipe, stop};
use std::mem::MaybeUninit;
use std::time::{Duration, Instant};

const MAX_REPORT_BYTES: usize = 2 * 1024 * 1024;
const WRITE_CHUNK: usize = 8192;
const WRITE_BUDGET: Duration = Duration::from_secs(5);

trait Operations {
    fn require_pipe(&mut self, fd: i32) -> Result<(), ()>;
    fn nonblocking_stdout(&mut self) -> Result<(), ()>;
    fn empty_signal_set(&mut self) -> Result<(), ()>;
    fn add_sigpipe(&mut self) -> Result<(), ()>;
    fn block_sigpipe(&mut self) -> Result<(), ()>;
    fn start_clock(&mut self);
    fn elapsed(&mut self) -> Duration;
    fn write(&mut self, bytes: &[u8]) -> Result<usize, i32>;
    fn pause(&mut self);
    fn close(&mut self, fd: i32) -> Result<(), ()>;
}

// This private seam returns only a nominal exit code, never an observation or
// execution permission. Its only production caller consumes the process below.
fn deliver(report: &[u8], exit_code: u8, operations: &mut impl Operations) -> Result<u8, ()> {
    if exit_code > 1 || report.len() > MAX_REPORT_BYTES {
        return Err(());
    }
    for fd in 0..=2 {
        operations.require_pipe(fd)?;
    }
    operations.nonblocking_stdout()?;
    operations.empty_signal_set()?;
    operations.add_sigpipe()?;
    operations.block_sigpipe()?;
    operations.start_clock();
    let mut offset = 0;
    while offset < report.len() {
        if operations.elapsed() >= WRITE_BUDGET {
            return Err(());
        }
        let end = report.len().min(offset + WRITE_CHUNK);
        match operations.write(&report[offset..end]) {
            Ok(count) if count > 0 && count <= end - offset => offset += count,
            Err(libc::EAGAIN) => operations.pause(),
            // Zero, impossible counts, EINTR, EPIPE and all other failures are
            // terminal. Never retry an uncertain write or reset the deadline.
            _ => return Err(()),
        }
    }
    for fd in [1, 0, 2] {
        operations.close(fd)?;
    }
    Ok(exit_code)
}

struct Native {
    blocked: Option<libc::sigset_t>,
    origin: Option<Instant>,
}

impl Operations for Native {
    fn require_pipe(&mut self, fd: i32) -> Result<(), ()> {
        require_pipe(fd)
    }
    fn nonblocking_stdout(&mut self) -> Result<(), ()> {
        nonblocking(1)
    }
    fn empty_signal_set(&mut self) -> Result<(), ()> {
        // glibc may clear only the kernel signal words, not the entire public
        // sigset_t array. Initialize its full storage before assume_init can
        // create a Rust value containing the otherwise untouched tail words.
        let mut blocked = MaybeUninit::<libc::sigset_t>::zeroed();
        if unsafe { libc::sigemptyset(blocked.as_mut_ptr()) } != 0 {
            Err(())
        } else {
            self.blocked = Some(unsafe { blocked.assume_init() });
            Ok(())
        }
    }
    fn add_sigpipe(&mut self) -> Result<(), ()> {
        let blocked = self.blocked.as_mut().ok_or(())?;
        if unsafe { libc::sigaddset(blocked, libc::SIGPIPE) } != 0 {
            Err(())
        } else {
            Ok(())
        }
    }
    fn block_sigpipe(&mut self) -> Result<(), ()> {
        let blocked = self.blocked.as_ref().ok_or(())?;
        // The dedicated process never resumes its caller. Keep any generated
        // SIGPIPE pending through _exit so EPIPE selects fail-stop 126. No tool
        // or ordinary CLI policy changes; never restore and deliver the signal.
        if unsafe { libc::sigprocmask(libc::SIG_BLOCK, blocked, std::ptr::null_mut()) } != 0 {
            Err(())
        } else {
            Ok(())
        }
    }
    fn start_clock(&mut self) {
        self.origin = Some(Instant::now());
    }
    fn elapsed(&mut self) -> Duration {
        self.origin.as_ref().map_or(Duration::MAX, Instant::elapsed)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, i32> {
        let count = unsafe { libc::write(1, bytes.as_ptr().cast(), bytes.len()) };
        if count < 0 {
            Err(errno())
        } else {
            Ok(count as usize)
        }
    }
    fn pause(&mut self) {
        std::thread::sleep(Duration::from_millis(1));
    }
    fn close(&mut self, fd: i32) -> Result<(), ()> {
        if unsafe { libc::close(fd) } != 0 {
            Err(())
        } else {
            Ok(())
        }
    }
}

pub(super) fn finish(report: &[u8], exit_code: u8) -> ! {
    let mut native = Native {
        blocked: None,
        origin: None,
    };
    match deliver(report, exit_code, &mut native) {
        Ok(status) => unsafe { libc::_exit(i32::from(status)) },
        Err(()) => stop(),
    }
}

#[cfg(test)]
mod tests;
