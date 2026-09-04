# Offline Multi-Package Source Capsule v1

Status: **authored, unrun, unpublished, and unpromoted**.
Audience: compiler, package-tooling, and semantic-evidence contributors.

Offline Multi-Package Source Capsule v1 is an additive, authority-free bridge
between exact Resolver-v1 selection and a future linked package build. It
authenticates two through four caller-owned canonical SEMAPRAX sources, proves
that their source-derived import graph is exactly the selected Subject-v2
dependency graph, checks their typed scalar interfaces against the selected
Report-v2 facts, and retains the ordinary verified linked HIR for an internal
consumer. Dependency metadata is never parsed or executed as source.

## Frozen API

`package_source_capsule` exposes:

- `PackageSource { package, report, source }`;
- `SourceCapsuleOptions { root_package, max_bytes }`, with `new` and `default`;
- `generate(sources, resolution_evidence, resolution_input,
  resolution_options, options)`;
- `verify(...) -> VerifiedSourceCapsule`; and
- authority-neutral receipt getters for schema, digest, bytes,
  source-set/link digests, exact root package, selected coordinates, source
  revisions, and root-owned explicit export stable IDs.

The root is explicit input. It is bound into the wrapper and link digest and
is never inferred from requirements, dependency indegree, or a `main`
declaration. `exports()` contains only sorted, unique, explicit stable IDs
owned by that root module. Provider identities cannot become build exports.
At least one root export must have the exact signature `fn() -> i64`; the
byte-lowest such stable ID is the internal HIR anchor. This does not grant its
display name special meaning, infer a `main` declaration, or remove any other
selected root export from the retained call closure.

The crate-private `verify_for_linked_build` seam performs the same complete
replay and returns the public receipt plus the retained `hir::ResolvedProgram`,
selected source-authenticated subjects, package facts, and per-function import
facts. A later build may consume that seam; this tranche does not emit Wasm,
publish files, or add a build-v2 public surface.

## Admission and meaning

The closed v1 profile requires:

- 2..=4 strictly byte-sorted unique packages selected by exact Resolver-v1
  evidence for `wasm32` with an empty capability allowlist;
- an explicit selected root and one caller-owned canonical source and exact
  selected Report-v2 envelope per coordinate;
- only effect-free by-value package scalars — `i64`, `i32`, `u8`, `char`,
  `f32`, `f64`, `bool`, and `usize` — as results, and those same scalars plus
  exactly one owner-view pair, `own Bytes` and `borrow Slice<u8>`, as
  parameters. A package interface is a SEMAPRAX-to-SEMAPRAX fact linked from
  exact source rather than a host ABI boundary, so the length type stays inside
  it; the host-facing
  [Public Scalar Export Profile v1](WASM-SCALAR-EXPORTS-V1.md) keeps excluding
  `usize`, and a package whose interface uses it therefore has no scalar Wasm
  export. Owning `string`, `borrow str`, owned or borrowed results, authored
  nominal types, effects, and capabilities remain outside the profile. The
  capsule additionally requires at least one explicit root-owned `fn() -> i64`
  HIR anchor, and no declared permits, nominal types, interfaces, templates, or
  type imports;
- exact equality between each source-derived typed function fact vector and
  the selected Report-v2 interface after display and parameter names are
  omitted; and
- equality between the unique source-derived `(dependent, dependency)` module
  edges and the complete direct Subject-v2 dependency graph.

Function import target, alias, and ordinal remain separate authenticated link
facts. A package-source build is the one build that also admits importing a
whole `own Bytes` parameter across packages, under the same closed condition
the borrowed view already carries: the byte parameter takes no lifetime out, so
the imported result must stay a non-borrowing scalar. That admission is scoped
to this build alone. The ordinary Project, draft, and candidate linkers keep the
value and borrowed-view boundary
[Workspace Semantic Graph v1](WORKSPACE-SEMANTIC-GRAPH-V1.md) states, and no
other owned, borrowed, or aggregate cross-file composition is granted. The
ordinary semantic-workspace graph builds synthetic logical paths only. A package-only linker retains every authenticated root export plus its
transitive function-call closure while leaving the Project linker's authored
`main` contract unchanged. Every selected module must be reachable from the
explicit root. Capsule sources are
the only executable code; the source embedded in Report v2 proves interface
facts but is not linked as an implementation.

## Canonical carrier and bounds

The schema is
`semaprax.offline-multi-package-source-capsule.v1`. The compact JSON wrapper
binds its exact payload byte length and a domain-separated SHA-256 digest. The
payload binds exact Resolver/Lock wrapper digests and byte lengths, root,
coordinate-sorted package/source/interface facts, ordered function-import
facts, sorted linked IDs, source-set and link digests, frozen limits, budget
accounting, and explicit nonclaims.

The frozen limits are four packages, 1 MiB per source, 4 MiB total source,
256 function imports, 128 MiB cumulative render strings, and 32 MiB output.
Submitted JSON is scanned before deserialization under closed depth, value,
object, and key counts; whitespace/noncanonical strings, duplicate keys,
integer overflow, BOM/CR/trailing-newline input, wrapper rebinding, and any
non-exact regenerated bytes fail closed. Rendering uses budgeted formatting
and joining, including every fixed-point probe and digest string.

## Diagnostics

| Code | Meaning |
| --- | --- |
| `SPX-PS501` | invalid options or caller inventory |
| `SPX-PS502` | resolver, subject, report, or source authentication failed |
| `SPX-PS503` | selected coordinate, root, report, interface, or dependency association disagrees |
| `SPX-PS504` | source or linked program is outside the closed scalar profile |
| `SPX-PS505` | a count, byte, work, render, import, or output bound failed |
| `SPX-PS506` | submitted capsule wire is malformed or rebound |
| `SPX-PS507` | submitted bytes do not exactly replay caller inputs |

## Authored evidence and nonclaims

Focused source evidence covers two-package linking, exact replay, root-only
exports, and rejection when dependency metadata names an edge absent from the
implementation source. Additional hostile and preservation evidence is
required before promotion. No test or quality gate was run while authoring
this tranche.

The capsule performs no discovery, acquisition, registry/cache access,
networking, filesystem or process authority, scripts, external tools, target
execution, publication, provenance/signature trust, runtime capability
enforcement, dynamic linking, WASI, Component Model work, or hermetic OS
sandboxing. Evidence bytes grant no authority.
