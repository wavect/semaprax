//! Actual B/carriers and authenticated rows; E/store are synthetic here.
//! These tests confer no physical restoration, retention ACK or dispatch proof.
use super::super::{fold::tests as seed, model::OwnedBodyV8 as Body};
use super::*;
use serde_json::json;
fn digest(domain: &'static [u8], v: &Value) -> String {
    crate::live_invocation::identity::digest(domain, &wire::canonical(v))
}
fn change(rows: &mut [EntryV8], index: usize, f: impl FnOnce(&mut Value)) {
    let EntryV8::Owned(b) = &rows[index] else {
        panic!()
    };
    let mut v = serde_json::to_value(b).unwrap();
    f(&mut v);
    rows[index] = EntryV8::Owned(serde_json::from_value(v).unwrap());
}
fn state() -> Value {
    json!({"declaration":"fixture.agent.type.state","fields":[
    {"identity":"fixture.agent.type.state.objective","value":{"kind":"bytes","hex":"6162"}},
    {"identity":"fixture.agent.type.state.budget","value":{"tag":"i64","value":10}},
    {"identity":"fixture.agent.type.state.epoch","value":{"tag":"i64","value":0}}]})
}
fn fixtures() -> (
    super::super::FoldContextV8,
    SourceCheckpointKey,
    Vec<EntryV8>,
) {
    let c = seed::context();
    let key = SourceCheckpointKey::new([44; 32]);
    let mut rows = seed::fixtures(&c)
        .into_iter()
        .map(|r| r.entry)
        .collect::<Vec<_>>();
    let b = &c.checked_binding;
    let scope = scope(&c).unwrap();
    let state = state();
    let arg = digest(b"semaprax.source-owned-frame-args.v2\0", &state);
    let ordinary_state = v2::owned_wait_ordinary_state_digest_v8(b, &state).unwrap();
    change(&mut rows, 2, |v| {
        v["state"] = state.clone();
        v["argument_digest"] = json!(arg)
    });
    let EntryV8::Ordinary(Ordinary::TurnObserved {
        state: recorded, ..
    }) = &mut rows[4]
    else {
        panic!()
    };
    *recorded = ordinary_state;
    change(&mut rows, 5, |v| v["argument_digest"] = json!(arg));
    let EntryV8::Owned(Body::OwnedWaitCreated { copy_arguments, .. }) = &rows[5] else {
        panic!()
    };
    let obs = observation(&c, &scope, copy_arguments).unwrap();
    let expected = v2::OwnedWaitCheckpointExpectationV8 {
        scope: &scope,
        argument_digest: &arg,
        observation: &obs,
        sequence: 7,
        reserved_total: 20,
        consumed_total: 2,
    };
    let checkpoint = v2::test_encode_owned_wait_checkpoint_v8(b, &key, &expected, &state);
    change(&mut rows, 7, |v| {
        v["checkpoint"] = json!(crate::live_invocation::identity::hex(&checkpoint));
        v["checkpoint_digest"] = json!(wire::checkpoint_bytes_digest(&checkpoint).unwrap())
    });
    let response=format!(r#"{{"schema":"semaprax.agent-proposal.v1","agent_id":"fixture.agent","proposal_schema_digest":"{}","value":{{"fields":{{"fixture.agent.type.proposal.budget":"3","fixture.agent.type.proposal.urgent":false,"fixture.agent.type.proposal.sequence":"1"}}}}}}
"#,b.lifecycle().proposal_schema().schema().digest()).into_bytes();
    let decoded = b
        .lifecycle()
        .proposal_schema()
        .decode(std::str::from_utf8(&response).unwrap())
        .unwrap();
    let proposal = v2::bind_owned_wait_proposal_v8(b, &scope, &decoded).unwrap();
    rows[9] = EntryV8::Ordinary(Ordinary::AttemptSettled {
        turn: 0,
        attempt: 0,
        response_digest: super::super::super::source_response_digest(&response),
        response,
    });
    change(&mut rows, 12, |v| {
        v["proposal"] = proposal.value().clone();
        v["proposal_digest"] = json!(proposal.ordinary_digest());
        v["result_digest"] = json!(proposal.result_digest(&arg).unwrap())
    });
    rows[13] = EntryV8::Ordinary(Ordinary::ProposalAdmitted {
        turn: 0,
        attempt: 0,
        proposal_digest: proposal.ordinary_digest().into(),
    });
    let Body::OwnedRunCreated {
        scope,
        execution,
        binding,
        store_identity,
        limits,
        ..
    } = &c.created
    else {
        panic!()
    };
    let generation=wire::recipe_digest(wire::RecipeV8::Generation,&json!({"scope":scope,"execution":execution,"binding":binding,"store_identity":store_identity,"limits":limits})).unwrap();
    let EntryV8::Owned(Body::OwnedStateTransferReserved { wait, .. }) = &rows[14] else {
        panic!()
    };
    let transfer=wire::recipe_digest(wire::RecipeV8::Transfer,&json!({"scope":scope,"generation":generation,"turn":0,"attempt":0,"wait":wait,"from":c.helper,"to":c.authorize,"state_digest":arg,"proposal_digest":proposal.ordinary_digest()})).unwrap();
    for i in [14, 15, 17] {
        change(&mut rows, i, |v| {
            v["state_digest"] = json!(arg);
            v["proposal_digest"] = json!(proposal.ordinary_digest());
            if i != 17 {
                v["transfer_digest"] = json!(transfer);
            }
        });
    }
    change(&mut rows, 15, |v| {
        v["state"] = state;
        v["proposal"] = proposal.value().clone()
    });
    let fields = b
        .helper()
        .program()
        .declarations
        .case_fields(b.authorize().granted())
        .unwrap();
    let decision = json!({"declaration":b.authorize().decision().as_str(),"case":b.authorize().granted().as_str(),"fields":[
        {"identity":fields[0].id.as_str(),"value":{"kind":"bytes","hex":"415a"}},
        {"identity":fields[1].id.as_str(),"value":{"tag":"i64","value":3}}]});
    let dd = wire::recipe_digest(
        wire::RecipeV8::Decision,
        &json!({"scope":scope,"turn":0,"attempt":0,"authorize":c.authorize,"decision":decision}),
    )
    .unwrap();
    change(&mut rows, 17, |v| {
        v["decision"] = decision;
        v["decision_digest"] = json!(dd)
    });
    rows.truncate(18);
    (c, key, rows)
}
fn authenticated(
    c: &super::super::FoldContextV8,
    key: &SourceCheckpointKey,
    rows: &[EntryV8],
) -> Vec<EntryV8> {
    let generation = format!("sha256:{}", "a".repeat(64));
    let mut prev = "0".repeat(64);
    let mut bytes = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let expected = ExpectedRowV8 {
            invocation: c.ordinary.invocation(),
            generation: &generation,
            seq: i as u32,
            prev_mac: &prev,
            ordinary: &c.ordinary,
        };
        let encoded = wire::encode(row, &expected, key).unwrap();
        let value = wire::parse(&encoded[..encoded.len() - 1]).unwrap();
        prev = value["authentication"].as_str().unwrap().into();
        bytes.extend(encoded);
    }
    wire::decode_inventory(
        &bytes,
        &ExpectedRowV8 {
            invocation: c.ordinary.invocation(),
            generation: &generation,
            seq: 0,
            prev_mac: &"0".repeat(64),
            ordinary: &c.ordinary,
        },
        key,
    )
    .unwrap()
}
#[test]
fn owned_wait_inventory_authenticates_actual_nominal_wait_and_sdk_projection() {
    let (c, key, rows) = fixtures();
    let checked = check_entries(&c, &key, authenticated(&c, &key, &rows)).unwrap();
    assert_eq!(checked.entries().len(), 18);
    assert_eq!(
        fold::fold(&c, checked.entries()).unwrap().tail,
        fold::TailV8::PendingReady
    );
}
#[test]
fn owned_wait_inventory_rejects_authenticated_reminted_state_and_raw_sdk_settlement() {
    let (c, key, rows) = fixtures();
    for mode in ["nominal", "ordinary_state", "sdk_raw", "sdk_carrier"] {
        let mut bad = rows.clone();
        match mode {
            "nominal" => change(&mut bad, 2, |v| {
                v["state"]["declaration"] = json!("forged.state")
            }),
            "ordinary_state" => {
                let EntryV8::Ordinary(Ordinary::TurnObserved { state, .. }) = &mut bad[4] else {
                    panic!()
                };
                *state = format!("sha256:{}", "b".repeat(64));
            }
            "sdk_raw" => {
                let EntryV8::Ordinary(Ordinary::AttemptSettled {
                    response,
                    response_digest,
                    ..
                }) = &mut bad[9]
                else {
                    panic!()
                };
                *response = b"wrong SDK settlement".to_vec();
                *response_digest = super::super::super::source_response_digest(response);
            }
            "sdk_carrier" => change(&mut bad, 12, |v| {
                v["proposal"]["fields"][0]["value"] = json!(4)
            }),
            _ => unreachable!(),
        }
        assert!(
            check_entries(&c, &key, authenticated(&c, &key, &bad)).is_err(),
            "{mode}"
        );
    }
}
#[test]
fn owned_wait_inventory_rejects_remac_checkpoint_seq_totals_and_unsupported_ready_cleanup() {
    let (c, key, rows) = fixtures();
    for field in ["sequence", "reserved_total", "consumed_total"] {
        let mut bad = rows.clone();
        change(&mut bad, 7, |v| {
            let bytes =
                crate::live_invocation::identity::unhex(v["checkpoint"].as_str().unwrap()).unwrap();
            let mut envelope: Value = serde_json::from_slice(&bytes).unwrap();
            envelope["payload"][field] = json!(999);
            envelope["authentication"] =
                json!(crate::live_invocation::identity::hex(&key.authenticate(
                    b"semaprax.source-owned-frame-checkpoint-authentication.v2\0",
                    &wire::canonical(&envelope["payload"])
                )));
            let mut bytes = wire::canonical(&envelope);
            bytes.push(b'\n');
            v["checkpoint"] = json!(crate::live_invocation::identity::hex(&bytes));
            v["checkpoint_digest"] = json!(wire::checkpoint_bytes_digest(&bytes).unwrap());
        });
        assert!(
            check_entries(&c, &key, authenticated(&c, &key, &bad)).is_err(),
            "{field}"
        );
    }
    let mut ready = rows.clone();
    let old = seed::fixtures(&c);
    ready.push(old[18].entry.clone());
    assert!(check_entries(&c, &key, authenticated(&c, &key, &ready)).is_err());
    let mut cleanup = rows;
    cleanup.push(EntryV8::Owned(Body::OwnedCleanupStarted {
        turn: 0,
        attempt: Some(0),
        wait: Some(format!("sha256:{}", "a".repeat(64))),
        owner: super::super::model::OwnerV8::State,
        basis: 15,
        terminal: json!({"failure":"host_abandoned","language_status":null}),
        operations: json!([]),
        operations_digest: format!("sha256:{}", "a".repeat(64)),
    }));
    assert!(check_entries(&c, &key, authenticated(&c, &key, &cleanup)).is_err());
}
