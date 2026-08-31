//! Scripted control-flow evidence over drive(), not process/OS evidence.
use super::*;
use std::collections::VecDeque;

#[derive(Debug)]
enum Step {
    Read(usize, Result<Option<usize>, ()>),
    Observe(Result<bool, ()>),
    Time(Duration),
    Kill(Result<(), ()>),
    Reap(Result<Option<bool>, ()>),
    Pause,
    Stop,
}
use Step::*;

struct Script {
    steps: VecDeque<Step>,
    stopped: bool,
}
impl Script {
    fn new(steps: Vec<Step>) -> Self {
        Self {
            steps: steps.into(),
            stopped: false,
        }
    }
    fn next(&mut self) -> Step {
        self.steps.pop_front().expect("unexpected later action")
    }
}
impl Operations for Script {
    fn read(&mut self, stream: usize, buffer: &mut [u8; 8192]) -> Result<Option<usize>, ()> {
        match self.next() {
            Read(expected, result) => {
                assert_eq!(stream, expected, "stream fairness/order");
                buffer.fill(if stream == 0 { b'a' } else { b'b' });
                result
            }
            step => panic!("expected read, got {step:?}"),
        }
    }
    fn observe_exit(&mut self) -> Result<bool, ()> {
        match self.next() {
            Observe(value) => value,
            step => panic!("expected observe, got {step:?}"),
        }
    }
    fn kill_owned(&mut self) -> Result<(), ()> {
        match self.next() {
            Kill(value) => value,
            step => panic!("expected kill, got {step:?}"),
        }
    }
    fn reap_owned(&mut self) -> Result<Option<bool>, ()> {
        match self.next() {
            Reap(value) => value,
            step => panic!("expected reap, got {step:?}"),
        }
    }
    fn now(&mut self) -> Duration {
        match self.next() {
            Time(value) => value,
            step => panic!("expected clock, got {step:?}"),
        }
    }
    fn pause(&mut self) {
        assert!(matches!(self.next(), Pause));
    }
    fn fail_stop(&mut self) -> ! {
        assert!(matches!(self.next(), Stop));
        self.stopped = true;
        panic!("scripted fail-stop; no physical process exists")
    }
}
fn time(seconds: u64) -> Step {
    Time(Duration::from_secs(seconds))
}
fn data(stream: usize, length: usize) -> Step {
    Read(stream, Ok(Some(length)))
}
fn again(stream: usize) -> Step {
    Read(stream, Ok(None))
}

fn complete(steps: Vec<Step>, expected: Result<Vec<u8>, ProbeError>) {
    let mut script = Script::new(steps);
    let mut output = Vec::with_capacity(65_536);
    let allocation = (output.as_ptr(), output.capacity());
    let result = drive(&mut script, &mut output);
    assert!(!script.stopped);
    assert!(
        script.steps.is_empty(),
        "missing actions: {:?}",
        script.steps
    );
    assert_eq!((output.as_ptr(), output.capacity()), allocation);
    match expected {
        Ok(bytes) => {
            assert_eq!(result, Ok(()));
            assert_eq!(output, bytes);
        }
        Err(error) => {
            assert_eq!(result, Err(error));
            assert!(output.is_empty());
        }
    }
}

fn fatal(mut steps: Vec<Step>) {
    steps.push(Stop);
    let mut script = Script::new(steps);
    let mut output = Vec::with_capacity(65_536);
    let mut returned = false;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _ = drive(&mut script, &mut output);
        returned = true;
    }));
    assert!(result.is_err());
    assert!(
        script.stopped,
        "unexpected assertion panic is not fail-stop evidence"
    );
    assert!(
        script.steps.is_empty(),
        "missing actions: {:?}",
        script.steps
    );
    assert!(!returned, "no ordinary result after uncertainty");
}

#[test]
fn eof_is_not_exit_and_success_requires_one_kill_reap_and_full_drain() {
    complete(
        vec![
            data(0, 0),
            data(1, 0),
            Observe(Ok(false)),
            time(0),
            Pause,
            Observe(Ok(true)),
            time(1),
            Kill(Ok(())),
            Reap(Ok(None)),
            time(1),
            Pause,
            Reap(Ok(Some(true))),
        ],
        Ok(Vec::new()),
    );
    complete(
        vec![
            data(0, 3),
            data(1, 2),
            Observe(Ok(true)),
            time(0),
            Kill(Ok(())),
            Reap(Ok(Some(true))),
            time(0),
            data(0, 2),
            data(1, 1),
            time(0),
            data(0, 0),
            data(1, 0),
        ],
        Ok(vec![b'a'; 5]),
    );
}

#[test]
fn exact_combined_output_bound_counts_stderr_without_publishing_it() {
    let mut steps = Vec::new();
    for index in 0..8 {
        steps.extend([data(0, 4096), data(1, 4096), Observe(Ok(index == 7))]);
        if index != 7 {
            steps.extend([time(0), Pause]);
        }
    }
    steps.extend([
        time(0),
        Kill(Ok(())),
        Reap(Ok(Some(true))),
        time(0),
        data(0, 0),
        data(1, 0),
    ]);
    complete(steps, Ok(vec![b'a'; 32_768]));
}

#[test]
fn overflow_retains_first_failure_over_same_turn_io_and_killed_exit() {
    let mut steps = Vec::new();
    for _ in 0..8 {
        steps.extend([data(0, 8192), again(1), Observe(Ok(false)), time(0), Pause]);
    }
    steps.extend([
        data(0, 1),
        Read(1, Err(())),
        time(0),
        Kill(Ok(())),
        Reap(Ok(Some(false))),
        time(0),
        data(0, 0),
        data(1, 0),
    ]);
    complete(steps, Err(ProbeError::OutputLimit));
}

#[test]
fn read_and_observation_failures_clear_partial_output_after_settlement() {
    for first in [Read(0, Err(())), data(0, 8193)] {
        complete(
            vec![
                first,
                data(1, 1),
                time(0),
                Kill(Ok(())),
                Reap(Ok(Some(false))),
                time(0),
                data(0, 0),
                data(1, 0),
            ],
            Err(ProbeError::Io),
        );
    }
    complete(
        vec![
            data(0, 3),
            again(1),
            Observe(Err(())),
            time(0),
            Kill(Ok(())),
            Reap(Ok(Some(false))),
            time(0),
            data(0, 2),
            data(1, 0),
            time(0),
            data(0, 0),
        ],
        Err(ProbeError::Io),
    );
    complete(
        vec![
            data(0, 3),
            data(1, 0),
            Observe(Ok(true)),
            time(0),
            Kill(Ok(())),
            Reap(Ok(Some(false))),
            time(0),
            data(0, 0),
        ],
        Err(ProbeError::Exit),
    );
}

#[test]
fn deadline_includes_preclone_time_and_settlement_budget_is_not_reset_by_drain() {
    // Native's origin is captured before clone; a first observation at 10s
    // must already time out rather than receive another execution budget.
    complete(
        vec![
            again(0),
            again(1),
            Observe(Ok(false)),
            time(10),
            time(10),
            Kill(Ok(())),
            Reap(Ok(Some(false))),
            time(10),
            data(0, 0),
            data(1, 0),
        ],
        Err(ProbeError::Timeout),
    );
    fatal(vec![
        again(0),
        again(1),
        Observe(Ok(false)),
        time(10),
        time(10),
        Kill(Ok(())),
        Reap(Ok(None)),
        time(14),
        Pause,
        Reap(Ok(Some(false))),
        time(15),
    ]);
}

#[test]
fn uncertain_kill_wait_drain_or_time_never_returns_or_performs_later_actions() {
    fatal(vec![
        data(0, 1),
        data(1, 0),
        Observe(Ok(true)),
        time(0),
        Kill(Err(())),
    ]);
    fatal(vec![
        data(0, 1),
        data(1, 0),
        Observe(Ok(true)),
        time(0),
        Kill(Ok(())),
        Reap(Err(())),
    ]);
    fatal(vec![
        data(0, 1),
        data(1, 0),
        Observe(Ok(true)),
        time(0),
        Kill(Ok(())),
        Reap(Ok(None)),
        time(5),
    ]);
    fatal(vec![
        data(0, 1),
        data(1, 0),
        Observe(Ok(true)),
        time(0),
        Kill(Ok(())),
        Reap(Ok(Some(true))),
        time(0),
        Read(0, Err(())),
    ]);
    fatal(vec![
        data(0, 1),
        data(1, 0),
        Observe(Ok(true)),
        Time(Duration::MAX),
    ]);
}

#[test]
fn accounting_rejects_overflow_and_capacity_before_copy_without_growth() {
    let mut output = Vec::new();
    let mut total = 0;
    assert_eq!(
        account(&mut total, b"x", Some(&mut output)),
        Err(ProbeError::OutputLimit)
    );
    assert!(output.is_empty());
    assert_eq!(output.capacity(), 0);
    total = usize::MAX;
    assert_eq!(
        account(&mut total, b"x", None),
        Err(ProbeError::OutputLimit)
    );
    assert_eq!(total, usize::MAX);
    total = 65_536;
    assert_eq!(
        account(&mut total, b"x", None),
        Err(ProbeError::OutputLimit)
    );
}
