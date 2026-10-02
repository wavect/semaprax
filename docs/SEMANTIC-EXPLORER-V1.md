# Semantic Explorer View v1

`semaprax.explorer-view.v1` is a bounded, read-only visualization projection over a held checked Project revision. It is an adapter over `ProjectSemanticImage`, `ProjectCandidate`, `workspace_graph`, and `workspace_analysis`; it accepts no graph JSON, source path, or cache input.

## Methods

The default v5 read policy selects `image/explorer-summary` and `image/explorer-page`. Candidate preparation additionally selects `candidate/explorer-summary` and `candidate/explorer-page`. Every request binds the current outer `image_revision`; candidate requests also bind an exact retained `candidate_revision` and select `side: base|candidate`. Image requests always use `current` internally. A candidate base is the candidate's retained original base, never the currently open image.

A summary has `mode`, `target`, `query`, an artifact digest, truncation and coverage facts, and handles for `modules`, `declarations`, `relations`, and `frontier`. A page returns one selected inventory in compiler order. Its handle and cursor bind subject, candidate side, mode, target, direction, depth, node and byte bounds, view, offset, and page options. Page defaults are 32 rows and 64 KiB; fixed maxima are 128 rows and 512 KiB. Summaries are at most 64 KiB. A row that cannot fit fails closed.

`overview` has no target and uses the retained workspace graph. `context` requires a declaration ID and delegates to the existing context kernel with `forward`, `reverse`, or `both`. `impact` requires a declaration ID and delegates to the existing reverse impact kernel. Query defaults are depth 1, 256 nodes, and 256 KiB. Existing kernel limits remain authoritative.

Node keys are `project_name:project_revision:side:stable_id`, so the same identity on base and candidate sides cannot collide. A declaration without a file path has `source_reference.kind: "non_file_node"`; an analysis row whose source binding is not carried by its owning kernel uses the explicit `authenticated_source_reference_unavailable_in_analysis_projection` state rather than a fabricated reference. Relation rows preserve their original family and provenance, including parallel sites. The only admitted structural families are `function_import`, `type_import`, `call`, `type_reference`, `effect_requirement`, and `capability_authority`; an unfamiliar retained family is never rewritten as a call. Module totals are presentation facts. Capability/prelude nodes without paths remain non-file nodes. Loans and cleanup plans remain function facets and are not relations.

A response supplies no source, execution, test, or publication authority. It does not claim runtime liveness, coverage, external/dynamic absence, or a complete result when the kernel reports truncation. Source drift stays absorbing at the enclosing v5 session boundary.
