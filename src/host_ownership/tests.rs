use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::cleanup_plan::ContractPhase;
use crate::conformance::NormalizedStatus;

use super::*;

fn id(value: &str) -> HostIdentity {
    HostIdentity::try_new(value).unwrap()
}

fn provenance(
    module: &str,
    adapter: &str,
    resource_type: &str,
    lifecycle: &str,
    thread: u64,
) -> HostResourceProvenance {
    HostResourceProvenance::try_new(
        id(module),
        id(adapter),
        id(resource_type),
        id(lifecycle),
        thread,
    )
    .unwrap()
}

fn requirement(resource_type: &str, lifecycle: &str) -> HostResourceRequirement {
    HostResourceRequirement::new(id(resource_type), id(lifecycle))
}

fn contract(
    module: &str,
    adapter: &str,
    thread: u64,
    inputs: Vec<HostResourceRequirement>,
    result: HostResultPlan,
) -> HostCallContract {
    HostCallContract::try_new(
        id(module),
        id(adapter),
        id("token.function"),
        thread,
        inputs,
        result,
    )
    .unwrap()
}

fn request(
    contract: HostCallContract,
    executing_thread: u64,
    owners: Vec<HostOwnerToken>,
) -> HostCallRequest {
    HostCallRequest::new(contract, executing_thread, owners)
}

fn rejected(result: HostBoundaryResult) -> HostBoundaryRejection {
    let HostBoundaryResult::Rejected(rejection) = result else {
        panic!("expected rejection");
    };
    rejection
}

#[test]
fn trusted_contract_rejections_are_atomic_and_do_not_advance_invocations() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let first = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 11),
            0,
        )
        .unwrap();
    let second = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 11),
            u64::MAX,
        )
        .unwrap();
    let original_owners = registry.owners.clone();
    let original_invocation = registry.next_invocation;

    let cases = [
        (
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    11,
                    vec![requirement("token.type", "token.drop"); 2],
                    HostResultPlan::Scalar,
                ),
                11,
                vec![first, first],
            ),
            HostBoundaryRejection::DuplicateOwner,
        ),
        (
            request(
                contract(
                    "module.other",
                    "adapter.one",
                    11,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                11,
                vec![first],
            ),
            HostBoundaryRejection::WrongModule,
        ),
        (
            request(
                contract(
                    "module.one",
                    "adapter.other",
                    11,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                11,
                vec![first],
            ),
            HostBoundaryRejection::WrongAdapter,
        ),
        (
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    11,
                    vec![requirement("other.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                11,
                vec![first],
            ),
            HostBoundaryRejection::WrongResourceType,
        ),
        (
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    11,
                    vec![requirement("token.type", "other.drop")],
                    HostResultPlan::Scalar,
                ),
                11,
                vec![first],
            ),
            HostBoundaryRejection::WrongLifecycle,
        ),
        (
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    11,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                12,
                vec![first],
            ),
            HostBoundaryRejection::WrongThread,
        ),
    ];

    for (call, expected) in cases {
        assert_eq!(
            rejected(registry.execute_scalar(call, |_| panic!("rejected call executed"))),
            expected
        );
        assert_eq!(registry.owners, original_owners);
        assert_eq!(registry.next_invocation, original_invocation);
        assert!(registry.active.is_none());
    }
    assert!(registry.is_live(first));
    assert!(registry.is_live(second));
}

#[test]
fn detached_preflight_is_non_mutating_and_predicts_the_committed_sequence() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let owner = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 17),
            41,
        )
        .unwrap();
    let request = request(
        contract(
            "module.one",
            "adapter.one",
            17,
            vec![requirement("token.type", "token.drop")],
            HostResultPlan::Scalar,
        ),
        17,
        vec![owner],
    );
    let original_owners = registry.owners.clone();

    assert_eq!(registry.preflight_prepared(&request), Ok(1));
    assert_eq!(registry.preflight_prepared(&request), Ok(1));
    assert_eq!(registry.owners, original_owners);
    assert_eq!(registry.next_invocation, 1);
    assert!(registry.active.is_none());

    let prepared = registry.prepare_scalar(request).unwrap();
    assert_eq!(prepared.sequence(), 1);
    assert_eq!(prepared.payloads(), &[41]);
    assert_eq!(registry.next_invocation, 2);
    assert!(!registry.is_live(owner));

    assert_eq!(
        registry.complete_prepared_scalar(prepared, Ok(9)),
        HostCallOutcome::ExecutedSuccess(HostPublishedValue::Scalar(9))
    );
    assert!(!registry.is_live(owner));
}

#[test]
fn detached_plan_allocates_without_mutation_then_commits_exact_sequence() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let owner = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 17),
            u64::MAX,
        )
        .unwrap();
    let request = request(
        contract(
            "module.one",
            "adapter.one",
            17,
            vec![requirement("token.type", "token.drop")],
            HostResultPlan::Scalar,
        ),
        17,
        vec![owner],
    );
    let original_owners = registry.owners.clone();
    let plan = registry.plan_scalar(&request).unwrap();
    assert_eq!(plan.sequence(), 1);
    assert_eq!(plan.payloads(), &[u64::MAX]);
    assert_eq!(registry.owners, original_owners);
    assert_eq!(registry.next_invocation, 1);
    assert!(registry.active.is_none());

    let prepared = registry.commit_plan(plan).unwrap();
    assert_eq!(prepared.sequence(), 1);
    assert_eq!(prepared.payloads(), &[u64::MAX]);
    assert_eq!(registry.next_invocation, 2);
    assert!(!registry.is_live(owner));
    assert_eq!(
        registry.complete_prepared_scalar(prepared, Ok(7)),
        HostCallOutcome::ExecutedSuccess(HostPublishedValue::Scalar(7))
    );
}

#[test]
fn completion_invariant_panic_retains_prepared_state_for_guard_abandonment() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let owner = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 17),
            41,
        )
        .unwrap();
    let request = request(
        contract(
            "module.one",
            "adapter.one",
            17,
            vec![requirement("token.type", "token.drop")],
            HostResultPlan::Scalar,
        ),
        17,
        vec![owner],
    );
    let plan = registry.plan_scalar(&request).unwrap();
    let prepared = registry.commit_plan(plan).unwrap();
    registry.owners.get_mut(&owner.slot).unwrap().state = HostOwnerState::Live;

    let panicked = catch_unwind(AssertUnwindSafe(|| {
        registry.complete_prepared_scalar_ref(&prepared, Ok(7));
    }));
    assert!(panicked.is_err());
    assert!(registry.active.is_some());

    registry.abandon_prepared_ref(&prepared);
    assert!(registry.active.is_none());
    assert_eq!(
        registry.owners.get(&owner.slot).unwrap().state,
        HostOwnerState::Dead
    );
    assert!(registry.take_last_abandonment_flag());
}

#[test]
fn checked_owned_completion_mismatch_is_non_mutating_and_abandonable() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let owner = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 17),
            u64::MAX,
        )
        .unwrap();
    let request = request(
        contract(
            "module.one",
            "adapter.one",
            17,
            vec![requirement("token.type", "token.drop")],
            HostResultPlan::OwnedInput { input_index: 0 },
        ),
        17,
        vec![owner],
    );
    let plan = registry.plan_owned(&request).unwrap();
    let prepared = registry.commit_plan(plan).unwrap();
    let expected = owner.next_generation().unwrap();
    let hostile = HostOwnerToken {
        generation: expected.generation + 1,
        ..expected
    };

    let completion = catch_unwind(AssertUnwindSafe(|| {
        registry.complete_prepared_owned_expected_ref(&prepared, hostile)
    }));
    assert_eq!(completion.unwrap(), Err(HostBoundaryRejection::StalePlan));
    assert!(registry.active.is_some());
    assert_eq!(
        registry.owners.get(&owner.slot).unwrap().state,
        HostOwnerState::InInvocation(prepared.sequence())
    );
    assert_eq!(registry.live_owner_count(), 0);

    registry.abandon_prepared_ref(&prepared);
    assert!(registry.active.is_none());
    assert_eq!(registry.live_owner_count(), 0);
    assert_eq!(
        registry.owners.get(&owner.slot).unwrap().state,
        HostOwnerState::Dead
    );
    assert!(registry.take_last_abandonment_flag());
}

#[test]
fn detached_plan_rejects_foreign_or_stale_state_without_mutation() {
    let mut first = HostOwnershipRegistry::try_new().unwrap();
    let mut second = HostOwnershipRegistry::try_new().unwrap();
    let owner = first
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 17),
            9,
        )
        .unwrap();
    let request = request(
        contract(
            "module.one",
            "adapter.one",
            17,
            vec![requirement("token.type", "token.drop")],
            HostResultPlan::Scalar,
        ),
        17,
        vec![owner],
    );
    let foreign_plan = first.plan_scalar(&request).unwrap();
    assert_eq!(
        second.commit_plan(foreign_plan).err(),
        Some(HostBoundaryRejection::StalePlan)
    );
    assert!(first.is_live(owner));
    assert_eq!(second.next_invocation, 1);
    assert!(second.active.is_none());

    let stale_plan = first.plan_scalar(&request).unwrap();
    assert_eq!(first.retire_owner(owner), Ok(9));
    let before = first.owners.clone();
    assert_eq!(
        first.commit_plan(stale_plan).err(),
        Some(HostBoundaryRejection::StalePlan)
    );
    assert_eq!(first.owners, before);
    assert_eq!(first.next_invocation, 1);
    assert!(first.active.is_none());
}

#[test]
fn adapter_owner_rollback_restores_the_exact_slot_reservation() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let provenance = provenance("module.one", "adapter.one", "token.type", "token.drop", 17);
    let first = registry
        .register_adapter_owner(provenance.clone(), 0)
        .unwrap();
    assert_eq!(registry.live_owner_count(), 1);
    registry.rollback_adapter_owner(first).unwrap();
    assert_eq!(registry.live_owner_count(), 0);
    assert!(registry.owners.is_empty());

    let replacement = registry
        .register_adapter_owner(provenance, u64::MAX)
        .unwrap();
    assert_eq!(replacement, first);
    assert_eq!(registry.payload(replacement), Some(u64::MAX));
}

#[test]
fn separately_allocated_registries_never_accept_each_others_tokens() {
    let mut first_registry = HostOwnershipRegistry::try_new().unwrap();
    let mut second_registry = HostOwnershipRegistry::try_new().unwrap();
    assert_ne!(first_registry.nonce, second_registry.nonce);
    let first = first_registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 21),
            1,
        )
        .unwrap();
    let second = second_registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 21),
            2,
        )
        .unwrap();
    assert_eq!(first.slot, second.slot);
    assert_eq!(first.generation, second.generation);
    assert_eq!(
        rejected(second_registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    21,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                21,
                vec![first],
            ),
            |_| panic!("cross-registry call executed"),
        )),
        HostBoundaryRejection::UnknownOwner
    );
    assert!(first_registry.is_live(first));
    assert!(second_registry.is_live(second));

    assert_eq!(
        rejected(second_registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    21,
                    vec![requirement("token.type", "token.drop"); 2],
                    HostResultPlan::Scalar,
                ),
                21,
                vec![second, first],
            ),
            |_| panic!("mixed-registry call executed"),
        )),
        HostBoundaryRejection::UnknownOwner
    );
    assert!(second_registry.is_live(second));
}

#[test]
fn panicking_execution_consumes_inputs_clears_active_and_records_adapter_failure() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let owner = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 25),
            17,
        )
        .unwrap();
    let panicked = catch_unwind(AssertUnwindSafe(|| {
        registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    25,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                25,
                vec![owner],
            ),
            |resources| {
                assert_eq!(resources[0].payload(), 17);
                panic!("simulated generated execution panic")
            },
        )
    }));
    assert!(panicked.is_err());
    assert!(registry.active.is_none());
    assert!(!registry.is_live(owner));
    let abandonment = registry.take_last_abandonment().unwrap();
    assert_eq!(abandonment.class(), StatusClass::Adapter);
    assert_eq!(
        abandonment.domain_id(),
        "semaprax.adapter.host-ownership.v1"
    );
    assert_eq!(abandonment.code(), 1);

    let replacement = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 25),
            18,
        )
        .unwrap();
    assert!(registry.is_live(replacement));
}

#[test]
fn scalar_success_and_executed_failure_consume_every_owner() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let first = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 31),
            0,
        )
        .unwrap();
    let second = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 31),
            u64::MAX,
        )
        .unwrap();
    assert_eq!(
        registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    31,
                    vec![requirement("token.type", "token.drop"); 2],
                    HostResultPlan::Scalar,
                ),
                31,
                vec![first, second],
            ),
            |resources| {
                assert_eq!(
                    resources
                        .iter()
                        .map(|item| item.payload())
                        .collect::<Vec<_>>(),
                    vec![0, u64::MAX]
                );
                Ok(42)
            },
        ),
        HostBoundaryResult::Executed(HostCallOutcome::ExecutedSuccess(
            HostPublishedValue::Scalar(42)
        ))
    );
    assert!(!registry.is_live(first));
    assert!(!registry.is_live(second));

    let third = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 31),
            3,
        )
        .unwrap();
    let status = NormalizedStatus::contract(ContractPhase::Requires);
    assert_eq!(
        registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    31,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                31,
                vec![third],
            ),
            |_| Err(status.clone()),
        ),
        HostBoundaryResult::Executed(HostCallOutcome::ExecutedFailure(status))
    );
    assert!(!registry.is_live(third));

    let fourth = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 31),
            4,
        )
        .unwrap();
    let status = NormalizedStatus::contract(ContractPhase::Ensures);
    assert_eq!(
        registry.execute_owned(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    31,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::OwnedInput { input_index: 0 },
                ),
                31,
                vec![fourth],
            ),
            |_| Err(status.clone()),
        ),
        HostBoundaryResult::Executed(HostCallOutcome::ExecutedFailure(status))
    );
    assert!(!registry.is_live(fourth));
}

#[test]
fn owned_success_rotates_only_the_published_generation_and_preserves_payload() {
    for payload in [0, u64::MAX] {
        let mut registry = HostOwnershipRegistry::try_new().unwrap();
        let result_owner = registry
            .register_adapter_owner(
                provenance("module.one", "adapter.one", "token.type", "token.drop", 41),
                payload,
            )
            .unwrap();
        let discarded = registry
            .register_adapter_owner(
                provenance("module.one", "adapter.one", "token.type", "token.drop", 41),
                7,
            )
            .unwrap();
        let HostBoundaryResult::Executed(HostCallOutcome::ExecutedSuccess(
            HostPublishedValue::Owner(published),
        )) = registry.execute_owned(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    41,
                    vec![requirement("token.type", "token.drop"); 2],
                    HostResultPlan::OwnedInput { input_index: 0 },
                ),
                41,
                vec![result_owner, discarded],
            ),
            |resources| {
                assert_eq!(resources[0].payload(), payload);
                assert_eq!(resources[1].payload(), 7);
                Ok(())
            },
        )
        else {
            panic!("owned result was not published");
        };
        assert_ne!(published, result_owner);
        assert!(!registry.is_live(result_owner));
        assert!(!registry.is_live(discarded));
        assert!(registry.is_live(published));
        assert_eq!(registry.payload(published), Some(payload));
        assert_eq!(
            rejected(registry.execute_scalar(
                request(
                    contract(
                        "module.one",
                        "adapter.one",
                        41,
                        vec![requirement("token.type", "token.drop")],
                        HostResultPlan::Scalar,
                    ),
                    41,
                    vec![result_owner],
                ),
                |_| panic!("stale owner call executed"),
            )),
            HostBoundaryRejection::StaleOwner
        );
    }
}

#[test]
fn max_generation_is_safe_for_dead_inputs_and_rejected_for_publication() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let token = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 51),
            9,
        )
        .unwrap();
    registry.owners.get_mut(&token.slot).unwrap().generation = u64::MAX;
    let max_token = HostOwnerToken {
        generation: u64::MAX,
        ..token
    };
    assert_eq!(
        registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    51,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                51,
                vec![max_token],
            ),
            |_| Ok(0),
        ),
        HostBoundaryResult::Executed(HostCallOutcome::ExecutedSuccess(
            HostPublishedValue::Scalar(0)
        ))
    );

    let publish = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 51),
            10,
        )
        .unwrap();
    registry.owners.get_mut(&publish.slot).unwrap().generation = u64::MAX;
    let max_publish = HostOwnerToken {
        generation: u64::MAX,
        ..publish
    };
    assert_eq!(
        rejected(registry.execute_owned(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    51,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::OwnedInput { input_index: 0 },
                ),
                51,
                vec![max_publish],
            ),
            |_| panic!("maximum-generation owned call executed"),
        )),
        HostBoundaryRejection::RegistryExhausted
    );
    assert!(registry.is_live(max_publish));
}

#[test]
fn copied_stale_and_malformed_requests_fail_closed() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let owner = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 61),
            1,
        )
        .unwrap();
    assert_eq!(
        rejected(registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    61,
                    vec![requirement("token.type", "token.drop"); 2],
                    HostResultPlan::Scalar,
                ),
                61,
                vec![owner],
            ),
            |_| panic!("malformed call executed"),
        )),
        HostBoundaryRejection::InputCountMismatch
    );
    assert!(registry.is_live(owner));

    assert!(matches!(
        registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    61,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                61,
                vec![owner],
            ),
            |_| Ok(0),
        ),
        HostBoundaryResult::Executed(HostCallOutcome::ExecutedSuccess(_))
    ));
    assert_eq!(
        rejected(registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    61,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                61,
                vec![owner],
            ),
            |_| panic!("dead copied owner call executed"),
        )),
        HostBoundaryRejection::OwnerNotLive
    );
}

#[test]
fn result_kind_and_invocation_exhaustion_reject_before_commit() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let owner = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 66),
            5,
        )
        .unwrap();
    let original_owners = registry.owners.clone();
    let original_invocation = registry.next_invocation;
    assert_eq!(
        rejected(registry.execute_owned(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    66,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                66,
                vec![owner],
            ),
            |_| panic!("result-kind mismatch executed"),
        )),
        HostBoundaryRejection::ResultKindMismatch
    );
    assert_eq!(registry.owners, original_owners);
    assert_eq!(registry.next_invocation, original_invocation);

    registry.next_invocation = u64::MAX;
    assert_eq!(
        rejected(registry.execute_scalar(
            request(
                contract(
                    "module.one",
                    "adapter.one",
                    66,
                    vec![requirement("token.type", "token.drop")],
                    HostResultPlan::Scalar,
                ),
                66,
                vec![owner],
            ),
            |_| panic!("exhausted invocation executed"),
        )),
        HostBoundaryRejection::InvocationExhausted
    );
    assert_eq!(registry.owners, original_owners);
    assert!(registry.active.is_none());
}

#[test]
fn identities_contracts_and_schema_are_validated() {
    assert_eq!(
        HostIdentity::try_new(""),
        Err(HostBoundaryRejection::InvalidIdentity)
    );
    assert_eq!(
        HostIdentity::try_new("bad\0identity"),
        Err(HostBoundaryRejection::InvalidIdentity)
    );
    assert_eq!(
        HostResourceProvenance::try_new(
            id("module.one"),
            id("adapter.one"),
            id("token.type"),
            id("token.drop"),
            0,
        ),
        Err(HostBoundaryRejection::WrongThread)
    );
    assert_eq!(
        HostCallContract::try_new(
            id("module.one"),
            id("adapter.one"),
            id("token.function"),
            71,
            vec![requirement("token.type", "token.drop")],
            HostResultPlan::OwnedInput { input_index: 1 },
        ),
        Err(HostBoundaryRejection::InvalidOwnedResult)
    );
    let registry = HostOwnershipRegistry::try_new().unwrap();
    assert_eq!(registry.schema(), HOST_OWNERSHIP_SCHEMA_V1);
    let call = contract(
        "module.one",
        "adapter.one",
        71,
        Vec::new(),
        HostResultPlan::Scalar,
    );
    assert_eq!(call.function().as_str(), "token.function");
}

#[test]
fn out_of_call_retirement_is_exact_and_observable() {
    let mut registry = HostOwnershipRegistry::try_new().unwrap();
    let owner = registry
        .register_adapter_owner(
            provenance("module.one", "adapter.one", "token.type", "token.drop", 71),
            0,
        )
        .unwrap();
    assert_ne!(owner.slot(), 0);
    assert_eq!(owner.generation(), 1);
    assert_eq!(registry.live_owner_count(), 1);
    assert_eq!(registry.retire_owner(owner), Ok(0));
    assert_eq!(registry.live_owner_count(), 0);
    assert_eq!(
        registry.retire_owner(owner),
        Err(HostBoundaryRejection::OwnerNotLive)
    );
}
