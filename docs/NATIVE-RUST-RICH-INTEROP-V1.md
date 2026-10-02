# Native Rust Rich Interoperability v1

Status: local additive bootstrap for [RI-01](https://github.com/wavect/semaprax/issues/359).
It defines compiler input and generated artifacts. The checked-in scalar
fixture proves one generated native round trip; it does not claim public Rust
support or completion of this contract's broader evidence gates.

## Scope and relationship to Native Rust Interoperability v1

This contract is a separate profile from
[Native Rust Interoperability v1](NATIVE-RUST-INTEROP-V1.md). The existing
profile continues to own its scalar SDK, descriptor schemas, same-thread and
non-reentrant bridge, and its published local evidence. A rich-binding plan
must never be accepted as a v1 Spec, descriptor, bundle, or Project subject.

RI-01 starts with a closed bootstrap: one selected Rust package exposes
ordinary Rust functions, an internal plan names those functions, and generated
code provides the only C-compatible boundary. The Rust package does not carry
SEMAPRAX annotations, FFI exports, or a guessed Rust symbol ABI. General Cargo
discovery, arbitrary crates, retained references, traits, async, resources,
aggregate values, dynamic loading, and registry publication are outside this
version.

## Names and identities

The canonical document schemas are:

| Document | Schema | Purpose |
| --- | --- | --- |
| rich interop Spec | `semaprax.native-rust-rich-interop-spec.v1` | selected target, package, bindings, and generated declarations |
| BindingPlan | `semaprax.native-rust-rich-binding-plan.v1` | internal typed compiler input after the Spec is authenticated |
| descriptor | `semaprax.native-rust-rich-interop-descriptor.v1` | replayable generated-boundary facts |
| bundle | `semaprax.native-rust-rich-interop-bundle.v1` | exact generated-file inventory |

Their SHA-256 digest domains are, in the same order,
`semaprax.native-rust-rich-interop.spec.v1\0`,
`semaprax.native-rust-rich-interop.plan.v1\0`,
`semaprax.native-rust-rich-interop.descriptor.v1\0`, and
`semaprax.native-rust-rich-interop.bundle.v1\0`. A renderer writes compact
JSON with one trailing LF, uses the ordered keys in this document, and emits
arrays in declared plan order. It rejects duplicate keys and values outside a
closed enum before allocating a generated artifact.

Every generated Semaprax declaration has a persistent `@id` from
`binding.semaprax_id`; the plan rejects duplicate IDs. A Rust path is data,
not an identity: renaming a Rust path changes the package and plan digests but
does not silently allocate a new Semaprax API identity. A future automatic
indexer must reproduce this identity selection or require an explicit migration.

## BindingPlan

`BindingPlan` is compiler input, not a capability or an instruction to execute
a process or foreign function. It is constructed only after the selected
package bytes, target profile, and authored Semaprax source are held and
authenticated. The implementation then validates it against those held inputs,
renders all artifacts from it, independently replays its descriptor and file
inventory, and only then uses ordinary build authority.

The canonical plan has these ordered top-level keys:

```text
schema, package, target, bindings, limits, nonclaims
```

`package` contains `name`, `version`, `source_digest`, and
`cargo_lock_digest`. `target` contains one admitted `triple` and
`backend: "native-c11-static"`. A plan never inherits a host target, Cargo
configuration, filesystem path, environment variable, or tool from ambient
state.

Each `bindings` item has these ordered keys:

```text
semaprax_id, semaprax_name, rust_path, receiver, arguments, result,
substitutions, effects, failure
```

The fields have the following closed meanings:

| Field | Required meaning in v1 |
| --- | --- |
| `semaprax_id` | persistent public declaration identity |
| `semaprax_name` | generated declaration spelling; unique in its generated module |
| `rust_path` | fully selected ordinary Rust item path within the exact package |
| `receiver` | `none`; methods are deferred |
| `arguments` | authored positional order; each row has `name`, `type`, and `mode` |
| `result` | one `type` and `mode` |
| `substitutions` | explicit ordered concrete type substitutions; `[]` for nongeneric calls |
| `effects` | sorted declared Semaprax effects required by the foreign action; `[]` grants none |
| `failure` | one closed mapping for semantic division failure, Rust `Result::Err`, and panic |

For the bootstrap, `type` is one of `i64` or `bool`; `mode` is `copy` for every
argument and result. The plan rejects every receiver, borrowed, owned,
aggregate, raw-pointer, resource, trait-object, `async`, inferred generic, or
unlisted substitution shape. This makes the first fixture's `add(i64, i64)`
and `checked_div(i64, i64)` precise without implying an aggregate or ownership
ABI.

Arguments are evaluated and staged left to right. They transfer together only
at the generated call boundary. A generated thunk may not call Rust until all
validation, conversion, required-effect admission, and failure output staging
have succeeded. Its cleanup follows the existing compiler-owned ordered cleanup
plan; a cleanup error cannot replace a previously selected failure.

## Failure and effect boundary

`failure` records three distinct outcomes:

| Case | Required result |
| --- | --- |
| Semaprax checked division failure | the selected Semaprax semantic failure; no success result is written |
| Rust `Result::Err` | the plan's named foreign-error status; no success result is written |
| Rust panic while inside the generated Rust adapter | the plan's named panic status after the adapter catches the unwind; no success result is written |

The bootstrap fixture must make these cases observably distinct. Panic payloads,
Rust error payloads, paths, pointers, and tool output are not diagnostic data.
An unwind crossing the C-compatible thunk is forbidden. Abort, OOM, signal and
process failure remain outside the recovery claim.

Effects name an already-declared Semaprax effect and are checked at the selected
generated declaration and every caller. Listing an effect in a plan does not
grant it to the adapter, Cargo, generated code, or a Rust crate. The plan also
does not grant filesystem, network, process, home, secret, key, wallet, or
signing authority.

## Generation and target selection

One accepted BindingPlan generates both the Semaprax declaration projection and
the Rust adapter. The declaration projection retains the plan's stable IDs,
parameter order, copy modes, declared effects, selected target, and failure
facts. The adapter calls selected monomorphized Rust items and exposes only
generated C-compatible thunks. The accompanying C11 artifact calls those
thunks through an exact generated header. Neither artifact resolves a guessed
Rust symbol or exposes a `repr(Rust)` value.

The native backend may select `native-c11-static` only after the exact plan,
package, generated adapter, C header, C artifact, descriptor and bundle all
replay. Ordinary interpreter and Core Wasm routes reject a rich binding before
foreign execution and before incrementing any fixture counter. They do not
simulate a result. A plan that selects any other backend is invalid in this
version.

The initial fixture package is checked in with `add` and `checked_div` only.
It is an executable bootstrap input for RI-01, not an automatic-indexing
interface or a supported general package workflow.

## Diagnostics

These identifiers are reserved after checking the current registry; their
implementations must use the exact messages chosen by the owning harness.

| Code | Condition |
| --- | --- |
| `SPX-B117` | rich BindingPlan is malformed, noncanonical, unauthenticated, or disagrees with held Spec/source/package facts |
| `SPX-B118` | selected rich target, value class, receiver, substitution, effect, or backend is outside this profile; refusal occurs before foreign invocation |
| `SPX-B119` | generated rich adapter selects the declared Rust-error or caught-panic status and suppresses success publication |
| `SPX-B120` | generated declaration, adapter, descriptor, or bundle fails independent replay |

Existing v1 diagnostic identifiers retain their current meanings. This version
does not reclassify `SPX-W114` or v1 bridge failures.

## Required evidence before implementation status changes

The owning native-Rust harness must retain all of the following at the same
revision:

1. A fresh Rust consumer executes Rust → Semaprax → Rust → Semaprax → Rust
   from authored sources and generated artifacts, with no handwritten adapter.
2. `add`, zero and negative arguments, semantic division failure, Rust
   `Result::Err`, and panic take their distinct selected paths; all failures
   prove that no success output was published.
3. Canonical source round-trip, explicit import identities, selected HIR facts,
   descriptor generation, and injected descriptor/plan disagreement are
   covered by stable tests.
4. Unsupported target and signature cases reject before foreign invocation; a
   fixture counter remains zero. Existing scalar SDK and owned-data bytes and
   refusal tests remain unchanged.
5. Native, interpreter, and Wasm target selection is asserted: native executes
   only the admitted profile, while interpreter and Wasm refuse before the
   foreign call.

The implemented bootstrap test is
generated_rich_fixture_adapter_round_trips_without_a_handwritten_host in the
native-Rust builder harness. It compiles the ordinary fixture crate, generated
adapter, generated C11 bundle, and a fresh Rust consumer; that consumer proves
positive, zero, and negative add calls through Rust → Semaprax → generated
adapter → Rust. The fixture's checked_div exists as the next selected Result
shape, but it is not yet imported through the generated plan. The
semantic-division, Rust-Err, panic, descriptor-disagreement, and target refusal
rows above remain required before this profile can be called complete.

This does not make any target, generated package, Rust ABI, Cargo integration,
or ecosystem binding supported.
