//! Exactly-once settlement of an owned `Bytes` value carried across a
//! control-dependent suspension (issue #296, spec section 11.6): the value
//! is bound into the durable driver's existing journaled
//! `CleanupStarted`/`CleanupSettled` window (no second window), on
//! completion (already settled in-process by the interpreter, so nothing is
//! pending), on abandon (the only source-visible case this profile admits
//! where a carried value is still pending when the sticky outcome is
//! recorded), and under a crash at every record leading to each.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// One owned `Bytes` local (`buf`, three bytes) created before its function's
/// only `yield` -- inside an `if` branch, so the plan is control-dependent --
/// and consumed after it.
const OWNED: &str = r#"
module test.durable_owned;
@id("bytes.make")
fn make_buf() -> Bytes {
    let bytes = [1u8, 2u8, 3u8];
    bytes_copy(array_as_slice(bytes))
}
@id("bytes.consume")
fn consume(value: own Bytes) -> i64 {
    let _ = bytes_as_slice(value);
    100
}
@id("app.ask")
fn ask(seed: i64, flag: bool) -> i64
    yields i64 -> i64
{
    let outcome = if flag {
        let buf = make_buf();
        let answer = yield seed;
        let used = consume(buf);
        answer + used
    } else {
        0
    };
    outcome
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const REQUEST: i64 = 7;
const ANSWER: i64 = 5;
const RESULT: i64 = ANSWER + 100;
/// Started, Yielded, Dispatched, Answered, Completed, CleanupStarted,
/// CleanupSettled.
const COMPLETED_RECORDS: usize = 7;
/// Started, Yielded, Dispatched, Failed, CleanupStarted, CleanupSettled.
const ABANDONED_RECORDS: usize = 6;

/// A [`CleanupHandler`] whose `run_carried` -- called by the driver itself,
/// once per pending carried item, mirroring
/// `resumable_effects::core::resume`'s own per-op audit shape -- counts a
/// real settlement so a test can observe whether it ran 0 (a leak), 1
/// (correct), or more (a double free) times. `skip_carried` is the
/// negative-control knob: `true` reproduces a mutant handler that forgets to
/// settle a carried value, which the driver -- not this counter -- must
/// then report as `CleanupSettlement::Failed` rather than `Completed`.
struct CountingCleanup<'a> {
    settled: &'a AtomicUsize,
    skip_carried: bool,
}

impl CleanupHandler<DurableOutcome> for CountingCleanup<'_> {
    fn run(&mut self, _: &DurableOutcome) -> Result<(), String> {
        Ok(())
    }

    fn run_carried(&mut self, item: &[u8]) -> Result<(), String> {
        if self.skip_carried {
            return Err("mutant: skips carried-value settlement".to_owned());
        }
        let _ = item;
        self.settled.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

impl<'a> CountingCleanup<'a> {
    fn honest(settled: &'a AtomicUsize) -> Self {
        Self {
            settled,
            skip_carried: false,
        }
    }

    fn mutant_skipping_carried_cleanup(settled: &'a AtomicUsize) -> Self {
        Self {
            settled,
            skip_carried: true,
        }
    }
}

#[derive(Default)]
struct Host;

impl EffectHandler<ArgumentValue, ArgumentValue> for Host {
    fn dispatch(&mut self, request: &ArgumentValue) -> Result<ArgumentValue, String> {
        assert_eq!(*request, ArgumentValue::Int(REQUEST));
        Ok(ArgumentValue::Int(ANSWER))
    }
}

fn open<'a>(
    scratch: &Scratch,
    key: &'a SourceCheckpointKey,
    program: &'a ResolvedProgram,
    invocation: &str,
    fresh: bool,
) -> Result<DurableInvocation<'a>, ContinuationError> {
    let arguments = [ArgumentValue::Int(REQUEST), ArgumentValue::Bool(true)];
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

/// Positive run: the carried `buf` is consumed by the resumed suffix like
/// any other local, so the interpreter's own in-process drop already retires
/// it before the driver ever records a terminal outcome. Settlement therefore
/// has nothing pending, at every crash point leading there.
#[test]
fn completing_normally_leaves_nothing_pending_to_settle_at_every_crash_point() {
    let (key, program) = (key(), program(OWNED));
    for at in 0..COMPLETED_RECORDS {
        for after in [false, true] {
            let scratch = Scratch::new(0o700);
            let invocation = format!("inv-owned-complete-{at}-{after}");
            let mut host = Host;
            let settled = AtomicUsize::new(0);
            arm(at, after);
            let crashed = open(&scratch, &key, &program, &invocation, true).and_then(|mut live| {
                let mut cleanup = CountingCleanup::honest(&settled);
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
            let mut cleanup = CountingCleanup::honest(&settled);
            let mut status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
            if let ContinuationStatus::AwaitingAnswer { request, in_doubt } = status.clone() {
                // Dispatched is durable but its answer is not: `drive` never
                // redispatches an in-doubt request on its own, exactly like
                // the non-owned control crash matrix.
                assert!(in_doubt);
                let ArgumentValue::Int(value) = request.request else {
                    panic!()
                };
                recovered
                    .answer(&policy(), &request.bind_answer(ArgumentValue::Int(ANSWER)))
                    .unwrap();
                assert_eq!(value, REQUEST);
                let mut cleanup = CountingCleanup::honest(&settled);
                status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
            }
            if matches!(status, ContinuationStatus::CleanupInDoubt { .. }) {
                // `CleanupStarted` was already durable when the fault fired;
                // the existing in-doubt rule settles it on the host's word
                // rather than running the handler a second time.
                recovered.confirm_cleanup().unwrap();
                status = recovered.status();
            }
            assert_eq!(settled_value(&status), ArgumentValue::Int(RESULT));
            assert_eq!(
                settled.load(Ordering::SeqCst),
                0,
                "nothing should be pending at {at}/{after}: the interpreter already dropped it"
            );
        }
    }
}

/// `abandon` is the one outcome this profile admits where the carried value
/// is still sitting, un-dropped in-process, in the last journaled envelope:
/// the driver never re-enters the interpreter on this path. Settlement must
/// run exactly once, at every crash point leading up to the abandon.
#[test]
fn abandon_settles_the_carried_value_exactly_once_at_every_crash_point() {
    let (key, program) = (key(), program(OWNED));
    // Only `Started`, `Yielded`, `Dispatched` precede the abandon.
    for at in 0..3 {
        for after in [false, true] {
            let scratch = Scratch::new(0o700);
            let invocation = format!("inv-owned-abandon-{at}-{after}");
            let mut host = Host;
            let settled = AtomicUsize::new(0);
            arm(at, after);
            let crashed: Result<(), ContinuationError> =
                open(&scratch, &key, &program, &invocation, true).and_then(|mut live| {
                    let request = live.dispatch(&policy())?;
                    let _ = host.dispatch(&request.request);
                    Ok(())
                });
            // `at` ranges only over the three records ("Started", "Yielded",
            // "Dispatched") this restricted run appends, so the fault always
            // fires before it returns.
            assert!(
                matches!(crashed, Err(ContinuationError::Storage)),
                "fault {at}/{after} did not fire: {crashed:?}"
            );
            let _ = armed_fault();
            let mut recovered = open(&scratch, &key, &program, &invocation, false).unwrap();
            // Recovery replays exactly the durable prefix: unless
            // `Dispatched` itself was already durable before the fault
            // fired, recovery only reaches `AwaitingDispatch` and the site
            // must be (re-)dispatched -- a legitimate dispatch of a request
            // that was never handed out before -- to reach `AwaitingAnswer`.
            if matches!(recovered.status(), ContinuationStatus::AwaitingDispatch(_)) {
                let request = recovered.dispatch(&policy()).unwrap();
                let _ = host.dispatch(&request.request);
            }
            assert!(matches!(
                recovered.status(),
                ContinuationStatus::AwaitingAnswer { .. }
            ));
            recovered.abandon(&policy()).unwrap();
            assert_eq!(recovered.pending_cleanup_carried().len(), 1);
            assert_eq!(recovered.pending_cleanup_carried()[0], vec![1u8, 2u8, 3u8]);
            let mut cleanup = CountingCleanup::honest(&settled);
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
            // The existing in-doubt window already refuses a second run: a
            // fresh recovery from the now-fully-settled journal replays
            // straight to `Settled` without invoking `settle` again.
            let mut cleanup_again = CountingCleanup::honest(&settled);
            assert!(matches!(
                recovered.settle(&mut cleanup_again),
                Err(ContinuationError::NotAwaitingCleanup)
            ));
            assert_eq!(settled.load(Ordering::SeqCst), 1, "no double settlement");
        }
    }
}

/// Negative control: a mutant [`CleanupHandler`] that forgets to settle the
/// carried value on abandon. The driver itself catches it -- `settle`
/// refuses `CleanupSettlement::Completed` unless every carried item's own
/// `run_carried` call actually succeeded, so the mutant's one failure
/// surfaces in `status()` and the durable journal's own `CleanupSettled`
/// record as `Failed`, not silently as `Completed` -- exactly the defect
/// `docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md` section 11.6 asks this
/// increment to demonstrate. The leak counter corroborates it (stays at
/// zero) but is not what a caller would actually rely on: the driver's own
/// reported outcome is. The mutant lives only in this one test; every other
/// test in this module uses the honest handler.
#[test]
fn a_mutant_that_skips_carried_cleanup_on_abandon_is_reported_failed_by_the_driver() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(OWNED));
    let mut invocation = open(&scratch, &key, &program, "inv-owned-mutant", true).unwrap();
    let mut host = Host;
    let request = invocation.dispatch(&policy()).unwrap();
    let _ = host.dispatch(&request.request);
    invocation.abandon(&policy()).unwrap();
    assert_eq!(invocation.pending_cleanup_carried().len(), 1);

    let settled = AtomicUsize::new(0);
    let mut mutant = CountingCleanup::mutant_skipping_carried_cleanup(&settled);
    invocation.settle(&mut mutant).unwrap();
    assert_eq!(
        settled.load(Ordering::SeqCst),
        0,
        "the mutant must leak: it never ran the carried value's own settlement"
    );
    // The driver, not a test-local counter, is the one reporting the skip:
    // `CleanupSettlement::Failed`, never `Completed`, and the same class in
    // the durable journal's own `CleanupSettled` record.
    assert!(matches!(
        invocation.status(),
        ContinuationStatus::Settled {
            outcome: DurableOutcome::Failed(DurableFailure::HostAbandoned),
            cleanup: CleanupSettlement::Failed,
        }
    ));
    let text = fs::read_to_string(scratch.journal("inv-owned-mutant")).unwrap();
    assert!(text.contains("cleanup_settled"));
    assert!(
        !text.contains("\"completed\""),
        "the durable journal must not record Completed for a skipped carried item: {text}"
    );
}
