//! Production ownership decisions over authority-free scripted operations.
use super::*;
use std::collections::VecDeque;

#[derive(Debug)]
enum Step {
    Wait(Wait, Result<Option<Exit>, i32>),
    Kill(Result<(), i32>),
    Close(Descriptor, Result<(), i32>),
    Time(Duration),
    Pause,
    Stop,
}
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
    fn exhausted(&self) {
        assert!(self.steps.is_empty(), "missing actions: {:?}", self.steps);
    }
}
impl Operations for Script {
    fn wait(&mut self, mode: Wait) -> Result<Option<Exit>, i32> {
        match self.next() {
            Step::Wait(expected, result) => {
                assert_eq!(mode, expected);
                result
            }
            step => panic!("expected wait, got {step:?}"),
        }
    }
    fn kill(&mut self) -> Result<(), i32> {
        match self.next() {
            Step::Kill(result) => result,
            step => panic!("expected kill, got {step:?}"),
        }
    }
    fn close(&mut self, descriptor: Descriptor) -> Result<(), i32> {
        match self.next() {
            Step::Close(expected, result) => {
                assert_eq!(descriptor, expected);
                result
            }
            step => panic!("expected close, got {step:?}"),
        }
    }
    fn elapsed(&mut self) -> Duration {
        match self.next() {
            Step::Time(value) => value,
            step => panic!("expected time, got {step:?}"),
        }
    }
    fn pause(&mut self) {
        assert!(matches!(self.next(), Step::Pause));
    }
    fn stop(&mut self) -> ! {
        assert!(matches!(self.next(), Step::Stop));
        self.stopped = true;
        panic!("scripted fail-stop, no process authority exists")
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
fn time(seconds: u64) -> Step {
    Step::Time(Duration::from_secs(seconds))
}
fn authenticate() -> Step {
    Step::Wait(Wait::Observe, Ok(None))
}
fn reap() -> Step {
    Step::Wait(Wait::Reap, Ok(Some(exit())))
}
fn stopped(steps: Vec<Step>, action: impl FnOnce(&mut State, &mut Script)) -> Phase {
    let mut script = Script::new(steps);
    let mut state = State::new();
    let mut returned = false;
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        action(&mut state, &mut script);
        returned = true;
    }));
    assert!(caught.is_err());
    assert!(
        script.stopped,
        "an assertion panic is not fail-stop evidence"
    );
    assert!(!returned, "uncertainty must not return an ordinary result");
    script.exhausted();
    state.phase
}

#[test]
fn invalid_or_nonchild_authentication_never_grants_signaling_authority() {
    for result in [
        Err(libc::ECHILD),
        Err(libc::EBADF),
        Err(libc::EINVAL),
        Ok(Some(Exit { pid: -1, ..exit() })),
        Ok(Some(Exit { pid: 0, ..exit() })),
        Ok(Some(Exit { signo: 0, ..exit() })),
        Ok(Some(Exit {
            code: libc::CLD_STOPPED,
            ..exit()
        })),
        Ok(Some(Exit {
            code: libc::CLD_CONTINUED,
            ..exit()
        })),
    ] {
        let phase = stopped(
            vec![Step::Wait(Wait::Observe, result), Step::Stop],
            |state, script| {
                assert_eq!(state.authenticate(script), Err(()));
                assert_eq!(state.observe(script), Err(()));
                assert_eq!(state.reap(script), Err(()));
                state.abort(script);
            },
        );
        assert_eq!(phase, Phase::Unvalidated);
    }
    assert_eq!(
        stopped(vec![Step::Stop], |state, script| state
            .cleanup_on_drop(script)),
        Phase::Unvalidated
    );
    assert_eq!(
        stopped(vec![Step::Stop], |state, script| state.complete(script)),
        Phase::Unvalidated
    );
}

#[test]
fn successful_child_wait_even_without_exit_grants_only_owned_unreaped_state() {
    for observed in [
        None,
        Some(exit()),
        Some(Exit {
            code: libc::CLD_KILLED,
            status: 9,
            ..exit()
        }),
        Some(Exit {
            code: libc::CLD_DUMPED,
            status: 11,
            ..exit()
        }),
    ] {
        assert_eq!(
            stopped(
                vec![
                    Step::Wait(Wait::Observe, Ok(observed)),
                    time(0),
                    Step::Kill(Ok(())),
                    reap(),
                    Step::Stop
                ],
                |state, script| {
                    assert_eq!(state.authenticate(script), Ok(()));
                    assert_eq!(state.phase, Phase::Owned);
                    assert_eq!(state.authenticate(script), Err(()));
                    state.abort(script);
                }
            ),
            Phase::Reaped
        );
    }
}

#[test]
fn consuming_wait_latches_before_caller_checks_and_never_consumes_twice() {
    for consumed in [
        exit(),
        Exit { pid: 124, ..exit() },
        Exit {
            status: 7,
            ..exit()
        },
        Exit {
            code: libc::CLD_KILLED,
            status: 9,
            ..exit()
        },
    ] {
        assert_eq!(
            stopped(
                vec![
                    authenticate(),
                    Step::Wait(Wait::Observe, Ok(Some(exit()))),
                    Step::Wait(Wait::Reap, Ok(Some(consumed))),
                    Step::Stop
                ],
                |state, script| {
                    assert_eq!(state.authenticate(script), Ok(()));
                    assert_eq!(state.observe(script), Ok(Some(exit())));
                    assert_eq!(state.reap(script), Ok(Some(consumed)));
                    assert_eq!(state.phase, Phase::Reaped);
                    assert_eq!(state.reap(script), Err(()));
                    assert_eq!(state.observe(script), Err(()));
                    assert_eq!(state.authenticate(script), Err(()));
                    // Capture may now reject the event comparison. No kill/wait.
                    state.cleanup_on_drop(script);
                }
            ),
            Phase::Reaped
        );
    }
}

#[test]
fn uncertain_observation_or_unconsumed_wait_keeps_cleanup_owned() {
    for outcome in [Ok(None), Err(libc::EINTR)] {
        assert_eq!(
            stopped(
                vec![
                    authenticate(),
                    Step::Wait(Wait::Observe, Err(libc::EINTR)),
                    Step::Wait(Wait::Reap, outcome),
                    time(8),
                    Step::Kill(Ok(())),
                    reap(),
                    Step::Stop
                ],
                |state, script| {
                    assert_eq!(state.authenticate(script), Ok(()));
                    assert_eq!(state.observe(script), Err(()));
                    assert_eq!(state.reap(script), outcome.map_err(|_| ()));
                    assert_eq!(state.phase, Phase::Owned);
                    state.cleanup_on_drop(script);
                }
            ),
            Phase::Reaped
        );
    }
}

#[test]
fn kill_failure_stops_and_esrch_still_requires_exact_consumption() {
    for error in [libc::EPERM, libc::EINVAL, libc::EINTR] {
        assert_eq!(
            stopped(
                vec![authenticate(), time(0), Step::Kill(Err(error)), Step::Stop],
                |state, script| {
                    assert_eq!(state.authenticate(script), Ok(()));
                    state.abort(script);
                }
            ),
            Phase::Owned
        );
    }
    assert_eq!(
        stopped(
            vec![
                authenticate(),
                time(10),
                Step::Kill(Err(libc::ESRCH)),
                Step::Wait(Wait::Reap, Ok(None)),
                time(11),
                Step::Pause,
                reap(),
                Step::Stop
            ],
            |state, script| {
                assert_eq!(state.authenticate(script), Ok(()));
                state.abort(script);
            }
        ),
        Phase::Reaped
    );
    assert_eq!(
        stopped(
            vec![
                authenticate(),
                time(0),
                Step::Kill(Err(libc::ESRCH)),
                Step::Wait(Wait::Reap, Err(libc::ECHILD)),
                Step::Stop
            ],
            |state, script| {
                assert_eq!(state.authenticate(script), Ok(()));
                state.abort(script);
            }
        ),
        Phase::Owned
    );
}

#[test]
fn settlement_deadline_is_one_fixed_budget_and_wait_errors_are_terminal() {
    for error in [libc::ECHILD, libc::EINTR, libc::EINVAL] {
        stopped(
            vec![
                authenticate(),
                time(0),
                Step::Kill(Ok(())),
                Step::Wait(Wait::Reap, Err(error)),
                Step::Stop,
            ],
            |state, script| {
                assert_eq!(state.authenticate(script), Ok(()));
                state.abort(script);
            },
        );
    }
    assert_eq!(
        stopped(
            vec![
                authenticate(),
                time(20),
                Step::Kill(Ok(())),
                Step::Wait(Wait::Reap, Ok(None)),
                time(24),
                Step::Pause,
                Step::Wait(Wait::Reap, Ok(None)),
                time(25),
                Step::Stop
            ],
            |state, script| {
                assert_eq!(state.authenticate(script), Ok(()));
                state.abort(script);
            }
        ),
        Phase::Owned
    );
    stopped(
        vec![authenticate(), Step::Time(Duration::MAX), Step::Stop],
        |state, script| {
            assert_eq!(state.authenticate(script), Ok(()));
            state.abort(script);
        },
    );
}

#[test]
fn completion_requires_reap_before_first_close_and_closes_exact_inventory_once() {
    assert_eq!(
        stopped(
            vec![
                authenticate(),
                time(0),
                Step::Kill(Ok(())),
                reap(),
                Step::Stop
            ],
            |state, script| {
                assert_eq!(state.authenticate(script), Ok(()));
                state.complete(script);
            }
        ),
        Phase::Reaped
    );
    let mut script = Script::new(vec![
        authenticate(),
        reap(),
        Step::Close(Descriptor::Request, Ok(())),
        Step::Close(Descriptor::Bundle, Ok(())),
        Step::Close(Descriptor::Reply, Ok(())),
        Step::Close(Descriptor::Stderr, Ok(())),
        Step::Close(Descriptor::Pidfd, Ok(())),
    ]);
    let mut state = State::new();
    assert_eq!(state.authenticate(&mut script), Ok(()));
    assert_eq!(state.reap(&mut script), Ok(Some(exit())));
    state.complete(&mut script);
    assert_eq!(state.phase, Phase::Closed);
    assert_eq!(state.authenticate(&mut script), Err(()));
    assert_eq!(state.observe(&mut script), Err(()));
    assert_eq!(state.reap(&mut script), Err(()));
    state.cleanup_on_drop(&mut script);
    script.exhausted();
    assert!(!script.stopped);
    // Explicit abort is still terminal even after successful disarm.
    script.steps.push_back(Step::Stop);
    let stopped =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.abort(&mut script)));
    assert!(stopped.is_err() && script.stopped);
    script.exhausted();
}

#[test]
fn every_close_uncertainty_stops_before_retry_later_close_or_disarm() {
    let inventory = [
        Descriptor::Request,
        Descriptor::Bundle,
        Descriptor::Reply,
        Descriptor::Stderr,
        Descriptor::Pidfd,
    ];
    for failed in 0..inventory.len() {
        for error in [libc::EINTR, libc::EBADF, libc::EIO] {
            let mut steps = vec![authenticate(), reap()];
            for (index, descriptor) in inventory.iter().copied().enumerate().take(failed + 1) {
                steps.push(Step::Close(
                    descriptor,
                    if index == failed { Err(error) } else { Ok(()) },
                ));
            }
            steps.push(Step::Stop);
            assert_eq!(
                stopped(steps, |state, script| {
                    assert_eq!(state.authenticate(script), Ok(()));
                    assert_eq!(state.reap(script), Ok(Some(exit())));
                    state.complete(script);
                }),
                Phase::Reaped
            );
        }
    }
}
