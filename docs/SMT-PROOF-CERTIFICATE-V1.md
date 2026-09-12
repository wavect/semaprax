# SMT Proof Certificate v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: agent and tool authors and third-party reviewers who need to
independently re-check a bounded SMT discharge result (#184) without
trusting the SEMAPRAX exporter that produced it, plus compiler contributors
extending the proof-kernel export line (#186) or the Assurance Manifest
obligation join (#129, #183).

`semaprax.smt-proof-certificate.v1`
(`../src/assurance_manifest/proof_certificate/`) exports one [Bounded SMT
Discharge v1](SMT-DISCHARGE-V1.md) attempt as a standalone, self-contained
document bound to the exact source bytes, declaration, and ensures clause it
was produced from. It is proof data, not permission: exporting or verifying
a certificate never runs a target, discovers or runs project tests, writes
source, or removes a runtime guard, and a certificate grants no execution,
publication, or signing authority.

## Why a separate document, not a manifest merge

[Bounded SMT Discharge v1](SMT-DISCHARGE-V1.md) documents two real
integration gaps between its own `MethodRecord` output and the Assurance
Manifest's `generate()` pipeline: `render.rs`'s `nonclaims` array
unconditionally asserting `no_smt_solver_invoked`, and `ExternalRecords`
failing closed (`SPX-Z101`) when a caller tries to merge an SMT method into
an obligation `derive.rs` already populated with a `runtime_guarded` record
for the same clause. This tranche does not attempt either fix — both belong
to the candidate-assurance join (#129) that owns multi-producer method
merging, and both files were under separate concurrent development. Instead,
this tranche routes around the gap entirely: a proof certificate is its own
document with its own schema, produced and verified independently of
`assurance_manifest::generate`/`verify_envelope`. Merging a certificate's
finding into a manifest, when that join lands, is expected to consume this
document's `verdict`/`script`/`counterexample` fields as its `MethodRecord`
inputs, not to change this schema.

## Independence, not merely a wrapper

A certificate that only the exporter which produced it can check is not
independently checked. Every field a third party needs is embedded verbatim,
not merely referenced by digest:

- The exact `QF_LIA` SMT-LIB2 script text (`script`), byte-identical to what
  [`smt_discharge::render_postcondition_script`](../src/assurance_manifest/smt_discharge.rs)
  produced. Any third party can feed this text to *any* QF_LIA-conformant
  solver — not only the one this exporter happened to run — and read the
  `unsat`/`sat` verdict for themselves.
- For a `refuted` certificate, the exact concrete counterexample values
  (`counterexample.model`), so a third party can hand-evaluate the source
  declaration's `requires`/`ensures`/body against SEMAPRAX's documented
  checked-arithmetic semantics without running any code this compiler
  ships, let alone trusting it.

Two independent-replay functions build on this:

- `verify_certificate` replays the certificate's own internal consistency
  (envelope/payload digests, byte counts, closed vocabularies, the
  `proved`/`refuted` verdict's coherence with `counterexample`) without
  touching a filesystem. It cannot by itself confirm the certificate was
  produced from any particular source.
- `verify_certificate_against_source` additionally re-derives the SMT-LIB2
  script from the **current** source bytes, using the same deterministic
  translation the exporter used, and requires byte-for-byte equality with
  the embedded `script` before accepting anything. For a `refuted`
  certificate it also independently re-evaluates the recorded
  counterexample against `smt_discharge::replay_function`'s
  checked-arithmetic evaluator, which shares no code with the SMT-LIB2
  translator. Neither of these steps trusts the certificate's own recorded
  verdict for what it can independently recompute; only the "does an SMT
  solver actually say `unsat` for this exact script" step is left to a real
  solver run.
- `verify_certificate_with_solver` performs everything
  `verify_certificate_against_source` does, plus — only for a `proved`
  verdict, and only because the caller explicitly supplied a provisioned
  solver — re-runs the certificate's exact embedded script and requires the
  fresh result to also be `unsat`. It never spawns a process for a
  `refuted` certificate (already independently validated by checked-
  arithmetic replay, strictly stronger evidence than a second `sat`), and it
  never spawns a process at all unless the caller passes an explicit
  `Provisioning` — this module never itself consults
  `SEMAPRAX_SMT_Z3_PATH`.

## The bug class this design specifically defends against

Issue #184's own worst bug gave `result` the same unconditional range axiom
a genuine parameter gets, when `result` is *defined* to equal the body's
term; asserting both made a real overflow query vacuously `unsat`, and a
genuine `i64::MAX + 1` defect was reported as proved. A certificate that
only recorded "the solver said unsat" would have no way to catch a
recurrence of that bug, or a hand-edited script exploiting the same
pattern — it would just repeat the false claim.

Because `verify_certificate_against_source` instead re-derives the *exact*
script from the unchanged source and requires byte equality with the
embedded script, it rejects any certificate whose script was not genuinely
produced by the real translator for the exact source and ensures clause it
claims. `proof_certificate::tests::verify_certificate_against_source_rejects_a_script_tampered_with_the_known_vacuous_result_axiom_bug_class`
reproduces this exact tampering (injecting an unconditional range axiom for
`result` into an otherwise-honest script) and confirms it is rejected, even
though the tampered certificate is perfectly self-consistent by its own
internal digests. `provisioned_z3_verify_certificate_with_solver_rejects_a_falsely_claimed_proved_certificate`
goes one step further with a live solver: a certificate whose script is the
honest translation of its bound source, but whose `verdict` field falsely
claims `proved` for a postcondition that is not actually unsat, is rejected
only once `verify_certificate_with_solver` actually re-runs the exact
embedded script and gets `sat` back — demonstrating the property that
matters most: a third party does not need to trust this exporter.

## Wire schema

```json
{
  "schema": "semaprax.smt-proof-certificate.v1",
  "digest": "sha256:<64 lowercase hex>",
  "bytes": 1234,
  "payload": {
    "schema": "semaprax.smt-proof-certificate.v1",
    "source": { "path": "...", "revision": "...", "sha256": "sha256:..." },
    "declaration_id": "app.mod.f",
    "obligation_id": "semaprax.obligation.v1:...",
    "ensures_index": 0,
    "compiler_version": "0.4.1",
    "bounds": "semaprax-smt-discharge-bounded-subset-v1: ...",
    "solver": { "identity": "z3", "version": "4.12.5" },
    "limits": { "timeout_ms": 5000, "max_output_bytes": 65536 },
    "script": "(set-option :timeout 5000)\n(set-logic QF_LIA)\n...",
    "script_sha256": "sha256:...",
    "verdict": "proved",
    "counterexample": null,
    "nonclaims": ["no_assurance_manifest_merge", "..."]
  }
}
```

`verdict` is `"proved"` (a genuine `unsat` result; `counterexample` must be
`null`) or `"refuted"` (a `sat` model that independently replayed as a
validated concrete counterexample; `counterexample` must be non-null).
There is no third verdict: an inconclusive discharge attempt (no solver
provisioned, unsupported subset, timeout, `unknown`, a crashed/malformed
run, or an unvalidated `sat` model) has nothing to certify, so
`export_postcondition_certificate` returns a diagnostic (`SPX-Z105`)
instead of a certificate.

`counterexample`, when present:

```json
{
  "kind": "trapped",
  "detail": "checked add overflowed: 9223372036854775807 + 1",
  "ensures_index": null,
  "model": [{ "name": "a", "sort": "int", "value": "9223372036854775807" }]
}
```

`kind` is `"trapped"` (`detail` non-null, `ensures_index` null) or
`"ensures_violated"` (`detail` null, `ensures_index` the violated clause's
index) — mirroring `smt_discharge::ReplayOutcome`'s two certifiable
variants exactly (`Inconsistent` is never certified: an unvalidated model
carries no claim). `model` entries are sorted in strict ascending `name`
order; each entry's `sort` is `"int"` or `"bool"` and `value` is that
sort's plain decimal/`true`/`false` text — not an SMT-LIB2 term — since
this is a certificate field, not solver input.

## Digest domains

Three independently domain-separated SHA-256 digests, each computed as
`sha256(domain \0 || len(bytes) as u64 LE || bytes)` exactly like the
Assurance Manifest's own digests, but with this schema's own domain
strings so the two documents' digests are never confusable:

- `semaprax.smt-proof-certificate.source.v1\0` binds `source.sha256` to the
  exact source bytes.
- `semaprax.smt-proof-certificate.payload.v1\0` binds the envelope `digest`
  to the exact `payload` bytes (the literal substring between `"payload":`
  and the envelope's final `}`, not a re-serialization — re-serializing
  through `serde_json` would reorder object keys and silently produce a
  different byte sequence than the one actually digested).
- `semaprax.smt-proof-certificate.script.v1\0` binds `script_sha256` to the
  exact embedded `script` bytes.

## Determinism

`export_postcondition_certificate` is a deterministic function of the exact
source bytes, the declaration, the ensures index, the solver's own
determinism, and the fixed rendering in
`../src/assurance_manifest/proof_certificate/render.rs`:
`proof_certificate::tests::rendering_is_deterministic_for_a_proved_certificate`
and the live `provisioned_z3_certificate_export_is_byte_identical_across_repeated_runs`
both confirm repeated export from identical inputs produces byte-identical
certificates.

## Drift and the mutation ladder

`verify_certificate_against_source` fails closed (`SPX-Z107`, drift) when:

- the current source bytes' digest no longer matches `source.sha256`
  (`verify_certificate_against_source_rejects_after_source_drift`);
- the current source's semantic revision no longer matches `source.revision`;
- the named declaration is no longer present in the (digest-matching) bound
  source, or is no longer inside the bounded SMT-discharge subset, or no
  longer has an `ensures` clause at the recorded index — reachable only
  through a hand-crafted certificate whose claims do not match its own
  (legitimately bound) source file, since matching source bytes
  deterministically reproduce the same declaration set
  (`verify_certificate_against_source_rejects_a_declaration_absent_from_the_bound_source`);
- the certificate's `compiler_version` does not match the currently running
  compiler's `env!("CARGO_PKG_VERSION")` — one of the issue's named failure
  modes, "toolchain version drift can change accepted proofs"
  (`verify_certificate_against_source_rejects_a_compiler_version_mismatch`).

It fails closed for a different reason (`SPX-Z106`, consistency) when the
recomputed script does not byte-match the embedded one, or a `refuted`
certificate's recorded counterexample does not independently replay —
see the bug-class section above.

## Scope and honest limitations

- Only postcondition (`ensures`) discharge is certified; precondition
  consistency (`unsat` meaning "contradictory `requires`", not a proof of
  any obligation) is out of this tranche's scope.
- No compiled backend artifact (native/Wasm) is bound; this certificate's
  only "artifact" is the SMT-LIB2 script text itself. Binding a proof to a
  generated backend artifact's digest is future work (issue #186's
  "Artifact binding for one target").
- A `proved` verdict's `unsat` claim is not, and cannot be, confirmed by
  `verify_certificate_against_source` alone — that requires a real solver.
  `verify_certificate_with_solver` provides this, strictly opt-in, but a
  genuinely third-party-only check still only requires the embedded script
  text and any conformant solver, not this crate.
- `verify_certificate_against_source`'s script re-derivation depends on
  this exact compiler's deterministic translator; it is the strongest check
  available without a solver, not a substitute for one.
- Evidence for this tranche is local, developer-machine: unit and
  integration tests exercise rendering, structural replay, drift, and the
  mutation ladder without a solver process; a second, `#[ignore]`d tranche
  requires `SEMAPRAX_SMT_Z3_PATH` and was run against a provisioned Z3
  4.12.5 to confirm export, independent replay, live solver re-verification,
  and the adversarial false-claim rejection all hold end-to-end. See the
  top-level report for exact counts; nothing here is hosted or CI-evidenced.
