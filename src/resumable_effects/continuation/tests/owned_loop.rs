//! Issue #296, spec section 11.6 (loop-embedded owned `Bytes` carrying, v4
//! lane): the P1 reproducer for an owned `Bytes` local carried across a
//! `while`-loop-embedded suspension, driven through the durable journal with
//! a crash simulated before and after every record, and exactly-once
//! settlement of the carried value when a suspension is abandoned instead of
//! answered.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// `buf` is created before the loop and carried across three loop-embedded
/// suspensions, live the whole time; it is consumed only after the loop
/// completes, so its carried value affects only the final return value,
/// never a branch or the loop's own trip count. `buf`'s own storage is never
/// touched inside the loop body, so it admits on the existing, unchanged v4
/// lane (`cleanup_plan::owned_liveness::slot_touched_inside_while`).
const OWNED_LOOP: &str = r#"
module test.durable_owned_loop;
@id("bytes.make")
fn make_buf() -> Bytes {
    let bytes = [9u8, 8u8, 7u8];
    bytes_copy(array_as_slice(bytes))
}
@id("bytes.consume")
fn consume(value: own Bytes) -> i64 {
    let _ = bytes_as_slice(value);
    100
}
@id("app.ask")
fn ask(limit: i64) -> i64
    yields i64 -> i64
{
    let buf = make_buf();
    let mut total = 0;
    let mut round = 0;
    while round < limit {
        let answer = yield round;
        total = total + answer;
        round = round + 1;
        round > 0
    }
    let used = consume(buf);
    total + used
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const LIMIT: i64 = 3;
/// Three loop suspensions at requests 0, 1, 2, answered `request * 10 + 5`:
/// total = 5 + 15 + 25 = 45. `used` (`consume`) always returns 100.
const RESULT: i64 = 45 + 100;
/// Started, 3 x (Yielded, Dispatched, Answered), Completed, CleanupStarted,
/// CleanupSettled.
const COMPLETED_RECORDS: usize = 1 + 3 * 3 + 1 + 1 + 1;
/// Started, (Yielded, Dispatched, Answered) once, then Yielded, Dispatched,
/// Failed, CleanupStarted, CleanupSettled.
const ABANDONED_RECORDS: usize = 1 + 3 + 3 + 1 + 1;

#[derive(Default)]
struct Host;

impl EffectHandler<ResumableChannelValue, ResumableChannelValue> for Host {
    fn dispatch(
        &mut self,
        request: &ResumableChannelValue,
    ) -> Result<ResumableChannelValue, String> {
        let ResumableChannelValue::Scalar(ArgumentValue::Int(value)) = request else {
            return Err("unexpected request".into());
        };
        Ok(ArgumentValue::Int(value * 10 + 5).into())
    }
}

/// Mirrors `owned.rs`'s own `CountingCleanup`: `run_carried` counts a real
/// settlement so a test can observe whether it ran 0 (a leak), 1 (correct),
/// or more (a double free) times.
struct CountingCleanup<'a> {
    settled: &'a AtomicUsize,
}

impl CleanupHandler<DurableOutcome> for CountingCleanup<'_> {
    fn run(&mut self, _: &DurableOutcome) -> Result<(), String> {
        Ok(())
    }

    fn run_carried(&mut self, item: &[u8]) -> Result<(), String> {
        assert_eq!(item, [9u8, 8u8, 7u8]);
        self.settled.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl<'a> CountingCleanup<'a> {
    fn new(settled: &'a AtomicUsize) -> Self {
        Self { settled }
    }
}

fn open<'a>(
    scratch: &Scratch,
    key: &'a SourceCheckpointKey,
    program: &'a ResolvedProgram,
    invocation: &str,
    fresh: bool,
) -> Result<DurableInvocation<'a>, ContinuationError> {
    let arguments = [ArgumentValue::Int(LIMIT)];
    if fresh {
        DurableInvocation::start(
            &scratch.dir(),
            key,
            program,
            "app.ask",
            &arguments,
            invocation,
            1,
            STEPS,
        )
    } else {
        DurableInvocation::recover(
            &scratch.dir(),
            key,
            program,
            "app.ask",
            &arguments,
            invocation,
            1,
            STEPS,
            TornTailPolicy::Refuse,
        )
    }
}

/// A single, uninterrupted, in-process run of the same function and
/// arguments -- no journal, no crash, no durable driver -- computed once as
/// the reference every crashed-and-recovered run below must match exactly.
fn uninterrupted_result() -> ArgumentValue {
    use crate::interpreter::resumable::control::{
        resume_control_resumable_effect, run_control_resumable_effect, ControlResumableStep,
    };
    let program = program(OWNED_LOOP);
    let arguments = [ArgumentValue::Int(LIMIT)];
    let mut step = run_control_resumable_effect(&program, "app.ask", &arguments, STEPS)
        .unwrap()
        .step;
    while let ControlResumableStep::Suspended { continuation } = &step {
        let ArgumentValue::Int(request) = continuation.request() else {
            panic!("scalar request")
        };
        let answer = ArgumentValue::Int(request * 10 + 5);
        step = resume_control_resumable_effect(
            &program,
            "app.ask",
            &arguments,
            continuation,
            &answer,
            STEPS,
        )
        .unwrap()
        .step;
    }
    let ControlResumableStep::Completed { result, .. } = step else {
        panic!("expected completion")
    };
    result
}

/// P1 reproducer (issue #296): resuming the durable journal's carried owned
/// `Bytes` local from every one of its three loop-embedded suspensions,
/// after a simulated crash immediately before and after each of the
/// [`COMPLETED_RECORDS`] journal records, reaches exactly the same result as
/// the uninterrupted in-process run -- across a full crash + recovery cycle
/// at every record, not merely the in-memory replay
/// `interpreter::resumable::control`'s own tests already cover.
#[test]
fn crash_at_every_record_recovers_the_same_result_as_an_uninterrupted_run() {
    let reference = uninterrupted_result();
    assert_eq!(reference, ArgumentValue::Int(RESULT));
    let (key, program) = (key(), program(OWNED_LOOP));
    for at in 0..COMPLETED_RECORDS {
        for after in [false, true] {
            let scratch = Scratch::new(0o700);
            let invocation = format!("inv-owned-loop-complete-{at}-{after}");
            let mut host = Host;
            let settled = AtomicUsize::new(0);
            arm(at, after);
            let crashed = open(&scratch, &key, &program, &invocation, true).and_then(|mut live| {
                let mut cleanup = CountingCleanup::new(&settled);
                live.drive(&policy(), &mut host, &mut cleanup)
            });
            assert!(
                matches!(crashed, Err(ContinuationError::Storage)),
                "fault {at}/{after} did not fire: {crashed:?}"
            );
            let _ = armed_fault();
            assert_eq!(
                line_count(&scratch.journal(&invocation)),
                at + usize::from(after)
            );
            let mut recovered = open(&scratch, &key, &program, &invocation, false).unwrap();
            let mut cleanup = CountingCleanup::new(&settled);
            let mut status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
            while let ContinuationStatus::AwaitingAnswer { request, in_doubt } = status.clone() {
                assert!(in_doubt);
                let ResumableChannelValue::Scalar(ArgumentValue::Int(value)) = request.request
                else {
                    panic!()
                };
                let answer = ArgumentValue::Int(value * 10 + 5);
                recovered
                    .answer(&policy(), &request.bind_answer(answer))
                    .unwrap();
                let mut cleanup = CountingCleanup::new(&settled);
                status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
            }
            if matches!(status, ContinuationStatus::CleanupInDoubt { .. }) {
                recovered.confirm_cleanup().unwrap();
                status = recovered.status();
            }
            assert_eq!(
                settled_value(&status),
                reference,
                "fault {at}/{after} diverged from the uninterrupted result"
            );
            assert_eq!(
                settled.load(Ordering::SeqCst),
                0,
                "nothing should be pending at {at}/{after}: the interpreter already dropped it"
            );
        }
    }
}

/// Exactly-once settlement of the carried value when a loop-embedded
/// suspension is abandoned rather than answered, at every crash point
/// leading up to the abandon: mirrors `owned.rs`'s own abandon test for the
/// if/else-nested profile, for the loop-embedded one.
#[test]
fn abandoning_a_loop_embedded_suspension_settles_its_carried_value_exactly_once() {
    let (key, program) = (key(), program(OWNED_LOOP));
    // Abandon after the *second* loop suspension is dispatched (having
    // already settled the first): Started, Yielded, Dispatched, Answered,
    // Yielded, Dispatched -- six records precede the fault window.
    for at in 0..6 {
        for after in [false, true] {
            let scratch = Scratch::new(0o700);
            let invocation = format!("inv-owned-loop-abandon-{at}-{after}");
            let mut host = Host;
            let settled = AtomicUsize::new(0);
            arm(at, after);
            let crashed: Result<(), ContinuationError> =
                open(&scratch, &key, &program, &invocation, true).and_then(|mut live| {
                    let request = live.dispatch(&policy())?;
                    let ResumableChannelValue::Scalar(ArgumentValue::Int(value)) = request.request
                    else {
                        panic!()
                    };
                    let answer = ArgumentValue::Int(value * 10 + 5);
                    live.answer(&policy(), &request.bind_answer(answer))?;
                    let request = live.dispatch(&policy())?;
                    let _ = host.dispatch(&request.request);
                    Ok(())
                });
            assert!(
                matches!(crashed, Err(ContinuationError::Storage)),
                "fault {at}/{after} did not fire: {crashed:?}"
            );
            let _ = armed_fault();
            let mut recovered = open(&scratch, &key, &program, &invocation, false).unwrap();
            // Recovery replays exactly the durable prefix: site 0 (the first
            // loop suspension) is settled again only if it was not yet
            // durably answered before the fault (which could have landed
            // anywhere in `Started..=Answered(0)`, so site 0 may already be
            // fully durable on entry), then site 1 (the second, the one
            // this test abandons) is dispatched again only if it was not
            // already dispatched -- but never answered, so it is left
            // `AwaitingAnswer` for `abandon`. `in_doubt` is `true` only when
            // site 1's own dispatch was itself durable before the fault;
            // when the fault fired earlier, this recovery dispatches it
            // fresh in this very session, which is not `in_doubt` -- either
            // way it is answered by nobody but `abandon`.
            loop {
                match recovered.status() {
                    ContinuationStatus::AwaitingDispatch(_) => {
                        let request = recovered.dispatch(&policy()).unwrap();
                        let _ = host.dispatch(&request.request);
                    }
                    ContinuationStatus::AwaitingAnswer { request, .. } if request.site == 0 => {
                        let ResumableChannelValue::Scalar(ArgumentValue::Int(value)) =
                            request.request
                        else {
                            panic!()
                        };
                        let answer = ArgumentValue::Int(value * 10 + 5);
                        recovered
                            .answer(&policy(), &request.bind_answer(answer))
                            .unwrap();
                    }
                    ContinuationStatus::AwaitingAnswer { request, .. } => {
                        assert_eq!(request.site, 1, "site 1 is the one this test abandons");
                        break;
                    }
                    other => panic!("unexpected status before abandon: {other:?}"),
                }
            }
            recovered.abandon(&policy()).unwrap();
            assert_eq!(recovered.pending_cleanup_carried().len(), 1);
            assert_eq!(recovered.pending_cleanup_carried()[0], vec![9u8, 8u8, 7u8]);
            let mut cleanup = CountingCleanup::new(&settled);
            recovered.settle(&mut cleanup).unwrap();
            assert!(matches!(
                recovered.status(),
                ContinuationStatus::Settled {
                    outcome: DurableOutcome::Failed(DurableFailure::HostAbandoned),
                    cleanup: CleanupSettlement::Completed,
                }
            ));
            assert_eq!(
                settled.load(Ordering::SeqCst),
                1,
                "carried value must settle exactly once at {at}/{after}"
            );
            assert_eq!(line_count(&scratch.journal(&invocation)), ABANDONED_RECORDS);
            let mut cleanup_again = CountingCleanup::new(&settled);
            assert!(matches!(
                recovered.settle(&mut cleanup_again),
                Err(ContinuationError::NotAwaitingCleanup)
            ));
            assert_eq!(settled.load(Ordering::SeqCst), 1, "no double settlement");
        }
    }
}
