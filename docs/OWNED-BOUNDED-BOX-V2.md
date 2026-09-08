# Owned Bounded Box v2

Audience: language users and compiler contributors.

Status: locally exercised additive `Box<Bytes>` tranche. Hosted/public
promotion remains pending; the completion matrix records local evidence only.

This document defines the v2 extension to the compiler-owned bounded Box
profile. The scalar v1 contract remains frozen in
[Owned Bounded Box v1](OWNED-BOUNDED-BOX-V1.md).

## Exact extension

Prelude v5 adds exactly two owned-Bytes operations to the existing
`core.box.new` and `core.box.into-inner` identities:

| Source | Signature |
| --- | --- |
| `box_new` | `<Bytes>(own Bytes) -> own Box<Bytes>` |
| `box_into_inner` | `<Bytes>(own Box<Bytes>) -> own Bytes` |

`box_get<Bytes>` remains closed. Returning a borrowed or copied Bytes value
would require a separate payload and cloning contract. No replacement,
mutable borrow, inference, or additional payload types are admitted.

`box_new<Bytes>` stages and transfers its owned Bytes payload only after the
allocation succeeds. Allocation refusal leaves the staged Bytes owner for the
ordinary failure cleanup path. `box_into_inner<Bytes>` detaches and returns the
same owned payload. A Box that remains live at scope exit recursively drops its
inner Bytes payload exactly once. The Box carrier is non-Copy, and its address,
allocation identity, and layout are unobservable.

## Target compatibility

Scalar-only programs preserve Prelude v4 bytes, scalar Box behavior, and the
legacy imports `spx_box_new`, `spx_box_get`, `spx_box_into_inner`, and
`spx_box_drop`.

Any retained compiler-owned `Box<Bytes>` use selects Prelude v5 and the
versioned Core-Wasm imports `spx_box_new_v2`, `spx_box_get_v2`,
`spx_box_into_inner_v2`, and `spx_box_drop_v2`. The Bytes payload uses type tag
9. The v2 drop operation recursively releases the stored Bytes handle;
`into_inner` detaches that handle so the caller owns it. A legacy host that
does not provide the v2 imports fails instantiation with a link error instead
of silently applying scalar drop meaning to an owned payload.

The frozen Prelude v5 contract digest is
`sha256:deeb4ca14e4a5a14e4b427bd75b4ca953ce2a335e725f616bcdad3c9e6fe1a58`.
Existing v1 through v4 prelude bytes and digests remain unchanged for their
admitted programs.

## Evidence boundary

The focused local tranche covers source/HIR admission, exact ownership and
lexical cleanup, interpreter execution, native C11 O0/O2 repeated execution,
allocation refusal after Bytes creation, contract failure before and after
Box creation, and Core-Wasm execution with a host that tracks Bytes handles.
The Wasm host also checks recursive drop, consuming detachment and legacy-host
refusal. Source admission rejects `box_get<Bytes>` with `SPX-T285`.

These tests are local evidence only. They do not establish hosted execution,
public ABI support, production allocator guarantees, or hosted promotion.

## Nonclaims

There is no public aggregate or FFI Box ABI, custom allocator, placement or
region allocation, shared ownership, weak ownership, pinning, mutable Box
borrow, cloning operation, replacement operation, String or Vec payload,
variant/resource payload, nested Box payload, or cross-thread sharing.

## Executable selectors

`cargo test --locked -p semaprax --lib owned_bytes_box` covers the
canonical round-trip, exact ownership and graph replay with the frozen v5
prelude digest and rejection of a forged v4 graph binding.
`cargo test --locked -p semaprax --test workspace owned_box_bytes_workspace`
binds the canonical semantic workspace to those exact frozen v5 bytes, checks
that a later scalar-v4 source cannot downgrade it, and replays ProgramRoot.
`cargo test --locked -p semaprax --test owned_data owned_box_bytes`
covers the five focused runtime/admission tests. The existing
`prelude::tests` and `owned_bounded_box` selectors protect the scalar profile.
