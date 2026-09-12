//! `WasmProvider`: the Core Wasm PHYSICAL adapter for [Public Generic
//! Carrier v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md) (issue #155),
//! driving [`CarrierCallMachine`] directly for every decision the shared
//! LOGICAL layer already owns — commit atomicity, phase ordering, sticky
//! settlement, exact-reverse release order — while this module owns exactly
//! the PHYSICAL facts the machine has no opinion on: where a handle's bytes
//! live in [`WasmLinearMemory`], when they are really allocated and really
//! released, and which numeric handle belongs to which provider.
//!
//! **Deferred scope**, exactly like [`crate::public_generic_abi::native`]'s
//! own native adapter: deriving a provider from a real checked *generic*
//! export needs #119's still-blocked owned-record ownership evidence, so
//! the bound endpoint here is the same fixture (byte-reversal per owned
//! leaf) operating on the same flat owned-`Bytes` shape, and the trusted
//! descriptor bytes an `open` caller replays against are a hand-constructed
//! fixture rather than [`crate::public_generic_abi::descriptor::verify`]
//! output — see [Public Generic Carrier
//! v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155)
//! for the full status table and the exact split between this Rust-hosted
//! adapter and [`super::probe`]'s Node-hosted real-`WebAssembly.Memory`
//! proof.
//!
//! **Known adapter-level gap, stated once here rather than hidden in
//! prose**: [`CarrierCallMachine`] exposes no "consume a successfully
//! transferred input on the call's success path" transition — only the
//! failure-shaped `release_input_after_transfer`. This adapter reuses that
//! same method unconditionally after execution finishes, success or
//! failure, exactly mirroring what
//! [`crate::public_generic_abi::native::provider_body`] (C, restated rather
//! than sharing this Rust type) does with its own `spx_pg_release_leaves`
//! helper: input bytes are always physically freed once the endpoint has
//! read them, regardless of outcome. This is the shared machine's own
//! documented scope gap, not something this physical adapter re-decides.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::boundary_profile::{
    MAX_BYTES_PER_LEAF, MAX_OWNED_LEAVES_PER_INSTANCE,
};
use crate::public_generic_abi::carrier::machine::CarrierCallMachine;
use crate::public_generic_abi::carrier::trace::TraceLabel;
use crate::public_generic_abi::carrier::{
    check_handle_capacity, Handle, Settlement, CARRIER_CAPACITY,
};
use crate::public_generic_abi::descriptor::{DESCRIPTOR_REPLAY_MISMATCH, MALFORMED_DESCRIPTOR};
use crate::public_generic_abi::wasm::binding::{
    replay_wasm_provider_binding, WasmProviderBindingV1, MALFORMED_WASM_BINDING,
    WASM_BINDING_REPLAY_MISMATCH,
};
use crate::public_generic_abi::wasm::memory::{StackAllocator, WasmLinearMemory};
use crate::public_generic_abi::wasm::registry::{HandleRegistry, HandleRole, RegistryEntry};

/// Result handles for one call live in a disjoint `id` range from that same
/// call's input handles, even though both share one `generation` (the call
/// is the one "carrier instance" [Public Generic Carrier
/// v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md) describes, encompassing
/// both directions). Without this offset, a call's result root would be
/// `Handle { id: 0, generation: call_id }` — byte-identical to that same
/// call's already-released input root — and this adapter's registry, keyed
/// on `Handle` alone, could not tell a stale input handle from the fresh
/// result handle that now occupies the identical key. Native's adapter
/// avoids this by using distinct pointer identities for value vs. result
/// objects; this Wasm adapter has no such natural per-role address space
/// and picks a disjoint `id` range instead — a physical-layer choice the
/// LOGICAL layer's `id` numbering (root `0`, leaves `1..=256`) leaves open
/// per direction.
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

/// The one fixture endpoint this round's adapter binds, matching
/// [`crate::public_generic_abi::native::binding`]'s own fixture endpoint
/// name convention for the equivalent C symbol.
pub const FIXTURE_ENDPOINT_EXPORT_NAME: &str = "spx_pg_wasm_endpoint_reverse_bytes_v1";

/// The closed, normalized status vocabulary this adapter returns. Every
/// integer value matches
/// [`crate::public_generic_abi::native::template::HEADER_V1`]'s
/// `spx_pg_status_v1` constants exactly (`OK` = 0 through
/// `NULL_OR_WRONG_KIND` = 13) — a deliberate convergence, not a shared
/// requirement (see [Public Generic Carrier
/// v1](../../../docs/PUBLIC-GENERIC-CARRIER-V1.md#logical-versus-physical):
/// physical layouts may differ), chosen because a caller-facing status
/// vocabulary the two physical adapters already happen to agree on is one
/// less translation issue 162's cross-engine comparison has to solve.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum WasmPgStatus {
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

fn status_from_diagnostic(error: &Diagnostic) -> WasmPgStatus {
    use crate::public_generic_abi::carrier::{
        HANDLE_GENERATION_MISMATCH, ILLEGAL_TRANSITION, MALFORMED_CARRIER,
        STICKY_SETTLEMENT_VIOLATION,
    };
    use crate::public_generic_abi::wasm::memory::{
        ALLOCATION_FAILURE as MEMORY_ALLOCATION_FAILURE, DEALLOC_NOT_TOP_OF_STACK, MEMORY_BOUNDS,
    };
    use crate::public_generic_abi::wasm::registry::{HANDLE_INVALID, WRONG_KIND};
    match error.code {
        MALFORMED_CARRIER => WasmPgStatus::MalformedCarrier,
        CARRIER_CAPACITY => WasmPgStatus::CarrierCapacity,
        ILLEGAL_TRANSITION => WasmPgStatus::IllegalTransition,
        HANDLE_GENERATION_MISMATCH => WasmPgStatus::HandleInvalid,
        STICKY_SETTLEMENT_VIOLATION => WasmPgStatus::StickySettlementViolation,
        MALFORMED_WASM_BINDING => WasmPgStatus::MalformedBinding,
        WASM_BINDING_REPLAY_MISMATCH => WasmPgStatus::BindingReplayMismatch,
        MEMORY_ALLOCATION_FAILURE | DEALLOC_NOT_TOP_OF_STACK | MEMORY_BOUNDS => {
            WasmPgStatus::AllocationFailure
        }
        HANDLE_INVALID => WasmPgStatus::HandleInvalid,
        WRONG_KIND => WasmPgStatus::NullOrWrongKind,
        MALFORMED_DESCRIPTOR => WasmPgStatus::MalformedDescriptor,
        DESCRIPTOR_REPLAY_MISMATCH => WasmPgStatus::DescriptorReplayMismatch,
        _ => WasmPgStatus::IllegalTransition,
    }
}

/// The same physical-status consolidation
/// [`crate::public_generic_abi::native::provider_body`] applies at each
/// injection site, restated here rather than reinvented: `ProviderFailure`
/// and every leaf/result-leaf/value-prepared allocation step maps to
/// `AllocationFailure`; execution and result-commit steps map to
/// `ContractFailure`; a release-time cleanup failure maps to
/// `ContractFailure` too, matching the exact site native's own adapter uses
/// it at.
fn status_from_settlement(settlement: Settlement) -> WasmPgStatus {
    match settlement {
        Settlement::Success => WasmPgStatus::Ok,
        Settlement::ProviderFailure => WasmPgStatus::IllegalTransition,
        Settlement::AllocationFailure => WasmPgStatus::AllocationFailure,
        Settlement::CopyOutFailure
        | Settlement::MalformedResult
        | Settlement::ContractFailure
        | Settlement::ConsumerRefusal
        | Settlement::CleanupFailure => WasmPgStatus::ContractFailure,
    }
}

/// An opaque handle this adapter hands back to a caller: an input value
/// handle from [`WasmProvider::input_prepare`], or a result handle from
/// [`WasmProvider::call`]. Fields are private; a caller may only hold this
/// and pass it back to this same [`WasmProvider`] instance. Hostile-handle
/// construction (wrong generation, wrong provider, wrong role) is exercised
/// entirely from inside this crate in [`tests`] — this type has no public
/// field or constructor for a foreign caller to forge one from raw parts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WasmHandle {
    tag: u64,
    handle: Handle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PhysicalSpan {
    offset: u32,
    len: u32,
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
    input_root: PhysicalSpan,
    input_leaves: Vec<PhysicalSpan>,
    result_root: Option<PhysicalSpan>,
    result_leaves: Vec<PhysicalSpan>,
}

static NEXT_PROVIDER_TAG: AtomicU64 = AtomicU64::new(1);

/// The Core Wasm physical adapter for one opened provider. See the module
/// documentation for scope and the shared-machine gap this adapter works
/// around identically to the native adapter.
#[derive(Debug)]
pub struct WasmProvider {
    tag: u64,
    memory: WasmLinearMemory,
    allocator: StackAllocator,
    registry: HandleRegistry,
    calls: HashMap<u32, CallState>,
    next_generation: u32,
    injected: Option<TraceLabel>,
    settlement_overwrite_attempts: u32,
    /// Test-only: a snapshot of the most recently settled call's normalized
    /// trace, taken at the same moment `settle` runs (every terminal path,
    /// success or failure). See `carrier::settlement_corpus` (issue #162),
    /// which is the reason this exists: cross-engine comparison needs the
    /// trace `CarrierCallMachine` already records, not a second one.
    last_trace: Vec<crate::public_generic_abi::carrier::trace::TraceEvent>,
}

impl WasmProvider {
    /// Open a provider bound to `trusted_descriptor_bytes` and
    /// `trusted_binding`. `descriptor_bytes` and `provider_binding_bytes`
    /// must byte-exactly replay those trusted values — the runtime half of
    /// the trust chain [`crate::public_generic_abi::descriptor::verify`]
    /// establishes once, independently, at generation time (deferred to a
    /// fixture this round; see the module documentation).
    pub fn open(
        descriptor_bytes: &[u8],
        trusted_descriptor_bytes: &[u8],
        provider_binding_bytes: &[u8],
        trusted_binding: &WasmProviderBindingV1,
    ) -> Result<Self, WasmPgStatus> {
        if descriptor_bytes != trusted_descriptor_bytes {
            return Err(WasmPgStatus::DescriptorReplayMismatch);
        }
        replay_wasm_provider_binding(provider_binding_bytes, trusted_binding)
            .map_err(|error| status_from_diagnostic(&error))?;
        Ok(Self {
            tag: NEXT_PROVIDER_TAG.fetch_add(1, Ordering::Relaxed),
            memory: WasmLinearMemory::new(),
            allocator: StackAllocator::new(),
            registry: HandleRegistry::new(),
            calls: HashMap::new(),
            next_generation: 1,
            injected: None,
            settlement_overwrite_attempts: 0,
            last_trace: Vec::new(),
        })
    }

    /// Test-only: the normalized trace of the most recently settled call,
    /// captured at settlement time regardless of success or failure.
    pub fn test_last_trace(&self) -> &[crate::public_generic_abi::carrier::trace::TraceEvent] {
        &self.last_trace
    }

    /// Live handle count: the exact-settlement counter a caller checks
    /// after every terminal case, success or failure.
    pub fn live_handles(&self) -> usize {
        self.registry.len()
    }

    /// Live allocation count and live byte count from the bounded
    /// allocator — exact counts, never samples.
    pub fn live_allocations(&self) -> u32 {
        self.allocator.live_allocations()
    }

    pub fn live_bytes(&self) -> u32 {
        self.allocator.live_bytes()
    }

    /// Test-only: not part of the production surface, never emitted for a
    /// support/publication claim (this whole module already carries that
    /// blanket claim). Arms deterministic failure at trace ordinal `label`
    /// for the next `input_prepare`/`call` sequence only; disarms itself
    /// once it fires.
    pub fn test_inject_failure(&mut self, label: TraceLabel) {
        self.injected = Some(label);
    }

    pub fn test_clear_failure_injection(&mut self) {
        self.injected = None;
    }

    pub fn test_settlement_overwrite_attempts(&self) -> u32 {
        self.settlement_overwrite_attempts
    }

    fn take_injection_if(&mut self, label: TraceLabel) -> bool {
        if self.injected == Some(label) {
            self.injected = None;
            true
        } else {
            false
        }
    }

    /// Attempt to settle `machine` at `outcome`. Sticky: a later, different
    /// outcome than one already selected is refused and counted, never
    /// applied — the direct proof that "cleanup cannot replace the selected
    /// status."
    fn settle(&mut self, machine: &mut CarrierCallMachine, outcome: Settlement) {
        if machine.settle(outcome).is_err() {
            self.settlement_overwrite_attempts += 1;
        }
        self.last_trace = machine.trace().events().to_vec();
    }

    fn fixture_endpoint(leaf: &[u8]) -> Vec<u8> {
        leaf.iter().rev().copied().collect()
    }

    /// Physically release one call's input spans in exact reverse
    /// (structural) order, zeroing every byte for real. `root` is always
    /// released last, matching the canonical obligation order.
    fn release_input_physical(&mut self, state: &CallState) -> Result<(), Diagnostic> {
        for span in state.input_leaves.iter().rev() {
            self.allocator
                .dealloc(&mut self.memory, span.offset, span.len)?;
        }
        self.allocator.dealloc(
            &mut self.memory,
            state.input_root.offset,
            state.input_root.len,
        )?;
        Ok(())
    }

    fn release_result_physical(&mut self, state: &CallState) -> Result<(), Diagnostic> {
        for span in state.result_leaves.iter().rev() {
            self.allocator
                .dealloc(&mut self.memory, span.offset, span.len)?;
        }
        if let Some(root) = state.result_root {
            self.allocator
                .dealloc(&mut self.memory, root.offset, root.len)?;
        }
        Ok(())
    }

    /// Validate carrier shape and physically allocate/copy every input
    /// leaf, driving [`CarrierCallMachine`] for the logical side and this
    /// provider's own registry/allocator for the physical side. Registers
    /// the resulting value handle and returns it on success.
    pub fn input_prepare(&mut self, leaves: &[Vec<u8>]) -> Result<WasmHandle, WasmPgStatus> {
        if leaves.len() > MAX_OWNED_LEAVES_PER_INSTANCE {
            return Err(WasmPgStatus::CarrierCapacity);
        }
        check_handle_capacity(leaves.len() + 1).map_err(|_| WasmPgStatus::CarrierCapacity)?;
        for leaf in leaves {
            if leaf.len() > MAX_BYTES_PER_LEAF {
                return Err(WasmPgStatus::CarrierCapacity);
            }
        }

        let call_id = self.next_generation;
        self.next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(WasmPgStatus::AllocationFailure)?;

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
            return Err(WasmPgStatus::AllocationFailure);
        }

        // Physical allocation, root then leaves, structural order — real
        // allocation and real copy-in, checked before any logical Fill.
        let mut allocated: Vec<PhysicalSpan> = Vec::with_capacity(leaves.len() + 1);
        let mut fail_after: Option<WasmPgStatus> = None;

        let root_offset = match self.allocator.alloc(&mut self.memory, 0) {
            Ok(offset) => offset,
            Err(_) => {
                self.settle(&mut machine, Settlement::AllocationFailure);
                return Err(WasmPgStatus::AllocationFailure);
            }
        };
        allocated.push(PhysicalSpan {
            offset: root_offset,
            len: 0,
        });

        for leaf in leaves {
            if self.take_injection_if(TraceLabel::LeafAllocationStarted) {
                fail_after = Some(WasmPgStatus::AllocationFailure);
                break;
            }
            let offset = match self.allocator.alloc(&mut self.memory, leaf.len() as u32) {
                Ok(offset) => offset,
                Err(_) => {
                    fail_after = Some(WasmPgStatus::AllocationFailure);
                    break;
                }
            };
            if self.take_injection_if(TraceLabel::LeafAllocationCommitted) {
                allocated.push(PhysicalSpan {
                    offset,
                    len: leaf.len() as u32,
                });
                fail_after = Some(WasmPgStatus::AllocationFailure);
                break;
            }
            if self.memory.write_at(offset, leaf).is_err() {
                allocated.push(PhysicalSpan {
                    offset,
                    len: leaf.len() as u32,
                });
                fail_after = Some(WasmPgStatus::AllocationFailure);
                break;
            }
            allocated.push(PhysicalSpan {
                offset,
                len: leaf.len() as u32,
            });
            if self.take_injection_if(TraceLabel::LeafPayloadCopied) {
                fail_after = Some(WasmPgStatus::AllocationFailure);
                break;
            }
        }

        if fail_after.is_none() && self.take_injection_if(TraceLabel::InputValuePrepared) {
            fail_after = Some(WasmPgStatus::AllocationFailure);
        }

        if let Some(status) = fail_after {
            for span in allocated.iter().rev() {
                let _ = self
                    .allocator
                    .dealloc(&mut self.memory, span.offset, span.len);
            }
            self.settle(&mut machine, Settlement::AllocationFailure);
            return Err(status);
        }

        // Every physical span exists; the logical fill can only succeed.
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
                    offset: span.offset,
                    len: span.len,
                },
            );
        }
        self.registry.insert(
            root_handle,
            RegistryEntry {
                role: HandleRole::InputRoot,
                offset: input_root.offset,
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
        Ok(WasmHandle {
            tag: self.tag,
            handle: root_handle,
        })
    }

    fn call_id_for(&self, value: WasmHandle, role: HandleRole) -> Result<u32, WasmPgStatus> {
        if value.tag != self.tag {
            return Err(WasmPgStatus::HandleInvalid);
        }
        self.registry
            .get(value.handle, role)
            .map_err(|error| status_from_diagnostic(&error))?;
        Ok(value.handle.generation)
    }

    /// Commit input transfer, run the bound fixture endpoint over every
    /// leaf, physically release the consumed input (always, success or
    /// failure — see the module documentation), stage and commit the
    /// result, and settle. Returns the result handle on success.
    pub fn call(&mut self, value: WasmHandle) -> Result<WasmHandle, WasmPgStatus> {
        let call_id = self.call_id_for(value, HandleRole::InputRoot)?;
        let mut state = self
            .calls
            .remove(&call_id)
            .ok_or(WasmPgStatus::HandleInvalid)?;
        if !matches!(state.stage, CallStage::PendingValue) {
            self.calls.insert(call_id, state);
            return Err(WasmPgStatus::NullOrWrongKind);
        }

        // Input side is removed from the registry as part of the one
        // atomic commit point, before any byte is read — the exact analog
        // of "transfer invalidates caller ownership exactly once."
        self.registry
            .remove(value.handle, HandleRole::InputRoot)
            .map_err(|error| status_from_diagnostic(&error))?;
        for (index, _) in state.input_leaves.iter().enumerate() {
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
            return Err(WasmPgStatus::IllegalTransition);
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
            return Err(WasmPgStatus::ContractFailure);
        }

        let mut outputs: Vec<Vec<u8>> = Vec::with_capacity(state.input_leaves.len());
        for span in &state.input_leaves {
            let bytes = self
                .memory
                .read_at(span.offset, span.len)
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

        // Non-result obligation: release the consumed input before the
        // result is ever published, success or failure — restating
        // native's own `spx_pg_release_leaves` call site exactly, and the
        // one place the two release-ordinal injection points
        // (`LeafRelease`, `CarrierRelease`) are exercised on every call.
        // The physical free always happens (`release_input_physical` above
        // already ran unconditionally); injection at either release
        // ordinal only ever contributes a *cleanup* outcome, which the
        // sticky rule accepts only if nothing failed earlier and discards
        // otherwise — mirroring native's own release-ordinal injection
        // sites exactly. The LOGICAL release (`release_input_after_transfer`)
        // runs alongside the physical free so the normalized trace this
        // call emits actually records the input side's release, on every
        // terminal path including success — without this, `machine.trace()`
        // would never carry a `LeafRelease`/`CarrierRelease` event for the
        // input handles at all, which issue #162's cross-engine corpus
        // caught by comparing the literal trace, not merely accept/reject.
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
            // A cleanup failure above already became sticky with nothing
            // earlier selected; the call fails with that outcome instead
            // of proceeding to publish a result.
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
            return Err(WasmPgStatus::ContractFailure);
        }

        let mut result_allocated: Vec<PhysicalSpan> = Vec::with_capacity(leaf_count + 1);
        let result_root_offset = match self.allocator.alloc(&mut self.memory, 0) {
            Ok(offset) => offset,
            Err(_) => {
                self.settle(&mut state.machine, Settlement::AllocationFailure);
                return Err(WasmPgStatus::AllocationFailure);
            }
        };
        result_allocated.push(PhysicalSpan {
            offset: result_root_offset,
            len: 0,
        });

        let mut result_fail: Option<WasmPgStatus> = None;
        for output in &outputs {
            if self.take_injection_if(TraceLabel::ResultLeafAllocationStarted) {
                result_fail = Some(WasmPgStatus::AllocationFailure);
                break;
            }
            let offset = match self.allocator.alloc(&mut self.memory, output.len() as u32) {
                Ok(offset) => offset,
                Err(_) => {
                    result_fail = Some(WasmPgStatus::AllocationFailure);
                    break;
                }
            };
            if self.memory.write_at(offset, output).is_err() {
                result_allocated.push(PhysicalSpan {
                    offset,
                    len: output.len() as u32,
                });
                result_fail = Some(WasmPgStatus::AllocationFailure);
                break;
            }
            result_allocated.push(PhysicalSpan {
                offset,
                len: output.len() as u32,
            });
            if self.take_injection_if(TraceLabel::ResultLeafAllocationCommitted) {
                result_fail = Some(WasmPgStatus::AllocationFailure);
                break;
            }
        }

        if result_fail.is_none() && self.take_injection_if(TraceLabel::ResultValuePrepared) {
            result_fail = Some(WasmPgStatus::ContractFailure);
        }

        if let Some(status) = result_fail {
            for span in result_allocated.iter().rev() {
                let _ = self
                    .allocator
                    .dealloc(&mut self.memory, span.offset, span.len);
            }
            let outcome = if matches!(status, WasmPgStatus::AllocationFailure) {
                Settlement::AllocationFailure
            } else {
                Settlement::ContractFailure
            };
            self.settle(&mut state.machine, outcome);
            let _ = state.machine.release_result_before_commit();
            return Err(status);
        }

        state
            .machine
            .prepare_result()
            .map_err(|error| status_from_diagnostic(&error))?;

        if self.take_injection_if(TraceLabel::ResultCommit) {
            for span in result_allocated.iter().rev() {
                let _ = self
                    .allocator
                    .dealloc(&mut self.memory, span.offset, span.len);
            }
            self.settle(&mut state.machine, Settlement::ContractFailure);
            let _ = state.machine.release_result_before_commit();
            return Err(WasmPgStatus::ContractFailure);
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
                    offset: span.offset,
                    len: span.len,
                },
            );
        }
        self.registry.insert(
            result_root_handle,
            RegistryEntry {
                role: HandleRole::ResultRoot,
                offset: result_root.offset,
                len: result_root.len,
            },
        );
        state.stage = CallStage::PendingResult;
        state.result_root = Some(result_root);
        state.result_leaves = result_leaves;
        self.calls.insert(call_id, state);
        Ok(WasmHandle {
            tag: self.tag,
            handle: result_root_handle,
        })
    }

    /// Two-pass, nonconsuming export, exactly like
    /// [`crate::public_generic_abi::native`]'s `spx_pg_result_export_v1`:
    /// `capacity == 0` (or too small) reports the exact required byte count
    /// without writing or consuming anything; a large-enough capacity
    /// returns the canonical, byte-identical-on-repeat carrier bytes.
    pub fn result_export(
        &self,
        result: WasmHandle,
        capacity: usize,
    ) -> Result<Vec<u8>, WasmPgStatus> {
        if result.tag != self.tag {
            return Err(WasmPgStatus::HandleInvalid);
        }
        self.registry
            .get(result.handle, HandleRole::ResultRoot)
            .map_err(|error| status_from_diagnostic(&error))?;
        let state = self
            .calls
            .get(&result.handle.generation)
            .filter(|state| matches!(state.stage, CallStage::PendingResult))
            .ok_or(WasmPgStatus::HandleInvalid)?;

        let mut required = Vec::new();
        for span in &state.result_leaves {
            let bytes = self
                .memory
                .read_at(span.offset, span.len)
                .map_err(|error| status_from_diagnostic(&error))?;
            crate::public_generic_abi::frame(&mut required, bytes);
        }
        if capacity < required.len() {
            return Err(WasmPgStatus::BufferTooSmall);
        }
        Ok(required)
    }

    fn remove_call_physical(&mut self, call_id: u32) -> Result<CallState, WasmPgStatus> {
        self.calls
            .remove(&call_id)
            .ok_or(WasmPgStatus::HandleInvalid)
    }

    /// Release a value handle before it is ever passed to [`Self::call`]
    /// (an abandoned call). A handle already consumed by `call`, or already
    /// released, is simply not live and fails closed with
    /// [`WasmPgStatus::HandleInvalid`].
    pub fn value_release(&mut self, value: WasmHandle) -> WasmPgStatus {
        if value.tag != self.tag {
            return WasmPgStatus::HandleInvalid;
        }
        if self
            .registry
            .remove(value.handle, HandleRole::InputRoot)
            .is_err()
        {
            return WasmPgStatus::HandleInvalid;
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
            return WasmPgStatus::HandleInvalid;
        };
        if state.machine.release_input_before_transfer().is_err() {
            return WasmPgStatus::IllegalTransition;
        }
        match self.release_input_physical(&state) {
            Ok(()) => {
                self.settle(&mut state.machine, Settlement::ConsumerRefusal);
                WasmPgStatus::Ok
            }
            Err(error) => status_from_diagnostic(&error),
        }
    }

    /// Release a fully exported (or not-yet-exported) result handle. Plain
    /// physical release: the owning call already reached its terminal
    /// settlement by the time a result handle exists, so nothing here
    /// drives `CarrierCallMachine` further.
    pub fn result_release(&mut self, result: WasmHandle) -> WasmPgStatus {
        if result.tag != self.tag {
            return WasmPgStatus::HandleInvalid;
        }
        if self
            .registry
            .remove(result.handle, HandleRole::ResultRoot)
            .is_err()
        {
            return WasmPgStatus::HandleInvalid;
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
            return WasmPgStatus::HandleInvalid;
        };
        match self.release_result_physical(&state) {
            Ok(()) => WasmPgStatus::Ok,
            Err(error) => status_from_diagnostic(&error),
        }
    }

    /// Refuses to close with any live handle, matching
    /// [`crate::public_generic_abi::native`]'s `spx_pg_provider_close_v1`.
    pub fn close(self) -> WasmPgStatus {
        if self.registry.is_empty() && self.calls.is_empty() {
            WasmPgStatus::Ok
        } else {
            WasmPgStatus::IllegalTransition
        }
    }
}

#[cfg(test)]
mod tests;
