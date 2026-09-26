//! The durable driver on a control-dependent program (issue #296): loop and
//! branch suspensions through the v3 envelope, and a crash at every record.

use super::*;
use crate::resumable_effects::lowering::control::MAX_CONTROL_SUSPENSIONS;

const CONTROL: &str = r#"
module test.durable_control;
@id("app.ask")
fn ask(limit: i64) -> i64
    yields i64 -> i64
{
    let mut total = 0;
    let mut round = 0;
    while round < limit {
        let answer = yield round;
        total = total + answer;
        round = round + 1;
        round > 0
    }
    let bonus = if total > 10 {
        let extra = yield total;
        extra
    } else {
        0
    };
    total + bonus
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

/// Loop requests 0, 1, 2 then the branch request 45 (= 5 + 15 + 25).
const REQUESTS: [i64; 4] = [0, 1, 2, 45];
const RESULT: i64 = 45 + 455;
/// Started, 4 x (Yielded, Dispatched, Answered), Completed, CleanupStarted,
/// CleanupSettled.
const RECORDS: usize = 16;

#[derive(Default)]
struct ControlHost {
    /// The ordinal per physical call, derived from the fixture's fixed
    /// request sequence, never from the host's own count.
    /// `control_crash_at_every_record_never_repeats_dispatch_or_cleanup`
    /// cross-checks this against the driver's own dynamic suspension
    /// ordinal (`ContinuationRequest::site`) after an in-doubt recovery.
    calls: Vec<usize>,
}

impl EffectHandler<ArgumentValue, ArgumentValue> for ControlHost {
    fn dispatch(&mut self, request: &ArgumentValue) -> Result<ArgumentValue, String> {
        let ArgumentValue::Int(value) = request else {
            return Err("unexpected request".into());
        };
        let ordinal = REQUESTS
            .iter()
            .position(|expected| expected == value)
            .expect("fixture request");
        self.calls.push(ordinal);
        Ok(answer(*value))
    }
}

fn answer(request: i64) -> ArgumentValue {
    ArgumentValue::Int(request * 10 + 5)
}

fn open<'a>(
    scratch: &Scratch,
    key: &'a SourceCheckpointKey,
    program: &'a ResolvedProgram,
    invocation: &str,
    fresh: bool,
) -> Result<DurableInvocation<'a>, ContinuationError> {
    let arguments = [ArgumentValue::Int(3)];
    if fresh {
        DurableInvocation::start(
            &scratch.dir(),
            key,
            program,
            "app.ask",
            &arguments,
            invocation,
            7,
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
            7,
            STEPS,
            TornTailPolicy::Refuse,
        )
    }
}

#[test]
fn control_dependent_program_runs_durably_through_loop_and_branch() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(CONTROL));
    let mut invocation = open(&scratch, &key, &program, "inv-control", true).unwrap();
    let (mut host, mut cleanup) = (ControlHost::default(), Cleanup::default());
    let status = invocation
        .drive(&policy(), &mut host, &mut cleanup)
        .unwrap();
    assert_eq!(settled_value(&status), ArgumentValue::Int(RESULT));
    assert_eq!(host.calls, [0, 1, 2, 3]);
    assert_eq!(cleanup.0, 1);
    let journal = scratch.journal("inv-control");
    assert_eq!(line_count(&journal), RECORDS);
    let text = fs::read_to_string(&journal).unwrap();
    assert_eq!(
        text.matches("semaprax.source-resumable-checkpoint.v3")
            .count(),
        4
    );
    assert!(!text.contains("semaprax.source-resumable-checkpoint.v2"));
}

#[test]
fn control_answers_are_bound_to_the_dynamic_suspension_ordinal() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(CONTROL));
    let mut invocation = open(&scratch, &key, &program, "inv-control-bound", true).unwrap();
    let first = invocation.dispatch(&policy()).unwrap();
    invocation
        .answer(&policy(), &first.bind_answer(answer(0)))
        .unwrap();
    let second = invocation.dispatch(&policy()).unwrap();
    // Same static loop site, next ordinal, new envelope.
    assert_eq!(second.site, 1);
    assert_ne!(second.envelope_digest, first.envelope_digest);
    assert!(matches!(
        invocation.answer(&policy(), &first.bind_answer(answer(0))),
        Err(ContinuationError::ReplayedAnswer)
    ));
    let mut stale = second.bind_answer(answer(1));
    stale.envelope_digest = first.envelope_digest;
    assert!(matches!(
        invocation.answer(&policy(), &stale),
        Err(ContinuationError::StaleEnvelope)
    ));
}

#[test]
fn control_crash_at_every_record_never_repeats_dispatch_or_cleanup() {
    let (key, program) = (key(), program(CONTROL));
    let (mut answers_in_doubt, mut cleanups_in_doubt) = (0, 0);
    for at in 0..RECORDS {
        for after in [false, true] {
            let scratch = Scratch::new(0o700);
            let invocation = format!("inv-control-crash-{at}-{after}");
            let (mut host, mut cleanup) = (ControlHost::default(), Cleanup::default());
            arm(at, after);
            let crashed = open(&scratch, &key, &program, &invocation, true)
                .and_then(|mut live| live.drive(&policy(), &mut host, &mut cleanup));
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
            let mut status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
            if let ContinuationStatus::AwaitingAnswer { request, in_doubt } = status.clone() {
                // Dispatched is durable but its answer is not: never redispatch.
                assert!(in_doubt);
                answers_in_doubt += 1;
                let dispatched_before = host.calls.len();
                let ArgumentValue::Int(value) = request.request else {
                    panic!()
                };
                recovered
                    .answer(&policy(), &request.bind_answer(answer(value)))
                    .unwrap();
                status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
                // Tie the fixture-derived ordinal to the driver's own
                // dynamic suspension ordinal (`request.site`, the journal's
                // actual key per `docs/RESUMABLE-EFFECTS-CONTINUATION-V1.md`
                // section 11.4): every dispatch from here on must be a later
                // site than the one just recovered in doubt, not merely an
                // as-yet-unseen request value.
                assert!(host.calls[dispatched_before..]
                    .iter()
                    .all(|ordinal| *ordinal as u32 > request.site));
            }
            let cleanup_in_doubt = matches!(status, ContinuationStatus::CleanupInDoubt { .. });
            if cleanup_in_doubt {
                cleanups_in_doubt += 1;
                recovered.confirm_cleanup().unwrap();
                status = recovered.status();
            }
            assert_eq!(settled_value(&status), ArgumentValue::Int(RESULT));
            let mut ordinals = host.calls.clone();
            ordinals.sort_unstable();
            ordinals.dedup();
            assert_eq!(
                ordinals.len(),
                host.calls.len(),
                "repeat dispatch at {at}/{after}"
            );
            if cleanup_in_doubt {
                assert!(cleanup.0 <= 1, "repeat cleanup at {at}/{after}");
            } else {
                assert_eq!(cleanup.0, 1, "cleanup count at {at}/{after}");
            }
        }
    }
    assert_eq!((answers_in_doubt, cleanups_in_doubt), (8, 2));
}

/// A loop offering more suspensions than [`MAX_CONTROL_SUSPENSIONS`] admits.
/// `limit` is chosen well past the bound so the durable run always settles
/// `SuspensionBoundExceeded` rather than completing.
const UNBOUNDED_LOOP: &str = r#"
module test.durable_control_bound;
@id("app.ask")
fn ask(limit: i64) -> i64
    yields i64 -> i64
{
    let mut total = 0;
    let mut round = 0;
    while round < limit {
        let answer = yield round;
        total = total + answer;
        round = round + 1;
        round > 0
    }
    total
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

#[derive(Default)]
struct CountingHost {
    calls: usize,
}

impl EffectHandler<ArgumentValue, ArgumentValue> for CountingHost {
    fn dispatch(&mut self, request: &ArgumentValue) -> Result<ArgumentValue, String> {
        let ArgumentValue::Int(value) = request else {
            return Err("unexpected request".into());
        };
        self.calls += 1;
        Ok(ArgumentValue::Int(value + 1))
    }
}

#[test]
fn control_dependent_program_exceeding_the_suspension_bound_settles_at_the_journal_ceiling() {
    // Section 11.5's `SPX-...` table has no source-level code for this: a
    // per-*invocation* runtime bound, not a compile-time refusal. The
    // interpreter (`interpreter::resumable::control::settle`) reports
    // `SuspensionBoundExceeded` the moment a would-be 17th suspension is
    // about to park, with the 16 preceding ones already durably committed;
    // it never attempts a 17th `Yielded` record. The journal therefore
    // reaches exactly `Started` + `MAX_CONTROL_SUSPENSIONS` x (`Yielded`,
    // `Dispatched`, `Answered`) + `Failed` + `CleanupStarted` +
    // `CleanupSettled` records -- the same 52-record arithmetic
    // `journal::MAX_RECORDS` uses to size its own ceiling -- never one more.
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(UNBOUNDED_LOOP));
    let arguments = [ArgumentValue::Int(20)];
    let mut invocation = DurableInvocation::start(
        &scratch.dir(),
        &key,
        &program,
        "app.ask",
        &arguments,
        "inv-control-bound-exceeded",
        7,
        STEPS,
    )
    .unwrap();
    let (mut host, mut cleanup) = (CountingHost::default(), Cleanup::default());
    let status = invocation
        .drive(&policy(), &mut host, &mut cleanup)
        .unwrap();
    assert!(matches!(
        status,
        ContinuationStatus::Settled {
            outcome: DurableOutcome::Failed(DurableFailure::SuspensionBoundExceeded),
            cleanup: CleanupSettlement::Completed,
        }
    ));
    assert_eq!(host.calls, MAX_CONTROL_SUSPENSIONS);
    assert_eq!(cleanup.0, 1);
    let bound_records = 1 + 3 * MAX_CONTROL_SUSPENSIONS + 1 + 2;
    assert_eq!(
        line_count(&scratch.journal("inv-control-bound-exceeded")),
        bound_records
    );
}
