# Lean obligation export v1

Audience: compiler contributors and proof-tooling authors.

Status: one tranche of [issue #186]. Owns three wire identities:

| Identity | Produced by | Consumed by |
| --- | --- | --- |
| `semaprax.lean-obligation-export.v1` | `proof_export::lean::export_module` | a pinned Lean 4 toolchain |
| `semaprax.lean-export-coverage.v1` | `proof_export::certificate::render_coverage` | a reader asking what was *not* covered |
| `semaprax.lean-proof-certificate.v1` | `proof_export::export_obligation_certificate` | `proof_export::verify::*` and any independent replayer |
| `semaprax.lean-proof-program-root-binding.v1` | `proof_export::bind_certificate_to_program_root` | `proof_export::verify_certificate_against_program_root` and the exact Assurance Manifest method attachment |
| `semaprax.law-trust-chain-view.v1` | `proof_export::render_trust_chain_view` | read-only clients displaying exact source, proof, artifact, lowering, and runtime status; see [Law Trust Chain v1](LAW-TRUST-CHAIN-V1.md) |

`src/proof_export/` implements this profile. This document owns translation,
trusted base, result grammar, and certificate schema. If code and specification
disagree, `src/proof_export/tests.rs` is the tiebreaker; fix the discrepancy,
never ignore it.

## What has and has not been executed

The pinned Lean 4.34.0 toolchain **has now been run** against the committed
golden export. `scripts/lean-export-gate.py --require-kernel` is a step of
the `kernel0-lean-proof-gate` job in `.github/workflows/ci.yml`, which
provisions a sha256-pinned `elan` and the exact toolchain named by
`proofs/kernel0-lean/lean-toolchain`, and which **is** in `release-gate`'s
blocker set; see [quality gates](QUALITY-GATES.md). `--require-kernel` makes
an absent or mismatched kernel a failure rather than a skip, so the job
cannot go green having checked nothing.

**What that does and does not license.** Once a hosted run of that job has
completed, its result is hosted evidence for the commit it ran on, and
nothing more. Until then, every kernel result quoted in this document was
produced on one developer host and is local evidence. Nothing here, in the
generated Lean, or in a certificate may be described as production,
physical-device, or current-head evidence on the strength of a wired job:
wiring a gate is not the same as having run it.

What is now evidenced, and by what:

- **Lean accepts the generated proofs.** `omega` discharged both the
  checked-range obligation and the postcondition of the golden document,
  with axiom sets `[propext, Classical.choice, Quot.sound]` and
  `[propext, Quot.sound]`. Three consecutive runs were byte-identical.
  Evidence: `src/proof_export/testdata/shifted.kernel-output.txt`, recorded
  verbatim, re-checked against the parser by ordinary `cargo test` and
  re-derived from the live kernel by `scripts/lean-export-gate.py`.
- **A real `sorry` is refused.** Seeded into the postcondition proof,
  the kernel reports it and `parse` returns `admitted_hole`. Evidence:
  `testdata/shifted.kernel-output.sorry.txt`.
- **A vacuously weakened theorem is genuinely accepted by the kernel**, with
  an axiom set *cleaner* than the honest proof's (`does not depend on any
  axioms`) and exit 0. This is recorded as data, not argued as prose:
  `testdata/shifted.kernel-output.weakened.txt`. It is why a certificate
  binds `lean_source_sha256` and embeds the document, and why
  `verify_certificate_against_source` re-renders from source rather than
  trusting the embedded bytes. Kernel output alone can never catch it.
- Deterministic rendering, total coverage accounting, refusal of every
  non-proof output shape, and fail-closed certificate replay remain **local
  test evidence**, verified by running them.

One real defect surfaced only by running the kernel: Lean 4.34.0 prints
``declaration uses `sorry` `` with **backticks**, so the two single-quoted
spellings `kernel_report::parse` was written against (from synthesized
fixtures) never matched, and the warning half of the admitted-hole check was
dead against the very toolchain this module pins. Only the `sorryAx`
axiom-line clause was load-bearing. `parse` now normalizes the quoting; the
regression is
`a_real_sorry_in_the_golden_document_is_refused_as_an_admitted_hole`.

## Relationship to what already existed

This is not a second proof pipeline. It reuses, rather than restates:

- the bounded-subset vocabulary of
  [`assurance_manifest::smt_discharge`](SMT-DISCHARGE-V1.md) —
  `UnsupportedReason`, `Sort`, `NumericMode` and its checked-range facts;
- `smt_discharge::postcondition_obligation_id` for postcondition identity,
  so one obligation has one id whichever method addresses it;
- `assurance_manifest::proof_certificate::ExternalKernelCapability`, the
  seam that module introduced for this issue, together with its
  "binding checks first, capability second" ordering guarantee;
- the Lean pin of `proofs/kernel0-lean/lean-toolchain`
  (`leanprover/lean4:v4.34.0`, issue #188). A test fails if this export's
  `PINNED_TOOLCHAIN` constant ever diverges from that file.

One piece is restated rather than reused: `subset::sort_of_type` is private
to `smt_discharge`, so `profile::sort_of_type` repeats its five-line type
mapping. The *types* are still the shared ones.

## The profile

`semaprax-lean-export-profile-v1` is a strict narrowing of the bounded
SMT-discharge subset. A declaration is admitted **wholesale or not at all**;
there is no partial translation, so an un-translated construct can never sit
silently inside an exported theorem.

Admitted:

- `fn` declarations with no type parameters, no declared effects, and every
  parameter by value;
- parameter, `let` and return types drawn from `{i64, i32, u8, usize}`;
- at least one `ensures` clause;
- expressions: integer and boolean literals, variables, `result` inside
  `ensures`, unary `-`/`!`, binary `+`/`-`/`*`, the six numeric
  comparisons, `&&`/`||`, block expressions whose statements are all
  immutable `let`.

Refused, each with its own closed reason code:

| Code | Construct |
| --- | --- |
| `generic_function` | `fn f<T>(..)` |
| `effectful_function` | any declared effect |
| `ownership_param_mode` | an `own`/`borrow`/`shared` parameter |
| `bool_valued_position` | `bool` as a parameter, `let`, or return type |
| `conditional_expression` | `if` |
| `expr` | calls, division/remainder, floats, strings, aggregates, `match`, `try`, projection, closures |
| `non_let_statement` | assignment, `unsafe`, `while`, `for` |
| `mutable_local_binding` | `let mut` |
| `operand_sort`, `operand_type_mismatch`, `type_mismatch` | operand sorts that disagree |
| `unknown_name` | an unresolved variable |
| `no_ensures_clause`, `no_contract_clauses` | nothing to export |
| `unsupported_value_type` | a type outside the five scalars |
| `non_function_declaration` | records, variants, classes, resources, interfaces, protocols, implementations, agents |

`bool` is excluded as a *value* because contract clauses translate to Lean
`Prop`s; admitting a `bool` value would need a `Bool`/`Prop` coercion this
profile does not introduce. `if` is excluded because `ite` in every goal
takes the emitted proof past the linear-arithmetic tactic budget — deferred
rather than emitted unproved.

## The translation, and why the model is faithful

Every admitted scalar becomes Lean's unbounded `Int`. On its own that would
be **unfaithful**: SEMAPRAX arithmetic is checked (trapping, never
wrapping), so `a + b` at `i64` is not mathematical addition.

The gap is closed exactly the way `smt_discharge` closes it for SMT:

- A **parameter's** declared range is a *hypothesis*
  (`h_lo_i : min ≤ v`, `h_hi_i : v ≤ max`).
- An **arithmetic node's** range is an *obligation*: a separate theorem
  `min ≤ term ∧ term ≤ max`.

These two shapes are deliberately not interchangeable. Giving a computed
term the unconditional range hypothesis a parameter gets is issue #184's
worst bug (it made a genuine overflow vacuously provable), and
`a_parameter_range_is_a_hypothesis_and_an_arithmetic_range_is_a_goal` pins
the distinction.

A postcondition is claimed only when every range obligation of the same
declaration was kernel-checked in the same run. Under that conjunction the
`Int` term and the runtime value coincide on every input satisfying the
hypotheses, so proving the postcondition over `Int` proves it for the
trapping semantics. `verify_certificate` re-checks this from the
certificate alone: an obligation belonging to the certified declaration that
carries no kernel axiom set is a rejection.

Hypothesis scoping follows evaluation order. Binders are appended as the
walk proceeds (parameters, parameter ranges, `requires_0..n`, each `let`
definition, `result`), and each obligation records how many binders were in
scope when it was discovered. A range obligation arising inside
`requires[1]` therefore may assume `requires[0]` but not `requires[2]`,
`result`, or any later `let`.

The right side of `&&` or `||` runs only when the left side requests it.
Range goals inside that right side now carry the corresponding Lean
propositional path guard; an overflow in an unevaluated right side is not
an unconditional goal. The shared typed VC subject records the same
authored order, scalar modes, and path choices for the common subset. Lean
lowers that typed expression tree directly, while retaining its narrower
no-conditional profile, and checks its operation inventory before exporting.

A source certificate also requires a concrete, checked precondition witness
from a fixed 256-state scalar search before invoking Lean. Failure to find
one is reported as unknown and refuses practical-law attachment; finite
search exhaustion is not a contradiction proof. This local witness is test
evidence for domain non-emptiness, not a Lean theorem of existence or a
claim about callers. Source-bound certificate replay repeats the same
deterministic check.

### Names

A theorem name is `spx_<escape(stable_id)>_<kind>_<index>` inside the fixed
namespace `SemapraxExport`. `escape` keeps ASCII alphanumerics and rewrites
every other byte as `_` plus two lowercase hex digits — including `_`
itself, which makes the encoding prefix-free and therefore injective. Two
distinct stable ids can never collide on one theorem name, which is the
issue's named "name sanitization can collide and misassociate declarations"
failure mode.

Local binders are `v_<escape(name)>` for parameters and
`v_<escape(name)>_<n>` for `let` bindings (the counter makes shadowing
harmless); hypotheses are positional (`h_lo_i`, `h_hi_i`, `h_req_i`,
`h_def_n`, `h_result`), so no source identifier can collide with one.

### Determinism

The rendered bytes are a pure function of the parsed program: source order
everywhere, no map iteration, no timestamps, and **no host path** — the
header carries the module name and semantic revision only, so two checkouts
render identical bytes. Rendering uses plain `format!`, not the budgeted
formatter, because these bytes are bound by digest and a silent truncation
under an ambient output budget would be a determinism bug.

`src/proof_export/testdata/shifted.lean.golden` pins the output for one
fixture module. Re-pin it deliberately with
`cargo test -p semaprax --lib rewrite_the_pinned_lean_golden -- --ignored`
and review the diff.

## Trusted base and assumptions

Eight entries, defined once in `lean::ASSUMPTIONS` and reproduced verbatim
in the generated Lean header, the coverage report, and every certificate:

| Id | Assumption |
| --- | --- |
| `A1-int-model` | scalars modeled as Lean `Int` with range hypotheses; faithful only with A2 |
| `A2-range-obligations-required` | a postcondition counts only when every range obligation of the same function was checked in the same file |
| `A3-requires-assumed` | `requires` clauses are assumed, never proved; nothing is claimed about callers |
| `A4-lean-standard-axioms` | `propext`, `Classical.choice`, `Quot.sound` trusted; anything else, `sorryAx` above all, invalidates |
| `A5-lean-kernel-tcb` | the pinned Lean toolchain and its kernel are trusted, not verified |
| `A6-translation-tcb` | this Rust translation is trusted and unverified; a translation bug is invisible to the Lean kernel |
| `A7-purity` | the profile is pure, total and effect-free, so evaluation order is unobservable and unmodeled |
| `A8-no-lowering-claim` | nothing claims the backend lowering preserves a proved source theorem |

## Running the kernel

Running Lean is the `proof_export::LeanKernel` capability, supplied by the
caller:

```rust
pub struct KernelRun { pub toolchain: String, pub output: String }
pub trait LeanKernel {
    fn check(&self, lean_source: &str) -> Result<KernelRun, Diagnostic>;
}
```

The opt-in [Installed Proof Tools v1](INSTALLED-PROOF-TOOLS-V1.md) adapter now
implements this capability using explicitly held installed executables and the
bounded registered process runner. The compiler gains no ambient process or
filesystem authority because a proof export exists.

The separate `scripts/lean-export-gate.py` golden regression remains available.
That script confirms the
host's toolchain is the pin, checks the golden document and two seeded
variants, and compares each result byte-for-byte against the committed
transcripts so recorded evidence cannot silently go stale. With no Lean
installed it prints a `SKIP` naming what was not checked and exits 0; it
never fetches a toolchain, and an unpinned or absent kernel is a skip, never
a substitution. `--require-kernel` turns every one of those skips into a
failure, which is how CI runs it: a skip is the right default for a
developer machine that may have no Lean, and the wrong one for a runner
that has just provisioned it.

## Result grammar

`kernel_report::parse(expected, toolchain, output)` is a pure function to a
closed verdict. It is fail-closed by construction: it starts from rejected
and reaches `Checked` only when every expected theorem produced exactly one
`#print axioms` line whose axiom set is a subset of the three standard
axioms.

Refusals, in evaluation order:

| Code | Trigger |
| --- | --- |
| `toolchain_drift` | the reported toolchain is not `leanprover/lean4:v4.34.0` |
| `timeout` | `(deterministic) timeout`, `maximum recursion depth has been reached`, `deep recursion was detected` |
| `admitted_hole` | `declaration uses \`sorry\`` or `'sorry'` (quoting is normalized; 4.34.0 uses backticks), `uses sorry`, or `sorryAx` anywhere in the output |
| `build_error` | any line containing `error:` |
| `unrecognized_output` | no `#print axioms` line at all |
| `missing_theorem` | an expected theorem has no line |
| `duplicate_theorem` | an expected theorem has more than one line |
| `forbidden_axiom` | any axiom outside the standard three |

A timeout, a build error and an admitted hole are refusals, not weaker
forms of success. This mirrors what `scripts/kernel0-lean-gate.py` already
does for the Kernel-0 proof: the elaborated proof term's axiom set, not a
source-text scan, is authoritative.

## Certificate schema

`semaprax.lean-proof-certificate.v1`, an outer
`{schema, digest, bytes, payload}` envelope with its own domain-separated
SHA-256 for each digest kind
(`semaprax.lean-proof-certificate.{source,payload,lean,artifact}.v1\0`).

Payload keys, all required and exact:

`schema`, `export_schema`, `source{path,revision,sha256}`, `module`,
`declaration_id`, `obligation_id`, `ensures_index`, `theorem_name`,
`profile`, `compiler_version`, `kernel{identity,toolchain,standard_axioms}`,
`artifact{target,bytes,sha256}`, `lean_source`, `lean_source_sha256`,
`obligations[]`, `assumptions[]`, `unsupported[]`, `verdict`, `nonclaims[]`.

`verdict` admits exactly one value, `kernel_checked`. The whole generated
Lean document is embedded verbatim, so a third party can hand it to their
own Lean toolchain without trusting this exporter; `unsupported[]` travels
with the claim so a reader is always told what the export refused.

### Additive ProgramRoot association

The certificate wire above remains deliberately unchanged: it is a
single-file artifact and its own `nonclaims` continue to say that it does not
bind a managed-workspace `ProgramRoot`. A caller that holds one immutable,
already admitted `ProjectRevision` may additionally render
`semaprax.lean-proof-program-root-binding.v1` with
`proof_export::bind_certificate_to_program_root`.

That association has its own exact `{schema,digest,bytes,payload}` envelope
and domain-separated payload digest. Its closed payload binds:

- the exact complete v1 certificate bytes, length and domain-separated digest;
- the retained Project's exact ProgramRoot identity, its exact canonical JSON
  byte length and a domain-separated digest of those bytes; and
- exactly one retained source row (`path`, source semantic revision and source
  digest) whose revision must equal the certificate's own source revision.

`verify_program_root_binding` validates the closed association and the exact
certificate bytes without reopening a source. `verify_certificate_against_program_root`
then replays the certificate against the already retained source bytes,
rederives the compiler artifact, and rederives the Project's canonical
workspace revision and ProgramRoot before comparing every association field.
`verify_certificate_with_kernel_against_program_root` performs all of those
checks first and only then invokes a caller-supplied `LeanKernel`; a wrong
source row, sibling Project, certificate, artifact, or ProgramRoot therefore
cannot dispatch a kernel check.

The association is not a second compiler or proof pipeline. It has no
filesystem, process, network, tool-discovery, target-execution, publication,
signing, or candidate-acceptance authority. It says neither that the source
theorem survives lowering nor that a candidate is accepted.

### Assurance Manifest and candidate composition

`proof_export::assurance_method_attachment` requires the exact certificate,
association, retained revision **and a caller-supplied `LeanKernel`**. It first
performs the complete binding replay, then invokes that explicit kernel, and
returns opaque `VerifiedProjectProof` evidence only on the kernel-confirmed
result. There is no public constructor or options field for this evidence.

`project::generate_from_snapshot_with_verified_proofs` is the only composition
route. Before it appends a `theorem_proved` method to the already-derived
`ensure:<index>` obligation, it independently rederives and compares the exact
retained Project revision, ProgramRoot, source row, certificate association,
postcondition identity, declaration identity and source path. The existing
`runtime_guarded` method stays present, and no duplicate obligation row is
rendered. A sibling Project with the same stable id, a changed source row, an
adjacent postcondition, or a stale certificate is a fail-closed `SPX-Z101`
refusal. The certificate's checked-range theorems remain proof-chain
prerequisites, not invented Assurance Manifest obligations.

The resulting envelope remains the unchanged
`semaprax.project-assurance-manifest.v1` schema, so existing Candidate
Assurance composition rebinds and summarizes it in the ordinary path. As with
all formal method records, the candidate summary requires a non-null
`proof_ref`; here it is the exact ProgramRoot-association payload digest. This
composition makes evidence available to the existing path; it does not give
either the manifest or the candidate authority to run Lean or accept a
candidate.

### Replay, fail-closed

1. `verify_certificate` — filesystem-free. Recomputes the envelope digest
   over the exact payload bytes, requires closed key sets and vocabularies,
   recomputes the embedded Lean document's digest, and rejects any recorded
   axiom outside the standard three. A certificate recording `sorryAx` dies
   here with no toolchain and no source in sight.
2. `verify_certificate_against_source` — additionally re-reads the source,
   re-parses and re-verifies it, re-derives the semantic revision,
   **re-renders the Lean document from scratch and requires byte
   equality**, checks the compiler identity, and recompiles the bound Wasm
   core module requiring digest equality. It also requires the payload's
   module, declaration id, ensures index, obligation id, and theorem name to
   select the exact re-derived obligation before any kernel is consulted.
   Re-rendering rather than trusting the embedded bytes is the point: a hand-edited document that weakens a
   theorem or drops an obligation no longer equals what the translator
   deterministically produces, so it is refused even though its recorded
   verdict still reads `kernel_checked`.
3. `verify_certificate_against_artifact` — compiler-free and
   filesystem-free: confirms artifact bytes a caller already holds are the
   bound ones, saying nothing about where they came from.
4. `verify_certificate_with_capability` — bindings first, then the supplied
   `ExternalKernelCapability`. A capability that always confirms can never
   widen what the binding layer already refused.
5. `verify_certificate_with_kernel` — bindings first, then the supplied
   `LeanKernel` rechecks the exact re-rendered document. Its closed Lean
   report parser must accept every current exported theorem, and the exact
   complete canonical obligation inventory and axiom sets for the certified
   declaration must reproduce the certificate's recorded results. A re-sealed
   removal of a non-headline range obligation, or a change from one standard axiom set to another,
   is therefore still refused; merely obtaining a second clean result is not
   enough.
6. `verify_certificate_against_program_root` — association, certificate,
   held source row, semantic revision, compiler artifact, canonical workspace
   and exact ProgramRoot all replay together. This route is in-memory after
   the Project is retained; it does not reopen a raw source path. The kernel
   variant preserves the same binding-first ordering.

Diagnostics: `SPX-Z110` nothing to certify, `SPX-Z111` certificate
inconsistency, `SPX-Z112` drift.

## Non-claims

Recorded in every certificate's `nonclaims`, and binding on any prose
written about this module:

- Kernel-checked status covers only the listed obligations of the listed
  declaration. Declarations under `unsupported` are not proved.
- The v1 certificate itself has **no ProgramRoot binding**: a managed-workspace
  `ProgramRoot` derives from a `SemanticWorkspaceRevision`, while that wire
  binds a single source semantic revision. The optional, separately versioned
  ProgramRoot association above is the only route that adds the wider binding.
- Artifact binding covers only the `wasm-core-module-v1` target. No native
  artifact is bound: native codegen emits C11 source text needing an
  external, unpinned C toolchain this crate does not invoke.
- Artifact binding does not by itself prove the backend lowering preserves
  the source theorem.
- The SEMAPRAX-to-Lean translation is trusted and unverified.
- `requires` clauses are assumed, not proved.
- No target execution, no project test discovery, no source writes.
- A v1 certificate alone is not merged into the Assurance Manifest lattice.
  Only a replayed ProgramRoot association can produce its one exact method
  attachment. In either form proof data grants no execution, publication,
  signing, merge, or candidate-acceptance permission.
- **Kernel evidence is local-host only.** Hosted CI provisions no Lean
  toolchain, so a certificate records a result obtained on whichever host
  ran the kernel. It is not hosted, production, or current-head CI
  evidence.

## Not done in this tranche

- **No hosted result is quoted here yet.** The gate is wired into the
  `kernel0-lean-proof-gate` release blocker, but every kernel result
  transcribed in this document was produced on one developer host. A hosted
  run's verdict becomes quotable when one exists, per commit.
- Installed execution is opt-in through [Installed Proof Tools v1](INSTALLED-PROOF-TOOLS-V1.md).
  `verify_certificate_with_kernel` retains binding-first replay and does not
  acquire tools itself. Complete protected build/transaction admission remains
  open LAW-04 work.
- The installed-tool physical gate additionally runs a newly authored bounded
  `fresh.seventeen` Project postcondition through real Lean, Project/candidate
  proof attachment and the CLI. The
  ordinary structural/replay corpus also contains one wholly admitted
  `app.scalar` module covering `i32` negation, `u8` increment, `usize`
  decrement, and an `i64` entry point; it reaches certificate, exact Wasm
  artifact, retained-Project ProgramRoot association, and one exact
  `theorem_proved` Assurance Manifest method. That second member uses the
  fixture kernel and is **not** evidence that Lean accepted its generated
  theorems. No broader live-kernel corpus is claimed.
- The ProgramRoot association's `program_root`, canonical-root digest, source
  path, source revision, and source digest are individually exercised by
  hostile mutations. Each fails its authenticated closed envelope before a
  supplied kernel sees any bytes. This is binding-first replay coverage, not
  a claim that a malformed association can be repaired or accepted.
- `project-proof-check` explicitly selects an installed kernel and one exact
  retained source postcondition. There is no automatic certificate discovery.
  Embedders can still explicitly supply certificates and a kernel capability
  for exact Project assurance composition.
  There remains no `ObligationKind` for checked-range obligations — they carry
  their own `semaprax.lean-export.range.v1:...` ids precisely so they are not
  mistaken for manifest obligations.
- No mutation ladder over *target* changes (only source, revision,
  compiler, Lean document, and artifact are exercised).
- `if`, division/remainder, `bool` values, records and calls remain outside
  the profile.

[issue #186]: https://github.com/wsdt/semaprax/issues/186
