# Bounded source review disposition

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

One tool-disabled Sonnet dispatch reviewed the exact5978-byte production diff
and existing helper snippets. It completed in39.15s, exit0, peak process-group
RSS321093632 bytes. Provider list costUSD0.058534; actual billing unknown.
Raw payload/result/receipt are retained. No execution or broad approval follows.

B1 was contingent on omitted owner teardown: nominal_payload.rs Owner<T>::Drop
always drains recursive types through its own retained scratch. It does not
require uniqueness at index drain time. Index drain_legacy_type_roots releases
case views before outer variants; even when inputs retain an Arc, final Owner
Drop remains iterative. Added a512-level synthetic final-owner control and a
missing-later-case partial-state/error control; both await execution.

B2 was contingent on omitted types: nodes.rs defines case.fields as Vec of
ResolvedFieldDeclaration; map.rs already implements From<Vec<_>> for CaseFields
as Owned. Existing hir module privacy permits this sibling path. No map type
or conversion was introduced by this patch.

U1: Shared DerefMut panics; mutation requires explicit paid try_make_mut, which
creates a new payload. This patch retains an immutable view of the original
payload; it does not grant mutation authority or change mutation semantics.
U2/U3: NominalPayload Deref is Vec for both representations, and is_shared is
crate-visible. enumerate order and ordinal indexing share the same Vec.
U4 shadowing has no live binding conflict and needs no semantic change.

The review omitted the full old source. Its complete archive and independently
compiled child are retained; import ordering and three method names are the only
source adaptations. Source proof reconstructs the exact complete archived file.
The additional four control bodies and custody joins were authored outside this
review packet and have not received an independent review or execution verdict.
