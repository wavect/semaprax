# Project Candidate ABI Delta v1

Status: Authored, unrun.

Audience: compiler contributors and agents reviewing immutable Project candidates.

## Scope

`ProjectCandidate::abi_delta(expected_candidate)` emits a deterministic,
candidate-bound comparison of ABI-shaped facts already admitted in the exact
immutable base and final candidate revisions. The selected function inventory
is the manifest's complete `web_exports` set plus its command function when
present. Selection is by stable identity, never display name.

Each function fact carries its stable ID, retained source module/path, effects,
manifest roles, and ordered resolved signature. Parameters retain their value
IDs, names, ownership modes, and exact resolved type identity keys. The result
retains its exact resolved type identity; `value` is descriptive and does not
establish a foreign calling convention.

The report follows every nominal type in those signatures through checked HIR.
It retains exact concrete type arguments and recursively follows record fields
and variant cases/payloads after substituting the selected generic arguments.
Record and variant members remain in compiler order and carry stable IDs,
names, indexes, and resolved type identities. Every concrete nominal fact also
carries the compiler's checked `copy`, `needs_drop`, `contains_resource`,
`sized`, and `layout_key` facts. Classes, resources, unresolved type parameters,
and missing facts fail closed.

The target inventory is the candidate pipeline's already-retained native C11
emission and structurally validated Core Wasm projection facts for entry and
test roles. This route does not invoke a compiler, execute target code, create
files, or claim that a target row is a public linkable ABI.

## Comparison and binding

Function, concrete public-nominal, and retained target facts are keyed in byte
ordered maps. Their union is classified only as `added`, `removed`, `changed`,
or `unchanged`; both exact side facts are retained, with an absent side encoded
as JSON `null`. No compatibility label is inferred from a classification.

The report binds the expected candidate digest, exact base/final Project and
workspace revisions, both semantic graph digests, and a domain-separated digest
of the canonical comparison facts. Compatibility, runtime, deployment, and
external-consumer status are always `not_assessed` in v1.

`ProjectCandidate::verify_abi_delta(expected_candidate, bytes)` bounds the
submitted bytes, independently replays the complete candidate from its retained
base and typed history, recomputes the report, and requires byte equality. It
returns a separate verification record containing the submitted report digest.
Submitted JSON is never treated as source, HIR, target evidence, or authority.

## Bounds and diagnostics

The report is limited to 4 MiB, charged retained-fact work to 32 MiB, facts to
65,536, traversal visits to 1,048,576, and nominal depth to 256. Existing
candidate replay and stale-selector diagnostics remain authoritative.
Inconsistent retained facts use `SPX-G522`, capacity/output failures use
`SPX-G523`, and a byte mismatch after independent replay uses `SPX-G524`.

## Nonclaims

This artifact is not an ABI compatibility assessment, semantic-versioning
decision, runtime or external-consumer observation, linker/loader contract,
package migration, deployment plan, or publication approval. It grants no
source, filesystem, process, network, execution, signing, publication, or
deployment authority. Native/Wasm rows are retained structural compiler facts,
not execution or platform availability evidence.

Authored tests in the consolidated Project-candidate harness cover unchanged
and changed exported signatures, reachable nominal shape changes, retained
target facts, exact replay, tampering, stale selectors, and capacity diagnostics.
They are intentionally unrun in this tranche.
