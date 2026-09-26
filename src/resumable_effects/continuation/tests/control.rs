//! The durable driver on a control-dependent program (issue #296): loop and
//! branch suspensions through the v3 envelope, and a crash at every record.

use super::*;

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
                assert!(in_doubt);
                answers_in_doubt += 1;
                let ArgumentValue::Int(value) = request.request else {
                    panic!()
                };
                recovered
                    .answer(&policy(), &request.bind_answer(answer(value)))
                    .unwrap();
                status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
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
