# Semantic Image Function Instances v1

Audience: agent authors, embedding hosts, and compiler contributors.

Status: implementation and regression cases authored, **unrun**. This query
does not promote language profiles, target support or programme completion.

An immutable semantic image can list the concrete generic-function instances
already retained from checked source and expand their exact facets. Canonical
`.spx` remains authoritative. The query never specializes a new instance,
executes a template, reads external files, or grants publication authority.

## Selection and navigation

`ProjectSemanticImage::function_instances(expected_image, template_id, cursor,
options)` returns `semaprax.image-function-instances.v1`. The first page binds
the source template and image, reports the complete retained-instance count,
and includes a bounded page of concrete instance summaries. Each row records
its instance ID, ordered concrete type arguments, signature and contract counts,
effects, and handles for the nine existing [function facets](SEMANTIC-IMAGE-FACETS-V1.md).
An unused checked template has an empty inventory; the query does not fabricate
the substitutions considered by generic-template verification.

`expand_instance_facet(expected_image, template_id, instance_id, facet, handle,
cursor, options)` returns `semaprax.image-instance-facet.v1`. Selection must
match an actual retained instance of the exact selected source template. A
well-formed instance identity or valid type argument alone is insufficient.
The query binds template, concrete instance, source provenance, image revision,
facet and pagination metadata around the selected items.

Both closed envelopes contain `schema`, `image_revision`, `project_revision`,
`template_id`, `path`, `module`, `source_revision`, `source_digest`,
`template_span`, `handle`, `offset`, `next_cursor`, `evidence_class`,
`source_authority`, `target_execution`, and `nonclaims`. A span is the existing
closed `{start, end, line, column}` source span. The evidence class is
`descriptive_projection_of_retained_generic_instance_hir`; both authority and
execution booleans are false.

The listing's other fields are `name`, `type_parameter_count`,
`total_instances`, and `instances`. Each closed instance row contains
`instance_id`, `type_arguments`, `parameter_count`, `return_type_id`, `effects`,
`requires_count`, `ensures_count`, and `facets`. Facets are the existing nine
ordered `{facet, handle}` pairs. The facet envelope instead adds `instance_id`,
`type_arguments`, `facet`, `total_items`, and `items`. The envelopes have 20
and 21 fields respectively; no source text or complete graph is embedded in an
instance summary.

Both calls reuse `ImageFacetOptions`: 1–128 items and 1,024–1,048,576 output
bytes, defaulting to 32 items and 65,536 bytes. An empty inventory has one empty
first page and no next cursor. A page that cannot fit fails rather than dropping
items. Concatenating pages reconstructs the selected inventory in its declared
order. Output strings have no terminal LF.

## Provenance and meaning

Templates and concrete executions have different identities. A retained
instance ID derives from the template ID and ordered type arguments; it is not
a digest of the template body. It can survive a source edit, which is why every
handle and response also binds the immutable image.

The checked instance's internal function ID remains its template declaration
ID. Source locations are locations in the authored template. They do not claim
that separately authored concrete source exists. Concrete expression/value IDs
and instantiated type facts belong to the selected instance; the outer instance
binding disambiguates existing inner projections that retain template IDs.

| Facet | Instance interpretation |
| --- | --- |
| `signature` | Actual substituted parameter/result types, ownership and effects from the retained concrete function. Parameter spans still refer to template source. |
| `contracts` | Existing compiler projection of concrete requires/ensures in source order. No predicate evaluation, implication or satisfaction proof. |
| `ownership` | Concrete parameter modes and structural cleanup inventory. Discovery order does not describe runtime liveness. |
| `loans`, `cleanup` | The existing complete plan projections, in their original section/vector order. No plan sorting, repair or reinterpretation. |
| `data-access`, `unsafe-boundaries` | Existing checked-HIR projections for the concrete body, with the selected instance supplied by the response envelope. Current source/profile exclusions remain in force. |
| `callers` | Exact instance-qualified direct call occurrences in retained ordinary functions and concrete instances, grouped by caller and source region. Template-only calls are not counted as concrete executions; occurrence counts are not dynamic execution counts. |
| `relationships` | Whole-Project profile admission and retained entry/test instance inventories matched by concrete identity. Project admission does not assert that this instance is admitted in an executable closure. Template export selection is separately descriptive and does not establish a concrete exported ABI, emitted artifact or execution. |

Generic-function imports remain rejected by the existing source admission
boundary. A local generic template can reference an already admitted imported
monomorphic function; that does not create a foreign generic template. Neither
an absent instance nor a missing caller edge proves absence of runtime use,
external consumers or future instantiations.

Source generic-to-generic calls also remain rejected by `SPX-T226`. The caller
collector distinguishes concrete callers structurally, but current admitted
source cannot produce a concrete generic-instance caller of another generic
instance. The positive evidence covers ordinary callers; it does not promote
that excluded source shape.

## References, limits and diagnostics

References are deterministic selectors, not secrets or bearer capabilities.
Listing handles bind the image and template; facet handles additionally bind
the exact instance and facet. Cursors bind the handle, canonical positive
offset and page size. As with ordinary facets, `max_bytes` may change between
pages. Old retained images remain queryable after disk changes, but their
references do not select facts in a newly derived image. Live transport retains
ordinary before/after source authentication.

SHA-256 handle domains are `semaprax.image-function-instances-handle.v1`
(image, template) and `semaprax.image-instance-facet-handle.v1` (image, template,
instance, facet). Each domain is followed by NUL; each field is preceded by its
u64 little-endian UTF-8 byte length. Cursors are `<offset>:<digest>` using domain
`semaprax.image-instance-cursor.v1` followed by NUL and the same framing around
handle, canonical decimal offset and canonical decimal page size. Offsets must
be page boundaries strictly inside the inventory. Instance rows sort by exact
instance ID; caller rows sort by caller kind, caller ID and source phase. This
presentation sorting does not alter any proof-plan vector.

Template selectors admit at most 4,096 UTF-8 bytes and concrete instance selectors
at most 65,536 bytes. Handles admit 71 bytes and cursors 100 bytes. Instance and
facet inventories admit 65,536 items, with the existing 16 MiB intermediate
rendering bound. The instance caller scan additionally visits at most 65,536
expressions across all retained ordinary functions and concrete instances at
depth at most 256, including expressions without a matching call. These are
input/output and report-construction limits,
not a global process-memory or latency guarantee. Existing image retention and
source admission bounds are unchanged.

`SPX-G227` reports unavailable or invalid selections and retained-identity joins;
`SPX-G228` reports capacity or option limits; `SPX-G229` reports malformed or
mismatched handles/cursors. Expected-image checks preserve `SPX-G219`–`SPX-G221`.
Underlying compiler projection diagnostics remain unchanged.

## Protocol and compatibility

Two additive default-read v5 methods expose the same library handlers:

- `image/function-instances`: `image_revision`, `target`, optional `cursor`,
  `page_size`, and `max_bytes`.
- `image/function-instance-facet`: the same fields plus `instance_id`, `facet`,
  and `handle`.

They require no candidate, build, test or publication grant and participate in
the existing authenticated read batches and catalogue-derived MCP adapter.
Ordinary outer frame limits still apply, so a maximal library payload may not
fit a transport response and a maximal selector may not fit a request envelope.

Request schemas, instance-list rows and response envelopes are structurally
described by closed bundled schemas. Heterogeneous facet items remain explicitly
unbundled under `urn:semaprax.image-instance-facet-item.v1`; the table above and
the existing facet specifications own their meaning. Generated clients expose
that interior as an opaque value rather than claiming complete typed or semantic
validation. Structural decoding is not proof or publication permission.

Existing function-summary/facet methods still select ordinary declarations;
their output and handles do not change. Image serialization/digests, semantic
Graph schemas, earlier protocol profiles and target admission are unchanged.

## Evidence

Library [regressions](../tests/image_function_instances_v1.rs) and transport
[regressions](../tests/image_function_instances_v5.rs) are authored but unrun. They cover actual
retained scalar instances, unused templates, exact-instance selection and
callers, page reconstruction, reference rejection, old-image preservation and
unchanged generic-import and generic-to-generic-call admission. No generic source is executed by this work.
Generated-client size/preservation and executable integration gates remain open,
as do measured task-level context improvements and broader generic admission.
