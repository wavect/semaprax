# Offline Linked Scalar Core-Wasm Package Build v2

Status: frozen additive v2 contract; implementation and evidence authored,
unrun, unpublished, and unpromoted.
Audience: compiler, package-tooling, and platform-authority contributors.

## Purpose and authority boundary

Build v2 deterministically projects one caller-owned, independently replayed
Package Source Capsule v1 into one effect-free scalar Core-Wasm module. The
compiler receives no filesystem, process, registry, network, cache, clock, or
publication authority. It consumes only
`package_source_capsule::verify_for_linked_build`; the returned retained linked
HIR is already authenticated and is never reconstructed from submitted build
artifacts. The capsule, build evidence, and verification receipts carry facts,
not authority.

The separate safe publisher may create one new directory, but `publish_linked`
uses the exact v1 held-authority state machine, fixed three-file inventory,
two compiler replays, byte authentication, settlement, no-replace publication,
and post-publication authentication. V2 does not duplicate platform logic.
On every supported platform the host must exclude uncooperative namespace and
content mutation of the destination, its parent, and its complete ancestor
chain for the invocation. Unix/macOS additionally requires a
current-effective-user-owned destination parent with exact mode `0700`.
POSIX creation cannot atomically return the created-directory handle, and the
Darwin mode check does not mechanically authenticate ACL authority. These are
mandatory host preconditions, not an advisory-lock or hermeticity claim.

## Inputs and closed profile

Generation and verification bind the exact capsule bytes, caller-owned
`PackageSource` values, resolver evidence and inputs, capsule options, and build
options. The build root must exactly equal both the capsule-option root and the
authenticated capsule root. Selected exports contain 1..=32 strictly
byte-sorted unique stable IDs. The capsule receipt's `exports` inventory is
root-owned, byte-sorted, and unique; provider-only IDs remain private evidence
and are rejected rather than inferred from linked HIR.
The capsule requires a byte-lowest root-owned `fn() -> i64` stable ID as its
internal HIR anchor; its display name need not be `main`, and all selected root
exports remain retained roots. The linked HIR is passed directly to the
existing scalar Wasm emitter.

The emitted module must structurally validate and contain exactly these seven
function imports, in order, from `env`: `spx_add`, `spx_sub`, `spx_mul`,
`spx_div`, `spx_rem`, `spx_neg`, and `spx_contract_fail`. Its public function
exports exactly equal the emitter's selected export receipt. No other imports,
exports, capabilities, effects, or adapters are admitted.

## Canonical artifacts

The artifact inventory is exactly:

1. `module.wasm`
2. `semaprax.package-build.evidence.json`
3. `semaprax.package-build.json`

The compact manifest schema is
`semaprax.offline-linked-scalar-wasm-package-build.v2`; its profile is
`linked-effect-free-core-wasm-scalar.v2`. In exact field order it contains
`schema`, `profile`, `root`, `packages`, `inputs`, `exports`,
`runtime_imports`, `module`, `compiler`, `limits`, and `nonclaims`. `inputs`
binds the capsule schema, domain-separated digest and bytes plus authenticated
source-set and link digests.

The compact evidence schema is
`semaprax.offline-linked-scalar-wasm-package-build-evidence.v2`. Its wrapper is
exactly `schema`, `digest`, `bytes`, `payload`; the payload binds the capsule,
capsule schema, root, complete linked package inventory, selected exports, source-set and link
digests, manifest and Wasm digests/bytes, limits, fixed-point budget, and
nonclaims. All arrays preserve authenticated or explicitly required canonical
order. Verification validates bounded submitted wire then independently
regenerates and byte-compares all three artifacts.

Limits are 4 KiB..=16 MiB for cumulative artifacts and final evidence. Evidence
fixed-point rendering has a separate cumulative 64 MiB builder ceiling,
converges within 32 probes, and counts only converged bytes in the final
artifact budget. Wasm and manifest builders use the frozen 16 MiB global
ceiling before their completed bytes are tested against the caller's cumulative
artifact limit; a small caller limit is not an allocation-failure injection
surface. All integer sums are checked.

## Diagnostics

- `SPX-PB601`: options
- `SPX-PB602`: capsule/source authentication
- `SPX-PB603`: package, root, dependency, import, or source association
- `SPX-PB604`: linked effect-free scalar profile or emitted inventory
- `SPX-PB605`: bounds or checked byte accounting
- `SPX-PB606`: submitted canonical wire
- `SPX-PB607`: exact replay

Package Source Capsule diagnostics `PS501..PS507` map monotonically to the
corresponding `PB601..PB607` family without copying nested diagnostic text;
unknown nested codes fail closed as `PB602`.

## Nonclaims

V2 does not claim target execution or conformance, Component Model, WASI,
dynamic linking, native output, external compiler/linker execution, registry or
network resolution, durable cache, cross-platform hermetic sandboxing,
signatures, publisher identity, provenance, licensing, or SBOM production.
It claims only deterministic linking of the exact capsule-authenticated closure
through the already implemented scalar Core-Wasm projection.
