//! Authority-free scripts execute the production collector capture control flow.
use super::*;
use std::collections::VecDeque;

#[derive(Debug)]
enum Step {
    Time(u64),
    Read(usize, Result<Option<usize>, ()>),
    Observe(Result<Option<Exit>, ()>),
    Reap(Result<Option<Exit>, ()>),
    Pause,
}
use Step::*;
struct Script(VecDeque<Step>);
impl Script {
    fn next(&mut self) -> Step {
        self.0.pop_front().expect("unexpected later action")
    }
}
impl Operations for Script {
    fn read(&mut self, stream: usize, bytes: &mut [u8; 8192]) -> Result<Option<usize>, ()> {
        match self.next() {
            Read(expected, result) => {
                assert_eq!(expected, stream);
                bytes.fill(b'x');
                result
            }
            step => panic!("expected read, got {step:?}"),
        }
    }
    fn observe(&mut self) -> Result<Option<Exit>, ()> {
        match self.next() {
            Observe(value) => value,
            step => panic!("expected observe, got {step:?}"),
        }
    }
    fn reap(&mut self) -> Result<Option<Exit>, ()> {
        match self.next() {
            Reap(value) => value,
            step => panic!("expected reap, got {step:?}"),
        }
    }
    fn elapsed(&mut self) -> Duration {
        match self.next() {
            Time(value) => Duration::from_secs(value),
            step => panic!("expected time, got {step:?}"),
        }
    }
    fn pause(&mut self) {
        assert!(matches!(self.next(), Pause));
    }
}
fn exit() -> Exit {
    Exit {
        pid: 123,
        signo: libc::SIGCHLD,
        code: libc::CLD_EXITED,
        status: 0,
    }
}
fn data(stream: usize, count: usize) -> Step {
    Read(stream, Ok(Some(count)))
}
fn again(stream: usize) -> Step {
    Read(stream, Ok(None))
}
fn check(steps: Vec<Step>, expected: Result<usize, ()>) {
    let mut script = Script(steps.into());
    let actual = collect(&mut script);
    assert!(script.0.is_empty(), "missing actions: {:?}", script.0);
    assert_eq!(actual.clone().map(Vec::len), expected);
    if let Ok(bytes) = actual {
        assert!(bytes.iter().all(|byte| *byte == b'x'));
    }
}

#[test]
fn requires_exact_exit_reap_and_eof_without_reobserving_reaped_worker() {
    check(
        vec![
            Time(0),
            data(0, 10),
            again(1),
            Observe(Ok(Some(exit()))),
            Reap(Ok(Some(exit()))),
            Pause,
            Time(1),
            data(0, 4),
            data(1, 0),
            Pause,
            Time(2),
            data(0, 0),
        ],
        Ok(14),
    );
    check(
        vec![
            Time(0),
            data(0, 0),
            data(1, 0),
            Observe(Ok(None)),
            Pause,
            Time(1),
            Observe(Ok(Some(exit()))),
            Reap(Ok(Some(exit()))),
        ],
        Ok(0),
    );
}

#[test]
fn eof_or_complete_bytes_never_substitute_for_successful_owned_exit() {
    for bad in [
        Exit {
            status: 1,
            ..exit()
        },
        Exit {
            code: libc::CLD_KILLED,
            ..exit()
        },
        Exit { pid: 0, ..exit() },
        Exit { signo: 0, ..exit() },
    ] {
        check(
            vec![Time(0), data(0, 100), data(1, 0), Observe(Ok(Some(bad)))],
            Err(()),
        );
    }
    check(
        vec![
            Time(0),
            data(0, 0),
            data(1, 0),
            Observe(Ok(None)),
            Pause,
            Time(60),
        ],
        Err(()),
    );
    check(vec![Time(60)], Err(()));
}

#[test]
fn missing_or_different_reap_and_io_error_stop_without_later_actions() {
    for result in [
        Ok(None),
        Err(()),
        Ok(Some(Exit { pid: 124, ..exit() })),
        Ok(Some(Exit {
            status: 1,
            ..exit()
        })),
    ] {
        check(
            vec![
                Time(0),
                data(0, 1),
                data(1, 0),
                Observe(Ok(Some(exit()))),
                Reap(result),
            ],
            Err(()),
        );
    }
    check(vec![Time(0), Read(0, Err(()))], Err(()));
    check(vec![Time(0), data(0, 8193)], Err(()));
    check(
        vec![Time(0), data(0, 1), data(1, 0), Observe(Err(()))],
        Err(()),
    );
    check(
        vec![
            Time(0),
            data(0, 1),
            data(1, 0),
            Observe(Ok(Some(exit()))),
            Reap(Ok(Some(exit()))),
            Pause,
            Time(1),
            Read(0, Err(())),
        ],
        Err(()),
    );
}

#[test]
fn stderr_is_rejected_and_reply_limit_is_exact_without_starving_other_stream() {
    check(vec![Time(0), data(0, 1), data(1, 1)], Err(()));
    let mut prefix = Vec::new();
    let full = wire::MAX_REPLY_BYTES / 8192;
    let tail = wire::MAX_REPLY_BYTES % 8192;
    for _ in 0..full {
        prefix.extend([Time(0), data(0, 8192), again(1), Observe(Ok(None)), Pause]);
    }
    prefix.extend([Time(0), data(0, tail), again(1), Observe(Ok(None)), Pause]);
    // Recreate the same prefix for exact/plus-one without cloning authority.
    let mut exact = Vec::new();
    for _ in 0..full {
        exact.extend([Time(0), data(0, 8192), again(1), Observe(Ok(None)), Pause]);
    }
    exact.extend([
        Time(0),
        data(0, tail),
        again(1),
        Observe(Ok(None)),
        Pause,
        Time(0),
        data(0, 0),
        data(1, 0),
        Observe(Ok(Some(exit()))),
        Reap(Ok(Some(exit()))),
    ]);
    check(exact, Ok(wire::MAX_REPLY_BYTES));
    prefix.extend([Time(0), data(0, 1)]);
    check(prefix, Err(()));
}
