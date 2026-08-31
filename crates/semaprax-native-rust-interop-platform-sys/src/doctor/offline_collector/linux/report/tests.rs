//! Scripts exercise the production delivery loop, not physical signal/pipe state.
use super::*;
use std::collections::VecDeque;

#[derive(Debug)]
enum Step {
    Pipe(i32, bool),
    Nonblocking(bool),
    EmptySignalSet(bool),
    AddSigpipe(bool),
    BlockSigpipe(bool),
    Start,
    Time(Duration),
    Write(Vec<u8>, Result<usize, i32>),
    Pause,
    Close(i32, bool),
}
use Step::*;

struct Script(VecDeque<Step>);

fn result(success: bool) -> Result<(), ()> {
    if success {
        Ok(())
    } else {
        Err(())
    }
}

impl Script {
    fn next(&mut self) -> Step {
        self.0
            .pop_front()
            .expect("unexpected later delivery action")
    }
}

impl Operations for Script {
    fn require_pipe(&mut self, fd: i32) -> Result<(), ()> {
        match self.next() {
            Pipe(expected, success) => {
                assert_eq!(fd, expected);
                result(success)
            }
            step => panic!("expected pipe check, got {step:?}"),
        }
    }
    fn nonblocking_stdout(&mut self) -> Result<(), ()> {
        match self.next() {
            Nonblocking(success) => result(success),
            step => panic!("expected nonblocking setup, got {step:?}"),
        }
    }
    fn empty_signal_set(&mut self) -> Result<(), ()> {
        match self.next() {
            EmptySignalSet(success) => result(success),
            step => panic!("expected signal set initialization, got {step:?}"),
        }
    }
    fn add_sigpipe(&mut self) -> Result<(), ()> {
        match self.next() {
            AddSigpipe(success) => result(success),
            step => panic!("expected SIGPIPE insertion, got {step:?}"),
        }
    }
    fn block_sigpipe(&mut self) -> Result<(), ()> {
        match self.next() {
            BlockSigpipe(success) => result(success),
            step => panic!("expected SIGPIPE blocking, got {step:?}"),
        }
    }
    fn start_clock(&mut self) {
        assert!(matches!(self.next(), Start));
    }
    fn elapsed(&mut self) -> Duration {
        match self.next() {
            Time(elapsed) => elapsed,
            step => panic!("expected elapsed time, got {step:?}"),
        }
    }
    fn write(&mut self, bytes: &[u8]) -> Result<usize, i32> {
        match self.next() {
            Write(expected, result) => {
                assert_eq!(bytes, expected, "wrong report bytes or write offset");
                assert!(!bytes.is_empty() && bytes.len() <= WRITE_CHUNK);
                result
            }
            step => panic!("expected report write, got {step:?}"),
        }
    }
    fn pause(&mut self) {
        assert!(matches!(self.next(), Pause));
    }
    fn close(&mut self, fd: i32) -> Result<(), ()> {
        match self.next() {
            Close(expected, success) => {
                assert_eq!(fd, expected);
                result(success)
            }
            step => panic!("expected owned close, got {step:?}"),
        }
    }
}

fn setup() -> Vec<Step> {
    vec![
        Pipe(0, true),
        Pipe(1, true),
        Pipe(2, true),
        Nonblocking(true),
        EmptySignalSet(true),
        AddSigpipe(true),
        BlockSigpipe(true),
        Start,
    ]
}

fn closure() -> [Step; 3] {
    [Close(1, true), Close(0, true), Close(2, true)]
}

fn check(report: &[u8], status: u8, steps: Vec<Step>, expected: Result<u8, ()>) {
    let mut script = Script(steps.into());
    assert_eq!(deliver(report, status, &mut script), expected);
    assert!(
        script.0.is_empty(),
        "missing delivery actions: {:?}",
        script.0
    );
}

#[test]
fn bounds_and_status_reject_before_any_operation() {
    for status in 2..=u8::MAX {
        check(b"report", status, vec![], Err(()));
    }
    check(&vec![0; MAX_REPORT_BYTES + 1], 0, vec![], Err(()));
}

#[test]
fn every_setup_failure_stops_before_write_clock_or_close() {
    for failure in 0..7 {
        let mut steps = setup();
        steps.truncate(failure + 1);
        steps[failure] = match steps[failure] {
            Pipe(fd, _) => Pipe(fd, false),
            Nonblocking(_) => Nonblocking(false),
            EmptySignalSet(_) => EmptySignalSet(false),
            AddSigpipe(_) => AddSigpipe(false),
            BlockSigpipe(_) => BlockSigpipe(false),
            _ => panic!("not a setup stage"),
        };
        check(b"report", 0, steps, Err(()));
    }
}

#[test]
fn both_statuses_require_exact_bytes_then_canonical_close_order() {
    for status in [0, 1] {
        let mut steps = setup();
        steps.extend([Time(Duration::ZERO), Write(b"report\n".to_vec(), Ok(7))]);
        steps.extend(closure());
        check(b"report\n", status, steps, Ok(status));
    }
    // Preserve the existing trusted-caller behavior, not a public empty-report
    // admission route. The real doctor renderer never supplies an empty report.
    let mut steps = setup();
    steps.extend(closure());
    check(b"", 0, steps, Ok(0));
}

#[test]
fn exact_limit_is_emitted_in_bounded_chunks_without_lost_or_repeated_bytes() {
    let report = (0..MAX_REPORT_BYTES)
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let mut steps = setup();
    for chunk in report.chunks(WRITE_CHUNK) {
        steps.extend([Time(Duration::ZERO), Write(chunk.to_vec(), Ok(chunk.len()))]);
    }
    steps.extend(closure());
    check(&report, 0, steps, Ok(0));
}

#[test]
fn partial_writes_and_repeated_eagain_keep_exact_remaining_suffix() {
    let mut steps = setup();
    steps.extend([
        Time(Duration::ZERO),
        Write(b"abcdefgh".to_vec(), Ok(3)),
        Time(Duration::from_secs(1)),
        Write(b"defgh".to_vec(), Err(libc::EAGAIN)),
        Pause,
        Time(Duration::from_secs(2)),
        Write(b"defgh".to_vec(), Err(libc::EAGAIN)),
        Pause,
        Time(Duration::from_secs(3)),
        Write(b"defgh".to_vec(), Ok(2)),
        Time(Duration::from_secs(4)),
        Write(b"gh".to_vec(), Ok(2)),
    ]);
    steps.extend(closure());
    check(b"abcdefgh", 1, steps, Ok(1));
}

#[test]
fn deadline_is_exact_and_eagain_does_not_restart_it() {
    let mut steps = setup();
    steps.push(Time(WRITE_BUDGET));
    check(b"report", 0, steps, Err(()));

    let mut steps = setup();
    steps.extend([
        Time(Duration::from_millis(4999)),
        Write(b"report".to_vec(), Err(libc::EAGAIN)),
        Pause,
        Time(WRITE_BUDGET),
    ]);
    check(b"report", 0, steps, Err(()));

    let mut steps = setup();
    steps.extend([
        Time(Duration::from_millis(4999)),
        Write(b"report".to_vec(), Ok(6)),
    ]);
    steps.extend(closure());
    check(b"report", 0, steps, Ok(0));
}

#[test]
fn zero_oversized_and_failed_writes_never_retry_close_or_publish_success() {
    for outcome in [
        Ok(0),
        Ok(7),
        Ok(usize::MAX),
        Err(libc::EPIPE),
        Err(libc::EINTR),
        Err(libc::EIO),
        Err(libc::EBADF),
        Err(0),
    ] {
        let mut steps = setup();
        steps.extend([Time(Duration::ZERO), Write(b"report".to_vec(), outcome)]);
        check(b"report", 0, steps, Err(()));
    }
    let mut steps = setup();
    steps.extend([
        Time(Duration::ZERO),
        Write(b"report".to_vec(), Ok(2)),
        Time(Duration::from_secs(1)),
        Write(b"port".to_vec(), Err(libc::EINTR)),
    ]);
    check(b"report", 1, steps, Err(()));
}

#[test]
fn every_uncertain_close_stops_without_retry_or_later_close() {
    for failure in 0..3 {
        let mut steps = setup();
        steps.extend([Time(Duration::ZERO), Write(b"report".to_vec(), Ok(6))]);
        for (index, fd) in [1, 0, 2].into_iter().enumerate().take(failure + 1) {
            steps.push(Close(fd, index != failure));
        }
        check(b"report", 1, steps, Err(()));
    }
}
