# Project Candidate Impact Navigation v1

Status: additive implementation and regression sources authored, **unrun**.
No generalized-impact, completeness, runtime, compatibility, or measured
context-economics claim is made.

Audience: agent authors, embedding hosts, and compiler contributors reviewing
one immutable Project candidate.

The existing `candidate/impact` route returns a candidate-bound Project
semantic-impact artifact through the general report-chunk protocol. Compact
impact navigation gives agents a small summary and opaque, pageable access to
the artifact's three existing ordered arrays. It recomputes the same compiler
artifact from the exact final candidate revision on every request. It adds no
edge family, impact inference, ranking, persistence or semantic-delta claim.

## Library API

```rust
pub fn ProjectCandidate::impact_summary(
    &self,
    expected_candidate: &str,
    target: &str,
    options: WorkspaceImpactOptions,
) -> Result<String, Vec<Diagnostic>>;

pub fn ProjectCandidate::impact_page(
    &self,
    expected_candidate: &str,
    target: &str,
    impact_options: WorkspaceImpactOptions,
    view: CandidateImpactView,
    expected_handle: &str,
    cursor: Option<&str>,
    page_options: CandidateImpactPageOptions,
) -> Result<String, Vec<Diagnostic>>;
```

`impact_summary` returns
`semaprax.project-candidate-impact-summary.v1`, capped at 65,536 bytes.
`impact_page` returns `semaprax.project-candidate-impact-page.v1`, capped by
the caller-selected page maximum. Both authenticate the exact candidate before
recomputing the retained Project impact artifact for a declaration target.

The summary's 19 fields bind the candidate and original base Project revision,
Project schema/name/revision, Workspace revision, Project graph digest, target,
compiler artifact digest, exact reverse query, truncation and budget objects,
three facet references, four false authority/state flags and fixed nonclaims.
The page's 27 fields retain those bindings and add view, handle, cursor, offset,
total, page options, continuation and items. JSON is compact canonical UTF-8
without a terminal LF.

The three views preserve the exact arrays from
`semaprax.project-semantic-impact.v1`:

| View | Meaning |
| --- | --- |
| `affected` | Existing potentially affected reverse-dependency nodes in compiler order. |
| `dependency_edges` | Existing six-family structural dependency edges in compiler order. |
| `frontier` | Existing omitted-boundary rows and their compiler reasons. |

Rows are intentionally not normalized, sorted, merged or reclassified. Each
unchanged owner row is wrapped as
`{schema:"semaprax.project-candidate-impact-item.v1",value:<row>}` so generated
clients can validate the page container while retaining the heterogeneous
owner value. The compact report is useful navigation over an existing artifact,
not independent evidence that its rows are complete or correct.

## Query, truncation and references

`WorkspaceImpactOptions` remains the owner of reverse depth, artifact bytes and
node bounds. The summary copies the exact compiler `query`, `truncation` and
`budget`. Pages can only expose rows that the bounded underlying artifact
already retained. Pagination never resumes compiler traversal, expands a
frontier, or recovers omitted rows. A truncated artifact remains explicitly
truncated on every page.

Candidate handles use SHA-256 domain
`semaprax.project-candidate-impact-handle.v1` followed by NUL. They frame the
candidate digest, target, artifact digest, canonical decimal impact depth,
impact byte/node limits and view with u64 little-endian UTF-8 lengths. A handle
from another candidate, target, query, artifact or view fails closed.

Cursors use domain `semaprax.project-candidate-impact-cursor.v1` followed by
NUL and bind the handle, canonical decimal offset, page size and page-byte
limit with the same framing. Cursor input is limited to 128 bytes; offsets are
positive canonical page boundaries no greater than 65,536. Cursors are opaque
deterministic selectors, not secrets, capabilities or durable recovery tokens.

Pages admit 1–128 items and 1,024–1,048,576 output bytes, defaulting to 32 and
65,536. A row that cannot fit fails rather than truncating the row. Empty views
return one empty first page with no continuation. Summary/page rendering
retains no derived artifact or candidate.

## Diagnostics, transport and nonclaims

Malformed or stale candidate selectors retain `SPX-G222` and `SPX-G224`.
`SPX-G333` covers unsupported views, invalid options, targets or compiler
artifact bindings. `SPX-G334` covers underlying artifact and output capacity.
`SPX-G335` covers malformed or mismatched handles and cursors.

Selected v5 hosts expose `candidate/impact-summary` and
`candidate/impact-page` only with `candidate_prepare`. Requests additionally
bind the live `image_revision`; source drift still fails at the session
boundary. Both methods are pure detached parallel reads and install no report,
image or candidate. Older candidate protocols and the existing
`candidate/impact` payload bytes remain unchanged.

Every report states `source_authority: false`, `execution: false`,
`publication_authority: false` and `candidate_retained: false`. Fixed nonclaims
state that the report is not a candidate semantic delta or behavioral change,
covers only potential reverse dependencies over the existing six edge
families, proves no runtime liveness, tests or external-consumer compatibility,
ranks no repair or intent, persists no index, treats bounded/truncated inventory
as incomplete, and grants no source, execution or publication authority.

Authored, unrun library regressions in
`tests/project_candidate/impact_navigation.rs` compare every paged row with
the independently recomputed candidate artifact, preserve compiler order and
exact metadata, retain truncation/frontier evidence, bind all handles and
cursors to candidate/query/view/page options, isolate sibling histories, reject
malformed/stale references, and leave candidates and source unchanged.
Transport regressions separately cover selected schemas, generated clients,
MCP, sequential/parallel byte parity and registry immutability. No tests,
compiler, target, client or application were run while authoring this tranche.
