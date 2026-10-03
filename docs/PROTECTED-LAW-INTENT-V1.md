# Protected Law Intent v1

LAW-03 adds an explicit host-selected specification boundary over LawSet v1.
The public API is `assurance_manifest::law_set::protected`; candidate review is
`ProjectCandidate::protected_law_review`. Neither a review nor an acceptance
record authenticates a person or grants filesystem authority.

## Independently held baseline

The trusted embedding host constructs and retains `ProtectedLawBaseline` from
its independently selected immutable base Project and LawSet. The candidate
cannot choose or replace that object. The baseline digest binds the base law
inventory, proof profile, Project revision, canonical specification projection,
and exact editable implementation identities.

The conservative specification closure contains every selected source and the
canonical Project manifest. The host may explicitly select top-level function
bodies as editable implementation dependencies. Signatures, preconditions,
postconditions, imports, type/domain declarations, helpers, model definitions,
lemmas and test roots remain protected. Changes to editable bodies invalidate
revision-bound proof work. A helper changed to constant true crosses the
specification boundary even if the law module bytes stay unchanged. Class
methods and other unsupported editable shapes are not implicitly exempted.

## Review and proposals

`semaprax.protected-law-review.v1` reports removed/added laws, moved law owners,
changed subjects/propositions, assumptions/lemma dependencies, evidence
requirements, backend/trust profiles and the protected specification closure.
The proposal binds both exact law digests, the baseline digest, base and candidate
revisions and the immutable candidate digest. Renames cannot erase a removal.

Canonical AST equivalence (including formatting and redundant parentheses)
requires no approval. Other semantic comparisons report `unknown`, including
potential strengthening: no implication solver is claimed. Unsupported native
quantification/model syntax is refused by admission rather than receiving a
non-weakening verdict. Admitted domain/model/bound/transition definitions in the
protected source closure cannot change without specification review.

A separate `SpecificationChangeAuthority` callback is implemented by the trusted
host using its own authentication. `SpecificationChangeApproval::request` calls
that host with the exact proposal and refuses a denial. The opaque approval has
no JSON import or public field constructor. Two proposer/reviewer strings and
existing candidate acceptance receipts cannot produce it. The host must not
expose its authority callback as an unauthenticated agent tool.

Approval is valid only for the exact review digest. Replays with another base,
candidate, law set, profile or editable policy fail `SPX-LW104`. Missing approval
for a specification change fails `SPX-LW120`; host denial fails `SPX-LW121`.
Approval of specification intent does not satisfy the law evidence gate.

## Publication and advisory repairs

A host configuring this baseline must use `apply_protected_law_publication`.
It rederives the review and checks approval inside the existing managed Workspace
publication lock, before staging. Existing exact-source drift checks and the
ordinary host publication authority remain required. The generic unprotected
route remains available for hosts that have not selected this profile; the
compiler does not govern arbitrary shell writes or repository administration.
CI must select its baseline and protected route from independently protected host
configuration. Candidate edits to CI configuration cannot replace the host-held
baseline or opt out of this route. Project manifest, source selection and test
input changes are part of the protected closure; protecting the CI gate itself
against arbitrary repository-administration changes is the host's responsibility.

`assurance_policy::evaluate_protected` replaces weakening advisory suggestions
with implementation/proof repair and the separate specification-change path.
The original `evaluate` profile retains historical advisory-only suggestion
labels and is explicitly unprotected. Neither advisory surface performs edits.

The focused executable gate is
`cargo test --locked -p semaprax --test workspace project_assurance_manifest::law_set::protected_law`.

The protected advisory regression is
`cargo test --locked -p semaprax --lib assurance_policy::tests::protected_laws_never_suggest_weakening_repairs`.
