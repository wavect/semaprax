//! The Core Wasm PHYSICAL adapter for [Public Generic Carrier
//! v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md) (issue #155), the second of
//! PG-7's per-target adapters after [`super::native`] (issue #154). This
//! module implements layout, allocation, and release for a Wasm linear
//! memory target; it does not re-decide legality that
//! [`super::carrier::machine::CarrierCallMachine`] and
//! [`super::descriptor::verify`] already own — see [Public Generic Carrier
//! v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md#logical-versus-physical)'s
//! LOGICAL/PHYSICAL split.
//!
//! Wasm linear memory is not native's pointer/registry world: there are no
//! raw addresses a foreign caller could forge into a dereferenceable
//! pointer, only `u32` byte offsets into one bounded, growable byte arena.
//! [`memory`] models that arena and its allocator; [`registry`] is the
//! host-side handle-safety bookkeeping Wasm itself has no built-in concept
//! of (mirroring, not reinventing, the same "scan the registry before
//! dereferencing" pattern [`super::native`] uses for its pointer world);
//! [`provider`] ties both to [`super::carrier::machine::CarrierCallMachine`]
//! for one whole call, exactly as [`super::native`]'s C provider does for
//! its own physical target. [`binding`] is the physical-layer-only
//! `WasmProviderBindingV1` artifact, layered on top of (never modifying)
//! [`super::carrier::CarrierBindingV1`], mirroring
//! [`super::native::binding::NativeProviderBindingV1`]'s own convention for
//! `TargetProfile::CoreWasm`. [`probe`] is a small, real, hand-assembled
//! `.mjs` script that proves the byte-reversal fixture endpoint and a
//! genuine `WebAssembly.Memory` allocate and release for real under Node —
//! see [Public Generic Carrier
//! v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md#core-wasm-physical-adapter-issue-155)
//! for the full status-code table, deferred-scope note, and the exact split
//! between the Rust-hosted protocol adapter and that Node-hosted proof.

pub mod binding;
pub mod memory;
pub mod probe;
pub mod provider;
pub mod registry;
