//! The reference interpreter PHYSICAL adapter for [Public Generic Carrier
//! v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md) (issue #162), the third of
//! PG-7's per-target adapters after [`super::native`] (issue #154) and
//! [`super::wasm`] (issue #155). This module implements layout, allocation,
//! and release for the reference interpreter's own physical model; it does
//! not re-decide legality that
//! [`super::carrier::machine::CarrierCallMachine`] and
//! [`super::descriptor::verify`] already own — see [Public Generic Carrier
//! v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md#logical-versus-physical)'s
//! LOGICAL/PHYSICAL split.
//!
//! **Why this exists.** Issue #162's crux is proving that the SAME LOGICAL
//! boundary produces equal observable behavior across engines. Before this
//! module, no adapter drove [`CarrierCallMachine`] as "the reference
//! interpreter route" at all: `carrier.rs`'s own doc note ("no provider, no
//! native or Wasm adapter, and no execution") and #153-#155's own scope
//! notes left the `TargetProfile::Interpreter` variant defined in
//! [`super::carrier::TargetProfile`] but never bound to a concrete adapter.
//! [`InterpreterProvider`] fills exactly that gap: a boundary adapter that
//! consumes a replayed carrier binding, stages owned leaves, commits the one
//! atomic transfer, invokes the same fixture endpoint native/wasm bind
//! (`spx_pg_*_endpoint_reverse_bytes_v1`'s logical twin), stages/commits a
//! result, and emits the identical normalized trace vocabulary — so
//! `carrier::settlement_corpus` (issue #162) can run one shared case table
//! against this adapter and [`super::wasm::provider::WasmProvider`] and
//! diff the two, rather than asserting each engine only against its own
//! expectation.
//!
//! **Physical model, deliberately different from both other adapters.**
//! Native's C11 adapter allocates from the process heap through pointer
//! identities; the Wasm adapter allocates from one bounded, page-grown
//! linear-memory arena through a strict LIFO stack allocator, because a
//! release out of allocation order is otherwise unobservable in a reused
//! byte array. The reference interpreter has neither constraint: it is an
//! ordinary Rust host, so [`Heap`] below is a plain slot table
//! (`HashMap<u32, Vec<u8>>`) with no arena, no address space, and no
//! forced LIFO discipline — release order is enforced once, upstream, by
//! [`CarrierCallMachine`]'s own reverse-obligation-order rule, not restated
//! by this physical layer. Exercising three genuinely different physical
//! representations against the same case table is the actual cross-engine
//! proof: agreement is not an artifact of one shared allocator.
//!
//! **Deferred scope**, identical to native's and Wasm's own: deriving a
//! provider from a real checked *generic* export needs #119's still-blocked
//! owned-record ownership evidence, so the bound endpoint here is the same
//! fixture (byte-reversal per owned leaf) operating on the same flat
//! owned-`Bytes` shape, and the trusted descriptor bytes an `open` caller
//! replays against are a hand-constructed fixture compared byte-for-byte,
//! not [`super::descriptor::verify`] output. Unlike native/Wasm, this
//! adapter introduces no new physical-layer wire-binding artifact
//! (`NativeProviderBindingV1`, `WasmProviderBindingV1`): the reference
//! interpreter is in-process Rust with no cross-language wire boundary to
//! cross, so `open` replays directly against
//! [`super::carrier::CarrierBindingV1`] naming `TargetProfile::Interpreter`,
//! introducing no new diagnostic code range.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::boundary_profile::{
    MAX_BYTES_PER_LEAF, MAX_OWNED_LEAVES_PER_INSTANCE, MAX_TOTAL_PAYLOAD_BYTES,
};
use crate::public_generic_abi::carrier::machine::CarrierCallMachine;
use crate::public_generic_abi::carrier::trace::TraceLabel;
use crate::public_generic_abi::carrier::{
    check_handle_capacity, replay_binding, CarrierBindingV1, Handle, Settlement, TargetProfile,
    CARRIER_CAPACITY, CARRIER_REPLAY_MISMATCH, HANDLE_GENERATION_MISMATCH, ILLEGAL_TRANSITION,
    MALFORMED_CARRIER, STICKY_SETTLEMENT_VIOLATION,
};
use crate::public_generic_abi::wasm::registry::{
    HandleRegistry, HandleRole, RegistryEntry, HANDLE_INVALID, WRONG_KIND,
};

/// Result handles for one call live in a disjoint `id` range from that same
/// call's input handles, mirroring
/// [`super::wasm::provider::WasmProvider`]'s own `RESULT_ID_OFFSET`
/// convention exactly, for the identical reason: without the split, a
/// call's result root would be byte-identical to that same call's
/// already-released input root.
const RESULT_ID_OFFSET: u32 = 1000;

fn result_root_handle(call_id: u32) -> Handle {
    Handle {
        id: RESULT_ID_OFFSET,
        generation: call_id,
    }
}

fn result_leaf_handle(index: u32, call_id: u32) -> Handle {
    Handle {
        id: RESULT_ID_OFFSET + 1 + index,
        generation: call_id,
    }
}

/// The one fixture endpoint this adapter binds, matching
/// [`super::native::binding`]'s and [`super::wasm::provider`]'s own fixture
/// endpoint naming convention.
pub const FIXTURE_ENDPOINT_EXPORT_NAME: &str = "spx_pg_interpreter_endpoint_reverse_bytes_v1";

/// The closed, normalized status vocabulary this adapter returns. Every
/// integer value matches
/// [`crate::public_generic_abi::wasm::provider::WasmPgStatus`]'s own
/// vocabulary exactly (`Ok` = 0 through `NullOrWrongKind` = 13) — the same
/// deliberate convergence Wasm's adapter documents relative to native's, so
/// a caller-facing status vocabulary all three physical adapters already
/// happen to agree on is one less translation the cross-engine comparison
/// in `carrier::settlement_corpus` has to solve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum InterpreterPgStatus {
    Ok = 0,
    MalformedDescriptor = 1,
    DescriptorReplayMismatch = 2,
    MalformedBinding = 3,
    BindingReplayMismatch = 4,
    MalformedCarrier = 5,
    CarrierCapacity = 6,
    IllegalTransition = 7,
    HandleInvalid = 8,
    StickySettlementViolation = 9,
    AllocationFailure = 10,
    ContractFailure = 11,
    BufferTooSmall = 12,
    NullOrWrongKind = 13,
}

fn status_from_diagnostic(error: &Diagnostic) -> InterpreterPgStatus {
    match error.code {
        MALFORMED_CARRIER => InterpreterPgStatus::MalformedCarrier,
        CARRIER_CAPACITY => InterpreterPgStatus::CarrierCapacity,
        ILLEGAL_TRANSITION => InterpreterPgStatus::IllegalTransition,
        HANDLE_GENERATION_MISMATCH => InterpreterPgStatus::HandleInvalid,
        STICKY_SETTLEMENT_VIOLATION => InterpreterPgStatus::StickySettlementViolation,
        CARRIER_REPLAY_MISMATCH => InterpreterPgStatus::BindingReplayMismatch,
        HANDLE_INVALID => InterpreterPgStatus::HandleInvalid,
        WRONG_KIND => InterpreterPgStatus::NullOrWrongKind,
        _ => InterpreterPgStatus::IllegalTransition,
    }
}

/// The same physical-status consolidation
/// [`super::wasm::provider`] applies, restated here rather than reinvented.
fn status_from_settlement(settlement: Settlement) -> InterpreterPgStatus {
    match settlement {
        Settlement::Success => InterpreterPgStatus::Ok,
        Settlement::ProviderFailure => InterpreterPgStatus::IllegalTransition,
        Settlement::AllocationFailure => InterpreterPgStatus::AllocationFailure,
        Settlement::CopyOutFailure
        | Settlement::MalformedResult
        | Settlement::ContractFailure
        | Settlement::ConsumerRefusal
        | Settlement::CleanupFailure => InterpreterPgStatus::ContractFailure,
    }
}

/// An opaque handle this adapter hands back to a caller. Fields are
/// private; a caller may only hold this and pass it back to this same
/// [`InterpreterProvider`] instance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InterpreterHandle {
    tag: u64,
    handle: Handle,
}

/// One live storage slot's byte length, so a release can verify it is
/// freeing exactly the span it allocated.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PhysicalSlot {
    slot: u32,
    len: u32,
}

/// A plain, arena-free heap: no address space, no LIFO discipline — see the
/// module documentation for why that is the point, not a shortcut.
#[derive(Debug, Default)]
struct Heap {
    slots: HashMap<u32, Vec<u8>>,
    next_slot: u32,
    live_bytes: u32,
}

fn capacity_error(message: &str) -> Diagnostic {
    Diagnostic::io(CARRIER_CAPACITY, message.to_owned())
}

fn internal_error(message: &str) -> Diagnostic {
    Diagnostic::io(ILLEGAL_TRANSITION, message.to_owned())
}

impl Heap {
    fn new() -> Self {
        Self::default()
    }

    fn live_allocations(&self) -> u32 {
        self.slots.len() as u32
    }

    fn live_bytes(&self) -> u32 {
        self.live_bytes
    }

    /// Reserve a fresh, zero-initialized slot of `len` bytes. Rejects a
    /// request over [`MAX_BYTES_PER_LEAF`] or one that would push live
    /// bytes over [`MAX_TOTAL_PAYLOAD_BYTES`], before writing anything —
    /// the interpreter-heap analog of Wasm's `StackAllocator::alloc`.
    fn alloc(&mut self, len: u32) -> Result<u32, Diagnostic> {
        if len as usize > MAX_BYTES_PER_LEAF {
            return Err(capacity_error("leaf allocation exceeds MAX_BYTES_PER_LEAF"));
        }
        let next_live = self
            .live_bytes
            .checked_add(len)
            .ok_or_else(|| capacity_error("live byte total overflows u32"))?;
        if next_live as usize > MAX_TOTAL_PAYLOAD_BYTES {
            return Err(capacity_error(
                "allocation would exceed MAX_TOTAL_PAYLOAD_BYTES",
            ));
        }
        let slot = self.next_slot;
        self.next_slot = self
            .next_slot
            .checked_add(1)
            .ok_or_else(|| capacity_error("slot id space exhausted"))?;
        self.slots.insert(slot, vec![0u8; len as usize]);
        self.live_bytes = next_live;
        Ok(slot)
    }

    fn write(&mut self, slot: u32, data: &[u8]) -> Result<(), Diagnostic> {
        let bytes = self
            .slots
            .get_mut(&slot)
            .ok_or_else(|| internal_error("write to an unallocated interpreter heap slot"))?;
        if bytes.len() != data.len() {
            return Err(internal_error(
                "write length does not match the slot's allocated length",
            ));
        }
        bytes.copy_from_slice(data);
        Ok(())
    }

    fn read(&self, slot: u32) -> Result<&[u8], Diagnostic> {
        self.slots
            .get(&slot)
            .map(Vec::as_slice)
            .ok_or_else(|| internal_error("read from an unallocated interpreter heap slot"))
    }

    /// Free `slot`, verifying its length matches `expected_len` — the
    /// interpreter-heap analog of Wasm's zero-on-free, standing in for
    /// "physical release is real, not bookkeeping": the slot's bytes are
    /// dropped for real, not merely marked free.
    fn free(&mut self, slot: u32, expected_len: u32) -> Result<(), Diagnostic> {
        let bytes = self
            .slots
            .remove(&slot)
            .ok_or_else(|| internal_error("free of an unallocated interpreter heap slot"))?;
        if bytes.len() as u32 != expected_len {
            return Err(internal_error(
                "free length does not match the allocated length",
            ));
        }
        self.live_bytes -= expected_len;
        Ok(())
    }
}

#[derive(Debug)]
enum CallStage {
    PendingValue,
    PendingResult,
}

#[derive(Debug)]
struct CallState {
    machine: CarrierCallMachine,
    stage: CallStage,
    input_root: PhysicalSlot,
    input_leaves: Vec<PhysicalSlot>,
    result_root: Option<PhysicalSlot>,
    result_leaves: Vec<PhysicalSlot>,
}

static NEXT_PROVIDER_TAG: AtomicU64 = AtomicU64::new(1);

/// The reference interpreter physical adapter for one opened provider. See
/// the module documentation for scope and the physical-model contrast with
/// [`super::native`] and [`super::wasm::provider::WasmProvider`].
#[derive(Debug)]
pub struct InterpreterProvider {
    tag: u64,
    heap: Heap,
    registry: HandleRegistry,
    calls: HashMap<u32, CallState>,
    next_generation: u32,
    /// Test-only: every trace ordinal currently armed to fail, each
    /// independently consumable. A plain `Option` cannot express issue
    /// #162's required "cleanup failure after an input/runtime failure"
    /// case: that needs a REAL earlier failure (e.g. `ExecutionStarted`)
    /// and the subsequent release-ordinal cleanup to ALSO be armed, in the
    /// same call, so the sticky rule has something genuine to discard.
    /// [`Self::test_inject_failure`] arms an ordinal by pushing it here
    /// rather than replacing the field, so every existing single-injection
    /// call site keeps its exact prior behavior (one entry in, one
    /// entry consumed).
    injected: Vec<TraceLabel>,
    settlement_overwrite_attempts: u32,
    /// Test-only: a snapshot of the most recently settled call's normalized
    /// trace, taken at the same moment `settle` runs. See
    /// `carrier::settlement_corpus` (issue #162): cross-engine comparison
    /// needs the trace `CarrierCallMachine` already records, not a second
    /// one, and mirrors `WasmProvider::test_last_trace` exactly.
    last_trace: Vec<crate::public_generic_abi::carrier::trace::TraceEvent>,
}

impl InterpreterProvider {
    /// Open a provider bound to `trusted_descriptor_bytes` and
    /// `trusted_binding`. `descriptor_bytes` and `carrier_binding_bytes`
    /// must byte-exactly replay those trusted values. `trusted_binding`
    /// must name [`TargetProfile::Interpreter`]; this constructor does not
    /// pick the target profile, callers do.
    pub fn open(
        descriptor_bytes: &[u8],
        trusted_descriptor_bytes: &[u8],
        carrier_binding_bytes: &[u8],
        trusted_binding: &CarrierBindingV1,
    ) -> Result<Self, InterpreterPgStatus> {
        if trusted_binding.target_profile() != TargetProfile::Interpreter {
            return Err(InterpreterPgStatus::MalformedBinding);
        }
        if descriptor_bytes != trusted_descriptor_bytes {
            return Err(InterpreterPgStatus::DescriptorReplayMismatch);
        }
        replay_binding(carrier_binding_bytes, trusted_binding)
            .map_err(|error| status_from_diagnostic(&error))?;
        Ok(Self {
            tag: NEXT_PROVIDER_TAG.fetch_add(1, Ordering::Relaxed),
            heap: Heap::new(),
            registry: HandleRegistry::new(),
            calls: HashMap::new(),
            next_generation: 1,
            injected: Vec::new(),
            settlement_overwrite_attempts: 0,
            last_trace: Vec::new(),
        })
    }

    /// Test-only: the normalized trace of the most recently settled call,
    /// captured at settlement time regardless of success or failure.
    pub fn test_last_trace(&self) -> &[crate::public_generic_abi::carrier::trace::TraceEvent] {
        &self.last_trace
    }

    pub fn live_handles(&self) -> usize {
        self.registry.len()
    }

    pub fn live_allocations(&self) -> u32 {
        self.heap.live_allocations()
    }

    pub fn live_bytes(&self) -> u32 {
        self.heap.live_bytes()
    }

    /// Test-only, never part of a support/publication claim. Arms
    /// deterministic failure at trace ordinal `label` for the next
    /// `input_prepare`/`call` sequence only; each armed ordinal disarms
    /// itself once it fires. Calling this more than once before a call
    /// arms every named ordinal simultaneously (issue #162's compound
    /// cleanup-after-an-earlier-failure cases) rather than replacing the
    /// previous one.
    pub fn test_inject_failure(&mut self, label: TraceLabel) {
        self.injected.push(label);
    }

    pub fn test_clear_failure_injection(&mut self) {
        self.injected.clear();
    }

    pub fn test_settlement_overwrite_attempts(&self) -> u32 {
        self.settlement_overwrite_attempts
    }

    fn take_injection_if(&mut self, label: TraceLabel) -> bool {
        if let Some(index) = self.injected.iter().position(|armed| *armed == label) {
            self.injected.remove(index);
            true
        } else {
            false
        }
    }

    fn settle(&mut self, machine: &mut CarrierCallMachine, outcome: Settlement) {
        if machine.settle(outcome).is_err() {
            self.settlement_overwrite_attempts += 1;
        }
        self.last_trace = machine.trace().events().to_vec();
    }

    fn fixture_endpoint(leaf: &[u8]) -> Vec<u8> {
        leaf.iter().rev().copied().collect()
    }

    fn release_input_physical(&mut self, state: &CallState) -> Result<(), Diagnostic> {
        for span in state.input_leaves.iter().rev() {
            self.heap.free(span.slot, span.len)?;
        }
        self.heap
            .free(state.input_root.slot, state.input_root.len)?;
        Ok(())
    }

    fn release_result_physical(&mut self, state: &CallState) -> Result<(), Diagnostic> {
        for span in state.result_leaves.iter().rev() {
            self.heap.free(span.slot, span.len)?;
        }
        if let Some(root) = state.result_root {
            self.heap.free(root.slot, root.len)?;
        }
        Ok(())
    }

    /// Validate carrier shape and physically allocate/copy every input
    /// leaf, driving [`CarrierCallMachine`] for the logical side and this
    /// provider's own registry/heap for the physical side. Control flow
    /// mirrors [`super::wasm::provider::WasmProvider::input_prepare`]
    /// exactly, ordinal for ordinal, so the same injected `TraceLabel`
    /// produces the same observable outcome on both adapters.
    pub fn input_prepare(
        &mut self,
        leaves: &[Vec<u8>],
    ) -> Result<InterpreterHandle, InterpreterPgStatus> {
        if leaves.len() > MAX_OWNED_LEAVES_PER_INSTANCE {
            return Err(InterpreterPgStatus::CarrierCapacity);
        }
        check_handle_capacity(leaves.len() + 1)
            .map_err(|_| InterpreterPgStatus::CarrierCapacity)?;
        for leaf in leaves {
            if leaf.len() > MAX_BYTES_PER_LEAF {
                return Err(InterpreterPgStatus::CarrierCapacity);
            }
        }

        let call_id = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(InterpreterPgStatus::AllocationFailure)?;

        let root_handle = Handle::root(call_id);
        let leaf_handles: Vec<Handle> = (0..leaves.len() as u32)
            .map(|index| Handle::leaf(index, call_id))
            .collect();
        let mut machine = CarrierCallMachine::new(root_handle, leaf_handles.clone());
        machine
            .validate()
            .map_err(|error| status_from_diagnostic(&error))?;

        if self.take_injection_if(TraceLabel::FrameValidated) {
            self.settle(&mut machine, Settlement::AllocationFailure);
            return Err(InterpreterPgStatus::AllocationFailure);
        }

        let mut allocated: Vec<PhysicalSlot> = Vec::with_capacity(leaves.len() + 1);
        let mut fail_after: Option<InterpreterPgStatus> = None;

        let root_slot = match self.heap.alloc(0) {
            Ok(slot) => slot,
            Err(_) => {
                self.settle(&mut machine, Settlement::AllocationFailure);
                return Err(InterpreterPgStatus::AllocationFailure);
            }
        };
        allocated.push(PhysicalSlot {
            slot: root_slot,
            len: 0,
        });

        for leaf in leaves {
            if self.take_injection_if(TraceLabel::LeafAllocationStarted) {
                fail_after = Some(InterpreterPgStatus::AllocationFailure);
                break;
            }
            let slot = match self.heap.alloc(leaf.len() as u32) {
                Ok(slot) => slot,
                Err(_) => {
                    fail_after = Some(InterpreterPgStatus::AllocationFailure);
                    break;
                }
            };
            if self.take_injection_if(TraceLabel::LeafAllocationCommitted) {
                allocated.push(PhysicalSlot {
                    slot,
                    len: leaf.len() as u32,
                });
                fail_after = Some(InterpreterPgStatus::AllocationFailure);
                break;
            }
            if self.heap.write(slot, leaf).is_err() {
                allocated.push(PhysicalSlot {
                    slot,
                    len: leaf.len() as u32,
                });
                fail_after = Some(InterpreterPgStatus::AllocationFailure);
                break;
            }
            allocated.push(PhysicalSlot {
                slot,
                len: leaf.len() as u32,
            });
            if self.take_injection_if(TraceLabel::LeafPayloadCopied) {
                fail_after = Some(InterpreterPgStatus::AllocationFailure);
                break;
            }
        }

        if fail_after.is_none() && self.take_injection_if(TraceLabel::InputValuePrepared) {
            fail_after = Some(InterpreterPgStatus::AllocationFailure);
        }

        if let Some(status) = fail_after {
            for span in allocated.iter().rev() {
                let _ = self.heap.free(span.slot, span.len);
            }
            self.settle(&mut machine, Settlement::AllocationFailure);
            return Err(status);
        }

        machine
            .prepare_input()
            .map_err(|error| status_from_diagnostic(&error))?;

        let input_root = allocated[0];
        let input_leaves = allocated[1..].to_vec();
        for (index, span) in input_leaves.iter().enumerate() {
            self.registry.insert(
                leaf_handles[index],
                RegistryEntry {
                    role: HandleRole::InputLeaf,
                    offset: span.slot,
                    len: span.len,
                },
            );
        }
        self.registry.insert(
            root_handle,
            RegistryEntry {
                role: HandleRole::InputRoot,
                offset: input_root.slot,
                len: input_root.len,
            },
        );
        self.calls.insert(
            call_id,
            CallState {
                machine,
                stage: CallStage::PendingValue,
                input_root,
                input_leaves,
                result_root: None,
                result_leaves: Vec::new(),
            },
        );
        Ok(InterpreterHandle {
            tag: self.tag,
            handle: root_handle,
        })
    }

    fn call_id_for(
        &self,
        value: InterpreterHandle,
        role: HandleRole,
    ) -> Result<u32, InterpreterPgStatus> {
        if value.tag != self.tag {
            return Err(InterpreterPgStatus::HandleInvalid);
        }
        self.registry
            .get(value.handle, role)
            .map_err(|error| status_from_diagnostic(&error))?;
        Ok(value.handle.generation)
    }

    /// Commit input transfer, run the bound fixture endpoint over every
    /// leaf, physically release the consumed input (always, success or
    /// failure — matching native's and Wasm's own shared-machine gap
    /// exactly), stage and commit the result, and settle. Control flow
    /// mirrors [`super::wasm::provider::WasmProvider::call`] exactly.
    pub fn call(
        &mut self,
        value: InterpreterHandle,
    ) -> Result<InterpreterHandle, InterpreterPgStatus> {
        let call_id = self.call_id_for(value, HandleRole::InputRoot)?;
        let mut state = self
            .calls
            .remove(&call_id)
            .ok_or(InterpreterPgStatus::HandleInvalid)?;
        if !matches!(state.stage, CallStage::PendingValue) {
            self.calls.insert(call_id, state);
            return Err(InterpreterPgStatus::NullOrWrongKind);
        }

        self.registry
            .remove(value.handle, HandleRole::InputRoot)
            .map_err(|error| status_from_diagnostic(&error))?;
        for index in 0..state.input_leaves.len() {
            let _ = self
                .registry
                .remove(Handle::leaf(index as u32, call_id), HandleRole::InputLeaf);
        }

        if let Err(error) = state.machine.commit_input_transfer() {
            self.settle(&mut state.machine, Settlement::ProviderFailure);
            let _ = self.release_input_physical(&state);
            let _ = state.machine.release_input_before_transfer();
            return Err(status_from_diagnostic(&error));
        }
        if self.take_injection_if(TraceLabel::InputTransferCommitted) {
            self.settle(&mut state.machine, Settlement::ProviderFailure);
            let _ = self.release_input_physical(&state);
            let _ = state.machine.release_input_after_transfer();
            return Err(InterpreterPgStatus::IllegalTransition);
        }

        if let Err(error) = state.machine.begin_execution() {
            self.settle(&mut state.machine, Settlement::ContractFailure);
            let _ = self.release_input_physical(&state);
            let _ = state.machine.release_input_after_transfer();
            return Err(status_from_diagnostic(&error));
        }
        if self.take_injection_if(TraceLabel::ExecutionStarted) {
            self.settle(&mut state.machine, Settlement::ContractFailure);
            let _ = self.release_input_physical(&state);
            let _ = state.machine.release_input_after_transfer();
            // Issue #162: "cleanup failure after an input/runtime
            // failure." `ExecutionStarted`'s own settle call above already
            // made `ContractFailure` sticky; if a release-ordinal
            // injection is ALSO armed for this same call, it is a
            // genuinely later, distinct attempt (`Settlement::CleanupFailure`,
            // not `Settlement::ContractFailure` again) that the sticky rule
            // must reject and count, never apply — this is the compound
            // case the corpus's own
            // `execution_failure_with_compounding_cleanup_injection` case
            // exercises, distinct from the pre-existing "cleanup failure
            // with no prior failure" case, which never reaches this
            // early-return site at all.
            if self.take_injection_if(TraceLabel::LeafRelease)
                || self.take_injection_if(TraceLabel::CarrierRelease)
            {
                self.settle(&mut state.machine, Settlement::CleanupFailure);
            }
            return Err(InterpreterPgStatus::ContractFailure);
        }

        let mut outputs: Vec<Vec<u8>> = Vec::with_capacity(state.input_leaves.len());
        for span in &state.input_leaves {
            let bytes = self
                .heap
                .read(span.slot)
                .map_err(|error| status_from_diagnostic(&error))?;
            outputs.push(Self::fixture_endpoint(bytes));
        }

        if let Err(error) = state.machine.finish_execution() {
            self.settle(&mut state.machine, Settlement::ContractFailure);
            let _ = self.release_input_physical(&state);
            let _ = state.machine.release_input_after_transfer();
            return Err(status_from_diagnostic(&error));
        }
        let injected_after_execution = self.take_injection_if(TraceLabel::ExecutionFinished);

        // The LOGICAL release (`release_input_after_transfer`) runs
        // alongside the physical free so the normalized trace this call
        // emits actually records the input side's release, on every
        // terminal path including success — matching the identical fix in
        // `WasmProvider::call`.
        let release_result = self.release_input_physical(&state);
        let _ = state.machine.release_input_after_transfer();
        if self.take_injection_if(TraceLabel::LeafRelease)
            || self.take_injection_if(TraceLabel::CarrierRelease)
        {
            self.settle(&mut state.machine, Settlement::CleanupFailure);
        }
        if let Err(error) = release_result {
            self.settle(&mut state.machine, Settlement::AllocationFailure);
            return Err(status_from_diagnostic(&error));
        }

        if injected_after_execution {
            self.settle(&mut state.machine, Settlement::ContractFailure);
            let outcome = state
                .machine
                .settlement()
                .unwrap_or(Settlement::ContractFailure);
            return Err(status_from_settlement(outcome));
        }
        if let Some(outcome) = state.machine.settlement() {
            return Err(status_from_settlement(outcome));
        }

        let leaf_count = outputs.len();
        let result_root_handle = result_root_handle(call_id);
        let result_leaf_handles: Vec<Handle> = (0..leaf_count as u32)
            .map(|index| result_leaf_handle(index, call_id))
            .collect();
        if state
            .machine
            .begin_result(result_root_handle, result_leaf_handles.clone())
            .is_err()
        {
            self.settle(&mut state.machine, Settlement::ContractFailure);
            return Err(InterpreterPgStatus::ContractFailure);
        }

        let mut result_allocated: Vec<PhysicalSlot> = Vec::with_capacity(leaf_count + 1);
        let result_root_slot = match self.heap.alloc(0) {
            Ok(slot) => slot,
            Err(_) => {
                self.settle(&mut state.machine, Settlement::AllocationFailure);
                return Err(InterpreterPgStatus::AllocationFailure);
            }
        };
        result_allocated.push(PhysicalSlot {
            slot: result_root_slot,
            len: 0,
        });

        let mut result_fail: Option<InterpreterPgStatus> = None;
        for output in &outputs {
            if self.take_injection_if(TraceLabel::ResultLeafAllocationStarted) {
                result_fail = Some(InterpreterPgStatus::AllocationFailure);
                break;
            }
            let slot = match self.heap.alloc(output.len() as u32) {
                Ok(slot) => slot,
                Err(_) => {
                    result_fail = Some(InterpreterPgStatus::AllocationFailure);
                    break;
                }
            };
            if self.heap.write(slot, output).is_err() {
                result_allocated.push(PhysicalSlot {
                    slot,
                    len: output.len() as u32,
                });
                result_fail = Some(InterpreterPgStatus::AllocationFailure);
                break;
            }
            result_allocated.push(PhysicalSlot {
                slot,
                len: output.len() as u32,
            });
            if self.take_injection_if(TraceLabel::ResultLeafAllocationCommitted) {
                result_fail = Some(InterpreterPgStatus::AllocationFailure);
                break;
            }
        }

        if result_fail.is_none() && self.take_injection_if(TraceLabel::ResultValuePrepared) {
            result_fail = Some(InterpreterPgStatus::ContractFailure);
        }

        if let Some(status) = result_fail {
            for span in result_allocated.iter().rev() {
                let _ = self.heap.free(span.slot, span.len);
            }
            let outcome = if matches!(status, InterpreterPgStatus::AllocationFailure) {
                Settlement::AllocationFailure
            } else {
                Settlement::ContractFailure
            };
            self.settle(&mut state.machine, outcome);
            let _ = state.machine.release_result_before_commit();
            // Not extended with a compounding cleanup-injection check like
            // the `ExecutionStarted` site above: `LeafRelease`/
            // `CarrierRelease` are already unconditionally checked at the
            // input-release site earlier in this same function (reached
            // by every call that gets this far), so arming either ordinal
            // before the call is consumed there first and never reaches
            // this block — see this module's settlement-corpus
            // documentation of that scope boundary.
            return Err(status);
        }

        state
            .machine
            .prepare_result()
            .map_err(|error| status_from_diagnostic(&error))?;

        if self.take_injection_if(TraceLabel::ResultCommit) {
            for span in result_allocated.iter().rev() {
                let _ = self.heap.free(span.slot, span.len);
            }
            self.settle(&mut state.machine, Settlement::ContractFailure);
            let _ = state.machine.release_result_before_commit();
            return Err(InterpreterPgStatus::ContractFailure);
        }

        state
            .machine
            .commit_result()
            .map_err(|error| status_from_diagnostic(&error))?;
        self.settle(&mut state.machine, Settlement::Success);

        let result_root = result_allocated[0];
        let result_leaves = result_allocated[1..].to_vec();
        for (index, span) in result_leaves.iter().enumerate() {
            self.registry.insert(
                result_leaf_handles[index],
                RegistryEntry {
                    role: HandleRole::ResultLeaf,
                    offset: span.slot,
                    len: span.len,
                },
            );
        }
        self.registry.insert(
            result_root_handle,
            RegistryEntry {
                role: HandleRole::ResultRoot,
                offset: result_root.slot,
                len: result_root.len,
            },
        );
        state.stage = CallStage::PendingResult;
        state.result_root = Some(result_root);
        state.result_leaves = result_leaves;
        self.calls.insert(call_id, state);
        Ok(InterpreterHandle {
            tag: self.tag,
            handle: result_root_handle,
        })
    }

    /// Two-pass, nonconsuming export, matching
    /// [`super::wasm::provider::WasmProvider::result_export`] exactly:
    /// `capacity == 0` (or too small) reports the exact required byte count
    /// without writing or consuming anything.
    pub fn result_export(
        &self,
        result: InterpreterHandle,
        capacity: usize,
    ) -> Result<Vec<u8>, InterpreterPgStatus> {
        if result.tag != self.tag {
            return Err(InterpreterPgStatus::HandleInvalid);
        }
        self.registry
            .get(result.handle, HandleRole::ResultRoot)
            .map_err(|error| status_from_diagnostic(&error))?;
        let state = self
            .calls
            .get(&result.handle.generation)
            .filter(|state| matches!(state.stage, CallStage::PendingResult))
            .ok_or(InterpreterPgStatus::HandleInvalid)?;

        let mut required = Vec::new();
        for span in &state.result_leaves {
            let bytes = self
                .heap
                .read(span.slot)
                .map_err(|error| status_from_diagnostic(&error))?;
            crate::public_generic_abi::frame(&mut required, bytes);
        }
        if capacity < required.len() {
            return Err(InterpreterPgStatus::BufferTooSmall);
        }
        Ok(required)
    }

    fn remove_call_physical(&mut self, call_id: u32) -> Result<CallState, InterpreterPgStatus> {
        self.calls
            .remove(&call_id)
            .ok_or(InterpreterPgStatus::HandleInvalid)
    }

    /// Release a value handle before it is ever passed to [`Self::call`]
    /// (an abandoned call).
    pub fn value_release(&mut self, value: InterpreterHandle) -> InterpreterPgStatus {
        if value.tag != self.tag {
            return InterpreterPgStatus::HandleInvalid;
        }
        if self
            .registry
            .remove(value.handle, HandleRole::InputRoot)
            .is_err()
        {
            return InterpreterPgStatus::HandleInvalid;
        }
        for index in 0.. {
            let leaf_handle = Handle::leaf(index, value.handle.generation);
            if self
                .registry
                .remove(leaf_handle, HandleRole::InputLeaf)
                .is_err()
            {
                break;
            }
        }
        let Ok(mut state) = self.remove_call_physical(value.handle.generation) else {
            return InterpreterPgStatus::HandleInvalid;
        };
        if state.machine.release_input_before_transfer().is_err() {
            return InterpreterPgStatus::IllegalTransition;
        }
        match self.release_input_physical(&state) {
            Ok(()) => {
                self.settle(&mut state.machine, Settlement::ConsumerRefusal);
                InterpreterPgStatus::Ok
            }
            Err(error) => status_from_diagnostic(&error),
        }
    }

    /// Release a fully exported (or not-yet-exported) result handle.
    pub fn result_release(&mut self, result: InterpreterHandle) -> InterpreterPgStatus {
        if result.tag != self.tag {
            return InterpreterPgStatus::HandleInvalid;
        }
        if self
            .registry
            .remove(result.handle, HandleRole::ResultRoot)
            .is_err()
        {
            return InterpreterPgStatus::HandleInvalid;
        }
        for index in 0.. {
            let leaf_handle = result_leaf_handle(index, result.handle.generation);
            if self
                .registry
                .remove(leaf_handle, HandleRole::ResultLeaf)
                .is_err()
            {
                break;
            }
        }
        let Ok(state) = self.remove_call_physical(result.handle.generation) else {
            return InterpreterPgStatus::HandleInvalid;
        };
        match self.release_result_physical(&state) {
            Ok(()) => InterpreterPgStatus::Ok,
            Err(error) => status_from_diagnostic(&error),
        }
    }

    /// Refuses to close with any live handle, matching
    /// [`super::native`]'s and [`super::wasm::provider::WasmProvider`]'s
    /// own `close` contract.
    pub fn close(self) -> InterpreterPgStatus {
        if self.registry.is_empty() && self.calls.is_empty() {
            InterpreterPgStatus::Ok
        } else {
            InterpreterPgStatus::IllegalTransition
        }
    }
}

#[cfg(test)]
mod tests;
