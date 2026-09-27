//! V2 durable whole-function carrier, including uncertain dispatch recovery.

use super::*;
use crate::hir::DeclarationId;

const SOURCE: &str = r#"
module test.durable_arguments;
@id("app.input")
record Input {
    @id("app.input.seed") seed: i64,
    @id("app.input.urgent") urgent: bool,
}
@id("app.output")
record Output {
    @id("app.output.value") value: i64,
    @id("app.output.urgent") urgent: bool,
}
@id("app.ask")
fn ask(input: Input) -> Output yields i64 -> i64 {
    let first = yield input.seed;
    let second = yield first;
    Output { value: second, urgent: input.urgent }
}
@id("app.main") fn main() -> i64 { 0 }
"#;

fn arguments() -> Vec<ResumableChannelValue> {
    vec![ResumableChannelValue::Record {
        declaration: DeclarationId::new("app.input"),
        fields: vec![ArgumentValue::Int(4), ArgumentValue::Bool(true)],
    }]
}

fn answer(request: &ContinuationRequest) -> ContinuationAnswer {
    let value = if request.site == 0 { 9 } else { 12 };
    request.bind_answer(ArgumentValue::Int(value))
}

#[derive(Default)]
struct Cleanup(usize);

impl CleanupHandler<AggregateDurableOutcome> for Cleanup {
    fn run(&mut self, _: &AggregateDurableOutcome) -> Result<(), String> {
        self.0 += 1;
        Ok(())
    }
}

#[test]
fn aggregate_two_sites_and_checked_result_are_durable() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    let arguments = arguments();
    let mut invocation = AggregateDurableInvocation::start(
        &scratch.dir(),
        &key,
        &program,
        "app.ask",
        &arguments,
        "aggregate-two",
        7,
        STEPS,
    )
    .unwrap();
    for site in 0..2 {
        let request = invocation.dispatch(&policy()).unwrap();
        assert_eq!(request.site, site);
        invocation.answer(&policy(), &answer(&request)).unwrap();
    }
    assert_eq!(
        invocation.status(),
        AggregateContinuationStatus::CleanupPending { failure: None }
    );
    let mut cleanup = Cleanup::default();
    invocation.settle(&mut cleanup).unwrap();
    assert_eq!(cleanup.0, 1);
    let expected = AggregateDurableOutcome::Completed(ResumableChannelValue::Record {
        declaration: DeclarationId::new("app.output"),
        fields: vec![ArgumentValue::Int(12), ArgumentValue::Bool(true)],
    });
    assert_eq!(
        invocation.status(),
        AggregateContinuationStatus::Settled {
            outcome: expected.clone(),
            cleanup: CleanupSettlement::Completed,
        }
    );
    drop(invocation);
    let recovered = AggregateDurableInvocation::recover(
        &scratch.dir(),
        &key,
        &program,
        "app.ask",
        &arguments,
        "aggregate-two",
        7,
        STEPS,
        TornTailPolicy::Refuse,
    )
    .unwrap();
    assert_eq!(
        recovered.status(),
        AggregateContinuationStatus::Settled {
            outcome: expected,
            cleanup: CleanupSettlement::Completed,
        }
    );
}

#[test]
fn aggregate_wrong_nominal_and_leaf_refuse_before_storage() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    for wrong in [
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("app.output"),
            fields: vec![ArgumentValue::Int(4), ArgumentValue::Bool(true)],
        },
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("app.input"),
            fields: vec![ArgumentValue::Bool(true), ArgumentValue::Bool(true)],
        },
        ResumableChannelValue::Record {
            declaration: DeclarationId::new("app.input"),
            fields: vec![ArgumentValue::Int(4)],
        },
    ] {
        assert!(AggregateDurableInvocation::start(
            &scratch.dir(),
            &key,
            &program,
            "app.ask",
            &[wrong],
            "bad-aggregate",
            7,
            STEPS,
        )
        .is_err());
        assert_eq!(fs::read_dir(&scratch.0).unwrap().count(), 0);
    }
}

#[test]
fn v2_recovery_binds_arguments_and_refuses_v1_schema() {
    let scratch = Scratch::new(0o700);
    let (key, program) = (key(), program(SOURCE));
    let arguments = arguments();
    let invocation = AggregateDurableInvocation::start(
        &scratch.dir(),
        &key,
        &program,
        "app.ask",
        &arguments,
        "binding",
        7,
        STEPS,
    )
    .unwrap();
    drop(invocation);
    let mut changed = arguments.clone();
    let ResumableChannelValue::Record { fields, .. } = &mut changed[0] else {
        panic!()
    };
    fields[0] = ArgumentValue::Int(5);
    assert!(matches!(
        AggregateDurableInvocation::recover(
            &scratch.dir(),
            &key,
            &program,
            "app.ask",
            &changed,
            "binding",
            7,
            STEPS,
            TornTailPolicy::Refuse,
        ),
        Err(ContinuationError::ArgumentsMismatch)
    ));
    assert!(matches!(
        DurableInvocation::recover(
            &scratch.dir(),
            &key,
            &program,
            "app.ask",
            &[ArgumentValue::Int(4)],
            "binding",
            7,
            STEPS,
            TornTailPolicy::Refuse,
        ),
        Err(ContinuationError::Admission(_)) | Err(ContinuationError::NotStarted)
    ));
}

#[test]
fn wrong_journal_schema_is_rejected_at_each_versioned_path() {
    let scratch = Scratch::new(0o700);
    let key = key();
    let (aggregate_program, scalar_program) = (program(SOURCE), program(super::SOURCE));
    let arguments = arguments();
    let v2 = AggregateDurableInvocation::start(
        &scratch.dir(),
        &key,
        &aggregate_program,
        "app.ask",
        &arguments,
        "cross-schema",
        7,
        STEPS,
    )
    .unwrap();
    drop(v2);
    let v1 = DurableInvocation::start(
        &scratch.dir(),
        &key,
        &scalar_program,
        "app.ask",
        &args(),
        "cross-schema",
        7,
        STEPS,
    )
    .unwrap();
    drop(v1);
    let v1_path = scratch.journal("cross-schema");
    let v2_path = scratch
        .0
        .join(journal::channel::channel_journal_name("cross-schema"));
    let v1_bytes = fs::read(&v1_path).unwrap();
    let v2_bytes = fs::read(&v2_path).unwrap();
    fs::write(&v2_path, &v1_bytes).unwrap();
    assert!(matches!(
        AggregateDurableInvocation::recover(
            &scratch.dir(),
            &key,
            &aggregate_program,
            "app.ask",
            &arguments,
            "cross-schema",
            7,
            STEPS,
            TornTailPolicy::Refuse,
        ),
        Err(ContinuationError::SchemaMismatch)
    ));
    fs::write(&v1_path, &v2_bytes).unwrap();
    assert!(matches!(
        DurableInvocation::recover(
            &scratch.dir(),
            &key,
            &scalar_program,
            "app.ask",
            &args(),
            "cross-schema",
            7,
            STEPS,
            TornTailPolicy::Refuse,
        ),
        Err(ContinuationError::SchemaMismatch)
    ));
}

#[test]
fn recovered_dispatched_sites_never_dispatch_again() {
    for crash_site in 0..2 {
        let scratch = Scratch::new(0o700);
        let (key, program) = (key(), program(SOURCE));
        let arguments = arguments();
        let invocation_id = format!("in-doubt-{crash_site}");
        let mut invocation = AggregateDurableInvocation::start(
            &scratch.dir(),
            &key,
            &program,
            "app.ask",
            &arguments,
            &invocation_id,
            7,
            STEPS,
        )
        .unwrap();
        if crash_site == 1 {
            let request = invocation.dispatch(&policy()).unwrap();
            invocation.answer(&policy(), &answer(&request)).unwrap();
        }
        let request = invocation.dispatch(&policy()).unwrap();
        assert_eq!(request.site, crash_site);
        drop(invocation);
        let mut recovered = AggregateDurableInvocation::recover(
            &scratch.dir(),
            &key,
            &program,
            "app.ask",
            &arguments,
            &invocation_id,
            7,
            STEPS,
            TornTailPolicy::Refuse,
        )
        .unwrap();
        assert_eq!(
            recovered.status(),
            AggregateContinuationStatus::AwaitingAnswer {
                request: request.clone(),
                in_doubt: true,
            }
        );
        assert!(matches!(
            recovered.dispatch(&policy()),
            Err(ContinuationError::NotAwaitingDispatch)
        ));
        recovered.answer(&policy(), &answer(&request)).unwrap();
    }
}

#[test]
fn every_v2_append_boundary_recovers_without_repeating_cleanup() {
    // Started, two (Yielded, Dispatched, Answered) groups, Completed,
    // CleanupStarted, CleanupSettled. Exercise both sides of every append.
    for at in 0..10 {
        for after in [false, true] {
            let scratch = Scratch::new(0o700);
            let (key, program) = (key(), program(SOURCE));
            let arguments = arguments();
            let invocation_id = format!("aggregate-crash-{at}-{after}");
            let mut cleanup = Cleanup::default();
            arm(at, after);
            let initial = AggregateDurableInvocation::start(
                &scratch.dir(),
                &key,
                &program,
                "app.ask",
                &arguments,
                &invocation_id,
                7,
                STEPS,
            );
            if let Ok(mut invocation) = initial {
                loop {
                    let result = match invocation.status() {
                        AggregateContinuationStatus::AwaitingDispatch(_) => {
                            invocation.dispatch(&policy()).map(|_| ())
                        }
                        AggregateContinuationStatus::AwaitingAnswer { request, .. } => {
                            invocation.answer(&policy(), &answer(&request))
                        }
                        AggregateContinuationStatus::CleanupPending { .. } => {
                            invocation.settle(&mut cleanup)
                        }
                        AggregateContinuationStatus::CleanupInDoubt { .. } => {
                            panic!("fresh cleanup cannot be in doubt")
                        }
                        AggregateContinuationStatus::Settled { .. } => break,
                    };
                    if result.is_err() {
                        break;
                    }
                }
                drop(invocation);
            }
            let mut recovered = AggregateDurableInvocation::recover(
                &scratch.dir(),
                &key,
                &program,
                "app.ask",
                &arguments,
                &invocation_id,
                7,
                STEPS,
                TornTailPolicy::Refuse,
            )
            .unwrap();
            loop {
                match recovered.status() {
                    AggregateContinuationStatus::AwaitingDispatch(_) => {
                        let request = recovered.dispatch(&policy()).unwrap();
                        recovered.answer(&policy(), &answer(&request)).unwrap();
                    }
                    AggregateContinuationStatus::AwaitingAnswer { request, in_doubt } => {
                        assert!(in_doubt);
                        assert!(matches!(
                            recovered.dispatch(&policy()),
                            Err(ContinuationError::NotAwaitingDispatch)
                        ));
                        recovered.answer(&policy(), &answer(&request)).unwrap();
                    }
                    AggregateContinuationStatus::CleanupPending { .. } => {
                        recovered.settle(&mut cleanup).unwrap()
                    }
                    AggregateContinuationStatus::CleanupInDoubt { .. } => {
                        recovered.confirm_cleanup().unwrap()
                    }
                    AggregateContinuationStatus::Settled { outcome, .. } => {
                        assert_eq!(
                            outcome,
                            AggregateDurableOutcome::Completed(ResumableChannelValue::Record {
                                declaration: DeclarationId::new("app.output"),
                                fields: vec![ArgumentValue::Int(12), ArgumentValue::Bool(true)],
                            })
                        );
                        break;
                    }
                }
            }
            assert!(
                cleanup.0 <= 1,
                "cleanup repeated across crash at {at}, after={after}"
            );
        }
    }
}

#[test]
fn failed_and_cleanup_append_boundaries_preserve_sticky_failure() {
    // In this run Failed is append 3, followed by the two cleanup records.
    for at in 3..6 {
        for after in [false, true] {
            let scratch = Scratch::new(0o700);
            let (key, program) = (key(), program(SOURCE));
            let arguments = arguments();
            let invocation_id = format!("aggregate-failed-{at}-{after}");
            arm(at, after);
            let mut invocation = AggregateDurableInvocation::start(
                &scratch.dir(),
                &key,
                &program,
                "app.ask",
                &arguments,
                &invocation_id,
                7,
                STEPS,
            )
            .unwrap();
            invocation.dispatch(&policy()).unwrap();
            let mut cleanup = Cleanup::default();
            if invocation.abandon(&policy()).is_ok() {
                let _ = invocation.settle(&mut cleanup);
            }
            drop(invocation);
            let mut recovered = AggregateDurableInvocation::recover(
                &scratch.dir(),
                &key,
                &program,
                "app.ask",
                &arguments,
                &invocation_id,
                7,
                STEPS,
                TornTailPolicy::Refuse,
            )
            .unwrap();
            if matches!(
                recovered.status(),
                AggregateContinuationStatus::AwaitingAnswer { .. }
            ) {
                assert!(matches!(
                    recovered.dispatch(&policy()),
                    Err(ContinuationError::NotAwaitingDispatch)
                ));
                recovered.abandon(&policy()).unwrap();
            }
            match recovered.status() {
                AggregateContinuationStatus::CleanupPending { failure } => {
                    assert_eq!(failure, Some(DurableFailure::HostAbandoned));
                    recovered.settle(&mut cleanup).unwrap();
                }
                AggregateContinuationStatus::CleanupInDoubt { failure } => {
                    assert_eq!(failure, Some(DurableFailure::HostAbandoned));
                    recovered.confirm_cleanup().unwrap();
                }
                AggregateContinuationStatus::Settled { .. } => {}
                other => panic!("unexpected recovery state: {other:?}"),
            }
            let AggregateContinuationStatus::Settled { outcome, .. } = recovered.status() else {
                panic!()
            };
            assert_eq!(
                outcome,
                AggregateDurableOutcome::Failed(DurableFailure::HostAbandoned)
            );
            assert!(cleanup.0 <= 1);
        }
    }
}
