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

The next gate is to compose the `SPXWIT01` result/status mapping with generated,
checked SEMAPRAX Wasm and execute the resulting component through a maintained
Component Model engine.

## Nonclaims

The scalar component is deliberately separate from `SPXWIT01`: its wrapping
`i64.add` core fixture proves binary/parser/runtime mechanics only. It does not
implement checked SEMAPRAX arithmetic, `result<s64, status>`, generated-program
semantic equivalence, or ownership.

This tranche does not provide engine-native Component Model instantiation,
import WIT, implement resources, handles, futures, streams, capabilities,
version negotiation, multi-language component composition, or expose a public
WIT API. It does not change the existing core-Wasm owned ABI or `SPX-B104`.
