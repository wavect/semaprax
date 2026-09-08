use super::*;
use crate::hir::DeclarationId;
use crate::interpreter::retained_call::{RetainedField, RetainedRecord};
fn identity() -> CheckpointIdentity {
    let hash = digest(b"fixture");
    CheckpointIdentity {
        execution_revision: hash.clone(),
        invocation: hash.clone(),
        registry: hash.clone(),
        program_root: hash,
    }
}
fn journal() -> OperationCheckpoint {
    OperationCheckpoint::new(
        identity(),
        CheckpointLimits {
            calls: 4,
            argument_bytes: 4096,
            result_bytes: 4096,
            total_bytes: 8192,
            reserved_fuel: 10000,
        },
    )
    .unwrap()
}
fn state() -> RetainedValue {
    RetainedValue::Record(RetainedRecord {
        record: DeclarationId::new("state"),
        fields: vec![
            RetainedField {
                field: DeclarationId::new("state.bytes"),
                value: RetainedValue::Bytes(vec![1, 2, 3]),
            },
            RetainedField {
                field: DeclarationId::new("state.count"),
                value: RetainedValue::I64(1),
            },
        ],
    })
}
fn context() -> EffectContext {
    EffectContext {
        turn: 0,
        operation: "fixture.read".into(),
        effect: "read".into(),
        authorization_binding: digest(b"grant"),
        state: state(),
        proposal: "{\"proposal\":1}\n".into(),
        arguments: vec![("query".into(), RetainedValue::I64(1))],
    }
}
#[derive(Default)]
struct Store {
    document: String,
    fail: bool,
    calls: usize,
}
impl CheckpointStore for Store {
    fn commit(&mut self, _: u64, document: &str) -> Result<(), CheckpointStoreError> {
        self.calls += 1;
        self.document = document.into();
        if self.fail {
            Err(CheckpointStoreError)
        } else {
            Ok(())
        }
    }
}
fn observed() -> (OperationCheckpoint, Store) {
    let mut journal = journal();
    let mut store = Store::default();
    journal
        .persist(
            JournalEvent::StageReservation {
                turn: 0,
                stage: "initialize".into(),
                fuel: 100,
            },
            CheckpointUsage {
                reserved_fuel: 100,
                ..Default::default()
            },
            &mut store,
        )
        .unwrap();
    journal
        .persist(
            JournalEvent::Intent(context()),
            CheckpointUsage {
                calls: 1,
                argument_bytes: value::transport_bytes(&context().arguments).unwrap(),
                reserved_fuel: 100,
                ..Default::default()
            },
            &mut store,
        )
        .unwrap();
    journal
        .persist(
            JournalEvent::Observed {
                context: context(),
                result: vec![("value".into(), RetainedValue::I64(3))],
                failure: None,
            },
            CheckpointUsage {
                calls: 1,
                argument_bytes: value::transport_bytes(&context().arguments).unwrap(),
                result_bytes: value::transport_bytes(&[("value".into(), RetainedValue::I64(3))])
                    .unwrap(),
                reserved_fuel: 100,
            },
            &mut store,
        )
        .unwrap();
    (journal, store)
}
#[test]
fn canonical_roundtrip_retains_context_and_recovery_phase() {
    let (journal, store) = observed();
    let replay = OperationCheckpoint::decode(&store.document, &identity()).unwrap();
    assert_eq!(replay.canonical_json(), journal.canonical_json());
    assert_eq!(replay.digest(), journal.digest());
    assert_eq!(
        replay.recovery_disposition(),
        RecoveryDisposition::ReplayObserved
    );
    assert_eq!(replay.events().count(), 3);
}
#[test]
fn uncertain_intent_and_lost_ack_never_admit_another_intent() {
    let mut journal = journal();
    let mut store = Store {
        fail: true,
        ..Default::default()
    };
    assert!(journal
        .persist(
            JournalEvent::Intent(context()),
            CheckpointUsage {
                calls: 1,
                argument_bytes: value::transport_bytes(&context().arguments).unwrap(),
                ..Default::default()
            },
            &mut store
        )
        .is_err());
    assert!(journal
        .persist(
            JournalEvent::Intent(context()),
            CheckpointUsage {
                calls: 2,
                ..Default::default()
            },
            &mut store
        )
        .is_err());
    assert_eq!(store.calls, 1);
    let replay = OperationCheckpoint::decode(&store.document, &identity()).unwrap();
    assert_eq!(
        replay.recovery_disposition(),
        RecoveryDisposition::UncertainIntent
    );
}
#[test]
fn duplicate_unknown_fields_changed_binding_and_predecessor_are_rejected() {
    let (_, store) = observed();
    let duplicate = store
        .document
        .replacen("{", "{\"schema\":\"duplicate\",", 1);
    assert!(OperationCheckpoint::decode(&duplicate, &identity()).is_err());
    let unknown = store.document.replacen("{", "{\"unknown\":0,", 1);
    assert!(OperationCheckpoint::decode(&unknown, &identity()).is_err());
    let mut other = identity();
    other.invocation = digest(b"other");
    assert!(OperationCheckpoint::decode(&store.document, &other).is_err());
    let mut changed: Value = serde_json::from_str(&store.document).unwrap();
    changed["entries"][1]["prior_digest"] = json!(digest(b"other"));
    assert!(OperationCheckpoint::decode(&format!("{changed}\n"), &identity()).is_err());
}
#[test]
fn replay_and_new_tail_reservations_never_refund_fuel() {
    let (mut journal, mut store) = observed();
    let before = journal.usage();
    let after = CheckpointUsage {
        reserved_fuel: before.reserved_fuel + 100,
        ..before
    };
    journal
        .persist(
            JournalEvent::StageReservation {
                turn: 0,
                stage: "reduce".into(),
                fuel: 100,
            },
            after,
            &mut store,
        )
        .unwrap();
    journal
        .persist(
            JournalEvent::Transition {
                context: context(),
                transition: "Continue".into(),
                value: state(),
            },
            after,
            &mut store,
        )
        .unwrap();
    assert_eq!(
        journal.recovery_disposition(),
        RecoveryDisposition::ContinueAfterTransition
    );
    assert!(journal
        .persist(
            JournalEvent::StageReservation {
                turn: 1,
                stage: "observe".into(),
                fuel: 100
            },
            before,
            &mut store
        )
        .is_err());
    journal
        .persist(
            JournalEvent::StageReservation {
                turn: 1,
                stage: "observe".into(),
                fuel: 100,
            },
            CheckpointUsage {
                reserved_fuel: after.reserved_fuel + 100,
                ..after
            },
            &mut store,
        )
        .unwrap();
    assert_eq!(journal.usage().reserved_fuel, 300);
}
#[test]
fn wrong_state_result_phase_or_terminal_carrier_is_rejected() {
    let (mut journal, mut store) = observed();
    let usage = journal.usage();
    let mut changed = context();
    changed.proposal.push(' ');
    assert!(journal
        .persist(
            JournalEvent::Transition {
                context: changed,
                transition: "Complete".into(),
                value: state()
            },
            usage,
            &mut store
        )
        .is_err());
    assert!(journal
        .persist(
            JournalEvent::Transition {
                context: context(),
                transition: "Complete".into(),
                value: RetainedValue::I64(1)
            },
            usage,
            &mut store
        )
        .is_err());
    journal
        .persist(
            JournalEvent::Transition {
                context: context(),
                transition: "Fail".into(),
                value: RetainedValue::I64(37),
            },
            usage,
            &mut store,
        )
        .unwrap();
    assert_eq!(
        journal.recovery_disposition(),
        RecoveryDisposition::Terminal
    );
    let mut next = context();
    next.turn = 1;
    assert!(journal
        .persist(
            JournalEvent::Intent(next),
            CheckpointUsage { calls: 2, ..usage },
            &mut store
        )
        .is_err());
}

fn remint(document: &mut Value) -> String {
    let binding = document["binding"].clone();
    let mut previous = digest(binding.to_string().as_bytes());
    for (index, row) in document["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .enumerate()
    {
        row["generation"] = json!(index as u64 + 1);
        row["prior_digest"] = json!(previous);
        let subject = json!({"binding":binding,"generation":row["generation"],"prior_digest":row["prior_digest"],"usage":row["usage"],"event":row["event"]});
        previous = digest(subject.to_string().as_bytes());
        row["digest"] = json!(previous);
    }
    document["generation"] = json!(document["entries"].as_array().unwrap().len());
    document["digest"] = json!(previous);
    format!("{document}\n")
}

#[test]
fn reminted_undercharges_and_live_limit_substitution_fail_closed() {
    let (journal, store) = observed();
    for counter in ["argument_bytes", "result_bytes"] {
        let mut document: Value = serde_json::from_str(&store.document).unwrap();
        for entry in document["entries"].as_array_mut().unwrap() {
            let count = entry["usage"][counter].as_u64().unwrap();
            if count > 0 {
                entry["usage"][counter] = json!(count - 1);
            }
        }
        assert!(OperationCheckpoint::decode(&remint(&mut document), &identity()).is_err());
    }
    let mut document: Value = serde_json::from_str(&store.document).unwrap();
    document["binding"]["limits"]["calls"] = json!(5);
    let reminted = remint(&mut document);
    // A codec alone proves content integrity; live binding must pin limits too.
    assert!(OperationCheckpoint::decode(&reminted, &identity()).is_ok());
    assert!(
        OperationCheckpoint::decode_with_limits(&reminted, &identity(), journal.limits()).is_err()
    );
    assert!(OperationCheckpoint::decode_with_limits(
        &store.document,
        &identity(),
        journal.limits()
    )
    .is_ok());
}

#[test]
fn failed_observation_is_metered_terminal_and_recovery_fuel_cannot_refund() {
    let mut journal = journal();
    let mut store = Store::default();
    let intent_usage = CheckpointUsage {
        calls: 1,
        argument_bytes: value::transport_bytes(&context().arguments).unwrap(),
        ..Default::default()
    };
    journal
        .persist(JournalEvent::Intent(context()), intent_usage, &mut store)
        .unwrap();
    let failed_usage = CheckpointUsage {
        result_bytes: 65537,
        ..intent_usage
    };
    for (reason, result, charge) in [
        ("invented", vec![], 1),
        ("handler_failed", vec![], 1),
        ("result_type", vec![], 0),
        ("result_budget", vec![], 65538),
        (
            "result_type",
            vec![("value".into(), RetainedValue::I64(1))],
            1,
        ),
    ] {
        assert!(journal
            .persist(
                JournalEvent::Observed {
                    context: context(),
                    result,
                    failure: Some(reason.into())
                },
                CheckpointUsage {
                    result_bytes: charge,
                    ..intent_usage
                },
                &mut store
            )
            .is_err());
    }
    journal
        .persist(
            JournalEvent::Observed {
                context: context(),
                result: vec![],
                failure: Some("result_budget".into()),
            },
            failed_usage,
            &mut store,
        )
        .unwrap();
    assert!(journal
        .persist(
            JournalEvent::Transition {
                context: context(),
                transition: "Complete".into(),
                value: state()
            },
            failed_usage,
            &mut store
        )
        .is_err());
    journal
        .persist(
            JournalEvent::StageReservation {
                turn: 0,
                stage: "initialize".into(),
                fuel: 100,
            },
            CheckpointUsage {
                reserved_fuel: 100,
                ..failed_usage
            },
            &mut store,
        )
        .unwrap();
    let replay =
        OperationCheckpoint::decode_with_limits(&store.document, &identity(), journal.limits())
            .unwrap();
    assert_eq!(replay.usage().result_bytes, 65537);
    assert_eq!(replay.usage().reserved_fuel, 100);
    assert_eq!(
        replay.recovery_disposition(),
        RecoveryDisposition::ReplayObserved
    );
    let calls = store.calls;
    assert!(journal
        .persist(
            JournalEvent::StageReservation {
                turn: 0,
                stage: "observe".into(),
                fuel: 100
            },
            CheckpointUsage {
                reserved_fuel: 200,
                ..intent_usage
            },
            &mut store
        )
        .is_err());
    assert_eq!(store.calls, calls);
}

#[test]
fn exact_scalar_codec_rejects_nested_duplicate_noncanonical_and_overflow_values() {
    for value in [
        RetainedValue::Bool(false),
        RetainedValue::I32(i32::MIN),
        RetainedValue::I64(i64::MAX),
        RetainedValue::U8(u8::MAX),
        RetainedValue::Usize(u64::MAX),
        RetainedValue::Bytes(vec![0, 255]),
        state(),
    ] {
        let encoded = value::encode(&value).unwrap();
        assert_eq!(value::decode(&encoded).unwrap(), value);
    }
    for value in [
        json!({"kind":"i64","value":"+1"}),
        json!({"kind":"i64","value":"01"}),
        json!({"kind":"i64","value":"9223372036854775808"}),
        json!({"kind":"usize","value":"-1"}),
        json!({"kind":"bytes","value":"FF"}),
        json!({"kind":"bytes","value":"f"}),
    ] {
        assert!(value::decode(&value).is_err());
    }
    let mut nested = value::encode(&state()).unwrap();
    nested["fields"][0]["value"] = value::encode(&state()).unwrap();
    assert!(value::decode(&nested).is_err());
    let mut duplicate = value::encode(&state()).unwrap();
    duplicate["fields"][1]["field"] = duplicate["fields"][0]["field"].clone();
    assert!(value::decode(&duplicate).is_err());
}
