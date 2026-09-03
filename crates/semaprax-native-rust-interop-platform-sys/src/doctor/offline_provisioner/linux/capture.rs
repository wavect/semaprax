//! Fair bounded report capture; EOF and bytes never substitute for child exit.
use super::lifetime::{Exit, Lifetime};
use std::time::Instant;

const REPORT_LIMIT: usize = 2 * 1024 * 1024;
const CHUNK: usize = 8192;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Stream {
    Report,
    Error,
}

#[derive(Debug)]
struct State {
    report: Vec<u8>,
    eof: [bool; 2],
    exit: Option<Exit>,
    failed: bool,
}

impl State {
    fn new() -> Result<Self, ()> {
        let mut report = Vec::new();
        report.try_reserve_exact(REPORT_LIMIT).map_err(|_| ())?;
        Ok(Self {
            report,
            eof: [false; 2],
            exit: None,
            failed: false,
        })
    }

    fn bytes(&mut self, stream: Stream, bytes: &[u8]) -> Result<(), ()> {
        if self.failed || self.eof[stream as usize] {
            self.failed = true;
            return Err(());
        }
        if stream == Stream::Error && !bytes.is_empty() {
            self.failed = true;
            return Err(());
        }
        if self
            .report
            .len()
            .checked_add(bytes.len())
            .filter(|length| *length <= REPORT_LIMIT)
            .is_none()
        {
            self.failed = true;
            return Err(());
        }
        if stream == Stream::Report {
            self.report.extend_from_slice(bytes);
        }
        Ok(())
    }

    fn eof(&mut self, stream: Stream) -> Result<(), ()> {
        if self.failed || self.eof[stream as usize] {
            self.failed = true;
            return Err(());
        }
        self.eof[stream as usize] = true;
        Ok(())
    }

    fn exit(&mut self, exit: Exit) -> Result<(), ()> {
        if self.failed
            || self.exit.is_some()
            || exit.code != libc::CLD_EXITED
            || !matches!(exit.status, 0 | 1)
        {
            self.failed = true;
            return Err(());
        }
        self.exit = Some(exit);
        Ok(())
    }

    fn ready(&self) -> bool {
        !self.failed && self.eof == [true, true] && self.exit.is_some() && !self.report.is_empty()
    }
}

pub(super) fn collect(
    lifetime: &mut Lifetime,
    report_fd: i32,
    error_fd: i32,
    deadline: Instant,
) -> Result<(Vec<u8>, u8), ()> {
    nonblocking(report_fd)?;
    nonblocking(error_fd)?;
    let mut state = State::new()?;
    let mut buffer = [0u8; CHUNK];
    loop {
        for (stream, fd) in [(Stream::Report, report_fd), (Stream::Error, error_fd)] {
            if state.eof[stream as usize] {
                continue;
            }
            let maximum = if stream == Stream::Report {
                let remaining = REPORT_LIMIT - state.report.len();
                if remaining == 0 {
                    1
                } else {
                    remaining.min(CHUNK)
                }
            } else {
                CHUNK
            };
            let count = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), maximum) };
            if count > 0 {
                state.bytes(stream, &buffer[..usize::try_from(count).map_err(|_| ())?])?;
            } else if count == 0 {
                state.eof(stream)?;
            } else if super::super::errno() != libc::EAGAIN
                && super::super::errno() != libc::EWOULDBLOCK
            {
                return Err(());
            }
        }
        if state.exit.is_none() {
            if let Some(exit) = lifetime.observe()? {
                state.exit(exit)?;
            }
        }
        if state.ready() {
            let exit = state.exit.ok_or(())?;
            lifetime.accept(exit)?;
            if super::close(report_fd).is_err() || super::close(error_fd).is_err() {
                return Err(());
            }
            return Ok((state.report, u8::try_from(exit.status).map_err(|_| ())?));
        }
        if Instant::now() >= deadline {
            return Err(());
        }
        let pause = libc::timespec {
            tv_sec: 0,
            tv_nsec: 1_000_000,
        };
        if unsafe { libc::nanosleep(&pause, std::ptr::null_mut()) } != 0
            && super::super::errno() != libc::EINTR
        {
            return Err(());
        }
    }
}

fn nonblocking(fd: i32) -> Result<(), ()> {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } != 0 {
        Err(())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{Exit, State, Stream, REPORT_LIMIT};

    fn exit(status: i32) -> Exit {
        Exit {
            pid: 7,
            signo: libc::SIGCHLD,
            code: libc::CLD_EXITED,
            status,
        }
    }

    #[test]
    fn complete_bytes_need_both_eofs_and_an_ordinary_exit() {
        let mut state = State::new().unwrap();
        state.bytes(Stream::Report, b"{}\n").unwrap();
        state.eof(Stream::Report).unwrap();
        state.exit(exit(0)).unwrap();
        assert!(!state.ready());
        state.eof(Stream::Error).unwrap();
        assert!(state.ready());
    }

    #[test]
    fn stderr_overflow_and_abnormal_exit_are_sticky() {
        let mut stderr = State::new().unwrap();
        assert!(stderr.bytes(Stream::Error, b"x").is_err());
        assert!(stderr.bytes(Stream::Report, b"{}\n").is_err());
        let mut abnormal = State::new().unwrap();
        assert!(abnormal.exit(exit(2)).is_err());
        assert!(abnormal.eof(Stream::Report).is_err());
    }

    #[test]
    fn report_budget_is_cumulative_and_exact() {
        let mut exact = State::new().unwrap();
        exact
            .bytes(Stream::Report, &vec![b'x'; REPORT_LIMIT])
            .unwrap();
        assert!(exact.bytes(Stream::Report, b"x").is_err());
    }
}
