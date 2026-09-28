use super::*;
#[test]
fn owned_reduce_capacity_reserves_actual_stage_and_all_checked_case_closures() {
    super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
        |context, _lease, _key| {
            let context = context.fold();
            let rooms = rooms(context).unwrap();
            let plan = v2::compile_owned_reduce_v2(&context.checked_binding).unwrap();
            assert_eq!(rooms.cases.len(), plan.transfers().cases.len());
            assert_eq!(rooms.cases.len(), 2);
            let stage = ordinary(SourceJournalEntry::StageReservation {
                turn: 0,
                attempt: Some(u32::MAX),
                role: super::super::super::super::SourceStageRole::Reduce,
                fuel: context.ordinary.max_steps_per_stage().unwrap(),
            })
            .unwrap();
            let limit = super::super::super::super::MAX_SOURCE_DOCUMENT_BYTES;
            let used = limit - rooms.before_stage.bytes;
            rooms.before_stage.check(used, 0).unwrap();
            assert_eq!(
                rooms.before_stage.check(used + 1, 0),
                Err(SourceJournalError::Capacity)
            );
            // Full original stage ACK consumes serialized room without borrowing from
            // any subsequent Step/failure settlement. Its allowance is still spent F.
            rooms.charged.check(used + stage.bytes, 1).unwrap();
            assert_eq!(rooms.before_stage.rows, rooms.charged.rows + 1);
            for c in &rooms.cases {
                assert!(c.staged.rows >= c.transfer.rows);
                assert!(c.transfer.bytes > c.completed.bytes);
                assert!(c.transfer.rows > c.completed.rows);
                assert!(rooms.charged.bytes > c.staged.bytes);
                assert!(rooms.charged.rows > c.staged.rows);
                let used = limit - c.transfer.bytes;
                c.transfer.check(used, 0).unwrap();
                assert_eq!(
                    c.transfer.check(used + 1, 0),
                    Err(SourceJournalError::Capacity)
                );
                c.completed.check(limit - c.completed.bytes, 0).unwrap();
                assert_eq!(
                    c.completed.check(limit - c.completed.bytes + 1, 0),
                    Err(SourceJournalError::Capacity)
                );
            }
            assert!(rooms.failed_state.rows >= 4);
            let before = rooms.after_effect();
            assert!(
                before.bytes >= rooms.before_stage.bytes
                    && before.bytes >= rooms.failed_state.bytes
            );
            assert!(
                before.rows >= rooms.before_stage.rows && before.rows >= rooms.failed_state.rows
            );
            rooms
                .failed_state
                .check(limit - rooms.failed_state.bytes, 0)
                .unwrap();
            assert_eq!(
                rooms
                    .failed_state
                    .check(limit - rooms.failed_state.bytes + 1, 0),
                Err(SourceJournalError::Capacity)
            );
            before.check(limit - before.bytes, 0).unwrap();
            assert_eq!(
                before.check(limit - before.bytes + 1, 0),
                Err(SourceJournalError::Capacity)
            );
        },
    );
}

#[test]
fn failed_state_started_ack_preserves_exact_reserved_closure_edge() {
    super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
        |context, _lease, _key| {
            let context = context.fold();
            let plan = v2::compile_owned_reduce_v2(&context.checked_binding).unwrap();
            let operations = v2::owned_wait_operations_v8(
                &context.checked_binding.helper().liveness().result_disposal,
            )
            .unwrap();
            let started = row(json!({
                "kind":"owned_effect_failure_state_cleanup_started","turn":0,
                "attempt":u32::MAX,"plan":plan.binding(),"settlement":u32::MAX,
                "recorded":u32::MAX,"decision_cleanup_settled":u32::MAX,
                "effect_failure":"handler_failed","state_digest":hash(),
                "operations":operations
            }))
            .unwrap();
            // Independently render the frozen receipt shape: it has no Decision
            // digest. The acknowledged Started row must consume only its own room.
            let settled = row(json!({
                "kind":"owned_effect_failure_state_cleanup_settled","turn":0,
                "attempt":u32::MAX,"started":u32::MAX,
                "receipt":templates::receipt(&operations).unwrap()
            }))
            .unwrap();
            assert_eq!(failed_state_receipt(&operations, 0).unwrap(), settled);
            let remaining = failed_state_receipt(&operations, 0)
                .unwrap()
                .add(terminal())
                .unwrap();
            let before = rooms(context).unwrap().failed_state;
            assert_eq!(before, started.add(remaining).unwrap());
            let byte_limit = super::super::super::super::MAX_SOURCE_DOCUMENT_BYTES;
            let row_limit = super::super::super::super::MAX_SOURCE_ENTRIES;
            let used_bytes = byte_limit - before.bytes;
            let used_rows = row_limit - before.rows;
            before.check(used_bytes, used_rows).unwrap();
            remaining
                .check(used_bytes + started.bytes, used_rows + started.rows)
                .unwrap();
            assert_eq!(
                remaining.check(used_bytes + started.bytes + 1, used_rows + started.rows),
                Err(SourceJournalError::Capacity)
            );
            assert_eq!(
                remaining.check(used_bytes + started.bytes, used_rows + started.rows + 1),
                Err(SourceJournalError::Capacity)
            );
        },
    );
}

// Original capacity enumeration: one status per compiler site, including repeats.
fn original_status_multiset(plan: &v2::CheckedOwnedReduceV2) -> Vec<Value> {
    let mut statuses = [
        "fuel_exhausted",
        "host_abandoned",
        "answer_type_mismatch",
        "evaluation_rejected",
        "handler_failed",
        "call_depth_exceeded",
    ]
    .into_iter()
    .map(|failure| json!({"failure":failure,"language_status":null}))
    .collect::<Vec<_>>();
    for source in &plan.function().cleanup_plan.status_sources {
        use crate::cleanup_plan::StatusProducer;
        let values = match &source.producer {
            StatusProducer::ContractFalse { phase, .. } => {
                vec![crate::conformance::NormalizedStatus::contract(*phase)]
            }
            StatusProducer::CheckedArithmetic {
                normalized_cases, ..
            } => normalized_cases
                .iter()
                .map(|n| crate::conformance::NormalizedStatus::arithmetic(*n))
                .collect(),
            StatusProducer::PropagatedCall { .. } => Vec::new(),
        };
        for status in values {
            statuses.push(json!({"failure":"language_failure",
                "language_status":wire::parse(status.to_json().as_bytes()).unwrap()}));
        }
    }
    statuses
}

#[test]
fn owned_reduce_capacity_status_dedup_preserves_every_original_failure_room() {
    super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_arithmetic_store(
        |context, _lease, _key, _directory| {
            let context = context.fold();
            let plan = v2::compile_owned_reduce_v2(&context.checked_binding).unwrap();
            let original = original_status_multiset(&plan);
            let unique = failure_statuses(&plan).unwrap();
            assert!(
                original.len() > unique.len(),
                "actual compiler repeats a status"
            );
            let mut first_occurrences = Vec::new();
            for status in &original {
                if !first_occurrences.contains(status) {
                    first_occurrences.push(status.clone());
                }
            }
            assert_eq!(unique, first_occurrences);
            let fuel = context.ordinary.max_steps_per_stage().unwrap();
            let mut bases = vec![(
                json!({"kind":"initial_failure","status":null}),
                v2::owned_wait_operations_v8(&plan.transfers().initial_disposal).unwrap(),
            )];
            for case in &plan.transfers().cases {
                for prefix in 0..=case.fields.len() {
                    let actions = &case.failure_by_prefix[prefix];
                    bases.push((json!({"kind":"partial_failure","status":null,
                        "constructor":case.constructor.as_str(),"case":case.case.as_str(),
                        "transfer_prefix":case.fields[..prefix].iter().map(|f|f.at.as_str()).collect::<Vec<_>>(),
                        "active_flags":actions.iter().map(|a|a.guard_flag.0).collect::<Vec<_>>()}),
                        v2::owned_wait_operations_v8(actions).unwrap()));
                }
                let mut flags = case
                    .completion_live_flags
                    .iter()
                    .map(|f| f.0)
                    .collect::<Vec<_>>();
                flags.extend(
                    plan.transfers()
                        .result_disposal
                        .iter()
                        .filter(|a| a.active_case.as_ref().is_some_and(|c| c.case == case.case))
                        .map(|a| a.guard_flag.0),
                );
                bases.push((json!({"kind":"provisional_failure","status":null,
                    "constructor":case.constructor.as_str(),"case":case.case.as_str(),"active_flags":flags}),
                    v2::owned_wait_operations_v8(&plan.transfers().provisional_failure).unwrap()));
            }
            // Exhaust every failure component used by rooms(); success components
            // do not consume failure_statuses and remain unchanged.
            let limit = super::super::super::super::MAX_SOURCE_DOCUMENT_BYTES;
            for (basis, operations) in bases {
                let before = failure(&plan, basis.clone(), &operations, fuel, &original).unwrap();
                let after = failure(&plan, basis, &operations, fuel, &unique).unwrap();
                assert_eq!(before, after);
                assert_eq!(after.either(after), after);
                after.check(limit - after.bytes, 0).unwrap();
                assert_eq!(
                    after.check(limit - after.bytes + 1, 0),
                    Err(SourceJournalError::Capacity)
                );
            }
        },
    );
}

#[test]
fn owned_reduce_cached_proof_matches_fresh_plan_and_every_room_on_genuine_context() {
    super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_arithmetic_store(
        |context, _lease, _key, _directory| {
            let context = context.fold();
            let first = context.checked_reduce().unwrap();
            assert!(std::sync::Arc::ptr_eq(
                first,
                context.checked_reduce().unwrap()
            ));
            let mut crossed = super::super::super::fold::tests::context();
            crossed.checked_reduce = Ok(std::sync::Arc::clone(first));
            assert_eq!(
                crossed.checked_reduce().err().unwrap(),
                SourceJournalError::Binding,
                "a retained proof cannot be rebound to another checked source/helper"
            );
            let fresh = v2::compile_owned_reduce_v2(&context.checked_binding).unwrap();
            assert_eq!(first.binding(), fresh.binding());
            assert_eq!(first.function().id, fresh.function().id);
            assert_eq!(
                first.transfers().initial_disposal,
                fresh.transfers().initial_disposal
            );
            assert_eq!(
                first.transfers().completion_cleanup,
                fresh.transfers().completion_cleanup
            );
            assert_eq!(
                first.transfers().provisional_failure,
                fresh.transfers().provisional_failure
            );
            assert_eq!(first.mappings().len(), fresh.mappings().len());
            for (cached, fresh) in first.mappings().iter().zip(fresh.mappings()) {
                assert_eq!(
                    (&cached.case, cached.role, &cached.target, &cached.fields),
                    (&fresh.case, fresh.role, &fresh.target, &fresh.fields)
                );
            }
            assert_eq!(
                rooms(context).unwrap(),
                rooms_with_plan(context, &fresh).unwrap()
            );
            super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
                |other, _lease, _key| {
                    let other = other.fold();
                    assert!(
                        !std::sync::Arc::ptr_eq(first, other.checked_reduce().unwrap()),
                        "separate contexts never share a mutable/global proof cache"
                    );
                    assert_ne!(
                        first.binding(),
                        other.checked_reduce().unwrap().binding(),
                        "different genuine source fixtures keep their distinct bindings"
                    );
                },
            );
        },
    );
}

#[test]
fn owned_reduce_cached_proof_preserves_deferred_refusal_for_real_unsupported_reducer() {
    let original = include_str!("../../../../../../examples/offline-repair-project/src/app.spx");
    let source = original.replace(
        "    runtime_v1 {",
        "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {",
    );
    let source = format!("{source}\n@id(\"fixture.agent.fn.park\")\nfn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {{\n    let proposal = yield observation;\n    state\n}}\n");
    let source = source.replace(
        "    if state.epoch < 2",
        "    let extra = bytes_zeroed(1usize);\n    if state.epoch < 2",
    );
    assert!(source.contains("let extra = bytes_zeroed"));
    let binding = v2::compile_owned_agent_wait_v8(
        &source,
        std::path::Path::new("deferred-reduce.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    assert_eq!(
        v2::compile_owned_reduce_v2(&binding).err().unwrap().code,
        "SPX-T303"
    );
    let context = super::super::super::fold::tests::context_with_binding(binding);
    assert_eq!(
        context.checked_reduce().err().unwrap(),
        SourceJournalError::Binding
    );
    let rows = super::super::super::fold::tests::fixtures(&context);
    assert!(
        super::super::super::fold::fold(&context, &rows[..2]).is_ok(),
        "unsupported reducer proof does not reject earlier default journal admission"
    );
    for _ in 0..2 {
        assert_eq!(rooms(&context).err().unwrap(), SourceJournalError::Binding);
        assert!(context.reduce_templates.entry.borrow().is_none());
    }
}

#[test]
fn owned_reduce_template_cache_matches_every_room_and_refuses_crossed_proof() {
    super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_reduce_arithmetic_store(
        |context, _lease, _key, _directory| {
            let context = context.fold();
            let plan = context.checked_reduce().unwrap();
            let expected = rooms_with_plan(context, plan).unwrap();
            for _ in 0..3 {
                assert_eq!(rooms(context).unwrap(), expected);
                let cache = context.reduce_templates.entry.borrow();
                let (_, actual_plan, retained) = cache.as_ref().unwrap();
                assert!(std::sync::Arc::ptr_eq(plan, actual_plan));
                assert_eq!(retained.as_ref().unwrap(), &expected);
            }
            let mut crossed = super::super::super::fold::tests::context();
            crossed.checked_reduce = Ok(std::sync::Arc::clone(plan));
            assert_eq!(rooms(&crossed).err(), Some(SourceJournalError::Binding));
            assert!(crossed.reduce_templates.entry.borrow().is_none());
        },
    );
}

#[test]
fn owned_reduce_template_cache_recomputes_changed_inputs_and_reset() {
    let mut context = super::super::super::fold::tests::context();
    let initial = rooms(&context).unwrap();
    let old_key = context
        .reduce_templates
        .entry
        .borrow()
        .as_ref()
        .unwrap()
        .0
        .clone();
    // Inert Context mutation exercises private key invalidation. It grants no
    // live profile, journal authority, or physical capacity check.
    let model::OwnedBodyV8::OwnedRunCreated { scope, .. } = &mut context.created else {
        panic!()
    };
    *scope = json!({"inert_larger_scope":"a different bounded scope"});
    let fresh = rooms_with_plan(&context, context.checked_reduce().unwrap()).unwrap();
    assert_eq!(rooms(&context).unwrap(), fresh);
    assert_ne!(
        context.reduce_templates.entry.borrow().as_ref().unwrap().0,
        old_key
    );
    assert_eq!(rooms(&context).unwrap(), fresh);
    assert_eq!(initial.cases.len(), fresh.cases.len());
    context.reduce_templates.reset();
    assert!(context.reduce_templates.entry.borrow().is_none());
    assert_eq!(rooms(&context).unwrap(), fresh);
}

#[test]
fn owned_reduce_template_cache_cumulative_profile_resets_and_keys_actual_maximum_turn() {
    super::super::super::CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(
        |context, lease, _key| {
            // These checked Context/template assertions perform no source Observe,
            // target exchange, physical continuation, or authenticated second turn.
            let default = rooms(context.fold()).unwrap();
            assert!(context.fold().reduce_templates.entry.borrow().is_some());
            let context = context.with_cumulative_initialization(&lease).unwrap();
            assert!(context.fold().reduce_templates.entry.borrow().is_none());
            let fold = context.fold();
            let maximum = context.ordinary().max_iterations() - 1;
            assert_eq!(maximum_turn(fold), maximum);
            let fresh = rooms_with_plan(fold, fold.checked_reduce().unwrap()).unwrap();
            for _ in 0..2 {
                assert_eq!(rooms(fold).unwrap(), fresh);
            }
            let key = fold
                .reduce_templates
                .entry
                .borrow()
                .as_ref()
                .unwrap()
                .0
                .clone();
            let key: Value = serde_json::from_str(&key).unwrap();
            assert_eq!(key["cumulative"], json!(true));
            assert_eq!(key["coordinate_turn"], json!(maximum));
            assert_eq!(default.cases.len(), fresh.cases.len());
        },
    );
}
