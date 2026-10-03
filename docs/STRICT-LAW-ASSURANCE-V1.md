# Strict Law Assurance v1

Status: first LAW-04 implementation batch, not completion of issue #379.
This additive library profile joins independently selected laws to an exact
retained Project and candidate. The opt-in [installed proof-tool adapter](INSTALLED-PROOF-TOOLS-V1.md) adds
bounded real Lean/Z3 source-postcondition checking. Global protected-route
admission and native relational-law checking remain open.

## Inventory and evidence ownership

`assurance_manifest::law_set::strict::StrictLawPolicy` retains a nonempty
independently selected LawSet and exactly one method requirement per baseline
law. Its digest binds the baseline and all requirements. Candidate-supplied
lists of successes do not select required laws. Missing law modules, removed
clauses and added laws without a requirement cannot produce an empty success.

`derive` reuses LAW-01 selection and exact proposition resolution, then derives
Project assurance itself. It accepts only the existing opaque
`VerifiedProjectProof` type for external kernel attachment. The Project owner
rechecks ProgramRoot, Project revision, exact source row, certificate and
postcondition identity before attachment. A wire report, URL, arbitrary digest
or nonempty `proof_ref` is never an attachment. Source-level proof and association
with an artifact hash do not prove lowering preservation.

The existing single-file candidate assurance summary now treats every supplied
SMT/model/theorem classification as an unsupported formal claim. A proof-ref
string does not lift this refusal. The existing acceptance record remains
read-only and authority-free; strict Project admission is a separate profile.

## Per-law method requirements

The profiles are deliberately separate, not a total evidence ordering:

- `compiler_static`: an independently derived compiler method or held static
  architecture claim for the exact law.
- `pinned_lean_source`: an opaque exact Project kernel attachment with the
  pinned toolchain. The host must accept every named assumption in the frozen
  Lean source export profile and the three standard Lean axioms. A policy that
  accepts fewer assumptions fails conservatively; this batch does not infer
  that a particular export did not need one of the profile assumptions.
- `reference_model`: the exact reference-model digest/domain and minimum state,
  depth and transition bounds. Only an independently completed model check
  counts. A weaker depth cannot satisfy a larger requested depth. This proves
  only the selected finite reference model, never an arbitrary Project protocol
  or an unbounded theorem.
- `pinned_smt_source`: exact installed Z3 Project evidence, pinned version and
  explicitly accepted checked-arithmetic translation profile. A structurally
  valid SMT certificate is insufficient. Unpinned `smt_source` still refuses.
- `verified_lowering`: currently refuses; artifact association is insufficient.

Open law assumptions and unmet prerequisite laws remain open. Runtime guards,
report-only settings and untrusted labels cannot satisfy a strict requirement.
Unknown, unavailable or missing evidence never becomes a successful result.

## Reports, candidate binding and publication

`semaprax.strict-law-assurance.v1` binds Project revision, ProgramRoot, policy,
baseline/current law digests and the independently derived inventory report.
It includes every required row, exact evidence, counts and an accepted flag.
The canonical JSON is bounded by LawSet's 1 MiB limit and ends in one LF.
`require` regenerates exact bytes from the independently held inputs before
consulting its own accepted flag. Mismatched/forged/stale receipts fail
`SPX-LW104`; an exact but unsatisfied report fails `SPX-LW130`.

`ProjectCandidate::strict_law_assurance` additionally binds the exact candidate
and requires the policy's base Project revision to match the candidate base.
The additive strict publication preparation and apply functions compose this
coverage gate with LAW-03's intent gate. Both acquire the ordinary Workspace
publication lock before replay. Strict publication evidence binds the law
report and policy; apply rechecks that exact association before staging.
Neither proof evidence nor a specification approval grants publication authority.

## Route inventory and remaining bypass work

| Route | First-batch behavior | Remaining issue #379 work |
| --- | --- | --- |
| Strict LawSet library derive/require | Complete independently selected inventory and exact method predicates | SMT Project attachment and richer proof-provider metadata |
| Strict candidate report/replay | Exact candidate, policy and Project binding | Persist policy selection across general transaction routes |
| Strict publication prepare/apply | Coverage plus intent under ordinary lock, exact proposal replay | Toolchain/CLI selection of this route |
| Existing LAW-03 protected publication | Intent protection only | Require strict coverage when a strict policy is configured |
| Generic candidate acceptance | Formal proof-ref claims refused; record remains authority-free | Unified opted-in strict acceptance configuration |
| Generic candidate/publication and semantic transaction routes | Existing contracts, no global strict-law configuration | Persist and enforce strict selection at every equivalent public route |
| Project/native/Wasm build and run | Existing admission only | Strict build/run joins and final-boundary proof freshness |
| Installed CLI Z3/Lean adapters | Explicit `project-proof-check` source-postcondition route; bounded trusted-local execution and strict-confinement refusal | Native relational laws, global strict policy selection and additional host profiles |

The existing `LeanKernel` embedding capability is a trusted host boundary.
This batch does not turn an arbitrary callback or recorded transcript into
physical Lean evidence. Tests of that boundary must remain labelled as such.
Installed tool execution requires the separate explicit capability or CLI
selection; no external artifact provider is implicit. Issue #379 remains open
until complete protected-route coverage and the remaining law profiles are
implemented and exercised.

## Focused gates

`cargo test --locked -p semaprax --test workspace project_assurance_manifest::law_set::strict_law`
checks complete inventory, method/scope/bound separation, candidate/report/policy
drift and strict managed publication. The candidate assurance unit regression
checks that nonempty forged proof references do not confer formal acceptance.
