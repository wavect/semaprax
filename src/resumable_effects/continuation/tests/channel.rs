//! Issue #296 R20: the durable journal driving a bounded record `yields`
//! request/response channel end to end -- the `v5` envelope, and a crash
//! simulated before and after every journal record, recovering without
//! repeating a dispatch or a cleanup.

use super::*;
use crate::hir::DeclarationId;

/// Two sequential record-channel yields: the second request is built from
/// the first answer's own field, exactly like
/// `interpreter::resumable::channel`'s own fixture, driven this time through
/// the durable journal.
const CHANNEL: &str = r#"
module test.durable_channel;
@id("app.prompt")
record Prompt {
    @id("app.prompt.seed") seed: i64,
    @id("app.prompt.urgent") urgent: bool,
}
@id("app.answer")
record Answer {
    @id("app.answer.value") value: i64,
    @id("app.answer.ok") ok: bool,
}
@id("app.ask")
fn ask(seed: i64) -> i64
    yields Prompt -> Answer
{
    let first = yield Prompt { seed: seed, urgent: false };
    let second = yield Prompt { seed: first.value, urgent: true };
    first.value + second.value
}
@id("app.main")
fn main() -> i64 { 0 }
"#;

const CHANNEL_ARGUMENT: i64 = 2;
/// seed 2 -> first request `Prompt{seed:2,...}`, answered `value:20`; second
/// request `Prompt{seed:20,...}`, answered `value:200`; result 20 + 200.
const CHANNEL_RESULT: i64 = 220;
/// Started, 2 x (Yielded, Dispatched, Answered), Completed, CleanupStarted,
/// CleanupSettled.
const CHANNEL_RECORDS: usize = 1 + 2 * 3 + 1 + 1 + 1;

#[derive(Default)]
struct ChannelHost {
    calls: Vec<(u32, i64)>,
}

fn prompt_seed(request: &ResumableChannelValue) -> i64 {
    let ResumableChannelValue::Record {
        declaration,
        fields,
    } = request
    else {
        panic!("expected a Prompt record request: {request:?}")
    };
    assert_eq!(*declaration, DeclarationId::new("app.prompt"));
    let ArgumentValue::Int(seed) = fields[0] else {
        panic!("Prompt's first field is not an i64: {fields:?}")
    };
    seed
}

fn answer(value: i64) -> ResumableChannelValue {
    ResumableChannelValue::Record {
        declaration: DeclarationId::new("app.answer"),
        fields: vec![ArgumentValue::Int(value), ArgumentValue::Bool(true)],
    }
}

/// The fixture's own fixed request sequence identifies which static site a
/// physical call answers, never the host's own count: `seed` is
/// `CHANNEL_ARGUMENT` only for the first request and `CHANNEL_ARGUMENT * 10`
/// only for the second, so the mapping is exact and total for this fixture.
fn expected_site(seed: i64) -> u32 {
    if seed == CHANNEL_ARGUMENT {
        0
    } else {
        assert_eq!(seed, CHANNEL_ARGUMENT * 10, "unexpected fixture request");
        1
    }
}

impl EffectHandler<ResumableChannelValue, ResumableChannelValue> for ChannelHost {
    fn dispatch(
        &mut self,
        request: &ResumableChannelValue,
    ) -> Result<ResumableChannelValue, String> {
        let seed = prompt_seed(request);
        self.calls.push((expected_site(seed), seed));
        Ok(answer(seed * 10))
    }
}

fn open<'a>(
    scratch: &Scratch,
    key: &'a SourceCheckpointKey,
    program: &'a ResolvedProgram,
    invocation: &str,
    fresh: bool,
) -> Result<DurableInvocation<'a>, ContinuationError> {
    let arguments = [ArgumentValue::Int(CHANNEL_ARGUMENT)];
    if fresh {
        DurableInvocation::start(
            &scratch.dir(),
            key,
            program,
            "app.ask",
            &arguments,
            invocation,
            3,
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
            3,
            STEPS,
            TornTailPolicy::Refuse,
        )
    }
}

#[test]
fn a_record_channel_runs_durably_through_the_v5_envelope() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(CHANNEL));
    let mut invocation = open(&scratch, &key, &program, "inv-channel", true).unwrap();
    let ContinuationStatus::AwaitingDispatch(first) = invocation.status() else {
        panic!("start did not suspend");
    };
    assert_eq!(first.site, 0);
    assert_eq!(
        first.request,
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("app.prompt"),
            fields: vec![
                ArgumentValue::Int(CHANNEL_ARGUMENT),
                ArgumentValue::Bool(false)
            ],
        }
    );
    let (mut host, mut cleanup) = (ChannelHost::default(), Cleanup::default());
    let status = invocation
        .drive(&policy(), &mut host, &mut cleanup)
        .unwrap();
    assert_eq!(settled_value(&status), ArgumentValue::Int(CHANNEL_RESULT));
    assert_eq!(host.calls, [(0, 2), (1, 20)]);
    assert_eq!(cleanup.0, 1);
    assert_eq!(line_count(&scratch.journal("inv-channel")), CHANNEL_RECORDS);
    let text = fs::read_to_string(scratch.journal("inv-channel")).unwrap();
    assert_eq!(
        text.matches("semaprax.source-resumable-checkpoint.v5")
            .count(),
        2,
        "the durable journal must carry the v5 envelope for a record channel: {text}"
    );
    assert!(!text.contains("semaprax.source-resumable-checkpoint.v2"));
}

#[test]
fn crash_at_every_record_recovers_the_channel_without_repeat_dispatch_or_cleanup() {
    let (key, program) = (key(), program(CHANNEL));
    let (mut answers_in_doubt, mut cleanups_in_doubt) = (0, 0);
    for at in 0..CHANNEL_RECORDS {
        for after in [false, true] {
            let scratch = Scratch::new(0o700);
            let invocation = format!("inv-channel-crash-{at}-{after}");
            let (mut host, mut cleanup) = (ChannelHost::default(), Cleanup::default());
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
                let seed = prompt_seed(&request.request);
                recovered
                    .answer(&policy(), &request.bind_answer(answer(seed * 10)))
                    .unwrap();
                status = recovered.drive(&policy(), &mut host, &mut cleanup).unwrap();
                assert!(host.calls[dispatched_before..]
                    .iter()
                    .all(|(site, _)| *site > request.site));
            }
            let cleanup_in_doubt = matches!(status, ContinuationStatus::CleanupInDoubt { .. });
            if cleanup_in_doubt {
                cleanups_in_doubt += 1;
                recovered.confirm_cleanup().unwrap();
                status = recovered.status();
            }
            assert_eq!(settled_value(&status), ArgumentValue::Int(CHANNEL_RESULT));
            let mut sites: Vec<u32> = host.calls.iter().map(|(site, _)| *site).collect();
            sites.sort_unstable();
            sites.dedup();
            assert_eq!(
                sites.len(),
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
    assert!(answers_in_doubt > 0 && cleanups_in_doubt > 0);
}
