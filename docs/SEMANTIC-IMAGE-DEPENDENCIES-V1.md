# Semantic Image Declaration Dependencies v1

Audience: compiler contributors, embedding hosts, and agent adapter authors.

Status: implementation and regression evidence authored, **unrun**. No measured
latency, memory, token savings, target execution, or completion promotion.

`ProjectSemanticImage::declaration_dependencies(expected_image, target)` returns
a bounded source-derived report for a stable declaration identity. The schema
is `semaprax.image-declaration-dependencies.v1`. Canonical `.spx` source remains
authoritative; the index is disposable and never adds meaning to image JSON.

Each immutable image lazily retains one dependency index, including a failed
construction result. Concurrent immutable queries share that initialization.
The index is neither serialized in Image v1 nor trusted from an external cache;
image bytes and digest remain unchanged. A refreshed image owns a fresh index.

The collector indexes retained source functions and generic templates, their
contracts and bodies, actual field/type/case accesses, and direct calls. It
records source and expression provenance. Declaration queries select the owner
and its members or the requested member, then report direct use sites, reverse
caller closure, and relevance to the declared test root. Test relevance does
not establish coverage, path feasibility, execution, or runtime liveness.
Materialized generic instances are not rescanned as independent source bodies.
Spans identify the containing checked expression; in-place write sites retain
the containing block identity and span, not an invented field-token span.
External and dynamic callers are not inferred, and whole-value accesses are not
expanded into an invented read of every field.

Candidate semantic deltas use this same collector through their before/after
images. Their legacy relationship projection preserves the prior row shape and
selection rules; the public dependency report additionally selects variant
cases and carries richer provenance. These are structural facts, not proof of
behavioral equivalence or permission to publish a candidate.

The index has global structural limits: 1,048,576 expression visits, depth 256,
65,536 sites, calls, and type-index items, and a 16 MiB retained-data budget.
The report has an 8 MiB byte limit. These are deterministic logical bounds,
not a measured process RSS guarantee. Pattern items additionally retain their
65,536 limit; expressions and patterns together cannot exceed 1,048,576 visits.
`SPX-G320` reports invalid/absent dependency targets or ambiguous index facts;
`SPX-G321` reports capacity failures. Existing stale-image diagnostics remain
unchanged. Capacity errors fail closed rather than
returning a partial dependency graph.

V5 exposes `image/dependencies` as an ordinary read-only method without a
candidate grant. Parameters are exact `image_revision`, `target`, optional
`offset`, and optional `chunk_bytes` (1,024–65,536, default 16,384). Each
`semaprax.image-declaration-dependencies-chunk.v1` payload binds the target,
image, report schema, total bytes, offset, UTF-8 chunk and next offset, with
`source_authority: false`. Chunk selection never skips source authentication.
The closed chunk wrapper is bundled in schema discovery; heterogeneous report
interiors remain explicitly unbundled. Earlier protocol profiles are unchanged.

The embedding-host [parallel read API](IMAGE-PARALLEL-READS-V1.md) admits this
immutable query. Its existing before/after source checks and join-before-return
boundary still apply. The stdio transport remains sequential.

[Dependency Navigation](SEMANTIC-IMAGE-DEPENDENCY-NAVIGATION-V1.md) adds compact
summaries and revision-bound detail pages over this same index. The full report
and chunk query remain available with unchanged payloads.

[Cleanup Dependencies](SEMANTIC-IMAGE-CLEANUP-DEPENDENCIES-V1.md) adds a lazy
child over existing checked inventory/cleanup/loan plans for reverse member
obligation queries. It preserves this report's payload and uses no second
source-expression reference collector.

Authored evidence lives in `tests/image_declaration_dependencies_v1.rs` and
`tests/image_transport_v5/declaration_dependencies.rs`. General package and
artifact consumers, runtime obligations, and measured index benefits remain
outstanding.
