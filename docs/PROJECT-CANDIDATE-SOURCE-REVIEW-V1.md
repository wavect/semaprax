# Candidate source review v1

Status: implementation and regression evidence authored, unrun.

Audience: reviewers, editor integrators and agent client authors.

`ProjectCandidate::source_review(expected_candidate)` returns a closed source
review without requiring a client to interpret the heterogeneous candidate
report. It preserves the [candidate](PROJECT-CANDIDATES-V1.md) identity and
publication boundaries. It does not produce an editor edit or grant permission
to write any path.

## Bound source pairs

The report schema is `semaprax.project-candidate-source-review.v1`. Its exact
root fields are:

| Field | Meaning |
| --- | --- |
| `schema` | Fixed report schema |
| `base_project_revision` | Original authenticated source revision |
| `candidate_project_revision` | Independently replayed candidate source revision |
| `candidate_revision` | Candidate digest, including its intention history |
| `source_authority` | Always `false` |
| `files` | Changed source pairs in canonical path order |
| `report_revision` | Digest of this exact review |

Each file has exactly `path`, `base_source`, `candidate_source`, `base_digest`,
`candidate_digest`, `source_diff` and `source_diff_digest`. There are at most
sixteen rows. Unchanged files are omitted; an unchanged candidate has an empty
array. Both texts come from the retained, independently replayed revisions.
The existing candidate diff renderer owns `source_diff`; the original candidate
report and its identity bytes are unchanged.

The implementation requires unchanged complete manifest/source inventories,
checks path order and source digests, and reconstructs the complete candidate
history through ordinary independent replay before deriving the review. It
never obtains base text by reading the current path from disk. A path in this
report identifies a retained source member, not a filesystem capability.

The lazy candidate-owned review cache contains only these derived bytes or a
deterministic failure. Its first use performs replay; subsequent reads share
the same immutable result. Every call still checks the expected candidate
digest. New candidates start with an empty cache, and cache state never enters
source, candidate identity, serialization or publication authority. Discarding
the candidate discards its cache.

## Digests and limits

Source digests use the existing `semaprax.semantic-review.source-digest.v1\0`
domain. Diff digests use `semaprax.candidate.source-diff.v1\0`. The report digest
uses `semaprax.project-candidate-source-review.v1\0` over the canonical report
with `report_revision` omitted. Every digest hashes the domain, the payload byte
length as an unsigned 64-bit little-endian value, then the exact payload bytes.
Canonical report JSON has sorted object keys and one terminal LF; the LF is
included in its hash.

The complete report, including its final LF, is bounded to 16 MiB. Bounds account
for both source texts, JSON escaping and the source diff. An oversized report
fails instead of dropping rows or truncating source. The cache may retain up to
16 MiB per candidate, separately from the existing candidate report; sixteen
retained candidates can therefore add up to 256 MiB of source-review text. This
is not a total process-heap, CPU or latency guarantee: drafts, attempts and
embedding-host references may retain other candidates, and public string copies
or replay work have separate transient costs.

Existing candidate diagnostics apply: `SPX-G222` for invalid report/selector
shape, `SPX-G223` for capacity and `SPX-G224` for a stale candidate selector or
failed exact replay. Ordinary compiler diagnostics from replay are preserved.

## Workspace transport

The v5 `candidate/source-review` method requires the existing candidate
preparation grant. Parameters are `image_revision`, `candidate_revision` and
optional `offset`/`chunk_bytes`. Chunks use the ordinary 1,024–65,536 byte range,
defaulting to 16,384. Offsets must be UTF-8 boundaries within the complete report.
`next_offset` is always present and becomes null at the end.

The closed `semaprax.image-source-review-chunk.v1` wrapper carries the report
schema, image/candidate selectors, offset, total byte count, chunk text,
continuation and `source_authority: false`. The immutable candidate selector
binds the report across chunks. Both report and wrapper schemas are bundled in
v5 discovery; the generated client types the wrapper, while the encoded report
string requires separate decoding. The MCP adapter discovers the same method
as `candidate__source-review` without adding a grant.

Sequential and detached parallel reads share the same implementation. The
ordinary transport still authenticates held source before and after the read
or joined batch. A cached report cannot make a stale live workspace current.
The pure library can review a retained historical revision even after the
original checkout changes; it makes no claim about current editor buffers.

`tests/project_candidate/source_review.rs` and `tests/image_v5/source_review.rs`
author exact source/diff/digest, signature-migration, stale selector, selected
grant, chunk and parallel-read evidence. They were not executed. This addition
does not promote a completion-matrix row or replace separate commit approval.
