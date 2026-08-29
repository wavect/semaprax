# Semantic Package Report v2

Status: additive implementation and evidence authored, not locally run or
hosted-promoted.
Audience: package tooling authors and compiler contributors.

Semantic Package Report v2 is a deterministic, authority-free semantic
subject for later offline compatibility analysis. It preserves every v1 API
and byte unchanged; v2 is a separate schema and library surface.

## Authenticated subject and replay

The envelope embeds the exact bounded canonical `.spx` source under
`semaprax.canonical-source.v1`. Verification rejects non-canonical source,
runs the ordinary source verifier, resolves validated HIR, reconstructs the
complete payload, regenerates the wrapper, and exact-compares all submitted
bytes. A digest-only or self-consistently re-minted semantic payload cannot
pass unless it is the deterministic projection of the embedded source.

The payload carries:

- stable function identity, positional parameters and result, recursive type
  trees, and exact ownership modes;
- canonical effect identities;
- requires/ensures facts normalized without revision-scoped expression IDs or
  display names. Places are rooted only in stable function identity plus a
  parameter position or result slot; contracts needing local/pattern identity
  make the whole export explicitly unproven;
- the reachable source-defined nominal closure, including stable type,
  field/case/drop identities and generic structure; missing definitions stay
  explicit in `unproven_types`;
- a closed target state: `available` requires a bounded successful production
  projection, `unavailable` requires positive proof from the closed source
  export inventory, and projection rejection/overflow is `unproven`.

## Bounds and authority

Source is capped at 1 MiB, functions at 1,024, contract depth at 48,
contract nodes at 65,536, and reachable types at 1,024. Native and Wasm target
projections each have an independent 16 MiB sink budget. Report construction
has a separate exact cumulative 64 MiB rendered/intermediate String-byte
budget, while the
requested final envelope limit remains independently enforced up to 16 MiB.
JSON strings use the repository's budgeted escaping sink; report formatting,
joins, contract facts, type facts, digests, payload, and wrapper share the
cumulative String budget. The source revision is one fixed-size digest String
derived directly from the already bounded canonical subject before that
render budget; target artifacts use their separate projection budgets.
Non-string parser/HIR/container allocations are not
described as metered by that budget: the 1 MiB source cap bounds parsing,
ordinary verification, and HIR resolution; the function cap is enforced
immediately after parsing; contract-node/depth and reachable-type cardinalities
bound only the subsequent report projection collections.

V2 grants no registry, resolver, fetch, build-script, compilation, linking,
signature, provenance, policy, cache, mutation, publication, or target
execution authority. `available` records only deterministic compiler
projection. Contract facts are structural identities, not implication proofs.
Compatibility classification is intentionally not part of this stage.

## Evidence state

Focused evidence is authored for source-bound replay, source/semantic
self-consistent outer re-mint rejection, contract display-rename stability,
closed ternary target states including forced projection overflow, exact/+1
limit helpers, malformed/duplicate/extra wire members, non-canonical source,
and preservation of the v1 golden envelope. The evidence is unrun; broader
integration evidence remains required before any completion or hosted
promotion claim.
