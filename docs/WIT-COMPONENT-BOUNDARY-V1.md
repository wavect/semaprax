# Private WIT boundary v1

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

The focused quality gate pins maintained upstream `wasmparser = 0.255.0` as a
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

The next gate is to compose the `SPXWIT01` result/status mapping with this
generated-core lane and execute the resulting artifact through a maintained
engine-native Component Model runtime.

## Nonclaims

The original scalar component remains deliberately separate from `SPXWIT01`.
Checked component v2 proves one generated zero-argument `i64` main and checked
trap semantics; it does not implement `result<s64, status>`, typed failure
publication, parameters, general export selection, records, or ownership.

This tranche does not provide engine-native Component Model instantiation,
import WIT, implement resources, handles, futures, streams, capabilities,
version negotiation, multi-language component composition, or expose a public
WIT API. It does not change the existing core-Wasm owned ABI or `SPX-B104`.
