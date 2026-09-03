# Candidate merge preview v1

Status: implementation and regression evidence authored, unrun.

Audience: agent builders, embedding hosts and compiler contributors.

The read-only merge preview compares two complete candidates by attempting the
existing semantic merge in both orders. It fills the gap between descriptive
target overlap and explicit creation of a retained merged candidate. The
original `compare` report and the mutation APIs remain unchanged.

## Exact parents and source replay

`ProjectCandidate::merge_preview(expected_candidate, other, expected_other)`
requires exact candidate digests and a common original Project base. It calls
the ordinary [semantic merge](PROJECT-CANDIDATE-REBASE-V1.md) implementation;
there is no second conflict classifier or relaxed admission path.

`left_then_right` applies the shared history prefix once, then the left suffix,
then the right suffix. Because the existing merge API applies its argument
first, this direction calls `other.merge(other_digest, self, self_digest)`.
`right_then_left` calls `self.merge(self_digest, other, other_digest)`.
Both preserve ordinary identity, conflict, history-length, canonical source,
independent Project verification and selected-target checks.

The resulting candidates exist only while computing the report. No candidate
object, retained registry entry, source write, filesystem authority, runtime
execution, or publication is returned. A result candidate digest is descriptive
identity, not a registered session handle. To obtain a usable merged candidate,
the caller must separately request the ordinary merge with its chosen order.

## Report and interpretation

The closed `semaprax.project-candidate-merge-preview.v1` report binds the common
`base_revision`, `left_candidate_revision` and `right_candidate_revision`.
Each direction has one of two closed variants:

| Status | Direction fields |
| --- | --- |
| `accepted` | `status`, `result_project_revision`, `result_candidate_revision`, `shared_history_prefix`, `source_file_count`, `source_bytes` |
| `rejected` | `status`, `diagnostics` containing only `code` and `message`, `interpretation: merge_rejected_not_proof_of_incompatibility` |

`same_source` is a boolean only when both directions succeed. It compares the
actual canonical manifest and ordered source paths and bytes, rather than
assuming equal candidate history digests. It is null when either order rejects.
Identical resulting source does not imply identical candidate histories.

A rejection means that the current merge route rejected that order. It may
reflect a conservative conflict rule, unsupported shape, capacity limit or
ordinary source verification failure. It is not proof that no valid manual or
future semantic merge exists. The preview does not retry with weaker guards or
choose an order automatically.

The report states `validation: ordinary_merge_with_full_candidate_admission`,
`tests: not_run`, `source_authority: false`, and `candidate_retained: false`.
The validation label names the selected mechanism; a rejected direction may
have failed conflict selection before reaching source replay. Only accepted
directions establish completed candidate admission.
Its nonclaims exclude behavioral equivalence, runtime or test execution,
external-consumer compatibility, and permission to publish or retain candidates;
they also identify conservative and capacity rejection limits. Successful static admission does not establish
contract satisfaction at runtime, runtime test coverage or publication approval.

## Independent report verification

The library method `verify_merge_preview(expected_candidate, other,
expected_other, bytes)` independently replays both complete parent histories,
recomputes both merge directions, and requires byte-for-byte equality with the
submitted report, including its final newline. Submitted report fields never
provide candidate or source authority. This verifier is a library API; it does
not add a transport method.

Its closed `semaprax.project-candidate-merge-preview-verification.v1` receipt
contains `schema`, `result: exact_source_history_recomputation`, `base_revision`,
both parent candidate revisions, `report_digest`, `tests: not_run`,
`source_authority: false`, and `candidate_retained: false`. The SHA-256 report
digest hashes the UTF-8 domain `semaprax.project-candidate-merge-preview.v1`
followed by a NUL byte, the report byte length as an unsigned little-endian
64-bit integer, and the exact report bytes. This receipt proves recomputation
under the current compiler; it is not independent runtime evidence.

## Bounds and failures

The report is bounded to 256 KiB. Each rejected direction admits at most 64
diagnostics and 16 KiB of combined code/message text. Oversized diagnostics
fail with `SPX-G226`; there is no silent truncation that could hide why an order
rejected. Existing candidate source, history, expression and merge bounds remain
in force. These are bounded computations, not latency, total heap or RSS claims.

Malformed or stale selectors and different original bases reject the request
before presenting any directional conclusion (`SPX-G222`, `SPX-G224`, and
`SPX-G235`, respectively). Verification rejects byte mismatch with `SPX-G235`
and oversized input with `SPX-G226`. Ordinary merge diagnostics are
returned as bounded descriptive excerpts; their message text is not a source
location, repair instruction or authority-bearing field.

## Protocol and concurrency

V5 adds `candidate/merge-preview` under the existing host-selected
`candidate_prepare` grant. Its required parameters are `image_revision`,
`candidate_revision`, and `other_candidate_revision`. The response payload is
the closed report above. Discovery exposes the request and response schema to
generated clients and MCP without changing older method sets or `candidate/compare`.

The diagnostic-attempt grant is not required: this operation returns ordinary
merge errors and retains no rejected attempt or invalid source. No build, test,
commit or artifact-materialization grant is implied. The editor's independent
allowlist does not automatically gain a new command.

The same pure payload handler serves sequential requests and explicitly allowed
parallel retained reads. The authenticated coordinator detaches the two exact
immutable candidates; workers receive no registry or host authority. Existing
source authentication surrounds the complete joined batch, including failures.
This is an expensive source-replay query, not a cheap overlap lookup. At most
two merge attempts occur per request; host worker and frame bounds still apply.

## Evidence

Authored regressions in `tests/project_candidate/merge_preview.rs` cover
ordered replay, exact parents, shared histories, unchanged source and candidate
state, accepted source comparisons and explicit rejections. Transport cases in
`tests/image_v5/candidate_merge_preview.rs` cover selected authority, exact
bindings and the retained-read path. No tests, compiler execution, throughput
measurement or hosted evidence was run for this change.
