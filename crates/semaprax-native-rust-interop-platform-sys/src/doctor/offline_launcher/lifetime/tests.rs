//! Resource-free scripts exercise production decisions, never native settlement.
use super::*;
use std::collections::VecDeque;

#[derive(Debug, Eq, PartialEq)]
enum Event {
    Clock,
    Kill(i32),
    Wait(i32),
    Pause(u64),
    Close(i32),
    Stop,
}

#[derive(Debug)]
struct Stopped;

struct Script {
    events: Vec<Event>,
    clocks: VecDeque<Result<u64, ()>>,
    waits: VecDeque<Result<Option<Exit>, ()>>,
    kill: Result<(), i32>,
    pause: Result<(), ()>,
    close: Result<(), ()>,
}

impl Script {
    fn completed(exit: Exit) -> Self {
        Self {
            events: Vec::new(),
            clocks: [Ok(100), Ok(100)].into(),
            waits: [Ok(Some(exit))].into(),
            kill: Ok(()),
            pause: Ok(()),
            close: Ok(()),
        }
    }
}

impl Operations for Script {
    fn now(&mut self) -> Result<u64, ()> {
        self.events.push(Event::Clock);
        self.clocks.pop_front().expect("unexpected clock operation")
    }
    fn kill(&mut self, fd: i32) -> Result<(), i32> {
        self.events.push(Event::Kill(fd));
        self.kill
    }
    fn wait(&mut self, fd: i32) -> Result<Option<Exit>, ()> {
        self.events.push(Event::Wait(fd));
        self.waits.pop_front().expect("unexpected wait operation")
    }
    fn pause(&mut self, nanos: u64) -> Result<(), ()> {
        self.events.push(Event::Pause(nanos));
        assert!((1..=1_000_000).contains(&nanos));
        self.pause
    }
    fn close(&mut self, fd: i32) -> Result<(), ()> {
        self.events.push(Event::Close(fd));
        self.close
    }
    fn stop(&mut self) -> ! {
        self.events.push(Event::Stop);
        std::panic::panic_any(Stopped)
    }
}

fn exited(code: i32, status: i32) -> Exit {
    Exit {
        pid: 77,
        signo: libc::SIGCHLD,
        code,
        status,
    }
}

fn abort(state: &mut State, script: &mut Script) {
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| state.abort(script)))
        .expect_err("abort must never return");
    assert!(
        stopped.is::<Stopped>(),
        "unexpected scripted operation or assertion"
    );
}

fn no_later_authority(state: &mut State, script: &mut Script) {
    let before = script.events.len();
    abort(state, script);
    assert_eq!(&script.events[before..], &[Event::Stop]);
}

#[test]
fn each_exact_terminal_event_reaps_once_then_closes_and_still_stops() {
    for (code, status) in [
        (libc::CLD_EXITED, 0),
        (libc::CLD_EXITED, 255),
        (libc::CLD_KILLED, libc::SIGKILL),
        (libc::CLD_DUMPED, libc::SIGSEGV),
    ] {
        let mut state = State::new(23, 77);
        let mut script = Script::completed(exited(code, status));
        abort(&mut state, &mut script);
        assert_eq!(state.phase, Phase::Closed);
        assert_eq!(
            script.events,
            [
                Event::Clock,
                Event::Kill(23),
                Event::Clock,
                Event::Wait(23),
                Event::Close(23),
                Event::Stop
            ]
        );
        no_later_authority(&mut state, &mut script);
    }
}

#[test]
fn redirect_switches_all_settlement_operations_before_old_descriptor_closure() {
    let mut state = State::new(23, 77);
    assert!(!state.redirect(-1));
    assert!(!state.redirect(23));
    assert_eq!(state.fd, 23);
    assert!(state.redirect(5));
    // Model a failure in the caller's old-fd close after this switch. There is
    // no old-fd close in this guard; every settlement effect uses the new pin.
    let mut script = Script::completed(exited(libc::CLD_KILLED, libc::SIGKILL));
    abort(&mut state, &mut script);
    assert_eq!(
        script.events,
        [
            Event::Clock,
            Event::Kill(5),
            Event::Clock,
            Event::Wait(5),
            Event::Close(5),
            Event::Stop
        ]
    );
    assert!(!state.redirect(40));
    assert_eq!(state.fd, 5);
}

#[test]
fn kill_uncertainty_stops_but_esrch_still_requires_the_exact_reap() {
    for error in [libc::EACCES, libc::EINVAL, libc::EINTR, libc::EBADF] {
        let mut state = State::new(23, 77);
        let mut script = Script::completed(exited(libc::CLD_EXITED, 0));
        script.kill = Err(error);
        abort(&mut state, &mut script);
        assert_eq!(script.events, [Event::Clock, Event::Kill(23), Event::Stop]);
        assert_eq!(state.phase, Phase::Failed);
        no_later_authority(&mut state, &mut script);
    }
    let mut state = State::new(23, 77);
    let mut script = Script::completed(exited(libc::CLD_EXITED, 42));
    script.kill = Err(libc::ESRCH);
    abort(&mut state, &mut script);
    assert_eq!(
        script.events,
        [
            Event::Clock,
            Event::Kill(23),
            Event::Clock,
            Event::Wait(23),
            Event::Close(23),
            Event::Stop
        ]
    );
}

#[test]
fn pending_events_wait_fairly_under_one_deadline_without_resending_signal() {
    let mut state = State::new(23, 77);
    let mut script = Script::completed(exited(libc::CLD_KILLED, libc::SIGKILL));
    script.clocks = [Ok(100), Ok(100), Ok(1_000_100), Ok(2_000_100)].into();
    script.waits = [
        Ok(None),
        Ok(None),
        Ok(Some(exited(libc::CLD_KILLED, libc::SIGKILL))),
    ]
    .into();
    abort(&mut state, &mut script);
    assert_eq!(
        script.events,
        [
            Event::Clock,
            Event::Kill(23),
            Event::Clock,
            Event::Wait(23),
            Event::Pause(1_000_000),
            Event::Clock,
            Event::Wait(23),
            Event::Pause(1_000_000),
            Event::Clock,
            Event::Wait(23),
            Event::Close(23),
            Event::Stop
        ]
    );
}

#[test]
fn timeout_is_not_reset_and_last_pause_cannot_exceed_remaining_budget() {
    let mut state = State::new(23, 77);
    let mut script = Script::completed(exited(libc::CLD_EXITED, 0));
    script.clocks = [Ok(1_000_000_000), Ok(5_999_999_999), Ok(6_000_000_000)].into();
    script.waits = [Ok(None)].into();
    abort(&mut state, &mut script);
    assert_eq!(
        script.events,
        [
            Event::Clock,
            Event::Kill(23),
            Event::Clock,
            Event::Wait(23),
            Event::Pause(1),
            Event::Clock,
            Event::Stop
        ]
    );
    assert_eq!(state.phase, Phase::Failed);
    no_later_authority(&mut state, &mut script);
}

#[test]
fn consumed_malformed_events_never_authorize_another_wait_or_signal_or_close() {
    let valid = exited(libc::CLD_EXITED, 0);
    for exit in [
        Exit { pid: 78, ..valid },
        Exit { pid: -1, ..valid },
        Exit {
            signo: libc::SIGUSR1,
            ..valid
        },
        Exit {
            code: libc::CLD_STOPPED,
            ..valid
        },
        Exit {
            status: -1,
            ..valid
        },
        Exit {
            status: 256,
            ..valid
        },
        exited(libc::CLD_KILLED, 0),
        exited(libc::CLD_DUMPED, 65),
    ] {
        let mut state = State::new(23, 77);
        let mut script = Script::completed(exit);
        abort(&mut state, &mut script);
        assert_eq!(
            script.events,
            [
                Event::Clock,
                Event::Kill(23),
                Event::Clock,
                Event::Wait(23),
                Event::Stop
            ]
        );
        assert_eq!(state.phase, Phase::Failed);
        no_later_authority(&mut state, &mut script);
    }
}

#[test]
fn wait_clock_and_pause_failures_are_terminal_without_retries() {
    let mut state = State::new(23, 77);
    let mut script = Script::completed(exited(libc::CLD_EXITED, 0));
    script.waits = [Err(())].into();
    abort(&mut state, &mut script);
    assert_eq!(
        script.events,
        [
            Event::Clock,
            Event::Kill(23),
            Event::Clock,
            Event::Wait(23),
            Event::Stop
        ]
    );
    no_later_authority(&mut state, &mut script);

    for clocks in [vec![Err(())], vec![Ok(u64::MAX)]] {
        let mut state = State::new(23, 77);
        let mut script = Script::completed(exited(libc::CLD_EXITED, 0));
        script.clocks = clocks.into();
        abort(&mut state, &mut script);
        assert_eq!(script.events, [Event::Clock, Event::Stop]);
        no_later_authority(&mut state, &mut script);
    }
    for later in [Ok(99), Err(())] {
        let mut state = State::new(23, 77);
        let mut script = Script::completed(exited(libc::CLD_EXITED, 0));
        script.clocks = [Ok(100), later].into();
        abort(&mut state, &mut script);
        assert_eq!(
            script.events,
            [Event::Clock, Event::Kill(23), Event::Clock, Event::Stop]
        );
    }
    let mut state = State::new(23, 77);
    let mut script = Script::completed(exited(libc::CLD_EXITED, 0));
    script.waits = [Ok(None)].into();
    script.pause = Err(());
    abort(&mut state, &mut script);
    assert_eq!(
        script.events,
        [
            Event::Clock,
            Event::Kill(23),
            Event::Clock,
            Event::Wait(23),
            Event::Pause(1_000_000),
            Event::Stop
        ]
    );
    no_later_authority(&mut state, &mut script);
}

#[test]
fn close_uncertainty_never_repeats_close_or_reopens_reaping_authority() {
    let mut state = State::new(23, 77);
    let mut script = Script::completed(exited(libc::CLD_KILLED, libc::SIGKILL));
    script.close = Err(());
    abort(&mut state, &mut script);
    assert_eq!(
        script.events,
        [
            Event::Clock,
            Event::Kill(23),
            Event::Clock,
            Event::Wait(23),
            Event::Close(23),
            Event::Stop
        ]
    );
    assert_eq!(state.phase, Phase::Failed);
    no_later_authority(&mut state, &mut script);
}
