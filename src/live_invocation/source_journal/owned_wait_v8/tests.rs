use super::*;
use crate::live_invocation::identity::digest;
use model::{OwnedBodyV8, PhaseV8};
use serde_json::json;

fn binding() -> SourceInvocationBinding {
    let d = |label: &str| digest(b"test\0", label.as_bytes());
    super::super::SourceInvocationBinding::bind_execution(
        super::super::SourceInvocationSeed {
            lifecycle_digest: d("lifecycle"),
            source_revision: d("source"),
            deployment_binding: d("deployment"),
            task: vec![],
            task_budget: 10,
            proposal_schema_digest: d("proposal"),
            response_limit: 4096,
            max_iterations: 2,
            max_stages: 10,
            max_attempts: 3,
            max_steps_per_stage: 10,
            max_total_steps: 100,
            ceiling: 100,
            reservation_units: 1,
            unit: "bytes".into(),
            clock_domain: "test".into(),
            initial_millis: 0,
            deadline_millis: 100,
            program_root: None,
        },
        &d("evaluator"),
    )
    .unwrap()
}
fn row_facts(binding: &SourceInvocationBinding) -> ExpectedRowV8<'_> {
    ExpectedRowV8 {
        invocation: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        generation: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        seq: 0,
        prev_mac: "0000000000000000000000000000000000000000000000000000000000000000",
        ordinary: binding,
    }
}
fn key() -> SourceCheckpointKey {
    SourceCheckpointKey::new([7; 32])
}
fn d() -> String {
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()
}
fn reserved() -> EntryV8 {
    EntryV8::Owned(OwnedBodyV8::OwnedWaitReserved {
        turn: 0,
        attempt: 0,
        wait: d(),
        phase: PhaseV8::Start,
        replay_of: None,
        fuel: 10,
    })
}
fn signed(mut row: Value) -> Vec<u8> {
    row.as_object_mut().unwrap().remove("authentication");
    let tag = crate::live_invocation::identity::hex(
        &key().authenticate(RECORD_DOMAIN, &wire::canonical(&row)),
    );
    row["authentication"] = tag.into();
    let mut out = wire::canonical(&row);
    out.push(b'\n');
    out
}

#[test]
fn canonical_record_hmac_has_an_independent_literal_known_answer() {
    let binding = binding();
    let expected = row_facts(&binding);
    let bytes = wire::encode(
        &EntryV8::Ordinary(SourceJournalEntry::RunOpened),
        &expected,
        &key(),
    )
    .unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        value["authentication"],
        "9468ac3b7fcb52adf6ce92f0a829c8eb8f4eb26018368f7e77cada189a97db16"
    );
    assert_eq!(
        wire::decode(&bytes, &expected, &key()).unwrap(),
        EntryV8::Ordinary(SourceJournalEntry::RunOpened)
    );
    assert_eq!(
        wire::canonical(&json!({"z":{"b":1,"a":2},"a":[{"z":1,"a":2},0]})),
        br#"{"a":[{"a":2,"z":1},0],"z":{"a":2,"b":1}}"#
    );
}

#[test]
fn strict_row_parser_refuses_duplicate_unknown_noncanonical_and_float_bytes() {
    let binding = binding();
    let expected = row_facts(&binding);
    let bytes = wire::encode(&reserved(), &expected, &key()).unwrap();
    let value: Value = serde_json::from_slice(&bytes).unwrap();
    for mutation in 0..8 {
        let mut row = value.clone();
        match mutation {
            0 => {
                row["extra"] = 1.into();
            }
            1 => {
                row["kind"] = "owned_unknown".into();
            }
            2 => {
                row["fuel"] = json!(1.5);
            }
            3 => {
                row["phase"] = "other".into();
            }
            4 => {
                row["replay_of"] = json!(-1);
            }
            5 => {
                row.as_object_mut().unwrap().remove("replay_of");
            }
            6 => {
                row["wait"] = "not-a-digest".into();
            }
            _ => {
                row["schema"] = "semaprax.live-invocation.source-persisted-journal.v7".into();
            }
        }
        assert!(
            wire::decode(&signed(row), &expected, &key()).is_err(),
            "mutation{mutation}"
        );
    }
    let text = String::from_utf8(bytes.clone()).unwrap();
    let duplicate = text.replacen("\"fuel\":10", "\"fuel\":10,\"fuel\":10", 1);
    assert_eq!(
        wire::decode(duplicate.as_bytes(), &expected, &key()),
        Err(SourceJournalError::Malformed)
    );
    let spaced = text.replacen('{', "{ ", 1);
    assert_eq!(
        wire::decode(spaced.as_bytes(), &expected, &key()),
        Err(SourceJournalError::Malformed)
    );
    assert!(wire::decode(&bytes[..bytes.len() - 1], &expected, &key()).is_err());
    let mut wrong = bytes;
    wrong[20] ^= 1;
    assert!(wire::decode(&wrong, &expected, &key()).is_err());
}

#[test]
fn every_external_chain_dimension_and_key_is_checked() {
    let binding = binding();
    let expected = row_facts(&binding);
    let bytes = wire::encode(&reserved(), &expected, &key()).unwrap();
    for mutation in 0..4 {
        let mut wrong = row_facts(&binding);
        match mutation {
            0 => wrong.seq = 1,
            1 => {
                wrong.prev_mac = "1111111111111111111111111111111111111111111111111111111111111111"
            }
            2 => wrong.invocation = wrong.generation,
            _ => wrong.generation = wrong.invocation,
        }
        assert!(wire::decode(&bytes, &wrong, &key()).is_err());
    }
    assert_eq!(
        wire::decode(&bytes, &expected, &SourceCheckpointKey::new([8; 32])),
        Err(SourceJournalError::Chain)
    );
}

#[test]
fn nested_duplicate_depth_and_size_caps_precede_materialization() {
    assert_eq!(
        wire::parse(br#"{"state":{"x":1,"x":2}}"#),
        Err(SourceJournalError::Malformed)
    );
    let too_deep = format!("{}0{}", "[".repeat(25), "]".repeat(25));
    assert_eq!(
        wire::parse(too_deep.as_bytes()),
        Err(SourceJournalError::Malformed)
    );
    assert!(wire::parse(format!("{}0{}", "[".repeat(24), "]".repeat(24)).as_bytes()).is_ok());
    let oversized = vec![b' '; super::super::MAX_SOURCE_DOCUMENT_BYTES + 1];
    assert_eq!(wire::parse(&oversized), Err(SourceJournalError::Capacity));
}

#[test]
fn owned_body_optional_fields_are_required_and_snapshot_caps_apply_to_producers() {
    let binding = binding();
    let expected = row_facts(&binding);
    let entry = EntryV8::Owned(OwnedBodyV8::OwnedWaitCompleted {
        turn: 0,
        attempt: 0,
        wait: d(),
        reservation: 1,
        proposal: json!({"field":"x".repeat(65536)}),
        proposal_digest: d(),
        result_digest: d(),
        consumed: 1,
    });
    assert_eq!(
        wire::encode(&entry, &expected, &key()),
        Err(SourceJournalError::Capacity)
    );
    let prepared = EntryV8::Owned(OwnedBodyV8::OwnedWaitPrepared {
        turn: 0,
        attempt: 0,
        wait: d(),
        reservation: 1,
        observation_digest: d(),
        checkpoint_digest: d(),
        checkpoint: "a".repeat(131073),
        consumed: 1,
    });
    assert_eq!(
        wire::encode(&prepared, &expected, &key()),
        Err(SourceJournalError::Capacity)
    );
}

#[test]
fn all_sixteen_closed_owned_bodies_roundtrip_and_refuse_each_missing_key() {
    // Independently transcribed normative §8.4 inventory, including nullable keys.
    let shapes=[
        ("owned_run_created","scope execution binding signature limits store_identity"),
        ("owned_state_committed","turn state argument_digest cleanup_plan_digest"),
        ("owned_wait_created","turn attempt wait plan_digest cleanup_plan_digest signature argument_digest copy_arguments copy_arguments_digest"),
        ("owned_wait_reserved","turn attempt wait phase replay_of fuel"),
        ("owned_wait_prepared","turn attempt wait reservation observation_digest checkpoint_digest checkpoint consumed"),
        ("owned_wait_completed","turn attempt wait reservation proposal proposal_digest result_digest consumed"),
        ("owned_wait_failed","turn attempt wait reservation status consumed"),
        ("owned_wait_replay_checked","turn attempt wait reservation original result_digest consumed"),
        ("owned_wait_retired","turn attempt wait prepared state_digest observation_digest"),
        ("owned_state_rearmed","turn attempt wait retired state state_digest observation observation_digest"),
        ("owned_cleanup_started","turn attempt wait owner basis terminal operations operations_digest"),
        ("owned_cleanup_settled","turn attempt wait owner started receipt receipt_digest"),
        ("owned_state_transfer_reserved","turn attempt wait from to state_digest proposal_digest transfer_digest"),
        ("owned_state_transfer_completed","turn attempt wait reservation state state_digest proposal proposal_digest transfer_digest"),
        ("owned_authorization_staged","turn attempt stage_reservation transfer state_digest proposal_digest decision decision_digest consumed"),
        ("owned_authorization_ready","turn attempt staged state_digest decision_digest grant_digest"),
    ];
    let binding = binding();
    let expected = row_facts(&binding);
    for (kind, fields) in shapes {
        let mut body = json!({"kind":kind});
        for field in fields.split_whitespace() {
            body[field] = if field.ends_with("_digest")
                || matches!(field, "wait" | "execution" | "binding")
            {
                d().into()
            } else {
                match field {
                    "phase" => "start".into(),
                    "owner" => "state".into(),
                    "checkpoint" => "00".into(),
                    "from" | "to" => "function.id".into(),
                    "replay_of" => Value::Null,
                    "scope" | "signature" | "limits" | "store_identity" | "state" | "proposal"
                    | "decision" | "observation" | "status" | "terminal" | "receipt" => json!({}),
                    "copy_arguments" | "operations" => json!([]),
                    _ => 0.into(),
                }
            };
        }
        let typed: OwnedBodyV8 = serde_json::from_value(body).unwrap();
        let entry = EntryV8::Owned(typed);
        let bytes = wire::encode(&entry, &expected, &key()).unwrap();
        assert_eq!(
            wire::decode(&bytes, &expected, &key()).unwrap(),
            entry,
            "{kind}"
        );
        let row: Value = serde_json::from_slice(&bytes).unwrap();
        for field in fields.split_whitespace() {
            let mut missing = row.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(
                wire::decode(&signed(missing), &expected, &key()).is_err(),
                "{kind}/{field}"
            );
        }
        let mut extra = row;
        extra["unrecognized"] = true.into();
        assert!(
            wire::decode(&signed(extra), &expected, &key()).is_err(),
            "{kind}/extra"
        );
    }
}
