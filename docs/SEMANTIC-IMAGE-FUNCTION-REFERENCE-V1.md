# Semantic Image Function Reference v1

Audience: agent client authors, embedding hosts, and compiler contributors.

Status: implementation and regression contracts authored, **unrun**. This
read-only protocol does not promote a target, grant authority, or complete the
graph-operational programme.

An immutable semantic image can export a small, self-authenticating reference
to one retained source function and later resolve that reference against an
independently rebuilt copy of the exact same image. This is the stable session
handoff missing from revision-bound facet handles. Canonical `.spx` remains the
only repository authority; a reference neither contains program meaning nor
permits a source, build, execution, test, candidate, or publication operation.

## Library contract

`ProjectSemanticImage::export_function_reference(expected_image, target,
facet)` returns canonical JSON with schema
`semaprax.image-function-reference.v1`. `facet` is either absent or one of the
closed `ImageFacet` values. The selected target must be an ordinary retained
source function accepted by `function_summary`; compiler-only declarations,
generic instances, missing declarations, and non-functions are not selectable.

The closed reference contains:

- `schema`, `reference_revision`, `image_revision`, `project_revision`,
  `workspace_revision`, and `project_graph_digest`;
- `target_kind: "function"`, the exact stable-ID `target`, and nullable `facet`;
- `source`, a closed object containing `path`, `module`, `source_revision`, and
  `source_digest`; and
- `source_authority`, `execution`, and `publication_authority`, all `false`,
  plus an explicit `nonclaims` inventory.

The source facts are authenticated provenance joins, not source bytes or a
request to read the current filesystem. The reference carries no function
summary or facet body. Its maximum canonical size is 16,384 bytes.

Reference `nonclaims` are ordered exactly as
`integrity_and_staleness_binding_not_capability_or_secret`,
`exact_revision_only_no_automatic_migration`,
`no_hir_graph_source_or_handle_facts_trusted_from_reference`,
`no_source_execution_candidate_retention_or_publication_authority`, and
`no_persistent_server_state_or_general_session_recovery`.

`resolve_function_reference(expected_image, reference_bytes)` parses and
authenticates that exact carrier, requires all revision and provenance facts to
equal the selected immutable image, and returns schema
`semaprax.image-function-reference-resolution.v1`. The closed resolution binds
the reference and current image/project/workspace/graph revisions, target and nullable facet, embeds
the same complete current `function_summary`, and returns `facet_handle` as
`null` for a function-only reference or the freshly derived handle advertised
by that summary for the selected facet. Its three authority/execution booleans
remain false and its `nonclaims` repeat the boundary. A resolution is capped at
131,072 bytes.

Resolution `nonclaims` are ordered exactly as
`resolved_only_against_exact_current_image_and_source_provenance`,
`function_summary_and_facet_handle_freshly_derived_not_trusted_from_reference`,
`no_cursor_persistence_or_automatic_migration`,
`no_source_execution_candidate_retention_or_publication_authority`, and
`no_ranking_or_general_session_recovery`.

Export followed by resolve on one image is byte-for-value identical to resolve
on a separately derived image with the same compiler identity, schemas,
manifest bytes, ordered paths and canonical source bytes. Absolute checkout
paths and process/session identities are not reference facts. A source edit,
manifest edit, schema/compiler change or different target produces a different
image and rejects the old reference rather than retargeting it by stable ID.

## Digest and grammar

Reference JSON is UTF-8 canonical compact JSON with no terminal LF. The
object is closed: missing, duplicate, additional, reordered/noncanonical,
ill-typed or invalidly encoded data fails. Strings and digests retain the
ordinary image bounds and grammar. The target is at most 4,096 UTF-8 bytes.

`reference_revision` is `sha256:` plus lowercase SHA-256 over domain
`semaprax.image-function-reference.payload.v1\0`, the payload byte length as a
u64 little-endian integer, and the exact canonical payload with
`reference_revision` omitted. This digest authenticates the
carrier but is not a signature, secret, capability token or proof of external
provenance. Editing any target, facet, image, project, workspace, graph or
source fact without recomputing the digest fails; recomputing a digest cannot
make mismatched facts belong to the selected image.

The resolver independently reselects the target and source provenance from the
retained image. It does not trust a carrier to supply a path, module, graph
schema, source revision/digest, summary or facet handle. Unknown facet strings
fail before any partial result. A valid digest with a missing target still
fails membership. No fallback to a name, span, path or current working tree is
defined.

| Code | Meaning |
| --- | --- |
| `SPX-G363` | Malformed, noncanonical, stale, tampered, mismatched, unknown-facet, or non-member reference. |
| `SPX-G364` | Reference input or resolution output exceeds its fixed bound. |

The expected-image check continues to use the existing Image v1 diagnostics.
Failures return no reference or resolution and mutate neither retained image
nor source.

## v5 transport

Two additive semantic-read methods expose the same library operations:

- `image/function-reference-export` takes `image_revision`, `target`, and
  optional nullable `facet`;
- `image/function-reference-resolve` takes `image_revision` and `reference`,
  where `reference` is the exact canonical carrier string with no terminal LF.

Both response payloads are their direct closed schemas. Both methods are pure,
default-policy reads, are eligible for direct parallel reads and
`workspace/read-batch`, and are generated for TypeScript, Python, and Rust.
The catalogue-derived MCP adapter exposes identical parameters and values. No
candidate, diagnostic, test, build, filesystem, execution, or publication
grant is required or inferred.

Protocol discovery bundles both complete payload documents; neither is listed
as an opaque or unbundled schema. Earlier protocol profiles return method-not-
found. Ordinary JSON-RPC request/frame limits still apply, so the transport can
apply a smaller practical request limit than the standalone library input cap.
Unknown properties, null where a string is required, extra carrier fields,
missing carrier fields, an unknown facet, bad digest, stale revision, embedded
NUL, oversized reference, or mismatched provenance fail closed. A failed frame
does not poison later valid reads.

## Evidence and limits

[Library regressions](../tests/image_function_reference_v1.rs) cover exact
carrier shape/digest/provenance, same-image and independent-rebuild parity,
function-only and facet resolution, stale and tampered carriers, extra and
missing fields, unknown facets, missing targets, bounds, unchanged images and
zero authority. [Transport regressions](../tests/image_function_reference_transport_v5.rs)
cover discovery, bundled schemas, generated TypeScript/Python/Rust clients,
MCP, direct and batched parity, older-profile isolation, hostile inputs,
recovery, and unchanged source bytes.

These tests are authored but intentionally unrun at the user's request.
Cross-revision rebasing, advisory search/ranking, dynamic consumers, external
API compatibility, target artifacts, runtime deployment, execution evidence,
source materialization and publication remain outside this reference contract.
