use super::*;
pub(in crate::live_invocation::source_journal::owned_wait_v8) fn plan() -> v2::CheckedOwnedReduceV2
{
    let source = include_str!("../../../../../examples/offline-repair-project/src/app.spx")
        .replace(
            "    runtime_v1 {",
            "    model_wait_v1 { propose = \"fixture.agent.fn.park\"; }\n    runtime_v1 {",
        );
    let source = format!(
        "{source}\n{}",
        r#"
@id("fixture.agent.fn.park")
fn park(state: own State, observation: Observation) -> State yields Observation -> Proposal {
    let proposal = yield observation;
    state
}
"#
    );
    let binding = v2::compile_owned_agent_wait_v8(
        &source,
        std::path::Path::new("reduce-inventory.spx"),
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    v2::compile_owned_reduce_v2(&binding).unwrap()
}
#[test]
fn owned_reduce_inventory_maps_actual_nominal_cases_without_target_substitution() {
    let p = plan();
    for mapping in p.mappings() {
        let fields = p
            .helper()
            .program()
            .declarations
            .case_fields(&mapping.case)
            .unwrap();
        let step = json!({"declaration":p.function().return_type.nominal_id().unwrap().as_str(),
            "case":mapping.case.as_str(),"fields":fields.iter().enumerate().map(|(i,f)|
                json!({"identity":f.id.as_str(),"value":if f.ty==crate::hir::ResolvedType::Bytes {
                    json!({"kind":"bytes","hex":"00ff"})
                } else {json!({"tag":"i64","value":i as i64 - 3})}})).collect::<Vec<_>>()});
        let scope =
            json!({"program_root":"source","invocation":"invocation","policy_epoch":"epoch"});
        let digest = recipe_digest(
            ReduceRecipeV8::Step,
            &json!({"scope":scope,"binding":p.binding(),
            "plan":p.binding(),"turn":0,"attempt":0,"stage_reservation":29,"step":step}),
        )
        .unwrap();
        let checked = checked_step(&p, &scope, 0, 0, 29, &step, &digest).unwrap();
        checked.matches_target(checked.target()).unwrap();
        assert_eq!(checked.step(), &step);
        assert_eq!(checked.case(), mapping.case.as_str());
        let mut hostile = checked.target().clone();
        if mapping.role == "Fail" {
            hostile["code"] = 4.into();
        } else {
            let key = if mapping.role == "Complete" {
                "report"
            } else {
                "state"
            };
            hostile[key]["fields"][0]["value"]["hex"] = "01".into();
        }
        assert!(checked.matches_target(&hostile).is_err());
        assert!(checked_step(&p, &scope, 0, 0, 30, &step, &digest).is_err());
        assert_ne!(
            checked.transfer_digest(&scope, &p, 0, 0, 31).unwrap(),
            checked.transfer_digest(&scope, &p, 0, 0, 32).unwrap()
        );
    }
}

#[test]
fn owned_reduce_outcome_uses_only_the_actual_checked_recorded_exchange_payload() {
    use super::super::super::{
        model, wire, CheckedOwnedWaitJournalContextV8, EntryV8, ExpectedRowV8, SourceJournalEntry,
    };
    use crate::agent_lifecycle::authorization::target_protocol::owned_wait_v8::settlement::{
        checked_owned_effect_settlement_v8, test_effect_exchange, OwnedEffectSettlementInputsV8,
    };
    CheckedOwnedWaitJournalContextV8::test_with_actual_runtime(|context, _lease, key| {
        let (base, _) = context.test_ready_documents(&key);
        let rows = wire::decode_inventory(
            &base,
            &ExpectedRowV8 {
                invocation: context.ordinary().invocation(),
                generation: context.generation(),
                seq: 0,
                prev_mac: &"0".repeat(64),
                ordinary: context.ordinary(),
            },
            &key,
        )
        .unwrap();
        let EntryV8::Owned(model::OwnedBodyV8::OwnedStateTransferCompleted { state, .. }) =
            &rows[15]
        else {
            panic!()
        };
        let EntryV8::Owned(model::OwnedBodyV8::OwnedAuthorizationStaged { decision, .. }) =
            &rows[17]
        else {
            panic!()
        };
        let EntryV8::Ordinary(SourceJournalEntry::AttemptSettled { response, .. }) = &rows[9]
        else {
            panic!()
        };
        let (runtime, execution) = context.test_runtime_execution();
        let scope = &context.registration().expected_facts().scope;
        let decoded = execution
            .wait()
            .lifecycle()
            .proposal_schema()
            .decode(std::str::from_utf8(response).unwrap())
            .unwrap();
        let proposal = v2::bind_owned_wait_proposal_v8(execution.wait(), scope, &decoded).unwrap();
        let inputs = || OwnedEffectSettlementInputsV8 {
            runtime,
            execution,
            scope,
            turn: 0,
            attempt: 0,
            state,
            decision,
            proposal: &proposal,
        };
        let (ordinary, evidence, result) = test_effect_exchange(&inputs());
        let checked =
            checked_owned_effect_settlement_v8(inputs(), &ordinary, &evidence, result.as_deref())
                .unwrap();
        let payload = checked.accepted_payload().unwrap();
        assert!(!payload.is_empty());
        let outcome = checked_outcome(execution.wait(), &checked).unwrap();
        let metadata = execution.wait().lifecycle().owned_wait_outcome_v8();
        assert_eq!(outcome["declaration"], metadata.id.as_str());
        let fields = outcome["fields"].as_array().unwrap();
        assert_eq!(fields.len(), 2);
        for (field, (id, ty)) in fields.iter().zip(metadata.fields()) {
            assert_eq!(field["identity"], id.as_str());
            if *ty == crate::hir::ResolvedType::Bytes {
                assert_eq!(
                    field["value"],
                    json!({"kind":"bytes","hex":crate::live_invocation::identity::hex(payload)})
                );
            } else {
                assert_eq!(field["value"], json!({"tag":"i64","value":0}));
            }
        }
        let mut hostile = ordinary.clone();
        let SourceJournalEntry::EffectObserved { observation, .. } = &mut hostile else {
            panic!()
        };
        observation.push(0);
        assert!(checked_owned_effect_settlement_v8(
            inputs(),
            &hostile,
            &evidence,
            result.as_deref()
        )
        .is_err());
    });
}
