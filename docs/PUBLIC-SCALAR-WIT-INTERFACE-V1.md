# Public Scalar WIT Interface v1

Status: Authored and locally unpromoted.

Audience: Project, interoperability, package, and host-tool maintainers.

This specification defines an authority-free public WIT interface artifact for
the exact retained Project-v1 scalar export surface. It does not emit or run a
WebAssembly Component. The distinction is mandatory: current public scalar
Core Wasm reports checked language failures through its existing host/runtime
boundary, so presenting a direct canonical lift as a typed WIT result would be
false.

## Closed admission

The subject is an already admitted immutable `ProjectRevision` whose manifest
has exactly schema `semaprax.project.v1` and profile `ScalarV1`. Its selected
surface is the manifest's complete, strictly ordered `web_exports` list. Each
selected stable ID must resolve to one explicit, effect-free monomorphic
function in retained HIR with zero through eight value parameters and one
value result. Every parameter and result is exactly `i64` or `bool`.

Ordinary Project-v1 scalar admission remains authoritative. Unsupported
signatures and capacity failures retain `SPX-W115` and `SPX-W116`; the WIT
accessor does not replace them with a later interoperability diagnostic.
Calling the accessor for Project v2 through v10 fails as `SPX-J105` without
target emission, filesystem activity, or publication.

## Stable WIT projection

The frozen package is `semaprax:project-scalar@1.0.0`, the interface is
`exports`, and the world is `project-scalar-v1`. It declares the canonical
status record:

```wit
record status {
  domain: string,
  code: u32,
  class: u8,
  retryable: option<bool>,
}
```

Each selected function is named `spx-` followed by the lowercase hexadecimal
UTF-8 bytes of its exact stable declaration ID. Parameter names are the
ordinal identities `arg-0` through `arg-7`. `i64` maps to `s64`, `bool` maps
to `bool`, and every result is wrapped as `result<T, status>`. Display names,
source paths, parameter spelling, declaration order outside `web_exports`, and
lossy punctuation normalization never determine WIT identities.

The complete WIT text is bounded by
`MAX_SCALAR_WIT_INTERFACE_BYTES = 65,536`. Stable-ID encoding is injective, so
IDs such as `a.b`, `a-b`, and `a_b` cannot collide.

## Descriptor and replay

The canonical descriptor schema is
`semaprax.project.scalar-wit-interface.v1`. It binds:

- the exact Project schema, name, project revision, workspace revision, and
  semantic-graph digest;
- the fixed WIT package, interface, world, mapping and byte limits;
- the complete selected export order;
- for each export, its stable ID, derived WIT identity, ordered scalar
  parameter facts, and scalar result fact; and
- the exact WIT bytes and their digest.

The descriptor's closed mapping object binds `i64` to `s64`, `bool` to
`bool`, every function result to `result<T, status>`, and each status field to
its exact WIT type. The status schema is `semaprax.status.v1`;
`status.domain` maps to its `domain_id` field and retains the exact 1..=255
UTF-8 byte/no-NUL constraint; status codes are nonzero; class ordinals are
Contract 1, Arithmetic 2, Import 3, ExplicitClose 4 and Adapter 5; retryability
maps false/true/unknown to `some(false)`/`some(true)`/`none`. No runtime adapter
or status-code translation implementation is claimed. The descriptor is
bounded by `MAX_SCALAR_WIT_DESCRIPTOR_BYTES = 262,144`. The artifact digest
uses a versioned domain and binds the complete descriptor bytes, not the WIT
text alone. Construction independently parses and replays the descriptor against
the retained manifest, HIR, revisions, graph digest, selected order, derived
identities, and regenerated WIT. Mutation, truncation, trailing bytes,
cross-subject pairing, or internally consistent reminting fails as
`SPX-WIT111`; exact interface or descriptor capacity failure is
`SPX-WIT112`. Replay never repairs, sorts, or trusts serialized semantic
facts.

## Authority and compatibility

The API is a library-only read from retained memory. It does not open source,
emit Core Wasm, create a Component binary, invoke a compiler or engine, write
an artifact, publish a package, or acquire filesystem, process, network,
environment, clock, randomness, secret, signing, WASI, or host-callback
authority.

A display-only function or parameter rename preserves WIT bytes because stable
IDs and ordinals own the public identity. The subject-bound descriptor and its
digest correctly change when the retained Project revision changes. Signature,
stable-ID, selection, order, or scalar-kind changes alter both the relevant
WIT and descriptor facts.

## Evidence and nonclaims

Required evidence before promotion includes a six-export calculator, zero- and
eight-parameter boundaries, both scalar kinds, canonical selection order,
punctuation-collision IDs, display rename stability, signature change, hostile
descriptor replay, cross-subject pairing, capacity and wrong-profile
precedence, and absence of new target/filesystem effects after the retained
Project has been admitted. A default-feature external consumer must be able to
read the public artifact without enabling the private Component harness.

The generated WIT must also be parsed by a maintained external WIT parser in an
isolated unpublished evidence crate; that parser is not added to the root
published dependency graph or Rust 1.85 surface.

This tranche does not provide Component bytes, canonical lowering, runtime or
engine execution, WIT imports, resources, application String values,
application records or variants, `Option`, user `Result`, owned or borrowed
handles, capabilities, WASI, host callbacks, async/futures/streams, package
publication, version negotiation, or multi-engine conformance. It does not
promote or relabel the default-off private WIT/Component v1-v10 harnesses.
Those remain separate proof evidence.
