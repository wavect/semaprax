//! The native C11 PHYSICAL adapter for [Public Generic Carrier
//! v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md) (issue #154), the first of
//! PG-7's per-target adapters (issues #154-#159). This module implements
//! layout, allocation, and release; it does not re-decide legality that
//! [`super::carrier::machine::CarrierCallMachine`] and
//! [`super::descriptor::verify`] already own — see [Public Generic Carrier
//! v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md#logical-versus-physical)'s
//! LOGICAL/PHYSICAL split.
//!
//! [`binding`] defines a new, physical-layer-only `NativeProviderBindingV1`
//! artifact (deliberately layered on top of, never modifying,
//! [`super::carrier::CarrierBindingV1`]) that adds the facts a physical
//! provider needs and the logical binding does not carry: native adapter ABI
//! version, provider artifact digest, exported endpoint symbol identity, and
//! a closed support/publication claim.
//!
//! [`template`] renders the versioned C11 ABI (`spx_pg_*_v1`, see
//! `spx_pg_v1.h`) and its hand-authored reference implementation
//! (`provider_body.c`) into one compilable translation unit, substituting
//! only the trusted descriptor/binding byte constants a real
//! `VerifiedPublicGenericDescriptor` and `NativeProviderBindingV1` supply.
//! Everything else in the emitted provider is a fixed, reviewed template:
//! deriving a provider from a real checked *generic* export requires #119's
//! still-blocked owned-record ownership evidence, so this round's bound
//! endpoint is a fixture (byte-reversal) operating on the existing owned-
//! Bytes shapes, exactly as this issue's brief scopes it. See
//! [Public Generic Carrier v1](../../docs/PUBLIC-GENERIC-CARRIER-V1.md#native-c11-physical-adapter-issue-154)
//! for the full status-code table, wire format, and deferred-scope note.

pub mod binding;
pub mod template;
