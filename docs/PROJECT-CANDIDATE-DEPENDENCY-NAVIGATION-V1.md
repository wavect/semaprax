# Project candidate dependency navigation v1

Audience: agent authors, embedding hosts, and compiler contributors.

Status: implementation and regression cases authored, **unrun**. No measured
token reduction, latency improvement, target execution, or completion promotion
is claimed.

Candidate dependency navigation exposes the existing four compact dependency
views over the exact fully admitted revision of one immutable candidate. It
lets an agent inspect declarations introduced or changed by candidate history
without registering a semantic image or accepting a base-image handle as proof
about candidate source.

Canonical `.spx` remains authoritative. Every request authenticates the exact
candidate digest, derives the candidate image from its retained checked
revision, and uses the existing immutable dependency index. Navigation neither
replays an untrusted graph nor mutates candidate history, source, disk, caches,
or publication state.

## Library API

`ProjectCandidate::dependency_summary(expected_candidate, target)` returns a
bounded `semaprax.project-candidate-dependency-summary.v1` report.

`ProjectCandidate::dependency_page(expected_candidate, target, view, handle,
cursor, options)` returns
`semaprax.project-candidate-dependency-page.v1`. It reuses the closed
`ImageDependencyView` and `ImageDependencyPageOptions` types from
[image dependency navigation](SEMANTIC-IMAGE-DEPENDENCY-NAVIGATION-V1.md).

Both reports retain the candidate image navigation fields and add exact
`candidate_revision` and `base_project_revision` bindings. `project_revision`,
`workspace_revision`, `image_digest`, source revisions and source digests refer
to the admitted candidate revision. The base revision is an ancestry fact; its
image, handles, pages and source facts are not silently substituted.

The four views preserve the existing collector and ordering:

| View | Candidate-bound inventory |
| --- | --- |
| `sites` | Actual retained field, type and case access sites. |
| `callers` | Reverse callable closure in stable-ID order with source provenance and structural inclusion reason. |
| `calls` | Direct call sites whose caller and callee belong to the selected closure, in retained traversal order. |
| `members` | Selected owner/member identities in stable-ID order with authenticated declaration facts. |

This is a final-candidate projection, not a before/after delta. A changed
declaration is visible with its candidate name, source binding and relationships.
An introduced declaration can be selected because the candidate rebuild has
already admitted its stable identity. To compare base and candidate inventories,
the caller must query the base image and candidate separately and preserve both
revision bindings.

## Pagination and references

Summaries are bounded to 65,536 bytes. Pages accept 1–128 items and
1,024–1,048,576 output bytes; defaults remain 32 items and 65,536 bytes. A page
that does not fit fails rather than truncating a row. Empty inventories admit
one empty first page. A continuation must advance on an exact page boundary and
remain inside the selected inventory.

Candidate handles use SHA-256 domain
`semaprax.project-candidate-dependency-handle.v1` followed by NUL, then the
candidate digest, target and view. Each value is framed by its u64
little-endian UTF-8 byte length. Candidate cursors use domain
`semaprax.project-candidate-dependency-cursor.v1` followed by NUL and bind the
handle, canonical decimal offset, page size and maximum bytes using the same
framing. Handles are exactly 71 bytes and input cursors are limited to 128
bytes.

The candidate selector is authenticated before deriving the image or accepting
a target, handle or cursor. Handles from a base image, sibling candidate,
earlier history node, other target or other view fail closed. Cursors also fail
when their candidate-bound handle or page options differ. References are
deterministic selectors, not secrets, capabilities or durable recovery tokens.

The route reuses `SPX-G322` for unsupported views and invalid options,
`SPX-G323` for output capacity, and `SPX-G324` for malformed or mismatched
handles and cursors. Existing candidate selector and dependency-target
diagnostics remain unchanged.

## Evidence and boundaries

Candidate navigation preserves the image dependency collector's structural
limits and item shapes. Declared test-root reachability is not test coverage.
Generic templates are not invented instances; external or dynamic callers,
whole-value leaf accesses, path feasibility, runtime liveness and behavior are
not inferred. Candidate source replay and ordinary Project admission remain the
owners of type, ownership, contract and target validity.

Reports state `source_authority: false`, `candidate_retained: false`,
`execution: false` and `publication_authority: false`. They retain no source,
test, build, commit or publication authority. Reading a candidate does not
retain a new candidate, merge siblings, complete a draft, refresh the workspace,
or make the candidate current. Sibling histories remain isolated even when
their canonical base and target stable IDs are equal.

Authored library evidence in
`tests/project_candidate_dependency_navigation_v1.rs` covers changed and
introduced declarations, exact candidate/base/Project bindings, all four paged
views, foreign and stale selectors/references/options, source immutability and
history isolation. The cases were not executed while authoring this tranche.
