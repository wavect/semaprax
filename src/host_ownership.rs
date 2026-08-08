//! Target-neutral ownership transaction model for future safe host adapters.
//!
//! Generated native resource entry points remain gated by `SPX-B104`. This
//! private reference model fixes the boundary semantics that an adapter must
//! implement first: registry-issued authority, trusted call contracts, fully
//! atomic `own` ingress, and an unambiguous split between rejection and an
//! executed language result. It contains no raw pointers, caller arenas, slot
//! reuse, imported finalizers, or public payload-adoption API.

#![cfg_attr(not(test), allow(dead_code))]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::conformance::{NormalizedStatus, Retryability, StatusClass};

pub(crate) const HOST_OWNERSHIP_SCHEMA_V1: &str = "semaprax.host-ownership.v1";

static NEXT_REGISTRY_NONCE: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HostIdentity(String);

impl HostIdentity {
    pub(crate) fn try_new(value: impl Into<String>) -> Result<Self, HostBoundaryRejection> {
        let value = value.into();
        if value.is_empty() || value.contains('\0') {
            return Err(HostBoundaryRejection::InvalidIdentity);
        }
        Ok(Self(value))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostResourceProvenance {
    module: HostIdentity,
    adapter: HostIdentity,
    resource_type: HostIdentity,
    lifecycle: HostIdentity,
    owner_thread: u64,
}

impl HostResourceProvenance {
    pub(crate) fn try_new(
        module: HostIdentity,
        adapter: HostIdentity,
        resource_type: HostIdentity,
        lifecycle: HostIdentity,
        owner_thread: u64,
    ) -> Result<Self, HostBoundaryRejection> {
        if owner_thread == 0 {
            return Err(HostBoundaryRejection::WrongThread);
        }
        Ok(Self {
            module,
            adapter,
            resource_type,
            lifecycle,
            owner_thread,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostResourceRequirement {
    resource_type: HostIdentity,
    lifecycle: HostIdentity,
}

impl HostResourceRequirement {
    pub(crate) fn new(resource_type: HostIdentity, lifecycle: HostIdentity) -> Self {
        Self {
            resource_type,
            lifecycle,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostResultPlan {
    Scalar,
    OwnedInput { input_index: usize },
}

/// Immutable compiler/adapter metadata for one exported function.
///
/// An external host caller supplies only opaque owner tokens. The trusted shim
/// constructs this contract and observes the executing thread; neither value
/// is part of the eventual untrusted host input surface.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostCallContract {
    module: HostIdentity,
    adapter: HostIdentity,
    function: HostIdentity,
    bound_thread: u64,
    inputs: Vec<HostResourceRequirement>,
    result: HostResultPlan,
}

impl HostCallContract {
    pub(crate) fn try_new(
        module: HostIdentity,
        adapter: HostIdentity,
        function: HostIdentity,
        bound_thread: u64,
        inputs: Vec<HostResourceRequirement>,
        result: HostResultPlan,
    ) -> Result<Self, HostBoundaryRejection> {
        if bound_thread == 0 {
            return Err(HostBoundaryRejection::WrongThread);
        }
        if let HostResultPlan::OwnedInput { input_index } = result {
            if input_index >= inputs.len() {
                return Err(HostBoundaryRejection::InvalidOwnedResult);
            }
        }
        Ok(Self {
            module,
            adapter,
            function,
            bound_thread,
            inputs,
            result,
        })
    }

    pub(crate) fn function(&self) -> &HostIdentity {
        &self.function
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct HostOwnerToken {
    registry_nonce: u64,
    slot: u64,
    generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct HostCallRequest {
    contract: HostCallContract,
    executing_thread: u64,
    owners: Vec<HostOwnerToken>,
}

impl HostCallRequest {
    pub(crate) fn new(
        contract: HostCallContract,
        executing_thread: u64,
        owners: Vec<HostOwnerToken>,
    ) -> Self {
        Self {
            contract,
            executing_thread,
            owners,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostPublishedValue {
    Scalar(i64),
    Owner(HostOwnerToken),
}

/// Only committed executions produce this type. Rejection exists solely in
/// [`HostCallStart`], before an invocation handle is created.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostCallOutcome {
    ExecutedSuccess(HostPublishedValue),
    ExecutedFailure(NormalizedStatus),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum HostBoundaryResult {
    Rejected(HostBoundaryRejection),
    Executed(HostCallOutcome),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HostBoundaryRejection {
    InvalidIdentity,
    RegistryExhausted,
    InvocationExhausted,
    ReentrantInvocation,
    RegistryPoisoned,
    InputCountMismatch,
    UnknownOwner,
    StaleOwner,
    OwnerNotLive,
    DuplicateOwner,
    WrongModule,
    WrongAdapter,
    WrongResourceType,
    WrongLifecycle,
    WrongThread,
    InvalidOwnedResult,
    ResultKindMismatch,
}

enum HostCallStart<'a> {
    Rejected(HostBoundaryRejection),
    Committed(HostCommittedCall<'a>),
}

enum HostCommittedCall<'a> {
    Scalar(HostScalarInvocation<'a>),
    Owned(HostOwnedInvocation<'a>),
}

/// Read-only physical payload view for the trusted generated executor.
///
/// The integer may necessarily cross into generated target code, so this type
/// is not physical non-escape enforcement. Only the audited backend/adapter may
/// receive the execution closure; untrusted hosts receive owner tokens.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HostCommittedResource {
    payload: u64,
}

impl HostCommittedResource {
    pub(crate) const fn payload(&self) -> u64 {
        self.payload
    }
}

/// Registry-bound completion guard kept inside `execute_scalar`.
struct HostScalarInvocation<'a> {
    registry: &'a mut HostOwnershipRegistry,
    sequence: u64,
    resources: Vec<HostCommittedResource>,
    completed: bool,
}

impl HostScalarInvocation<'_> {
    fn run<F>(mut self, execute: F) -> HostCallOutcome
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<i64, NormalizedStatus>,
    {
        let result = execute(&self.resources);
        let outcome = match result {
            Ok(value) => self.registry.finish(self.sequence, None, Some(value), None),
            Err(status) => self
                .registry
                .finish(self.sequence, None, None, Some(status)),
        };
        self.completed = true;
        outcome
    }
}

impl Drop for HostScalarInvocation<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.registry.abandon(self.sequence);
            self.completed = true;
        }
    }
}

/// Linear completion capability for an owned result. The selected result slot
/// was fixed by the trusted call contract before commit.
struct HostOwnedInvocation<'a> {
    registry: &'a mut HostOwnershipRegistry,
    sequence: u64,
    result_slot: u64,
    resources: Vec<HostCommittedResource>,
    completed: bool,
}

impl HostOwnedInvocation<'_> {
    fn run<F>(mut self, execute: F) -> HostCallOutcome
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<(), NormalizedStatus>,
    {
        let result = execute(&self.resources);
        let outcome = match result {
            Ok(()) => self
                .registry
                .finish(self.sequence, Some(self.result_slot), None, None),
            Err(status) => self
                .registry
                .finish(self.sequence, None, None, Some(status)),
        };
        self.completed = true;
        outcome
    }
}

impl Drop for HostOwnedInvocation<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.registry.abandon(self.sequence);
            self.completed = true;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HostOwnerState {
    Live,
    InInvocation(u64),
    Dead,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct HostOwnerEntry {
    generation: u64,
    provenance: HostResourceProvenance,
    payload: u64,
    state: HostOwnerState,
}

#[derive(Debug, Eq, PartialEq)]
struct ActiveInvocation {
    sequence: u64,
    slots: Vec<u64>,
}

/// Single-threaded reference ledger for a future adapter implementation.
///
/// The registry is deliberately non-`Clone`. Its nonce is allocated uniquely
/// within this linked runtime instance,
/// and a committed call returns a linear capability borrowing this exact
/// registry. A native implementation may use locks/atomics instead, but it
/// must preserve these observable transaction semantics.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct HostOwnershipRegistry {
    schema: &'static str,
    nonce: u64,
    next_slot: u64,
    next_invocation: u64,
    owners: BTreeMap<u64, HostOwnerEntry>,
    active: Option<ActiveInvocation>,
    last_abandonment: bool,
    poisoned: bool,
}

impl HostOwnershipRegistry {
    pub(crate) fn try_new() -> Result<Self, HostBoundaryRejection> {
        let nonce = NEXT_REGISTRY_NONCE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .map_err(|_| HostBoundaryRejection::RegistryExhausted)?;
        Ok(Self {
            schema: HOST_OWNERSHIP_SCHEMA_V1,
            nonce,
            next_slot: 1,
            next_invocation: 1,
            owners: BTreeMap::new(),
            active: None,
            last_abandonment: false,
            poisoned: false,
        })
    }

    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    /// Register an owner created by an already validated adapter/import.
    /// There is intentionally no public raw-payload adoption path.
    pub(crate) fn register_adapter_owner(
        &mut self,
        provenance: HostResourceProvenance,
        payload: u64,
    ) -> Result<HostOwnerToken, HostBoundaryRejection> {
        if self.poisoned {
            return Err(HostBoundaryRejection::RegistryPoisoned);
        }
        if self.active.is_some() {
            return Err(HostBoundaryRejection::ReentrantInvocation);
        }
        let slot = self.next_slot;
        self.next_slot = self
            .next_slot
            .checked_add(1)
            .ok_or(HostBoundaryRejection::RegistryExhausted)?;
        let generation = 1;
        self.owners.insert(
            slot,
            HostOwnerEntry {
                generation,
                provenance,
                payload,
                state: HostOwnerState::Live,
            },
        );
        Ok(HostOwnerToken {
            registry_nonce: self.nonce,
            slot,
            generation,
        })
    }

    /// Execute a scalar-result call inside a must-complete registry scope.
    ///
    /// If generated execution unwinds, the private completion guard consumes
    /// every committed input, clears the active invocation, records a stable
    /// adapter failure, and then allows the original panic to continue.
    pub(crate) fn execute_scalar<F>(
        &mut self,
        request: HostCallRequest,
        execute: F,
    ) -> HostBoundaryResult
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<i64, NormalizedStatus>,
    {
        if request.contract.result != HostResultPlan::Scalar {
            return HostBoundaryResult::Rejected(HostBoundaryRejection::ResultKindMismatch);
        }
        match self.begin_call(request) {
            HostCallStart::Rejected(rejection) => HostBoundaryResult::Rejected(rejection),
            HostCallStart::Committed(HostCommittedCall::Scalar(invocation)) => {
                HostBoundaryResult::Executed(invocation.run(execute))
            }
            HostCallStart::Committed(HostCommittedCall::Owned(_)) => {
                unreachable!("scalar result plan creates a scalar completion guard")
            }
        }
    }

    /// Execute an owned-result call inside a must-complete registry scope.
    pub(crate) fn execute_owned<F>(
        &mut self,
        request: HostCallRequest,
        execute: F,
    ) -> HostBoundaryResult
    where
        F: FnOnce(&[HostCommittedResource]) -> Result<(), NormalizedStatus>,
    {
        if !matches!(request.contract.result, HostResultPlan::OwnedInput { .. }) {
            return HostBoundaryResult::Rejected(HostBoundaryRejection::ResultKindMismatch);
        }
        match self.begin_call(request) {
            HostCallStart::Rejected(rejection) => HostBoundaryResult::Rejected(rejection),
            HostCallStart::Committed(HostCommittedCall::Owned(invocation)) => {
                HostBoundaryResult::Executed(invocation.run(execute))
            }
            HostCallStart::Committed(HostCommittedCall::Scalar(_)) => {
                unreachable!("owned result plan creates an owned completion guard")
            }
        }
    }

    pub(crate) fn take_last_abandonment(&mut self) -> Option<NormalizedStatus> {
        if std::mem::take(&mut self.last_abandonment) {
            Some(abandonment_status())
        } else {
            None
        }
    }

    /// Validate every fallible condition, allocate execution views, and then
    /// commit all owners together. No allocation occurs after the first owner
    /// state changes.
    fn begin_call(&mut self, request: HostCallRequest) -> HostCallStart<'_> {
        if let Err(rejection) = self.preflight(&request) {
            return HostCallStart::Rejected(rejection);
        }

        let slots = request
            .owners
            .iter()
            .map(|owner| owner.slot)
            .collect::<Vec<_>>();
        let resources = request
            .owners
            .iter()
            .map(|token| HostCommittedResource {
                payload: self
                    .lookup(*token)
                    .expect("preflight proved every owner exists")
                    .payload,
            })
            .collect::<Vec<_>>();
        let result_slot = match request.contract.result {
            HostResultPlan::Scalar => None,
            HostResultPlan::OwnedInput { input_index } => Some(slots[input_index]),
        };
        let sequence = self.next_invocation;
        self.next_invocation += 1;
        self.active = Some(ActiveInvocation { sequence, slots });
        for token in &request.owners {
            self.owners
                .get_mut(&token.slot)
                .expect("preflight proved every owner exists")
                .state = HostOwnerState::InInvocation(sequence);
        }

        match request.contract.result {
            HostResultPlan::Scalar => {
                HostCallStart::Committed(HostCommittedCall::Scalar(HostScalarInvocation {
                    registry: self,
                    sequence,
                    resources,
                    completed: false,
                }))
            }
            HostResultPlan::OwnedInput { .. } => {
                HostCallStart::Committed(HostCommittedCall::Owned(HostOwnedInvocation {
                    registry: self,
                    sequence,
                    result_slot: result_slot.expect("owned result slot was computed before commit"),
                    resources,
                    completed: false,
                }))
            }
        }
    }

    pub(crate) fn is_live(&self, token: HostOwnerToken) -> bool {
        self.lookup(token)
            .is_ok_and(|owner| owner.state == HostOwnerState::Live)
    }

    pub(crate) fn payload(&self, token: HostOwnerToken) -> Option<u64> {
        self.lookup(token)
            .ok()
            .filter(|owner| owner.state == HostOwnerState::Live)
            .map(|owner| owner.payload)
    }

    fn preflight(&self, request: &HostCallRequest) -> Result<(), HostBoundaryRejection> {
        if self.poisoned {
            return Err(HostBoundaryRejection::RegistryPoisoned);
        }
        if self.active.is_some() {
            return Err(HostBoundaryRejection::ReentrantInvocation);
        }
        if self.next_invocation == u64::MAX {
            return Err(HostBoundaryRejection::InvocationExhausted);
        }
        if request.executing_thread == 0
            || request.executing_thread != request.contract.bound_thread
        {
            return Err(HostBoundaryRejection::WrongThread);
        }
        if request.owners.len() != request.contract.inputs.len() {
            return Err(HostBoundaryRejection::InputCountMismatch);
        }

        let mut unique = BTreeSet::new();
        for (token, requirement) in request.owners.iter().zip(&request.contract.inputs) {
            if !unique.insert(*token) {
                return Err(HostBoundaryRejection::DuplicateOwner);
            }
            let owner = self.lookup(*token)?;
            if owner.state != HostOwnerState::Live {
                return Err(HostBoundaryRejection::OwnerNotLive);
            }
            compare_contract(
                &owner.provenance,
                &request.contract,
                requirement,
                request.executing_thread,
            )?;
        }

        if let HostResultPlan::OwnedInput { input_index } = request.contract.result {
            let result = self.lookup(request.owners[input_index])?;
            if result.generation == u64::MAX {
                return Err(HostBoundaryRejection::RegistryExhausted);
            }
        }
        Ok(())
    }

    fn lookup(&self, token: HostOwnerToken) -> Result<&HostOwnerEntry, HostBoundaryRejection> {
        if token.registry_nonce != self.nonce {
            return Err(HostBoundaryRejection::UnknownOwner);
        }
        let owner = self
            .owners
            .get(&token.slot)
            .ok_or(HostBoundaryRejection::UnknownOwner)?;
        if owner.generation != token.generation {
            return Err(HostBoundaryRejection::StaleOwner);
        }
        Ok(owner)
    }

    fn finish(
        &mut self,
        sequence: u64,
        published_slot: Option<u64>,
        scalar: Option<i64>,
        failure: Option<NormalizedStatus>,
    ) -> HostCallOutcome {
        let active = self
            .active
            .take()
            .expect("linear invocation capability requires one active call");
        assert_eq!(
            active.sequence, sequence,
            "linear invocation capability is registry-bound"
        );

        let mut published_owner = None;
        for slot in active.slots {
            let owner = self
                .owners
                .get_mut(&slot)
                .expect("committed owners cannot disappear");
            assert_eq!(
                owner.state,
                HostOwnerState::InInvocation(sequence),
                "committed owner state is immutable until completion"
            );
            if Some(slot) == published_slot {
                owner.generation = owner
                    .generation
                    .checked_add(1)
                    .expect("owned-result generation overflow was rejected before commit");
                owner.state = HostOwnerState::Live;
                published_owner = Some(HostOwnerToken {
                    registry_nonce: self.nonce,
                    slot,
                    generation: owner.generation,
                });
            } else {
                owner.state = HostOwnerState::Dead;
            }
        }

        if let Some(status) = failure {
            debug_assert!(published_slot.is_none() && scalar.is_none());
            return HostCallOutcome::ExecutedFailure(status);
        }
        match (scalar, published_owner) {
            (Some(value), None) => {
                HostCallOutcome::ExecutedSuccess(HostPublishedValue::Scalar(value))
            }
            (None, Some(owner)) => {
                HostCallOutcome::ExecutedSuccess(HostPublishedValue::Owner(owner))
            }
            _ => unreachable!("typed invocation capabilities fix the result shape"),
        }
    }

    fn abandon(&mut self, sequence: u64) {
        let Some(active) = self.active.take() else {
            self.poisoned = true;
            self.last_abandonment = true;
            return;
        };
        if active.sequence != sequence {
            self.poisoned = true;
        }
        for slot in active.slots {
            match self.owners.get_mut(&slot) {
                Some(owner) => {
                    if owner.state != HostOwnerState::InInvocation(sequence) {
                        self.poisoned = true;
                    }
                    owner.state = HostOwnerState::Dead;
                }
                None => self.poisoned = true,
            }
        }
        self.last_abandonment = true;
    }
}

fn abandonment_status() -> NormalizedStatus {
    NormalizedStatus::try_new(
        "semaprax.adapter.host-ownership.v1",
        1,
        StatusClass::Adapter,
        Retryability::Known(false),
    )
    .expect("the fixed host-ownership abandonment status is valid")
}

fn compare_contract(
    actual: &HostResourceProvenance,
    contract: &HostCallContract,
    requirement: &HostResourceRequirement,
    executing_thread: u64,
) -> Result<(), HostBoundaryRejection> {
    if actual.module != contract.module {
        return Err(HostBoundaryRejection::WrongModule);
    }
    if actual.adapter != contract.adapter {
        return Err(HostBoundaryRejection::WrongAdapter);
    }
    if actual.resource_type != requirement.resource_type {
        return Err(HostBoundaryRejection::WrongResourceType);
    }
    if actual.lifecycle != requirement.lifecycle {
        return Err(HostBoundaryRejection::WrongLifecycle);
    }
    if actual.owner_thread != executing_thread {
        return Err(HostBoundaryRejection::WrongThread);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
}
