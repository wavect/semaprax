//! Inert protocol tests, not production E/B authority or recovery evidence.
use super::*;
use crate::hir::DeclarationId;
use crate::live_invocation::identity::digest;
use serde_json::json;
use std::path::Path;

fn checked_binding() -> crate::resumable_effects::owned_frame::v2::CheckedOwnedAgentWaitBindingV8 {
    let source = include_str!("../../../../examples/offline-repair-project/src/app.spx");
    let source = format!(
        "{}\n{}",
        source.replace(
            "    runtime_v1 {",
            "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {"
        ),
        r#"
@id("fixture.agent.fn.park")
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
    let proposal = yield observation;
    state
}
"#
    );
    crate::resumable_effects::owned_frame::v2::compile_owned_agent_wait_v8(
        &source,
        Path::new("inert-v8-fold.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap_or_else(|e| panic!("{e:?}"))
}

fn hash(label: &str) -> String {
    digest(b"synthetic-v8-fold-test\0", label.as_bytes())
}
fn owned(value: Value) -> ValidatedEntryV8 {
    ValidatedEntryV8 {
        entry: EntryV8::Owned(serde_json::from_value(value).unwrap()),
        observation: None,
    }
}
fn ordinary(entry: SourceJournalEntry) -> ValidatedEntryV8 {
    ValidatedEntryV8 {
        entry: EntryV8::Ordinary(entry),
        observation: None,
    }
}
fn copy(rows: &[ValidatedEntryV8]) -> Vec<ValidatedEntryV8> {
    rows.iter()
        .map(|r| ValidatedEntryV8 {
            entry: r.entry.clone(),
            observation: r.observation.clone(),
        })
        .collect()
}
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn context() -> FoldContextV8 {
    let checked = checked_binding();
    let authorize = checked.authorize();
    assert_eq!(authorize.disposal().len(), 1);
    assert_eq!(authorize.partial_disposal().len(), 1);
    assert!(authorize.partial_disposal()[0].active_case.is_none());
    let ordinary = super::super::tests::binding();
    let signature = checked.signature().clone();
    let created = Body::OwnedRunCreated {
        scope: json!({"invocation":"inert-v8-fold","program_root":checked.binding(),"policy_epoch":0}),
        execution: ordinary.invocation().to_owned(),
        binding: checked.binding().to_owned(),
        signature: signature.clone(),
        limits: json!({"fuel":100}),
        store_identity: json!({"directory_device":1,"directory_inode":2,"file_device":1,"file_inode":3}),
    };
    // B and compiler metadata are checked. E and protocol/store identity remain
    // synthetic: these tests establish no typed execution or physical authority.
    FoldContextV8 {
        initialized_task: None,
        ordinary,
        created,
        plan_digest: checked.binding().into(),
        cleanup_plan_digest: checked.cleanup_digest().into(),
        signature,
        helper: checked.helper().function().id.as_str().into(),
        authorize: authorize.function().id.as_str().into(),
        granted: authorize.granted().as_str().into(),
        refused: authorize.refused().as_str().into(),
        refused_cleanup_empty: authorize.disposal().iter().all(|action| {
            action
                .active_case
                .as_ref()
                .is_some_and(|case| case.case != *authorize.refused())
        }),
        checked_binding: std::sync::Arc::new(checked),
    }
}
fn observation_facts(
    c: &FoldContextV8,
    budget: i64,
) -> crate::resumable_effects::owned_frame::v2::CheckedOwnedWaitObservationV8 {
    let Body::OwnedRunCreated { scope, .. } = &c.created else {
        panic!()
    };
    let scope = crate::resumable_effects::source_checkpoint::SourceCheckpointScope::new(
        scope["program_root"].as_str().unwrap(),
        scope["invocation"].as_str().unwrap(),
        scope["policy_epoch"].as_u64().unwrap(),
    )
    .unwrap();
    let observation = crate::interpreter::resumable::ResumableChannelValue::Record {
        declaration: DeclarationId::new("fixture.agent.type.observation"),
        fields: vec![
            crate::interpreter::ArgumentValue::Int(budget),
            crate::interpreter::ArgumentValue::Int(0),
        ],
    };
    crate::resumable_effects::owned_frame::v2::bind_owned_wait_observation_v8(
        &c.checked_binding,
        &scope,
        &observation,
    )
    .unwrap()
}
fn clone_row(row: &ValidatedEntryV8) -> ValidatedEntryV8 {
    ValidatedEntryV8 {
        entry: row.entry.clone(),
        observation: row.observation.clone(),
    }
}

pub(in crate::live_invocation::source_journal::owned_wait_v8) fn fixtures(
    c: &FoldContextV8,
) -> Vec<ValidatedEntryV8> {
    let Body::OwnedRunCreated {
        scope,
        execution,
        binding,
        ..
    } = &c.created
    else {
        panic!()
    };
    let invocation = wire::recipe_digest(
        wire::RecipeV8::Invocation,
        &json!({"execution":execution,"owned_wait_binding":binding}),
    )
    .unwrap();
    let wait = wire::recipe_digest(
        wire::RecipeV8::Attempt,
        &json!({"invocation":invocation,"turn":0,"attempt":0,"binding":binding}),
    )
    .unwrap();
    let generation = wire::generation_digest_from_created(&c.created).unwrap();
    let state = json!({"declaration":"state","fields":[{"identity":"state.z","value":{"kind":"bytes","hex":""}},{"identity":"state.a","value":{"kind":"bytes","hex":"00"}},{"identity":"state.budget","value":{"kind":"i64","value":10}}]});
    let state_digest = digest(
        b"semaprax.source-owned-frame-args.v2\0",
        &wire::canonical(&state),
    );
    let facts = observation_facts(c, 10);
    let args = facts.copy_arguments().clone();
    let args_digest = digest(
        b"semaprax.source-owned-frame-copy-args.v2\0",
        &wire::canonical(&args),
    );
    let proposal = json!({"kind":"record","declaration":"proposal","fields":[{"kind":"i64","value":3},{"kind":"bool","value":false},{"kind":"usize","value":1}]});
    let proposal_digest = hash("proposal sidecar");
    // Synthetic canonical envelope tests hash/causality only. It is not an
    // authenticated runtime checkpoint nor an owner-restoration credential.
    let mut checkpoint =
        wire::canonical(&json!({"payload":{"state":state},"authentication":"0".repeat(64)}));
    checkpoint.push(b'\n');
    let checkpoint_digest = wire::checkpoint_bytes_digest(&checkpoint).unwrap();
    let transfer_digest=wire::recipe_digest(wire::RecipeV8::Transfer,&json!({"scope":scope,"generation":generation,"turn":0,"attempt":0,"wait":wait,"from":c.helper,"to":c.authorize,"state_digest":state_digest,"proposal_digest":proposal_digest})).unwrap();
    let decision = json!({"declaration":"decision","case":c.granted,"fields":[{"identity":"decision.seal","value":{"kind":"bytes","hex":"415a"}},{"identity":"decision.budget","value":{"kind":"i64","value":3}}]});
    let decision_digest = wire::recipe_digest(
        wire::RecipeV8::Decision,
        &json!({"scope":scope,"turn":0,"attempt":0,"authorize":c.authorize,"decision":decision}),
    )
    .unwrap();
    let request = hash("request");
    let prompt = hash("prompt");
    let mut rows = vec![
        ValidatedEntryV8 {
            entry: EntryV8::Owned(c.created.clone()),
            observation: None,
        },
        ordinary(SourceJournalEntry::RunOpened),
        owned(
            json!({"kind":"owned_state_committed","turn":0,"state":state,"argument_digest":state_digest,"cleanup_plan_digest":c.cleanup_plan_digest}),
        ),
        ordinary(SourceJournalEntry::StageReservation {
            turn: 0,
            attempt: None,
            role: SourceStageRole::Observe,
            fuel: c.ordinary.max_steps_per_stage().unwrap(),
        }),
        ordinary(SourceJournalEntry::TurnObserved {
            turn: 0,
            state: hash("ordinary state"),
            observation: facts.ordinary_digest().into(),
            feedback: hash("feedback"),
        }),
        owned(
            json!({"kind":"owned_wait_created","turn":0,"attempt":0,"wait":wait,"plan_digest":c.plan_digest,"cleanup_plan_digest":c.cleanup_plan_digest,"signature":c.signature,"argument_digest":state_digest,"copy_arguments":args,"copy_arguments_digest":args_digest}),
        ),
        owned(
            json!({"kind":"owned_wait_reserved","turn":0,"attempt":0,"wait":wait,"phase":"start","replay_of":null,"fuel":c.ordinary.max_steps_per_stage().unwrap()}),
        ),
        owned(
            json!({"kind":"owned_wait_prepared","turn":0,"attempt":0,"wait":wait,"reservation":6,"observation_digest":facts.request_digest(),"checkpoint_digest":checkpoint_digest,"checkpoint":crate::live_invocation::identity::hex(&checkpoint),"consumed":2}),
        ),
        ordinary(SourceJournalEntry::AttemptIntent {
            turn: 0,
            attempt: 0,
            attempt_digest: c.ordinary.attempt_digest(0, 0, &request, &prompt, 12),
            request_digest: request,
            prompt_digest: prompt,
            request_bytes: 12,
            reserved_units: 1,
            response_limit: 4096,
        }),
        ordinary(SourceJournalEntry::AttemptSettled {
            turn: 0,
            attempt: 0,
            response: b"proposal".to_vec(),
            response_digest: super::super::super::source_response_digest(b"proposal"),
        }),
        ordinary(SourceJournalEntry::AttemptUsage {
            turn: 0,
            attempt: 0,
            reported: None,
        }),
        owned(
            json!({"kind":"owned_wait_reserved","turn":0,"attempt":0,"wait":wait,"phase":"resume","replay_of":null,"fuel":c.ordinary.max_steps_per_stage().unwrap()}),
        ),
        owned(
            json!({"kind":"owned_wait_completed","turn":0,"attempt":0,"wait":wait,"reservation":11,"proposal":proposal,"proposal_digest":proposal_digest,"result_digest":hash("result"),"consumed":3}),
        ),
        ordinary(SourceJournalEntry::ProposalAdmitted {
            turn: 0,
            attempt: 0,
            proposal_digest: proposal_digest.clone(),
        }),
        owned(
            json!({"kind":"owned_state_transfer_reserved","turn":0,"attempt":0,"wait":wait,"from":c.helper,"to":c.authorize,"state_digest":state_digest,"proposal_digest":proposal_digest,"transfer_digest":transfer_digest}),
        ),
        owned(
            json!({"kind":"owned_state_transfer_completed","turn":0,"attempt":0,"wait":wait,"reservation":14,"state":state,"state_digest":state_digest,"proposal":proposal,"proposal_digest":proposal_digest,"transfer_digest":transfer_digest}),
        ),
        ordinary(SourceJournalEntry::StageReservation {
            turn: 0,
            attempt: Some(0),
            role: SourceStageRole::Authorize,
            fuel: c.ordinary.max_steps_per_stage().unwrap(),
        }),
        owned(
            json!({"kind":"owned_authorization_staged","turn":0,"attempt":0,"stage_reservation":16,"transfer":15,"state_digest":state_digest,"proposal_digest":proposal_digest,"decision":decision,"decision_digest":decision_digest,"consumed":4}),
        ),
        owned(
            json!({"kind":"owned_authorization_ready","turn":0,"attempt":0,"staged":17,"state_digest":state_digest,"decision_digest":decision_digest,"grant_digest":hash("grant")}),
        ),
        ordinary(SourceJournalEntry::AuthorizationConsumed {
            turn: 0,
            attempt: 0,
            grant_digest: hash("grant"),
        }),
    ];
    rows[5].observation = Some(facts.clone());
    rows[7].observation = Some(facts);
    rows
}
fn body_mut(rows: &mut [ValidatedEntryV8], index: usize, change: impl FnOnce(&mut Value)) {
    let EntryV8::Owned(b) = &rows[index].entry else {
        panic!()
    };
    let mut value = serde_json::to_value(b).unwrap();
    change(&mut value);
    let facts = rows[index].observation.clone();
    rows[index] = owned(value);
    rows[index].observation = facts;
}
fn replay_wait(rows: &[ValidatedEntryV8], phase: PhaseV8, original: u32) -> ValidatedEntryV8 {
    let EntryV8::Owned(Body::OwnedWaitCreated { wait, .. }) = &rows[5].entry else {
        panic!()
    };
    owned(
        json!({"kind":"owned_wait_reserved","turn":0,"attempt":0,"wait":wait,"phase":phase,"replay_of":original,"fuel":10}),
    )
}
#[test]
fn partial_inventory_tracks_every_phase_and_only_recorded_consumption() {
    let c = context();
    let rows = fixtures(&c);
    let tails = [
        TailV8::Created,
        TailV8::Opened,
        TailV8::CommittedState,
        TailV8::ObserveReserved,
        TailV8::Observed,
        TailV8::WaitCreated,
        TailV8::StartReserved,
        TailV8::Prepared,
        TailV8::ModelDispatchInDoubt,
        TailV8::Settled,
        TailV8::Settled,
        TailV8::ResumeReserved,
        TailV8::Completed,
        TailV8::Admitted,
        TailV8::TransferReserved,
        TailV8::PendingAuthorize,
        TailV8::ChargedAuthorizeReplay,
        TailV8::PendingReady,
        TailV8::ResultDeliveryInDoubt,
        TailV8::ReadyPair,
    ];
    assert_eq!(fold(&c, &[]).unwrap().tail, TailV8::Empty);
    for (count, tail) in tails.into_iter().enumerate() {
        assert_eq!(
            fold(&c, &rows[..count + 1]).unwrap().tail,
            tail,
            "prefix {}",
            count + 1
        );
    }
    let f = fold(&c, &rows).unwrap();
    assert_eq!(
        (f.reserved_total, f.consumed_recorded, f.stages),
        (40, 9, 2)
    );
    let mut extra = copy(&rows);
    extra.push(ordinary(SourceJournalEntry::RunOpened));
    assert!(fold(&c, &extra).is_err());
}
#[test]
fn causal_bindings_reject_mutations_without_repair() {
    let c = context();
    let rows = fixtures(&c);
    for (index, key, value) in [
        (6, "fuel", json!(11)),
        (7, "reservation", json!(3)),
        (12, "reservation", json!(6)),
        (15, "proposal", json!({})),
        (15, "reservation", json!(7)),
        (17, "stage_reservation", json!(3)),
        (17, "decision_digest", json!(hash("wrong"))),
        (18, "staged", json!(15)),
        (18, "decision_digest", json!(hash("wrong"))),
    ] {
        let mut bad = copy(&rows);
        body_mut(&mut bad, index, |b| b[key] = value);
        assert!(fold(&c, &bad).is_err(), "{index}/{key}");
    }
    let mut bad = copy(&rows[..8]);
    bad.push(replay_wait(&rows, PhaseV8::Start, 7));
    assert!(fold(&c, &bad).is_err()); // closure is not reservation
}
#[test]
fn replay_start_must_close_before_resume_and_after_resume_before_admission() {
    let c = context();
    let rows = fixtures(&c);
    let mut before = copy(&rows[..11]);
    before.push(replay_wait(&rows, PhaseV8::Start, 6));
    before.push(clone_row(&rows[11]));
    assert!(fold(&c, &before).is_err());
    let mut after = copy(&rows[..13]);
    after.push(replay_wait(&rows, PhaseV8::Start, 6));
    after.push(clone_row(&rows[13]));
    assert!(fold(&c, &after).is_err());
    let EntryV8::Owned(Body::OwnedWaitCreated { wait, .. }) = &rows[5].entry else {
        panic!()
    };
    let EntryV8::Owned(Body::OwnedWaitPrepared {
        checkpoint_digest, ..
    }) = &rows[7].entry
    else {
        panic!()
    };
    let mut replay = copy(&rows[..11]);
    replay.push(replay_wait(&rows, PhaseV8::Start, 6));
    replay.push(replay_wait(&rows, PhaseV8::Start, 6));
    replay.push(owned(json!({"kind":"owned_wait_replay_checked","turn":0,"attempt":0,"wait":wait,"reservation":12,"original":7,"result_digest":checkpoint_digest,"consumed":2})));
    let f = fold(&c, &replay).unwrap();
    assert_eq!((f.reserved_total, f.consumed_recorded), (40, 4));
    body_mut(&mut replay, 13, |b| b["reservation"] = json!(11));
    assert!(fold(&c, &replay).is_err());
}

fn cleanup_start(
    rows: &[ValidatedEntryV8],
    owner: OwnerV8,
    basis: u32,
    pre_wait: bool,
    terminal: Value,
) -> ValidatedEntryV8 {
    let (attempt, wait) = if pre_wait {
        (Value::Null, Value::Null)
    } else {
        let EntryV8::Owned(Body::OwnedWaitCreated { wait, .. }) = &rows[5].entry else {
            panic!()
        };
        (json!(0), json!(wait))
    };
    // Synthetic operations test grammar/hash ordering only. Real compiler
    // operation admission is a separately sealed ValidatedEntry obligation.
    let operations = json!([{"compiler_test_slot":0}]);
    let operations_digest = wire::recipe_digest(
        wire::RecipeV8::Operations,
        &json!({"owner":owner,"basis":basis,"terminal":terminal,"operations":operations}),
    )
    .unwrap();
    owned(
        json!({"kind":"owned_cleanup_started","turn":0,"attempt":attempt,"wait":wait,"owner":owner,"basis":basis,"terminal":terminal,"operations":operations,"operations_digest":operations_digest}),
    )
}
fn cleanup_receipt(
    rows: &[ValidatedEntryV8],
    owner: OwnerV8,
    started: u32,
    pre_wait: bool,
    kind: &str,
) -> ValidatedEntryV8 {
    let (attempt, wait) = if pre_wait {
        (Value::Null, Value::Null)
    } else {
        let EntryV8::Owned(Body::OwnedWaitCreated { wait, .. }) = &rows[5].entry else {
            panic!()
        };
        (json!(0), json!(wait))
    };
    let receipt = json!({"kind":kind,"operations":[]});
    let receipt_digest = wire::recipe_digest(wire::RecipeV8::Receipt, &receipt).unwrap();
    owned(
        json!({"kind":"owned_cleanup_settled","turn":0,"attempt":attempt,"wait":wait,"owner":owner,"started":started,"receipt":receipt,"receipt_digest":receipt_digest}),
    )
}
#[test]
fn observe_failure_uses_committed_state_without_inventing_attempt_or_model_failure() {
    let c = context();
    let rows = fixtures(&c);
    let terminal = json!({"status":{"domain":1,"code":1,"class":"contract"}});
    let mut failed = copy(&rows[..4]);
    failed.push(cleanup_start(
        &rows,
        OwnerV8::State,
        2,
        true,
        terminal.clone(),
    ));
    assert_eq!(fold(&c, &failed).unwrap().tail, TailV8::CleanupInDoubt);
    let mut bad = copy(&failed);
    bad.push(ordinary(SourceJournalEntry::ReplayStageReservation {
        replay: 0,
        causal_seq: 3,
        role: SourceStageRole::Observe,
        fuel: 10,
    }));
    assert!(fold(&c, &bad).is_err());
    failed.push(cleanup_receipt(&rows, OwnerV8::State, 4, true, "observed"));
    let f = fold(&c, &failed).unwrap();
    assert_eq!(
        (f.tail, f.reserved_total, f.consumed_recorded),
        (TailV8::MetadataOnly, 10, 0)
    );
    assert!(f.state_basis.is_none());
    let mut wrong = copy(&failed[..4]);
    wrong.push(cleanup_start(&rows, OwnerV8::State, 3, true, terminal));
    assert!(fold(&c, &wrong).is_err());
}
#[test]
fn failed_unstaged_authorize_replays_full_fuel_and_pins_partial_obligation_basis() {
    let c = context();
    let rows = fixtures(&c);
    let terminal = json!({"status":{"domain":1,"code":2,"class":"arithmetic"}});
    let mut failed = copy(&rows[..17]);
    for replay in 0..2 {
        failed.push(ordinary(SourceJournalEntry::ReplayStageReservation {
            replay,
            causal_seq: 3,
            role: SourceStageRole::Observe,
            fuel: 10,
        }));
        failed.push(ordinary(SourceJournalEntry::ReplayStageReservation {
            replay,
            causal_seq: 16,
            role: SourceStageRole::Authorize,
            fuel: 10,
        }));
    }
    assert_eq!(fold(&c, &failed).unwrap().reserved_total, 80);
    let mut old = copy(&failed);
    old.push(cleanup_start(
        &rows,
        OwnerV8::Decision,
        16,
        false,
        terminal.clone(),
    ));
    assert!(fold(&c, &old).is_err());
    failed.push(cleanup_start(
        &rows,
        OwnerV8::Decision,
        20,
        false,
        terminal.clone(),
    ));
    assert_eq!(fold(&c, &failed).unwrap().tail, TailV8::CleanupInDoubt);
    let mut host = copy(&failed);
    host.push(cleanup_receipt(
        &rows,
        OwnerV8::Decision,
        21,
        false,
        "host_confirmed",
    ));
    assert_eq!(fold(&c, &host).unwrap().tail, TailV8::CleanupInDoubt);
    host.push(cleanup_start(
        &rows,
        OwnerV8::State,
        15,
        false,
        terminal.clone(),
    ));
    assert!(fold(&c, &host).is_err());
    failed.push(cleanup_receipt(
        &rows,
        OwnerV8::Decision,
        21,
        false,
        "observed",
    ));
    assert_eq!(fold(&c, &failed).unwrap().tail, TailV8::PendingStateCleanup);
    let mut replaced = copy(&failed);
    replaced.push(cleanup_start(
        &rows,
        OwnerV8::State,
        15,
        false,
        json!({"status":{"code":99}}),
    ));
    assert!(fold(&c, &replaced).is_err());
    failed.push(cleanup_start(&rows, OwnerV8::State, 15, false, terminal));
    failed.push(cleanup_receipt(
        &rows,
        OwnerV8::State,
        23,
        false,
        "observed",
    ));
    let f = fold(&c, &failed).unwrap();
    assert_eq!(
        (f.tail, f.reserved_total, f.consumed_recorded),
        (TailV8::MetadataOnly, 80, 5)
    );
    assert!(failed.iter().all(|r| !matches!(
        r.entry,
        EntryV8::Owned(Body::OwnedAuthorizationStaged { .. })
    )));
}
#[test]
fn unstaged_failure_without_decision_cleans_state_and_never_emits_ready_or_early_stop() {
    let c = context();
    let rows = fixtures(&c);
    let mut failed = copy(&rows[..17]);
    let stop = ordinary(SourceJournalEntry::Stop {
        turn: Some(0),
        attempt: Some(0),
        status: super::super::super::SourceStopStatus::BudgetExhausted,
        reason: super::super::super::SourceStopReason::BudgetExhausted,
    });
    let previous = fold(&c, &failed).unwrap();
    assert!(validate_producer_transition(&previous, &stop).is_err());
    let mut early = copy(&failed);
    early.push(stop);
    let uncertain = fold(&c, &early).unwrap();
    assert_eq!(uncertain.tail, TailV8::StopInDoubt);
    assert!(
        uncertain.state_basis.is_none()
            && uncertain.decision.is_none()
            && uncertain.transfer.is_none()
    );
    early.push(clone_row(&rows[17]));
    assert!(fold(&c, &early).is_err());
    failed.push(cleanup_start(
        &rows,
        OwnerV8::State,
        15,
        false,
        json!({"status":{"class":"arithmetic","code":2}}),
    ));
    failed.push(cleanup_receipt(
        &rows,
        OwnerV8::State,
        17,
        false,
        "observed",
    ));
    assert_eq!(fold(&c, &failed).unwrap().tail, TailV8::MetadataOnly);
    failed.push(clone_row(&rows[18]));
    assert!(fold(&c, &failed).is_err());
}
#[test]
fn bare_model_intent_never_allows_failure_cleanup_replay_or_redispatch() {
    let c = context();
    let rows = fixtures(&c);
    let terminal = json!({"status":{"code":1}});
    let mut failed = copy(&rows[..9]);
    failed.push(cleanup_start(&rows, OwnerV8::State, 7, false, terminal));
    assert!(fold(&c, &failed).is_err());
    let mut replay = copy(&rows[..9]);
    replay.push(replay_wait(&rows, PhaseV8::Start, 6));
    assert!(fold(&c, &replay).is_err());
    let mut dispatch = copy(&rows[..9]);
    dispatch.push(clone_row(&rows[8]));
    assert!(fold(&c, &dispatch).is_err());
    let EntryV8::Owned(Body::OwnedWaitCreated { wait, .. }) = &rows[5].entry else {
        panic!()
    };
    let mut fake_failure = copy(&rows[..9]);
    fake_failure.push(owned(json!({"kind":"owned_wait_failed","turn":0,"attempt":0,"wait":wait,"reservation":null,"status":{"code":1},"consumed":0})));
    assert!(fold(&c, &fake_failure).is_err());
}

#[test]
fn refusal_retirement_rearm_and_retry_preserve_the_exact_original_carriers() {
    let c = context();
    let rows = fixtures(&c);
    let EntryV8::Owned(Body::OwnedWaitCreated {
        wait,
        copy_arguments,
        ..
    }) = &rows[5].entry
    else {
        panic!()
    };
    let EntryV8::Owned(Body::OwnedStateCommitted {
        state,
        argument_digest,
        ..
    }) = &rows[2].entry
    else {
        panic!()
    };
    let EntryV8::Owned(Body::OwnedWaitPrepared {
        observation_digest, ..
    }) = &rows[7].entry
    else {
        panic!()
    };
    let mut retry = copy(&rows[..11]);
    retry.push(ordinary(SourceJournalEntry::ProposalRefused {
        turn: 0,
        attempt: 0,
        reason: super::super::super::SourceProposalRefusal::MalformedDecode,
    }));
    let refusal_room = capacity::outstanding(&c, &fold(&c, &retry).unwrap()).unwrap();
    let settled_room = capacity::outstanding(&c, &fold(&c, &rows[..10]).unwrap()).unwrap();
    let usage_room = capacity::outstanding(&c, &fold(&c, &rows[..11]).unwrap()).unwrap();
    assert_phase_room_edge(&c, &rows[10], 10, settled_room, usage_room);
    assert_phase_room_edge(&c, &retry[11], 11, usage_room, refusal_room);
    retry.push(owned(json!({"kind":"owned_wait_retired","turn":0,"attempt":0,"wait":wait,"prepared":7,"state_digest":argument_digest,"observation_digest":observation_digest})));
    let retired = fold(&c, &retry).unwrap();
    assert_eq!(retired.tail, TailV8::TransferInDoubt);
    assert!(retired.state_basis.is_none());
    let retired_room = capacity::outstanding(&c, &retired).unwrap();
    assert_phase_room_edge(&c, &retry[12], 12, refusal_room, retired_room);

    let mut forbidden = copy(&retry);
    forbidden.push(replay_wait(&rows, PhaseV8::Start, 6));
    assert!(fold(&c, &forbidden).is_err());
    retry.push(owned(json!({"kind":"owned_state_rearmed","turn":0,"attempt":0,"wait":wait,"retired":12,"state":state,"state_digest":argument_digest,"observation":copy_arguments[0]["value"],"observation_digest":observation_digest})));
    assert_eq!(fold(&c, &retry).unwrap().tail, TailV8::RearmedState);
    let rearmed_room = capacity::outstanding(&c, &fold(&c, &retry).unwrap()).unwrap();
    assert_phase_room_edge(&c, &retry[13], 13, retired_room, rearmed_room);

    let mut wrong = copy(&retry);
    body_mut(&mut wrong, 13, |b| {
        b["state"]["fields"][1]["value"]["hex"] = json!("01")
    });
    assert!(fold(&c, &wrong).is_err());
    let Body::OwnedRunCreated {
        execution, binding, ..
    } = &c.created
    else {
        panic!()
    };
    let invocation = wire::recipe_digest(
        wire::RecipeV8::Invocation,
        &json!({"execution":execution,"owned_wait_binding":binding}),
    )
    .unwrap();
    let next_wait = wire::recipe_digest(
        wire::RecipeV8::Attempt,
        &json!({"invocation":invocation,"turn":0,"attempt":1,"binding":binding}),
    )
    .unwrap();
    let mut next = clone_row(&rows[5]);
    let EntryV8::Owned(Body::OwnedWaitCreated { attempt, wait, .. }) = &mut next.entry else {
        panic!()
    };
    *attempt = 1;
    *wait = next_wait;
    retry.push(next);
    let f = fold(&c, &retry).unwrap();
    let next_room = capacity::outstanding(&c, &f).unwrap();
    assert_phase_room_edge(&c, &retry[14], 14, rearmed_room, next_room);
    assert!(
        next_room.bytes_for_inert_test() > 131072,
        "next attempt checkpoint and settlement remain reserved"
    );
    assert_eq!(
        (f.tail, f.stages, f.reserved_total),
        (TailV8::WaitCreated, 1, 20)
    );
}
#[test]
fn source_sdk_failure_never_becomes_a_compiler_retry_and_future_stages_remain_refused() {
    let c = context();
    let rows = fixtures(&c);
    let mut failed = copy(&rows[..9]);
    failed.push(ordinary(SourceJournalEntry::AttemptFailed {
        turn: 0,
        attempt: 0,
        reason: super::super::super::SourceAttemptFailure::MalformedResponse,
        attempted_bytes: 1,
    }));
    failed.push(ordinary(SourceJournalEntry::AttemptUsage {
        turn: 0,
        attempt: 0,
        reported: None,
    }));
    failed.push(ordinary(SourceJournalEntry::ProposalRefused {
        turn: 0,
        attempt: 0,
        reason: super::super::super::SourceProposalRefusal::MalformedDecode,
    }));
    assert!(fold(&c, &failed).is_err());
    for role in [SourceStageRole::Initialize, SourceStageRole::Reduce] {
        let mut unsupported = copy(&rows[..3]);
        unsupported.push(ordinary(SourceJournalEntry::StageReservation {
            turn: 0,
            attempt: None,
            role,
            fuel: 10,
        }));
        assert!(fold(&c, &unsupported).is_err());
    }
}

#[test]
fn refused_decision_skip_requires_exact_checked_empty_case_disposal_proof() {
    let mut c = context();
    let mut rows = fixtures(&c);
    let Body::OwnedRunCreated { scope, .. } = &c.created else {
        panic!()
    };
    let decision = json!({"declaration":"decision","case":c.refused,"fields":[{"identity":"decision.code","value":{"kind":"i64","value":1}}]});
    let decision_digest = wire::recipe_digest(
        wire::RecipeV8::Decision,
        &json!({"scope":scope,"turn":0,"attempt":0,"authorize":c.authorize,"decision":decision}),
    )
    .unwrap();
    body_mut(&mut rows, 17, |b| {
        b["decision"] = decision;
        b["decision_digest"] = json!(decision_digest);
    });
    let mut refused = copy(&rows[..18]);
    refused.push(ordinary(SourceJournalEntry::AuthorizationRefused {
        turn: 0,
        attempt: 0,
        reason: super::super::super::SourceAuthorizationRefusal::GateDenied,
    }));
    refused.push(cleanup_start(
        &rows,
        OwnerV8::State,
        15,
        false,
        json!({"status":{"code":1}}),
    ));
    assert!(c.refused_cleanup_empty);
    assert_eq!(fold(&c, &refused).unwrap().tail, TailV8::CleanupInDoubt);
    // Removing the sealed compiler-empty fact cannot be replaced by the wire
    // case name: an outstanding Decision then requires observed settlement.
    c.refused_cleanup_empty = false;
    assert!(fold(&c, &refused).is_err());
}

#[test]
fn interrupted_original_start_keeps_origin_unknown_and_closes_only_newest_funder() {
    let c = context();
    let rows = fixtures(&c);
    let mut retry = copy(&rows[..7]);
    retry.push(replay_wait(&rows, PhaseV8::Start, 6));
    retry.push(replay_wait(&rows, PhaseV8::Start, 6));
    let mut prepared = clone_row(&rows[7]);
    body_mut(std::slice::from_mut(&mut prepared), 0, |b| {
        b["reservation"] = json!(8)
    });
    retry.push(prepared);
    let f = fold(&c, &retry).unwrap();
    let wait = f.wait.as_ref().unwrap();
    assert!(wait.original(PhaseV8::Start).unwrap().closure.is_none());
    assert!(wait.original(PhaseV8::Start).unwrap().consumed.is_none());
    assert_eq!(
        (
            wait.prepared,
            wait.first_funder(PhaseV8::Start).unwrap().seq
        ),
        (Some(9), 8)
    );
    assert_eq!((f.reserved_total, f.consumed_recorded), (40, 2));
    retry.push(replay_wait(&rows, PhaseV8::Start, 6));
    let EntryV8::Owned(Body::OwnedWaitCreated { wait, .. }) = &rows[5].entry else {
        panic!()
    };
    let EntryV8::Owned(Body::OwnedWaitPrepared {
        checkpoint_digest, ..
    }) = &rows[7].entry
    else {
        panic!()
    };
    retry.push(owned(json!({"kind":"owned_wait_replay_checked","turn":0,"attempt":0,"wait":wait,"reservation":10,"original":9,"result_digest":checkpoint_digest,"consumed":2})));
    let f = fold(&c, &retry).unwrap();
    assert_eq!((f.reserved_total, f.consumed_recorded), (50, 4));
    let mut wrong = copy(&retry);
    body_mut(&mut wrong, 11, |b| b["original"] = json!(6));
    assert!(fold(&c, &wrong).is_err());
    let mut old = copy(&retry[..9]);
    let mut stale = clone_row(&retry[9]);
    body_mut(std::slice::from_mut(&mut stale), 0, |b| {
        b["reservation"] = json!(7)
    });
    old.push(stale);
    assert!(fold(&c, &old).is_err());
}
#[test]
fn interrupted_resume_requires_its_own_fresh_start_reconstruction_before_retry() {
    let c = context();
    let rows = fixtures(&c);
    let mut retry = copy(&rows[..12]);
    let mut skipped = copy(&retry);
    skipped.push(replay_wait(&rows, PhaseV8::Resume, 11));
    assert!(fold(&c, &skipped).is_err());
    let EntryV8::Owned(Body::OwnedWaitCreated { wait, .. }) = &rows[5].entry else {
        panic!()
    };
    let EntryV8::Owned(Body::OwnedWaitPrepared {
        checkpoint_digest, ..
    }) = &rows[7].entry
    else {
        panic!()
    };
    retry.push(replay_wait(&rows, PhaseV8::Start, 6));
    let mut unclosed = copy(&retry);
    unclosed.push(replay_wait(&rows, PhaseV8::Resume, 11));
    assert!(fold(&c, &unclosed).is_err());
    retry.push(owned(json!({"kind":"owned_wait_replay_checked","turn":0,"attempt":0,"wait":wait,"reservation":12,"original":7,"result_digest":checkpoint_digest,"consumed":2})));
    // A reconstructed Start cannot close/credit the old unfunded Resume.
    let mut unfunded = copy(&retry);
    unfunded.push(clone_row(&rows[12]));
    assert!(fold(&c, &unfunded).is_err());
    retry.push(replay_wait(&rows, PhaseV8::Resume, 11));
    let mut completed = clone_row(&rows[12]);
    body_mut(std::slice::from_mut(&mut completed), 0, |b| {
        b["reservation"] = json!(14)
    });
    retry.push(completed);
    let f = fold(&c, &retry).unwrap();
    let w = f.wait.as_ref().unwrap();
    assert!(w.original(PhaseV8::Resume).unwrap().closure.is_none());
    assert_eq!(
        (w.completed, w.first_funder(PhaseV8::Resume).unwrap().seq),
        (Some(15), 14)
    );
    let mut stale = copy(&retry);
    stale.push(replay_wait(&rows, PhaseV8::Resume, 11));
    assert!(fold(&c, &stale).is_err());
    retry.push(replay_wait(&rows, PhaseV8::Start, 6));
    retry.push(owned(json!({"kind":"owned_wait_replay_checked","turn":0,"attempt":0,"wait":wait,"reservation":16,"original":7,"result_digest":checkpoint_digest,"consumed":2})));
    retry.push(replay_wait(&rows, PhaseV8::Resume, 11));
    retry.push(owned(json!({"kind":"owned_wait_replay_checked","turn":0,"attempt":0,"wait":wait,"reservation":18,"original":15,"result_digest":hash("result"),"consumed":3})));
    let f = fold(&c, &retry).unwrap();
    assert_eq!((f.reserved_total, f.consumed_recorded), (70, 12));
}
#[test]
fn wait_failure_entire_frozen_pair_is_sticky_across_reminted_cleanup() {
    let c = context();
    let rows = fixtures(&c);
    let EntryV8::Owned(Body::OwnedWaitCreated { wait, .. }) = &rows[5].entry else {
        panic!()
    };
    let status = json!({"failure":"fuel_exhausted","language_status":null});
    let mut failed = copy(&rows[..7]);
    failed.push(owned(json!({"kind":"owned_wait_failed","turn":0,"attempt":0,"wait":wait,"reservation":6,"status":status,"consumed":10})));
    let mut replaced = copy(&failed);
    replaced.push(cleanup_start(
        &rows,
        OwnerV8::State,
        2,
        false,
        json!({"failure":"host_abandoned","language_status":null}),
    ));
    assert!(fold(&c, &replaced).is_err());
    failed.push(cleanup_start(&rows, OwnerV8::State, 2, false, status));
    assert_eq!(fold(&c, &failed).unwrap().tail, TailV8::CleanupInDoubt);
}

#[test]
fn sealed_observation_cannot_be_substituted_with_reminted_same_shape_carriers() {
    let c = context();
    let rows = fixtures(&c);
    // Positive shape control: the new value is accepted by the actual checked
    // nominal/channel binding, yet does not belong to this recorded Observe.
    let changed = observation_facts(&c, 11);
    let mut substituted = copy(&rows[..6]);
    body_mut(&mut substituted, 5, |b| {
        b["copy_arguments"] = changed.copy_arguments().clone();
        b["copy_arguments_digest"] = json!(digest(
            b"semaprax.source-owned-frame-copy-args.v2\0",
            &wire::canonical(changed.copy_arguments())
        ));
    });
    substituted[5].observation = Some(changed.clone());
    assert!(fold(&c, &substituted).is_err());
    let mut prepared = copy(&rows[..8]);
    body_mut(&mut prepared, 7, |b| {
        b["observation_digest"] = json!(changed.request_digest())
    });
    prepared[7].observation = Some(changed);
    assert!(fold(&c, &prepared).is_err());
    let mut mismatch = copy(&rows[..8]);
    body_mut(&mut mismatch, 7, |b| {
        b["observation_digest"] = json!(hash("reminted request"))
    });
    assert!(fold(&c, &mismatch).is_err());
    let mut unsealed = copy(&rows[..6]);
    unsealed[5].observation = None;
    assert!(fold(&c, &unsealed).is_err());
    assert_eq!(fold(&c, &rows[..8]).unwrap().tail, TailV8::Prepared);
}
#[test]
fn sealed_observation_binds_exact_scope_and_preserves_observed_value_across_retry() {
    let c = context();
    let rows = fixtures(&c);
    let scope = crate::resumable_effects::source_checkpoint::SourceCheckpointScope::new(
        c.checked_binding.binding(),
        "different-invocation",
        0,
    )
    .unwrap();
    let observation = crate::interpreter::resumable::ResumableChannelValue::Record {
        declaration: DeclarationId::new("fixture.agent.type.observation"),
        fields: vec![
            crate::interpreter::ArgumentValue::Int(10),
            crate::interpreter::ArgumentValue::Int(0),
        ],
    };
    let other = crate::resumable_effects::owned_frame::v2::bind_owned_wait_observation_v8(
        &c.checked_binding,
        &scope,
        &observation,
    )
    .unwrap();
    let mut bad = copy(&rows[..6]);
    bad[5].observation = Some(other);
    assert!(fold(&c, &bad).is_err());
    let mut changed = copy(&rows[..6]);
    if let EntryV8::Ordinary(SourceJournalEntry::TurnObserved { observation, .. }) =
        &mut changed[4].entry
    {
        *observation = hash("different observed turn");
    } else {
        panic!()
    };
    assert!(fold(&c, &changed).is_err());
}

#[test]
fn inert_candidate_requires_pending_before_bytes_and_exact_predecessor_ack() {
    let c = context();
    let rows = fixtures(&c);
    let key = SourceCheckpointKey::new([73; 32]);
    let mut inventory = candidate::InventoryV8::synthetic_fresh(&c, &key).unwrap();
    let mut document = Vec::new();
    for (seq, row) in rows.into_iter().enumerate() {
        let pending = inventory
            .synthetic_prepare(row)
            .unwrap_or_else(|e| panic!("candidate {seq}: {:?}", e.error))
            .into_pending();
        document.extend_from_slice(pending.bytes());
        let ack = pending.synthetic_ack_for_inert_test();
        inventory = pending
            .acknowledge(ack)
            .unwrap_or_else(|e| panic!("ack: {:?}", e.error));
        assert_eq!(inventory.sequence(), seq + 1);
        assert_eq!(inventory.acknowledged_bytes(), document.len());
    }
    let recovered =
        candidate::InventoryV8::synthetic_recover(&c, &key, &document, fixtures(&c)).unwrap();
    assert_eq!(recovered.sequence(), inventory.sequence());
    assert_eq!(
        recovered.acknowledged_bytes(),
        inventory.acknowledged_bytes()
    );
    assert!(candidate::InventoryV8::synthetic_recover(
        &c,
        &SourceCheckpointKey::new([74; 32]),
        &document,
        fixtures(&c)
    )
    .is_err());
    let first = clone_row(&fixtures(&c)[0]);
    let pending = candidate::InventoryV8::synthetic_fresh(&c, &key)
        .unwrap()
        .synthetic_prepare(first)
        .unwrap_or_else(|e| panic!("{:?}", e.error))
        .into_pending();
    let mut ack = pending.synthetic_ack_for_inert_test();
    ack.alter_predecessor_for_inert_test();
    assert!(matches!(
        pending.acknowledge(ack),
        Err(candidate::AckRejectionV8 {
            error: SourceJournalError::Binding,
            ..
        })
    ));
}

#[test]
fn inert_candidate_rejects_early_stop_and_preserves_prior_inventory_without_io() {
    let c = context();
    let key = SourceCheckpointKey::new([73; 32]);
    let mut inventory = candidate::InventoryV8::synthetic_fresh(&c, &key).unwrap();
    for row in fixtures(&c).into_iter().take(3) {
        let pending = inventory
            .synthetic_prepare(row)
            .unwrap_or_else(|e| panic!("{:?}", e.error))
            .into_pending();
        let ack = pending.synthetic_ack_for_inert_test();
        inventory = pending
            .acknowledge(ack)
            .unwrap_or_else(|e| panic!("{:?}", e.error));
    }
    let before = inventory.acknowledged_bytes();
    let rejected = inventory
        .synthetic_prepare(ordinary(SourceJournalEntry::Stop {
            turn: Some(0),
            attempt: None,
            status: super::super::super::SourceStopStatus::Cancelled,
            reason: super::super::super::SourceStopReason::Cancelled,
        }))
        .err()
        .expect("producer cannot emit early Stop");
    assert_eq!(rejected.error, SourceJournalError::Order);
    assert_eq!(rejected.inventory.sequence(), 3);
    assert_eq!(rejected.inventory.acknowledged_bytes(), before);
    assert!(matches!(
        rejected.row,
        EntryV8::Ordinary(SourceJournalEntry::Stop { .. })
    ));
}

#[test]
fn closure_room_removes_checkpoint_raw_response_usage_and_current_replay_only() {
    let c = context();
    let rows = fixtures(&c);
    let room = |count| capacity::outstanding(&c, &fold(&c, &rows[..count]).unwrap()).unwrap();
    // Compute boundary room privately; exact and +1 include the full JSONL row.
    let room_before = room(9);
    let room_after_raw = room(10);
    let exact = super::super::super::MAX_SOURCE_DOCUMENT_BYTES - room_before.bytes_for_inert_test();
    assert!(room_before.check(exact, 9).is_ok());
    assert_eq!(
        room_before.check(exact + 1, 9),
        Err(SourceJournalError::Capacity)
    );
    let exact_rows = super::super::super::MAX_SOURCE_ENTRIES - room_before.rows_for_inert_test();
    assert!(room_before.check(0, exact_rows).is_ok());
    assert_eq!(
        room_before.check(0, exact_rows + 1),
        Err(SourceJournalError::Capacity)
    );
    assert!(room_after_raw.bytes_for_inert_test() < room_before.bytes_for_inert_test());
    assert!(room(11).bytes_for_inert_test() < room_after_raw.bytes_for_inert_test());
    assert!(room(8).bytes_for_inert_test() + 131072 <= room(7).bytes_for_inert_test());
    let mut replayed = copy(&rows[..8]);
    replayed.push(replay_wait(&replayed, PhaseV8::Start, 6));
    let pending = capacity::outstanding(&c, &fold(&c, &replayed).unwrap()).unwrap();
    replayed.push(owned(json!({"kind":"owned_wait_replay_checked","turn":0,"attempt":0,"wait":match &rows[5].entry {EntryV8::Owned(Body::OwnedWaitCreated{wait,..})=>wait,_=>panic!()},"reservation":8,"original":7,"result_digest":match &rows[7].entry {EntryV8::Owned(Body::OwnedWaitPrepared{checkpoint_digest,..})=>checkpoint_digest,_=>panic!()},"consumed":1})));
    let closed = capacity::outstanding(&c, &fold(&c, &replayed).unwrap()).unwrap();
    assert!(pending.bytes_for_inert_test() > closed.bytes_for_inert_test());
    assert_eq!(closed, room(8));
}

#[test]
fn near_capacity_authenticated_replays_leave_room_for_the_already_reserved_closure() {
    // Only this inert fixture has a larger synthetic total-work limit. Its
    // actual checked B remains unchanged; no production profile is patched.
    let mut c = context();
    match &mut c.ordinary.profile {
        super::super::super::SourceProfile::ExecutionV2 {
            max_total_steps, ..
        } => {
            *max_total_steps = 1_000_000_000;
        }
        _ => panic!("inert execution fixture"),
    }
    let mut rows = fixtures(&c).into_iter().take(8).collect::<Vec<_>>();
    let key = SourceCheckpointKey::new([73; 32]);
    let Body::OwnedRunCreated {
        execution, binding, ..
    } = &c.created
    else {
        panic!()
    };
    let invocation = wire::recipe_digest(
        wire::RecipeV8::Invocation,
        &json!({"execution":execution,"owned_wait_binding":binding}),
    )
    .unwrap();
    let generation = wire::generation_digest_from_created(&c.created).unwrap();
    let mut document = Vec::new();
    let mut mac = "0".repeat(64);
    let encode = |row: &ValidatedEntryV8, seq: usize, mac: &str| {
        wire::encode(
            &row.entry,
            &ExpectedRowV8 {
                invocation: &invocation,
                generation: &generation,
                seq: u32::try_from(seq).unwrap(),
                prev_mac: mac,
                ordinary: &c.ordinary,
            },
            &key,
        )
        .unwrap()
    };
    let append = |bytes: Vec<u8>, document: &mut Vec<u8>, mac: &mut String| {
        *mac = wire::parse(&bytes[..bytes.len() - 1]).unwrap()["authentication"]
            .as_str()
            .unwrap()
            .to_owned();
        document.extend_from_slice(&bytes);
    };
    for (seq, row) in rows.iter().enumerate() {
        append(encode(row, seq, &mac), &mut document, &mut mac);
    }
    let closed_room = capacity::outstanding(&c, &fold(&c, &rows).unwrap()).unwrap();
    let wait = match &rows[5].entry {
        EntryV8::Owned(Body::OwnedWaitCreated { wait, .. }) => wait.clone(),
        _ => panic!(),
    };
    let result = match &rows[7].entry {
        EntryV8::Owned(Body::OwnedWaitPrepared {
            checkpoint_digest, ..
        }) => checkpoint_digest.clone(),
        _ => panic!(),
    };
    let mut cycles = 0;
    loop {
        let reservation = replay_wait(&rows, PhaseV8::Start, 6);
        let checked = owned(
            json!({"kind":"owned_wait_replay_checked","turn":0,"attempt":0,"wait":wait,"reservation":rows.len(),"original":7,"result_digest":result,"consumed":1}),
        );
        let reserved_bytes = encode(&reservation, rows.len(), &mac);
        let next_mac = wire::parse(&reserved_bytes[..reserved_bytes.len() - 1]).unwrap()
            ["authentication"]
            .as_str()
            .unwrap()
            .to_owned();
        let checked_bytes = encode(&checked, rows.len() + 1, &next_mac);
        let future_len = document.len() + reserved_bytes.len() + checked_bytes.len();
        if closed_room.check(future_len, rows.len() + 2).is_err() {
            break;
        }
        append(reserved_bytes, &mut document, &mut mac);
        append(checked_bytes, &mut document, &mut mac);
        rows.push(reservation);
        rows.push(checked);
        cycles += 1;
    }
    assert!(cycles > 1000, "nontrivial repeated-crash inventory");
    assert!(
        document.len() > 15 * 1024 * 1024,
        "near whole-document byte cap"
    );
    let folded = fold(&c, &rows).unwrap();
    assert_eq!(folded.reserved_total, 20 + cycles * 10);
    assert_eq!(folded.consumed_recorded, 2 + cycles);
    let offset = rows.len() - 8;
    // Recover authenticated bytes once, then exercise consuming Candidate/ACK.
    // The cfg(test) carrier path is explicitly not physical recovery evidence.
    let mut inventory =
        candidate::InventoryV8::synthetic_recover(&c, &key, &document, rows).unwrap();
    let mut future = fixtures(&c).into_iter().skip(8).collect::<Vec<_>>();
    let response = vec![255; 4096];
    future[1] = ordinary(SourceJournalEntry::AttemptSettled {
        turn: 0,
        attempt: 0,
        response_digest: super::super::super::source_response_digest(&response),
        response,
    });
    for row in &mut future {
        if let EntryV8::Owned(body) = &row.entry {
            let mut value = serde_json::to_value(body).unwrap();
            for key in ["reservation", "stage_reservation", "transfer", "staged"] {
                if let Some(original) = value[key].as_u64() {
                    value[key] = json!(original + offset as u64);
                }
            }
            row.entry = EntryV8::Owned(serde_json::from_value(value).unwrap());
        }
    }
    for row in future {
        let pending = inventory
            .synthetic_prepare(row)
            .unwrap_or_else(|e| panic!("reserved closure: {:?}", e.error))
            .into_pending();
        document.extend_from_slice(pending.bytes());
        let ack = pending.synthetic_ack_for_inert_test();
        inventory = pending
            .acknowledge(ack)
            .unwrap_or_else(|e| panic!("ACK: {:?}", e.error));
    }
    assert_eq!(inventory.sequence(), 20 + offset);
    assert_eq!(inventory.acknowledged_bytes(), document.len());
    assert!(document.len() <= super::super::super::MAX_SOURCE_DOCUMENT_BYTES);
}

fn assert_phase_room_edge(
    c: &FoldContextV8,
    row: &ValidatedEntryV8,
    seq: usize,
    before: capacity::RoomV8,
    after: capacity::RoomV8,
) {
    let id = hash("inert room identity");
    let zero = "0".repeat(64);
    let bytes = wire::encode(
        &row.entry,
        &ExpectedRowV8 {
            invocation: &id,
            generation: &id,
            seq: u32::try_from(seq).unwrap(),
            prev_mac: &zero,
            ordinary: &c.ordinary,
        },
        &SourceCheckpointKey::new([73; 32]),
    )
    .unwrap();
    let exact = super::super::super::MAX_SOURCE_DOCUMENT_BYTES - before.bytes_for_inert_test();
    assert!(before.check(exact, seq).is_ok());
    assert!(
        after.check(exact + bytes.len(), seq + 1).is_ok(),
        "legal phase edge must retain its full remaining closure room"
    );
    assert!(before.bytes_for_inert_test() >= after.bytes_for_inert_test() + bytes.len());
}
#[test]
fn authorization_stage_room_is_reserved_before_the_ordinary_charge() {
    let c = context();
    let rows = fixtures(&c);
    let pending = fold(&c, &rows[..16]).unwrap();
    let charged = fold(&c, &rows[..17]).unwrap();
    assert_eq!(pending.tail, TailV8::PendingAuthorize);
    assert_eq!(charged.tail, TailV8::ChargedAuthorizeReplay);
    assert_phase_room_edge(
        &c,
        &rows[16],
        16,
        capacity::outstanding(&c, &pending).unwrap(),
        capacity::outstanding(&c, &charged).unwrap(),
    );
}

#[test]
fn every_remaining_attempt_reserves_the_next_malformed_refusal_branch() {
    let c = context();
    let original = fixtures(&c);
    let mut rows = copy(&original[..11]);
    let Body::OwnedRunCreated {
        execution, binding, ..
    } = &c.created
    else {
        panic!()
    };
    let invocation = wire::recipe_digest(
        wire::RecipeV8::Invocation,
        &json!({"execution":execution,"owned_wait_binding":binding}),
    )
    .unwrap();
    let EntryV8::Owned(Body::OwnedStateCommitted {
        state,
        argument_digest,
        ..
    }) = &original[2].entry
    else {
        panic!()
    };
    let EntryV8::Owned(Body::OwnedWaitCreated { copy_arguments, .. }) = &original[5].entry else {
        panic!()
    };
    let EntryV8::Owned(Body::OwnedWaitPrepared {
        observation_digest, ..
    }) = &original[7].entry
    else {
        panic!()
    };
    let mut prepared_seq = 7;
    for attempt in 0..c.ordinary.max_attempts() {
        let wait = wire::recipe_digest(
            wire::RecipeV8::Attempt,
            &json!({"invocation":invocation,"turn":0,"attempt":attempt,"binding":binding}),
        )
        .unwrap();
        let refusal = ordinary(SourceJournalEntry::ProposalRefused {
            turn: 0,
            attempt,
            reason: super::super::super::SourceProposalRefusal::MalformedDecode,
        });
        let before = capacity::outstanding(&c, &fold(&c, &rows).unwrap()).unwrap();
        let seq = rows.len();
        rows.push(refusal);
        let after = capacity::outstanding(&c, &fold(&c, &rows).unwrap()).unwrap();
        assert_phase_room_edge(&c, &rows[seq], seq, before, after);
        if attempt + 1 == c.ordinary.max_attempts() {
            break;
        }
        let retired_seq = rows.len();
        rows.push(owned(json!({"kind":"owned_wait_retired","turn":0,"attempt":attempt,"wait":wait,"prepared":prepared_seq,"state_digest":argument_digest,"observation_digest":observation_digest})));
        rows.push(owned(json!({"kind":"owned_state_rearmed","turn":0,"attempt":attempt,"wait":wait,"retired":retired_seq,"state":state,"state_digest":argument_digest,"observation":copy_arguments[0]["value"],"observation_digest":observation_digest})));
        let next_attempt = attempt + 1;
        let next_wait = wire::recipe_digest(
            wire::RecipeV8::Attempt,
            &json!({"invocation":invocation,"turn":0,"attempt":next_attempt,"binding":binding}),
        )
        .unwrap();
        let start_seq = rows.len() + 1;
        for index in 5..=7 {
            let mut row = clone_row(&original[index]);
            let EntryV8::Owned(body) = &row.entry else {
                panic!()
            };
            let mut value = serde_json::to_value(body).unwrap();
            value["attempt"] = json!(next_attempt);
            value["wait"] = json!(next_wait);
            if index == 7 {
                value["reservation"] = json!(start_seq);
                prepared_seq = rows.len();
            }
            row.entry = EntryV8::Owned(serde_json::from_value(value).unwrap());
            let before = capacity::outstanding(&c, &fold(&c, &rows).unwrap()).unwrap();
            let seq = rows.len();
            rows.push(row);
            let after = capacity::outstanding(&c, &fold(&c, &rows).unwrap()).unwrap();
            assert_phase_room_edge(&c, &rows[seq], seq, before, after);
        }
        for index in 8..=10 {
            let mut row = clone_row(&original[index]);
            match &mut row.entry {
                EntryV8::Ordinary(SourceJournalEntry::AttemptIntent {
                    attempt,
                    attempt_digest,
                    request_digest,
                    prompt_digest,
                    request_bytes,
                    ..
                }) => {
                    *attempt = next_attempt;
                    *attempt_digest = c.ordinary.attempt_digest(
                        0,
                        next_attempt,
                        request_digest,
                        prompt_digest,
                        *request_bytes,
                    );
                }
                EntryV8::Ordinary(
                    SourceJournalEntry::AttemptSettled { attempt, .. }
                    | SourceJournalEntry::AttemptUsage { attempt, .. },
                ) => *attempt = next_attempt,
                _ => panic!(),
            }
            let before = capacity::outstanding(&c, &fold(&c, &rows).unwrap()).unwrap();
            let seq = rows.len();
            rows.push(row);
            let after = capacity::outstanding(&c, &fold(&c, &rows).unwrap()).unwrap();
            assert_phase_room_edge(&c, &rows[seq], seq, before, after);
        }
    }
    assert_eq!(fold(&c, &rows).unwrap().tail, TailV8::ProposalRefused);
}
