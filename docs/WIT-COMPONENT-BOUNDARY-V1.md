# Private WIT boundary v1

Audience: maintainers, host integrators, and compiler contributors.

Status: implemented behind `unstable-wit-component-harness`, disabled by
default. This is deterministic private boundary evidence, not a public
Component Model backend.

`SPXWIT01` is an exact length-framed private bundle containing:

1. a versioned WIT 0.1 scalar evaluation interface;
2. canonical mapping JSON for `result<s64, status>`;
3. a strict JavaScript result/status adapter.

The bundle has a frozen SHA-256 known answer and an exact verifier. Tests reject
every single-byte mutation, every truncation, trailing bytes, and magic/version
confusion. Node executes both result branches and rejects hostile tags, scalar
types, status shapes, zero/overflow codes, empty/NUL/oversized UTF-8 domains,
invalid class/retryability values, accessors, symbol keys, and failing proxy
traps. Each property is snapshotted from one own data descriptor and never read
again from the caller's object, so a changing proxy cannot cause a later value
read. Lossless UTF-8 round-trip validation rejects lone UTF-16 surrogates,
accepts valid surrogate pairs, and enforces the exact 255-byte ceiling. The WIT
`status.domain` field is mapped explicitly to
`semaprax.status.v1.domain_id`; code and domain bounds match the language status
contract.

The same private feature now emits a separate, standards-valid scalar
Component Model binary. Its exact profile contains one import-free core module,
one core instance, one alias of the core `evaluate` export, one component
function type `(left: s64, right: s64) -> s64`, one canonical lift, and one
component export. The binary known answer is:

```text
sha256:3ed6bed8472eeae0ef17f96458622c9ae032dd7a13b115d2d7fea7fcfecde643
```

An independent bounded parser accepts only that profile. It requires canonical
unsigned LEB128 lengths and integers, rejects unknown, reordered, truncated,
trailing, or mutated structure, and validates the embedded core module instead
of trusting the emitter. The frozen component was also cross-checked with the
upstream `wasmparser` Component Model validator during development.

The dependency-free private JavaScript runtime independently parses the outer
component, authenticates the exact embedded core bytes, and lets Node's standard
WebAssembly engine validate and execute its `evaluate` export. Input bytes are
copied before parsing and before the asynchronous engine call, preventing
caller mutation from changing the admitted module. The wrapper admits only
signed-64-bit `bigint` arguments instead of allowing WebAssembly's modulo
coercion of out-of-range values. This is real runtime execution of the embedded
core module after component-profile validation; it is not engine-native
Component Model instantiation. The default feature surface has an external
compile-fail test proving that consumers cannot import this harness.

## Checked generated-core component v2

The same private feature also composes a scalar Component Model artifact from
the exact bytes returned by the checked SEMAPRAX core-Wasm backend. The
compiler-generated module is not rewritten or replaced by a fixture. A frozen,
import-free checked runtime core supplies its seven `env` arithmetic and
contract imports; the component instantiates that runtime, instantiates the
generated core with the runtime instance, aliases `semaprax_main() -> i64`, and
canonically lifts it as `evaluate() -> s64`.

The deterministic fixture known answers are:

```text
component sha256:c0bfa3e1b8883237ec9934520cf9b1cdb249289d318d6cd0413e63b716703bc0
runtime-core sha256:6bda6fa499ed5fbdd52409264c73d3975b1ed6b33adc54fba8fa5fa63111942d
```

Emission records the canonical source revision and generated-core digest. An
independent bounded parser admits the exact two-core instance/alias/lift/export
profile, checks the frozen runtime digest, requires the caller-provided
generated-core digest, and independently enforces the scalar generated-core
import/export profile. Owned/resource-shaped generated modules fail closed as
`SPX-WIT106`. Every component-byte mutation, every truncation, and trailing
bytes are rejected in tests. Artifact, generated-core, and runtime-core digests
are private metadata exposed only by read-only accessors. JavaScript derives
its embedded expected digest from the artifact's exact private bytes rather
than trusting metadata; a forged-metadata regression locks this boundary.

The focused quality gate pins maintained upstream `wasmparser = 0.256.0` as a
development dependency and validates every emitted v2 artifact with its
Component Model validator. Independently rehashed invalid runtime signatures,
core bodies, section cardinalities, and canonical-lift cross-typing must all be
rejected; this gate does not depend on the private exact-profile parser.

An artifact-bound dependency-free JavaScript runtime snapshots and hashes the
whole component before parsing it, then executes both admitted core instances
through Node's standard WebAssembly engine. Tests execute SEMAPRAX-generated
`19 + 23`, exercise valid checked runtime operations, require traps for signed
64-bit overflow, division/remainder edge cases, negation overflow, and contract
failure, and cover argument rejection, asynchronous caller mutation, changed
bytes, truncation, and trailing bytes. Dedicated generated programs exercise
overflow and `requires false` through the composed JavaScript `evaluate()` API,
not by calling runtime helper exports directly.

## Portable result component v3

The same default-off private feature now emits one deterministic, import-free
Portable Result Component v3 for the exact WIT function
`evaluate(left: s64, right: s64) -> result<s64, status>`. Its canonical lift
uses a distinct checked status/out core generated from the fixture program;
the component, generated core, profile, and source revision are bound by frozen
known answers. An independent bounded parser authenticates the exact component
and embedded-core profile, and maintained upstream `wasmparser` validation
independently accepts the artifact and rejects rehashed hostile type/lift
mutations.

Local engine-native evidence uses an unpublished standalone Wasmtime 47.0.3
runner and generated typed bindings. It invokes success, addition overflow,
division by zero, false precondition, and false postcondition twice on one
instance and again on fresh instances. A competing `(i64::MAX, 0)` case proves
that addition overflow remains the first typed failure instead of being replaced
by the later division-by-zero condition. The typed results preserve the exact
`semaprax.status.v1` domain/code/class/retryability mapping; lower-level Node
evidence separately freezes poisoned result-slot preservation, the same sticky
first-failure behavior, and executable add/subtract/multiply/divide/remainder/
negate overflow and zero-divisor status paths. Wasmtime fuel exhaustion remains
an out-of-band engine error rather than a forged typed SEMAPRAX status.

The runner requires zero component imports, instantiates with an empty linker,
provides no WASI context or host callback, and grants no filesystem, network,
environment, clock, randomness, process, logging, or mutable ambient authority.
Its unpublished crate, exact Rust 1.97.1 evidence toolchain, Wasmtime
dependency, lockfile, and denial policy are isolated from the root workspace,
so they cannot widen the public compiler dependency graph or Rust 1.85 MSRV.
The prelude-bound runner is hosted green in [run 31347109201, job
93330959212](https://github.com/wavect/semaprax/actions/runs/31347109201/job/93330959212)
and again in [run 31353051690, job
93347328905](https://github.com/wavect/semaprax/actions/runs/31353051690/job/93347328905).

## Private source-result component v4

V4 is a separate WIT 0.2 profile that connects one exact verified source
closure to an engine-native Component boundary:

```wit
package semaprax:private@0.2.0;

interface evaluation {
  record status { domain: string, code: u32, class: u8, retryable: option<bool> }
  type language-result = result<bool, bool>;
  evaluate: func(value: s64, reject: bool, divisor: s64) -> result<language-result, status>;
}
```

The admitted source closure contains only `component.source` and
`component.evaluate`. It is effect-free, uses compiler-owned
`Result<i64, bool>` and `Result<bool, bool>`, and exercises postfix `?` before
a checked add and division. The compiler derives the core from validated HIR
and CleanupPlan v2, then independently binds the exact source revision,
prelude digest, both Wasm32 layout-v2 digests, selected signatures and closure,
generated core, and component profile. User nominal types, resources,
interfaces, permits/effects, extra reachable functions, and altered signatures
fail closed.

The WIT result is deliberately nested. A source-language `Ok(bool)` becomes
outer `ok(inner ok(bool))`; a propagated source-language `Err(bool)` becomes
outer `ok(inner err(bool))`; and a recognized compiler contract or arithmetic
status becomes outer `err(status)`. Status is selected before the poisoned
source-result area is read. The adapter reconstructs canonical memory field by
field, validates boolean values, publishes tags last, and traps on invalid
internal tags, unknown statuses, or invalid canonical inputs. It never
memcpy/transmutes the compiler's internal `u32`-tagged variant into the WIT
canonical representation.

The frozen local known answers are:

```text
source revision: sha256:4391bc27b5db547f2b162c2b5467c2b75797e8a5ef64e4ffe4abef15678c6254
generated core:  54fa2822c51a71cebfd88d379b45c37ffd3d0f0b2893cb4f2966f9e2db6d5e5f
component bytes: 3e7b9c2ddc8ca6fdfa801eb50ae3a21531fce44677345ddea68d20581c79b23b
artifact DAG:    f5fa5ae3905d30c998f783e9b77867986813b0e8b4412fa4afa98e932eda4d40
```

An independent bounded parser verifies the exact core signatures, function
indices, memory/global, exports, code/data/custom manifest, component types,
aliases, canonical lift, named interface, and versioned export. Maintained
upstream validation and every-byte, truncation, trailing, noncanonical-length,
rehashed type/lift, cross-version, and admission hostiles are locally green.
The default consumer still cannot name the feature-gated API.

The isolated Wasmtime 47.0.3 runner is extended with checked-in v4 WIT and
generated typed bindings. It is configured for ten outcomes: both inner result
arms and boolean payloads; an `Err` that skips division by zero; add overflow,
division by zero, and sticky first failure; false precondition; and false
postcondition after both ordinary and residual paths. It repeats the matrix on
one instance and fresh instances, requires zero imports and an empty linker,
provides no WASI or callbacks, and keeps fuel exhaustion out of band. The
source locks and compiler/component tests are green. The v4 Wasmtime execution
is hosted green in [run 31356536123, job
93357169796](https://github.com/wavect/semaprax/actions/runs/31356536123/job/93357169796).

## Nonclaims

The original scalar component, checked v2 artifact, Portable Result Component
v3, and Source-Result Component v4 remain separate profiles. V3 proves only one
private two-parameter scalar `result<s64, status>` export. V4 proves only one
exact effect-free source closure and its
`result<result<bool, bool>, status>` export; it does not generalize source
`Result`, `Option`, or `?`, exhaustive matching, records, user variants,
resources, handles, ownership, imports, async/futures/streams, callable/FFI
aggregate signatures, or export selection.

This tranche does not provide WIT imports, capabilities, version negotiation,
multi-language composition, browser or WASI execution, multi-engine
conformance, or any public compiler/API/backend surface. It does not change the
existing core-Wasm owned ABI or open `SPX-B104`.
