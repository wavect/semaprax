# Image Candidate Protocol v2

Status: additive implementation; regression tests are authored but deliberately
unrun for this change. No hosted or cross-platform completion claim.

Audience: agent builders, compiler contributors, and reviewers.

The host explicitly starts `semaprax serve-candidates <manifest>` or opens an
`ImageSession` with `ImageHostCapability::CandidateOnly`. This selects
`semaprax.image-agent-protocol.v2`; the existing `serve-image` command and
read-only v1 method/catalog/result bytes are unchanged. No request can switch
profiles or acquire source-write, runtime test, process, or build authority.

## Capabilities and lifecycle

V2 adds `candidate_prepare` to `semantic_read`. It inherits v1's host-bound
manifest, strict duplicate-key codec, 65,536-byte request frames, 1,048,576-byte
response frames, silent non-executing notifications, and held-source
authentication. Every admitted operation authenticates the original Project
before execution and after rendering its complete response. Any observed
source drift permanently invalidates all candidate and draft access.

Candidates and drafts are immutable process-local objects. A successful
operation returns a new digest handle and retains its predecessor unchanged.
No human source file, Git state, lock, managed generation, or disk cache is
written. EOF drops the registries after the final authentication check.

There are at most 16 candidate entries and 16 draft entries. A conservative
256 MiB serialized-report budget includes each candidate report and every
draft's report plus its privately retained last-valid candidate report, even
when the corresponding public candidate handle was discarded. Shared `Arc`
objects can be counted more than once. This is not a total-memory bound for
retained HIR. Exact duplicate handles reuse existing entries.

Registry mutation follows preparation, semantic validation, capacity admission,
complete response bounding, and the final held-input recheck. Any failure leaves
registry entries unchanged. An oversized v2 result returns `-32001` without
mutating the registry and permits a smaller follow-up query; oversized input
still terminates. Discard removes only the named handle. Sibling candidates and
drafts that retain their own private candidate remain usable.

## Methods

All new methods require the exact startup `image_revision`. Candidate methods
also require `candidate_revision` except initial open, recovery restore, and validation catalog.
Draft methods require `draft_revision` except initial hole creation. These are
content digests, not filesystem paths or authority credentials.

| Method | Additional parameters | Result |
| --- | --- | --- |
| `candidate/open` | none | Compact unchanged candidate handle |
| `candidate/apply-intent` | `candidate_revision`, structured `intent` | New independently source-validated candidate handle |
| `candidate/query` | `candidate_revision`; optional `offset`, `chunk_bytes` | Bounded UTF-8 chunk of exact canonical candidate report |
| `candidate/recovery-export` | `candidate_revision`; optional `offset`, `chunk_bytes` | Exact canonical complete-candidate capsule UTF-8 chunks |
| `candidate/recovery-restore` | structured `capsule` | Replayed candidate handle from this session's admitted original base |
| `candidate/validate` | `candidate_revision` | Independent replay of retained changes from base, tests not run |
| `candidate/impact` | `candidate_revision`, `target`; optional `depth`, `max_bytes`, `max_nodes` | Candidate-bound Project semantic impact |
| `candidate/compare` | `candidate_revision`, `other_candidate_revision` | Existing descriptive same-base comparison |
| `candidate/merge` | `candidate_revision`, `other_candidate_revision` | Reconciled candidate plus bounded report; shared original base retained |
| `candidate/rebase` | `candidate_revision`, `new_base_candidate_revision` | Replayed candidate on the selected retained candidate's admitted revision |
| `candidate/discard` | `candidate_revision` | Confirmation that only that handle was removed |
| `change/catalog` | `candidate_revision`, `target` | Target-specific constructor discovery; arbitrary payload legality is not promised |
| `expression/catalog` | `candidate_revision`, `target` | Revision-bound expression identities, expected types, scope, and replacement eligibility |
| `protocol/constructor-schemas` | none beyond `image_revision` | Self-contained closed typed-expression, intent, and change-envelope JSON Schemas |
| `validation/catalog` | none | Available independent replay and external gates still required |
| `hole/open` | `candidate_revision`, `target`, `hole_id`; optional `draft_revision` | New draft with a body hole, or new sibling with another hole |
| `hole/query` | `draft_revision`, `hole_id` | Bound typed context for one unresolved hole |
| `hole/fill` | `draft_revision`, `hole_id`, structured `expression` | New draft after full existing candidate admission |
| `hole/complete` | `draft_revision` | Candidate handle only when zero unresolved holes remain |
| `hole/discard` | `draft_revision` | Removes only the named draft |

`intent` is passed through `SemanticChange::new` with the selected candidate's
current Project revision and compiler-owned mandatory requirements. It is not
a graph mutation, source string, or arbitrary patch. The existing semantic
change implementation owns the closed nested constructor grammar.

Merge requires the library's shared original-base and conflict checks; its
resulting source diff remains anchored to that original base. Rebase selects
only an existing candidate registry entry's admitted Project revision. Neither
accepts source bytes, a path, a caller-created HIR, or an unregistered revision.
Both routes replay through the existing candidate source and target checks.
Their response combines a compact candidate handle and the exact reconciliation
report, capped at 65,536 bytes before wrapping. Report overflow, conflict,
registry capacity, or authentication failure leaves every registry entry
unchanged. The complete resulting candidate report remains available through
the existing chunk query. These are conservative bounded operations, not a
general source merge or behavioral-equivalence proof.

`candidate/query` starts at byte offset zero. `chunk_bytes` is 1024–65,536
(default 16,384). Offsets must be UTF-8 character boundaries within the canonical
report; the response supplies `next_offset` until complete. Concatenating chunks
reproduces the exact candidate report including its terminal LF. The digest
binds every chunk selection; clients cannot select an arbitrary file. Impact
uses the existing context byte/node/depth bounds and adds the selected candidate
handle without changing the nested Project impact schema.

Additive v5 `candidate/impact-summary` and `candidate/impact-page` methods
provide compact, candidate/query-bound navigation over the exact final
candidate impact artifact. [Project Candidate Impact Navigation
v1](PROJECT-CANDIDATE-IMPACT-NAVIGATION-V1.md) owns their bounds, opaque
references and strict nonclaims. They do not change the v2 `candidate/impact`
request or payload bytes.

`hole/open` without a draft begins from the specified candidate; with a draft,
that draft must belong to the same original candidate. Hole IDs are bounded to
128 ASCII identifier bytes; nested hole context and constructor limits belong to
[Project Candidate Holes v1](PROJECT-CANDIDATE-HOLES-V1.md). A draft never exposes
materializable source or a candidate revision for its incomplete state. Its
last-valid candidate can escape only through successful completion with no
remaining holes. Failed fills preserve the original draft and siblings.

## Discovery and nonclaims

The merged v2 method catalog drives dispatch, parameter schemas, query discovery,
version-matched instructions, and generated TypeScript/Python/Rust source
helpers. V2 results use `semaprax.image-agent-result.v2`; discovery payloads use
their additive `.v2` schemas. Existing semantic payload schemas stay unchanged.
Closed RPC envelopes retain their existing constructor URN references.
`protocol/constructor-schemas` supplies the matching self-contained closed
documents described in [Candidate Constructor Schemas
v1](CANDIDATE-CONSTRUCTOR-SCHEMAS-V1.md). These describe structural grammar;
the compiler still owns lexical, scope, type, effect, and ownership admission.
Complete nested semantic response/HIR schemas and a runtime JSON Schema
validator are not bundled. Generated helpers perform no transport or filesystem
I/O themselves.

Candidate validation reparses canonical source and independently replays
existing compiler-owned target projections. It does not execute generated code,
run Project tests, establish behavioral equivalence, approve changes, or commit
source. Required external gates remain reported as unrun. Comparison remains
descriptive; callers must explicitly select the separate merge/rebase methods
for their bounded reconciliation behavior.

`tests/image_protocol/candidate_transport_v2.rs` contains authored coverage for profile
separation, catalog/client consistency, immutable siblings, canonical report
chunks, replay validation, invalid-intent atomicity, stale handles, hole
completion boundaries, candidate/draft capacity, retained draft lifetime after
candidate discard, absorbing Unix source drift, retained-base merge/rebase,
closed constructor documents, and expression discovery. None were executed here.

## Caller-managed recovery

[Complete candidate recovery capsules](PROJECT-CANDIDATE-RECOVERY-V1.md) are
content-addressed replay inputs. `candidate/recovery-export` derives the whole
bounded capsule before slicing; chunk size is an output bound, not a work bound.
`candidate/recovery-restore` canonicalizes its structured object and independently
replays it against this session's startup Project revision. The existing strict
64 KiB request limit applies, including the JSON-RPC envelope; use the CLI for
larger capsules. Restore does not accept another retained candidate as a base.
A capsule originally based on a derived/rebased revision needs that exact source
revision independently admitted by a new host session.

Restore prepares a complete typed candidate and bounded response before registry
mutation and final held-source authentication. Malformed/stale/full-registry
failures leave existing handles untouched. A source-drift failure permanently
invalidates the session as before. No method persists or restores drafts,
unresolved holes, approvals, capabilities, session state, or warm HIR.
