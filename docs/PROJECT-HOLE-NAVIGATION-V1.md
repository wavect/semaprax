# Compact typed-hole navigation v1

Status: implementation and regression evidence authored, unrun.

Audience: agent builders, editor integrators and embedding hosts.

Typed-hole navigation exposes a compact summary and selected detail pages over
an existing immutable draft's full context. It covers body, body-expression and
contract-expression holes without inserting placeholders into source or
changing their admission rules. The full `hole_context` / `hole/query` report
remains available and byte-for-byte unchanged.

## Summary and detail selection

The separate [fill-suggestion query](PROJECT-HOLE-FILL-SUGGESTIONS-V1.md) derives
bounded place/call proposals and previews each through ordinary fill replay.
It leaves the summary and four facets below unchanged; lexical call rows remain
possibilities rather than prevalidated fills.

`ProjectCandidateDraft::hole_summary(expected_draft, hole_id)` returns
`semaprax.project-hole-summary.v1`. The summary binds the exact draft, hole,
last-valid project revision, target, existing hole handle and full context
digest. It reports the expected type, selected intention, effect policy and
four facet counts with opaque references:

| Facet | Detail items |
| --- | --- |
| `scope` | Binding identity, name, type identity, ownership and known mutability |
| `calls` | Existing local/import binding, signature, effects and effect-budget relation |
| `obligations` | The existing context's required fill/revalidation obligations |
| `constructors` | The existing context's available constructor-kind names |

The summary does not embed entire scope/call inventories, contract expression
graphs, prior loan/cleanup plans or aggregate descriptors. Its
`full_context_method: "hole/query"` directs clients to the original full context
when those details are needed. The original heterogeneous proof reports remain
explicitly unbundled; the compact schema does not claim to describe them.

The expected ownership is nullable: body contexts do not report an expression
ownership selector, so navigation does not invent one. Likewise, normalized
body-parameter scope rows use `mutable: null` where the original context does not
provide that fact. Null means unavailable in that projection, not false or no
ownership obligation. Expression scopes preserve their actual mutability and
ownership facts. Contract contexts preserve their empty allowed-effect budget
and separately reported enclosing effects.

`hole_page(expected_draft, hole_id, reference, offset, limit)` returns
`semaprax.project-hole-page.v1`. The opaque reference selects one of those four
facets; callers do not supply a source path or graph mutation. Pages include the
facet, reference, full context digest, total count, offset, items and nullable
`next_offset`. Array order is inherited from the owning context, without sorting
cleanup metadata or introducing relevance ranking.

## Freshness and authority

Navigation first uses the existing exact-draft and hole selection rules to
rebuild the full context. `context_revision` hashes its exact canonical bytes,
including the final LF. Facet references bind that revision together with the
draft, hole and facet. A reference from another hole or draft fails closed;
filling, rebasing or merging a draft requires new navigation references.

The context digest uses the domain `semaprax.project-hole-context.v1` followed
by NUL. The facet digest uses `semaprax.project-hole-facet.v1` followed by NUL
over canonical JSON containing `draft_revision`, `hole_id`, `context_revision`
and `facet`, including its terminal LF. Both hashes prefix their payload bytes
with the payload's u64 little-endian byte length after the domain.

References are deterministic selectors, not secrets or bearer capabilities.
No cursor registry, draft mutation, cache, filesystem read or publication handle
is introduced by the pure library navigation API. The retained last-valid
candidate is still the only source of context; navigation cannot make an
unresolved draft valid or materializable.

Every summary and page says `source_authority: false`. The summary also retains
`materializable: false` and pending fill validation. Accessible calls are lexical
possibilities, not promises that arguments type-check or that owned values
remain live. Effects in a context grant no runtime capability. Prior proofs in
the full context do not validate an unfilled hole.

## Protocol and bounds

Candidate-enabled v5 sessions add:

- `hole/summary`: exact `image_revision`, `draft_revision` and `hole_id`.
- `hole/page`: those fields plus a facet `reference`; optional `offset` defaults
  to zero, and optional `limit` defaults to sixteen.

The host still selects candidate preparation before the session starts. Neither
method creates a draft, fills a hole, runs a test or grants source authority.
Both methods share their pure handlers with detached parallel reads; ordinary
held-source authentication surrounds the live operation or complete read batch.
V1–v4 method sets and existing `hole/query` responses remain unchanged.

Each summary or page is bounded to 65,536 bytes. A page requests 1–64 items;
offsets and facet inventories are bounded to 16,384. Calls retain their original
1,024-entry bound. A page returns the longest prefix fitting its item and byte
bounds. If its first remaining item cannot fit, it fails rather than truncating
the item or returning a non-progressing continuation. An empty inventory has an
empty first page and null continuation. Out-of-range offsets fail.

Invalid navigation inputs use `SPX-G230`, capacity failures use `SPX-G231`, and
stale draft or foreign facet references use `SPX-G232`. Existing context and
compiler diagnostics retain their owners. V5 stale image checks remain separate.

## Schemas and evidence

V5 discovery bundles the closed summary and facet-discriminated page schemas.
Generated TypeScript, Python and Rust clients therefore expose concrete typed
summary/page payloads as well as request builders. The schemas describe only
these navigation outputs; they are not validity proofs or replacements for the
full compiler context.

Library and transport regressions are authored in
`tests/project/hole_navigation.rs` and `tests/image_v5/hole_navigation.rs`.
They cover all three hole kinds, scope/effect normalization, pagination,
reference binding, selected grants, schemas and parallel-read behavior. They
have not been executed. No tests, compiler, generated client or interpreter was
run for this change.

Navigation currently rebuilds the existing full context before selecting its
compact projection. Smaller wire payloads do not establish reduced compiler
work, model-token savings, latency or total-memory improvement. Those task-level
measurements and independent client execution remain outstanding. No completion
matrix row is promoted.
