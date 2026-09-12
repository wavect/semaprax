# Package Registry Snapshot v1

Status: implemented, **local-only** bounded module. Unit-tested in this
worktree (`cargo test --locked -p semaprax --lib package_registry`, 24
selected tests, all passing); not run through `scripts/quality.sh full`, not
released, not hosted, and not wired into any CLI route.

Audience: package-tool authors and compiler contributors working on
issue #195.

`crate::package_registry` is an authority-free, content-addressed model of a
published-package registry: a deterministic, immutable snapshot of package
coordinates that composes with the existing
[Offline Deterministic Package Resolver v2](OFFLINE-PACKAGE-RESOLVER-V2.md)
rather than reimplementing dependency solving.

## The two halves of issue #195

Issue #195 asks for both a *signed* registry and a *reproducible resolver*.

**Publication -- the signed half -- is `HUMAN_BLOCKED`.** No signing key,
keyless-signing identity, or signature-verification dependency exists in
this repository (issue #168, still open), and generated code and compiler
tooling gain no ambient signing authority (`AGENTS.md`). `RegistrySignature`
therefore carries `algorithm`/`identity`/`signature` as opaque,
structurally-checked strings only -- exactly as
[`crate::audit_capsule::SignatureEntry`](../src/audit_capsule.rs) already
does for the same reason. A forged signature naming an approved identity is
**not** rejected by this module; see
`tests::signature_is_opaque_and_never_cryptographically_checked`.
`provenance_digest` is the same kind of opaque, unverified reference.

**The reproducible-resolver half is what this module implements.**

## What a snapshot is

`build_snapshot` takes a complete, caller-owned list of `PublishedEntry`
values -- there is no ambient mutable registry server state anywhere in this
module -- and renders one canonical `semaprax.package-registry-snapshot.v1`
envelope. Each entry binds:

- a dotted lowercase `package` identity and canonical `version`;
- a `content_digest`: the plain SHA-256 of the entry's embedded Subject-v3
  `subject_bytes`, using the same digest convention as
  [`crate::audit_capsule::sha256_digest`](../src/audit_capsule.rs) and its
  `ObjectRef` binding, deliberately reused rather than reinvented;
- an opaque, caller-supplied `api_digest` and optional `provenance_digest`;
- a bounded `license` string;
- an opaque `signature`;
- a `status`: `Active`, or `Yanked { reason }`.

`subject_bytes` is authenticated with the same
[`crate::package_lock_v3::authenticate_subject_for_resolution`](../src/package_lock_v3.rs)
routine `package_resolver_v2`'s own catalog admission uses, and its embedded
coordinate is cross-checked against the entry's declared `package`/`version`.

"Publishing" a new version is a pure function from the complete prior entry
list plus one new entry -- calling `build_snapshot` again -- not a mutation
of retained state, matching how `package_lock_v3` and `package_resolver_v2`
already take a complete caller-owned catalog rather than an appended one.

## Determinism, structurally

Entries are keyed and iterated through one `BTreeMap<(String, Version),
PublishedEntry>` -- never a `HashMap`/`HashSet` -- so canonical bytes are a
pure function of entry content, never of call order or hidden iteration
order. Nothing in the module reads the clock, an environment variable, or a
file. `tests::determinism_argument_is_structural_not_just_repeated_runs`
greps the module's own source for exactly the constructs this claims are
absent (`HashMap<`, `HashSet<`, `SystemTime::now`, `Instant::now`,
`std::env::`, `std::fs::`, `read_dir(`), rather than only re-running the
build twice. `tests::entry_order_does_not_affect_canonical_bytes` builds the
same two entries in both orders and asserts byte-identical output;
`tests::one_changed_byte_changes_the_digest` asserts a changed input yields
a different digest; `verify_snapshot` recomputes the snapshot from scratch on
every call (no cache), so stale evidence that no longer matches its claimed
entries is refused (`tests::verify_snapshot_refuses_stale_evidence_after_entries_change`),
never silently accepted because a prior call happened to look similar.

## Reserved namespace

`crate::project::standard_dependencies` is the compiler's existing closed
bundled-dependency registry (`std.*`), widened from `pub(super)` to
`pub(crate)` for this issue so `package_registry` can reuse its `is_bundled`
check instead of duplicating a name list. `build_snapshot` refuses any
`std`/`std.*` package name outright (`SPX-PKR602`): the whole prefix is
reserved for the compiler-bundled closed registry, which is exactly the
dependency-confusion/squatting failure mode issue #195 names.

## Revocation

`PublicationStatus::Yanked` never removes or mutates an entry; it is carried
in the same canonical, digest-bound bytes as everything else. `YankPolicy`
gives three closed choices when projecting a snapshot into a resolver
catalog: exclude yanked versions silently (`ExcludeYanked`, the default),
refuse outright if any are present (`RefuseIfYanked`, `SPX-PKR610`), or
include them with an explicit `SPX-PKR609` warning diagnostic
(`AllowYankedWithWarning`) -- never silent inclusion.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-PKR601` | Shape/grammar: identity, non-canonical version text, digest shape, or bounded-text (license/signature/reason) violation. |
| `SPX-PKR602` | Package name is in the reserved `std.*` namespace. |
| `SPX-PKR603` | `content_digest` does not match the plain SHA-256 of `subject_bytes`, `subject_bytes` failed Subject-v3/Report-v2 replay, or its embedded coordinate differs from the declared `package`/`version`. |
| `SPX-PKR604` | Same `(package, version)` already published with a *different* `content_digest` (immutable conflict). |
| `SPX-PKR605` | Same `(package, version)` already published with the *same* `content_digest` (publication is one-time, not idempotent). |
| `SPX-PKR606` | Entry count or byte bound exceeded. |
| `SPX-PKR607` | Cumulative render-budget overflow. |
| `SPX-PKR608` | `verify_snapshot` evidence does not byte-replay the supplied entries. |
| `SPX-PKR609` | Warning: a yanked entry was included under `AllowYankedWithWarning`. |
| `SPX-PKR610` | A yanked entry is present and the active policy is `RefuseIfYanked`. |

## Composition with the resolver

`project_subjects` projects a snapshot's `Active` (and, per policy, `Yanked`)
entries' `subject_bytes` into the exact `subjects: Vec<String>` shape
[`crate::package_resolver_v2::ResolutionInput`](OFFLINE-PACKAGE-RESOLVER-V2.md)
already consumes, in the snapshot's canonical order.
`tests::projected_subjects_resolve_deterministically_through_package_resolver_v2`
feeds a projected catalog through `package_resolver_v2::generate`/`verify`
twice and asserts byte-identical resolver evidence, demonstrating the two
layers compose rather than duplicate one another's determinism.

## Evidence and nonclaims

`src/package_registry/tests.rs` pins 24 cases: determinism (repeat-call,
reorder, changed-input, stale-evidence-refusal), immutable no-overwrite
(duplicate and conflicting-digest refusal, each with a single-entry control
proving the coordinate alone is not the cause), reserved-namespace refusal
(both a synthetic name and the exact bundled `std.auth`), digest-binding and
coordinate-mismatch refusal, shape/grammar refusal, capacity refusal, all
three yank policies, the opaque signature/provenance nonclaim, and the
resolver-v2 composition round trip. Every refusal test asserts the exact
diagnostic code.

This module performs no publisher authentication, no cryptographic signature
or transparency-log verification, no network access, no filesystem access,
no CLI wiring, and no build execution. It does not compute `api_digest` or
`provenance_digest` itself; both are caller-owned opaque values it stores and
structurally bounds. It does not solve dependency graphs. It is not run
through the full quality-gate profile and is not a support or publication
decision for any package.
