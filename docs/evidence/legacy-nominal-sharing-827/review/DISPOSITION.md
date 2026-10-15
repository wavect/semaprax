# DISPOSITION (review)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Two actual bounded tool-disabled Sonnet calls completed. Their raw prompts,
outputs, argv/resource receipts and list-price usage are retained separately.
No actual billing or token/application savings are inferred.

Initial review: conditional mixed-owner/assertion and case-fact overwrite risks
were removed with a Legacy-only gate and full existing-value equality guard.
New recursive COW risk was removed with unique move-out plus iterative cloning.
Existing try_make_mut handles both Shared modes by explicitly paid detach; it
never mutates owner values or mode. Immutable-owner scratch cannot become stale.

Final review verified clone ordering, exhaustive field preservation, unique
move-out, no-payer Legacy acquisition, mode isolation and NLL borrow shapes.
It was conditional, not approval. Its recursive equality concern was subsequently
removed by a borrowed iterative Nominal/Function comparator. ResolvedType has
only those two recursive variants (unchanged source); leaf clone/equality cannot
recurse. The original NominalPayload Clone implementation is byte-identical and
Arc-clones either Shared mode. CaseFields has no paid detach API to misclassify:
funded coordinates continue to panic on implicit mutation. Existing field/case
capacity and drain algorithms are unchanged and census both recursive families.
The postpass is called once after the complete successful Legacy build and is
never called by cache decode. Owner is defined in the ancestor nominal_payload
module, so its private fields are accessible to its child implementation.
The review's reference to HashMap was inaccurate: the actual maps are BTreeMap;
no nondeterministic iteration was introduced. Paid/Legacy handles cannot mix on
one Arc because mode is immutable and no promotion API exists.

The equality helper and private funded-helper fail-closed guard were added after
the final reviewed excerpt; neither is described as independently approved.
All owning Rust execution, physical observations and host behavior remain pending.
