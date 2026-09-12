//! Reference implementation of [Public Generic Carrier
//! v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md): the pure logical value
//! state machine, the call phase ledger with sticky failure selection, exact
//! reverse-order release verification, and a `CarrierBindingV1` wire codec.
//!
//! Nothing here allocates, transfers, or releases a real value: every type
//! is a pure state machine or a byte codec, exercised only against
//! hand-constructed fixtures in tests. There is no provider, no native or
//! Wasm adapter, and no execution — that is PG-7's remaining work
//! (issues #154-#159), out of scope this round.

use crate::diagnostic::Diagnostic;
use crate::public_generic_abi::boundary_profile::MAX_LIVE_HANDLES;
use crate::public_generic_abi::{digest, frame, read_frame};

/// [Section B canonical carrier
/// bytes](../../docs/PUBLIC-GENERIC-CARRIER-V1.md#canonical-carrier-bytes):
/// the bounded, self-digested, leaf-payload-bearing wire frame and the plan
/// that validates one against a trusted `VerifiedPublicGenericDescriptor`-
/// derived binding. See [`frame::LogicalCarrierFrame`] and
/// [`frame::CarrierFrameBinding`].
pub mod frame;
/// The call-level orchestration that ties the state machine below and the
/// phase ledger together with a normalized event trace. See
/// [`machine::CarrierCallMachine`].
pub mod machine;
/// The engine-neutral normalized trace vocabulary. See [`trace::Trace`].
pub mod trace;

/// The versioned carrier schema.
pub const CARRIER_SCHEMA: &str = "semaprax.public-generic-carrier.v1";

const BINDING_DOMAIN: &[u8] = b"semaprax.public-generic-carrier.v1.binding\0";

/// Malformed carrier-binding bytes: framing, an unknown target profile, or
/// an unknown schema.
pub const MALFORMED_CARRIER: &str = "SPX-PG801";
/// A carrier bound was reached (handle count, byte total, or a framed
/// field's length).
pub const CARRIER_CAPACITY: &str = "SPX-PG802";
/// Independent replay found the recomputed binding preimage does not equal
/// the submitted one.
pub const CARRIER_REPLAY_MISMATCH: &str = "SPX-PG803";
/// An illegal state transition, or a submitted release order that is not
/// the exact reverse of the canonical obligation order, or an operation on
/// an `Invalid` handle.
pub const ILLEGAL_TRANSITION: &str = "SPX-PG804";
/// A handle presented with the wrong generation, or against a different
/// carrier/descriptor binding than the one it was minted for.
pub const HANDLE_GENERATION_MISMATCH: &str = "SPX-PG805";
/// An attempt to replace an already-sticky `Settled` outcome with a
/// different one.
pub const STICKY_SETTLEMENT_VIOLATION: &str = "SPX-PG806";

// ---------------------------------------------------------------------
// Handles
// ---------------------------------------------------------------------

/// An opaque, generation-scoped handle. `id` 0 is always the root aggregate
/// handle; `1..=256` are owned-leaf handles in the settlement plan's own
/// structural order.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct Handle {
    pub id: u32,
    pub generation: u32,
}

impl Handle {
    pub const ROOT_ID: u32 = 0;

    pub fn root(generation: u32) -> Self {
        Self {
            id: Self::ROOT_ID,
            generation,
        }
    }

    pub fn leaf(index: u32, generation: u32) -> Self {
        Self {
            id: index + 1,
            generation,
        }
    }
}

/// Require `handle` to belong to `generation`. A stale or forged handle from
/// a different carrier instance fails closed rather than being silently
/// accepted.
pub fn verify_generation(handle: Handle, generation: u32) -> Result<(), Diagnostic> {
    if handle.generation != generation {
        return Err(Diagnostic::io(
            HANDLE_GENERATION_MISMATCH,
            format!(
                "{CARRIER_SCHEMA}: handle {} generation {} does not match carrier generation {generation}",
                handle.id, handle.generation
            ),
        ));
    }
    Ok(())
}

/// Reject a handle count over [`MAX_LIVE_HANDLES`].
pub fn check_handle_capacity(count: usize) -> Result<(), Diagnostic> {
    if count > MAX_LIVE_HANDLES {
        return Err(Diagnostic::io(
            CARRIER_CAPACITY,
            format!("{CARRIER_SCHEMA} exceeded its live-handle limit of {MAX_LIVE_HANDLES}"),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------
// The logical value state machine
// ---------------------------------------------------------------------

/// The state of one handle (root or leaf).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum CarrierState {
    Created,
    Initialized,
    Transferred,
    Borrowed,
    Consumed,
    Released,
    /// Terminal, absorbing. Reached only by an illegal transition attempt;
    /// there is no event that legally targets it.
    Invalid,
}

/// One driven event against a handle's state machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Event {
    /// `Created -> Initialized`: the provider fills storage.
    Fill,
    /// `Initialized -> Transferred`: the call's one commit point.
    Commit,
    /// `Transferred -> Borrowed`: a read-only loan begins.
    BeginLoan,
    /// `Borrowed -> Transferred`: the loan ends.
    EndLoan,
    /// `Transferred -> Consumed`: the final owning read / copy-out.
    Consume,
    /// `Consumed -> Released`: ordinary discharge after a successful
    /// transfer.
    Discharge,
    /// `Created | Initialized -> Released`: provider-side failure before any
    /// transfer occurred.
    ReleaseBeforeTransfer,
    /// `Transferred -> Released`: consumer-side failure or refusal after
    /// transfer, without a `Consumed` step.
    ReleaseAfterTransfer,
}

fn illegal(state: CarrierState, event: Event) -> Diagnostic {
    Diagnostic::io(
        ILLEGAL_TRANSITION,
        format!("{CARRIER_SCHEMA}: {event:?} is not legal from {state:?}"),
    )
}

/// The pure transition function. Every pair not listed in [Public Generic
/// Carrier v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md#the-logical-value-state-machine)'s
/// table is illegal, including every transition attempted from `Invalid`.
pub fn transition(state: CarrierState, event: Event) -> Result<CarrierState, Diagnostic> {
    use CarrierState::*;
    use Event::*;
    match (state, event) {
        (Created, Fill) => Ok(Initialized),
        (Initialized, Commit) => Ok(Transferred),
        (Transferred, BeginLoan) => Ok(Borrowed),
        (Borrowed, EndLoan) => Ok(Transferred),
        (Transferred, Consume) => Ok(Consumed),
        (Consumed, Discharge) => Ok(Released),
        (Created, ReleaseBeforeTransfer) | (Initialized, ReleaseBeforeTransfer) => Ok(Released),
        (Transferred, ReleaseAfterTransfer) => Ok(Released),
        _ => Err(illegal(state, event)),
    }
}

/// One handle's own state, driven through [`transition`]. On any illegal
/// event the ledger latches to `Invalid` and stays there: `Invalid` has no
/// legal outgoing transition, so every later event on this ledger fails
/// closed too.
#[derive(Clone, Debug)]
pub struct HandleLedger {
    handle: Handle,
    state: CarrierState,
}

impl HandleLedger {
    pub fn new(handle: Handle) -> Self {
        Self {
            handle,
            state: CarrierState::Created,
        }
    }

    pub fn handle(&self) -> Handle {
        self.handle
    }

    pub fn state(&self) -> CarrierState {
        self.state
    }

    pub fn apply(&mut self, event: Event) -> Result<CarrierState, Diagnostic> {
        match transition(self.state, event) {
            Ok(next) => {
                self.state = next;
                Ok(next)
            }
            Err(error) => {
                self.state = CarrierState::Invalid;
                Err(error)
            }
        }
    }
}

// ---------------------------------------------------------------------
// The call phase ledger
// ---------------------------------------------------------------------

/// One whole call's non-committing preparation phase.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Phase {
    Preparing,
    Validated,
    Committed,
}

/// A sticky terminal settlement outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Settlement {
    Success,
    ProviderFailure,
    AllocationFailure,
    CopyOutFailure,
    MalformedResult,
    ContractFailure,
    ConsumerRefusal,
    CleanupFailure,
}

/// The phase ledger for one call: `Preparing -> Validated -> Committed`,
/// then exactly one sticky `Settlement`.
#[derive(Clone, Debug)]
pub struct CallLedger {
    phase: Phase,
    settlement: Option<Settlement>,
}

impl Default for CallLedger {
    fn default() -> Self {
        Self::new()
    }
}

impl CallLedger {
    pub fn new() -> Self {
        Self {
            phase: Phase::Preparing,
            settlement: None,
        }
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn settlement(&self) -> Option<Settlement> {
        self.settlement
    }

    /// Advance to the next non-committing phase. `Preparing -> Validated`
    /// and `Validated -> Committed` are the only legal advances; anything
    /// else, including advancing after settlement, is illegal.
    pub fn advance(&mut self, next: Phase) -> Result<(), Diagnostic> {
        let legal = matches!(
            (self.phase, next),
            (Phase::Preparing, Phase::Validated) | (Phase::Validated, Phase::Committed)
        );
        if !legal {
            return Err(Diagnostic::io(
                ILLEGAL_TRANSITION,
                format!(
                    "{CARRIER_SCHEMA}: {next:?} does not legally follow {:?}",
                    self.phase
                ),
            ));
        }
        self.phase = next;
        Ok(())
    }

    /// Select the terminal settlement outcome. Sticky: once set, a
    /// different outcome is rejected. Reasserting the identical outcome is
    /// idempotent, since it is not a replacement.
    pub fn settle(&mut self, outcome: Settlement) -> Result<(), Diagnostic> {
        match self.settlement {
            None => {
                self.settlement = Some(outcome);
                Ok(())
            }
            Some(existing) if existing == outcome => Ok(()),
            Some(_) => Err(Diagnostic::io(
                STICKY_SETTLEMENT_VIOLATION,
                format!(
                    "{CARRIER_SCHEMA}: the settlement outcome is already selected and is sticky"
                ),
            )),
        }
    }
}

// ---------------------------------------------------------------------
// Release order
// ---------------------------------------------------------------------

/// Require `submitted` to be exactly the reverse of `obligation_order` — the
/// canonical structural order a [Public Generic Settlement Obligations
/// v1](../../docs/PUBLIC-GENERIC-SETTLEMENT-V1.md) plan already fixes.
/// Releasing the same set of handles in any other order fails closed.
pub fn verify_release_order(
    obligation_order: &[Handle],
    submitted: &[Handle],
) -> Result<(), Diagnostic> {
    let expected: Vec<Handle> = obligation_order.iter().rev().copied().collect();
    if submitted != expected.as_slice() {
        return Err(Diagnostic::io(
            ILLEGAL_TRANSITION,
            format!(
                "{CARRIER_SCHEMA}: the release order is not the exact reverse of the canonical obligation order"
            ),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Carrier binding wire codec
// ---------------------------------------------------------------------

/// The closed set of targets a carrier binding may name in this version.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetProfile {
    Interpreter,
    NativeC11,
    CoreWasm,
}

impl TargetProfile {
    fn text(self) -> &'static str {
        match self {
            Self::Interpreter => "interpreter",
            Self::NativeC11 => "native-c11",
            Self::CoreWasm => "core-wasm",
        }
    }

    fn from_text(text: &str) -> Option<Self> {
        Some(match text {
            "interpreter" => Self::Interpreter,
            "native-c11" => Self::NativeC11,
            "core-wasm" => Self::CoreWasm,
            _ => return None,
        })
    }
}

/// One `CarrierBindingV1` value: which descriptor, which target, which
/// carrier schema version, and which opaque runtime identity a carrier
/// instance is bound to.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CarrierBindingV1 {
    schema: String,
    descriptor_identity_digest: String,
    target_profile: TargetProfile,
    runtime_identity: String,
}

impl CarrierBindingV1 {
    pub fn new(
        descriptor_identity_digest: impl Into<String>,
        target_profile: TargetProfile,
        runtime_identity: impl Into<String>,
    ) -> Self {
        Self {
            schema: CARRIER_SCHEMA.to_owned(),
            descriptor_identity_digest: descriptor_identity_digest.into(),
            target_profile,
            runtime_identity: runtime_identity.into(),
        }
    }

    pub fn descriptor_identity_digest(&self) -> &str {
        &self.descriptor_identity_digest
    }

    pub fn target_profile(&self) -> TargetProfile {
        self.target_profile
    }

    pub fn runtime_identity(&self) -> &str {
        &self.runtime_identity
    }

    fn preimage(&self) -> Vec<u8> {
        let mut preimage = Vec::new();
        frame(&mut preimage, self.schema.as_bytes());
        frame(&mut preimage, self.descriptor_identity_digest.as_bytes());
        frame(&mut preimage, self.target_profile.text().as_bytes());
        frame(&mut preimage, self.runtime_identity.as_bytes());
        preimage
    }

    /// The domain-separated binding digest. Never transmitted; always
    /// recomputed by [`replay_binding`].
    pub fn binding_digest(&self) -> String {
        digest(BINDING_DOMAIN, &self.preimage())
    }

    /// Canonical wire bytes. The binding carries no presentation field, so
    /// `encode` and the identity preimage are identical.
    pub fn encode(&self) -> Vec<u8> {
        self.preimage()
    }
}

const MAX_BINDING_FIELD_BYTES: usize = 64 * 1024;
const MAX_BINDING_WIRE_BYTES: usize = 256 * 1024;

fn malformed_binding(subject: &str) -> Diagnostic {
    Diagnostic::io(
        MALFORMED_CARRIER,
        format!("not a canonical {CARRIER_SCHEMA} binding: {subject}"),
    )
}

/// Parse well-formed wire bytes into a [`CarrierBindingV1`]. Framing and
/// schema/target-profile validity only; binding validation against a
/// trusted context is [`replay_binding`]'s job.
pub fn decode_binding(bytes: &[u8]) -> Result<CarrierBindingV1, Diagnostic> {
    if bytes.len() > MAX_BINDING_WIRE_BYTES {
        return Err(Diagnostic::io(
            CARRIER_CAPACITY,
            format!("{CARRIER_SCHEMA} exceeded its total wire-byte bound"),
        ));
    }
    let mut offset = 0usize;
    let next = |name: &'static str, offset: &mut usize| -> Result<String, Diagnostic> {
        let (field, next_offset) = read_frame(bytes, *offset, MAX_BINDING_FIELD_BYTES)
            .ok_or_else(|| malformed_binding(&format!("truncated or oversized {name} field")))?;
        *offset = next_offset;
        String::from_utf8(field.to_vec())
            .map_err(|_| malformed_binding(&format!("{name} is not UTF-8")))
    };

    let schema = next("schema", &mut offset)?;
    if schema != CARRIER_SCHEMA {
        return Err(malformed_binding("unknown carrier schema"));
    }
    let descriptor_identity_digest = next("descriptor_identity_digest", &mut offset)?;
    let target_profile_text = next("target_profile", &mut offset)?;
    let target_profile = TargetProfile::from_text(&target_profile_text)
        .ok_or_else(|| malformed_binding("unknown target profile"))?;
    let runtime_identity = next("runtime_identity", &mut offset)?;

    if offset != bytes.len() {
        return Err(malformed_binding(
            "trailing bytes after the carrier binding",
        ));
    }

    Ok(CarrierBindingV1 {
        schema,
        descriptor_identity_digest,
        target_profile,
        runtime_identity,
    })
}

/// Decode `candidate` and require its preimage to equal `trusted`'s,
/// byte-for-byte. A binding for one descriptor, target, or runtime
/// generation is never accepted against another.
pub fn replay_binding(
    candidate: &[u8],
    trusted: &CarrierBindingV1,
) -> Result<CarrierBindingV1, Diagnostic> {
    let decoded = decode_binding(candidate)?;
    if decoded.preimage() != trusted.preimage() {
        return Err(Diagnostic::io(
            CARRIER_REPLAY_MISMATCH,
            format!("{CARRIER_SCHEMA} independent replay does not match the trusted value"),
        ));
    }
    Ok(decoded)
}

#[cfg(test)]
mod tests;

/// The shared cross-engine settlement corpus (issue #162): one case table,
/// the interpreter and Core Wasm physical adapters each run against every
/// case, and one checker that diffs both against an independently pinned
/// expectation and against each other. See the module's own doc comment.
#[cfg(test)]
mod settlement_corpus;
