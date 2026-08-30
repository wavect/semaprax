# Semantic Image Dependency Navigation v1

Audience: agent authors, embedding hosts, and compiler contributors.

Status: implementation and regression cases authored, **unrun**. No measured
model-token savings, latency improvement, execution evidence, or completion
promotion is claimed.

Dependency navigation provides a summary followed by selected detail pages over
the existing [immutable-image index](SEMANTIC-IMAGE-DEPENDENCIES-V1.md). An agent
can inspect counts and expand only the relationships needed for its decision,
without receiving or reconstructing the complete dependency report.

Canonical `.spx` remains authoritative. Navigation adds neither graph meaning
nor a second HIR scan. The existing full dependency report, candidate delta
relationship projection, Image v1 bytes and image identity remain unchanged.

## Library API

`dependency_summary(expected_image, target)` returns
`semaprax.image-dependency-summary.v1`, bounded to 65,536 bytes. It binds the
image, Project, stable target and source, identifies the declaration, and
reports declared-test-root relevance. Its four `facets` contain a view name,
opaque handle and exact item count. The summary does not embed access sites,
caller inventories, complete typed declarations or the full graph.

`dependency_page(expected_image, target, view, handle, cursor, options)` returns
`semaprax.image-dependency-page.v1`. `ImageDependencyView` is closed:

| View | Selection and ordering |
| --- | --- |
| `sites` | The full report's actual field/type/case access sites, in the same retained traversal order. |
| `callers` | The reverse callable closure in stable-ID order, with source provenance and the structural reason for inclusion. |
| `calls` | The full report's actual direct call sites whose caller and callee belong to that closure, preserving retained traversal order. |
| `members` | The selected owner/member identities in stable-ID order, with authenticated declaration facts. |

`ImageDependencyPageOptions::new(page_size, max_bytes)` accepts 1–128 items and
1,024–1,048,576 output bytes. Defaults are 32 items and 65,536 bytes. These are
wire bounds, not total process-memory limits. Existing index-wide traversal,
item and retained-data budgets still apply. A page that cannot fit fails; it
does not omit relationships or return partial JSON. The caller can restart
with a smaller page size or larger byte budget within those fixed limits.

Each page binds its view and handle and reports offset, total items, selected
items and an optional next cursor. Concatenating a view's pages reconstructs
that exact inventory. Empty inventories admit one empty first page with no
continuation. A cursor beyond the inventory is invalid.

## References, freshness and authority

Handles bind exact image identity, target and view. Continuations additionally
bind the item offset, page size and maximum output bytes. Noncanonical or
mismatched references fail closed. References are deterministic selectors;
they are neither secrets nor bearer permissions. No mutable cursor registry,
disk cache, new publication handle or session recovery authority is introduced.

The handle digest uses SHA-256 domain `semaprax.image-dependency-handle.v1`
followed by NUL, then the image digest, target and view. Every field is preceded
by its u64 little-endian UTF-8 byte length. A cursor is `<offset>:<digest>`;
the digest uses domain `semaprax.image-dependency-cursor.v1` followed by NUL and
the same framing around the handle, canonical decimal offset, page size and
maximum bytes. Offsets must be positive page boundaries inside the inventory.
Handles are exactly 71 bytes; input cursors are limited to 128 bytes.

`SPX-G322` rejects unsupported views and invalid page options. `SPX-G323` reports
output capacity failures. `SPX-G324` rejects malformed, stale or mismatched
handles and cursors. Existing image, target and index diagnostics are unchanged.

An old retained image can still be queried through the library after files
change. This library API promises facts about that image, not current disk
freshness. Live transport queries retain their existing before/after held-source
authentication; refreshing creates another image and invalidates old references
for new-image requests.

Facts retain the dependency index's structural limits: declared test-root
relevance is not coverage, generic templates are not inferred runtime instances,
and external/dynamic callers, whole-value leaf accesses, runtime liveness and
path feasibility are not invented. No ranking affects ordering or meaning.

## V5 protocol

`image/dependency-summary` takes `image_revision` and `target`.
`image/dependency-page` additionally takes `view`, `handle`, optional `cursor`,
optional `page_size` and optional `max_bytes`. Both are read-only methods in the
default v5 policy, require no candidate grant, and are admitted by the existing
embedding-host parallel-read API. They return structured payloads rather than
text chunks. Earlier protocol profiles remain unchanged.

The ordinary v5 response limit also includes its outer envelope. A library
payload at the maximum byte limit may therefore exceed the transport envelope
budget; that response fails closed and the caller must request a smaller page.

Discovery, schemas and generated TypeScript/Python/Rust request builders derive
from the selected method registry. Payload schema claims are limited to the
structures actually bundled; heterogeneous compiler facts do not become proven
semantics merely by appearing inside a validated envelope.

Authored evidence is in `tests/image_dependency_navigation_v1.rs` and
`tests/image_dependency_navigation_transport_v5.rs`. Pagination completeness,
reference rejection, source drift, cross-root determinism and batch equivalence
remain unrun. Representative task-level token, tool-call, correctness and human
review measurements remain outstanding.
