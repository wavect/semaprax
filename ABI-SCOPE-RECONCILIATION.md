# ABI scope reconciliation — handoff note, not a versioned spec

This file is a handoff artifact for the coordinator only. It is deliberately
**not** committed under `docs/` (it is not a frozen specification, carries no
schema identity, and must not be read as one). It maps every issue in the
three overlapping packs — #132-#141, #149-#166, #170-#175 (34 issues) — onto
the three frozen specifications and one reference codec this worker produced
this round, and states plainly which criteria of each issue remain
unaddressed and need execution.

Specs produced this round:

- **BP** = `docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md` (admission profile,
  IN/DEFERRED/EXCLUDED table, bounds, freeze procedure — no classifier code)
- **DESC** = `docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md` +
  `src/public_generic_abi/descriptor.rs` (wire format spec and reference
  codec — no HIR-derived construction)
- **CARR** = `docs/PUBLIC-GENERIC-CARRIER-V1.md` +
  `src/public_generic_abi/carrier.rs` (logical state machine, phase ledger,
  binding codec — no physical/native/Wasm adapter, no execution)

"Owns" below means the artifact is the authoritative source for that issue's
contract, not that the issue is closed. "Remaining" states exactly what
still needs execution against real checked HIR, a real backend, or hosted CI
— none of which this round touches, per the worker contract's "scope
reconciliation and contract freeze only, no compiler/backend implementation."

## Pack 1 — #132-#141 (original design pack)

| # | Title | Owning spec(s) | Remaining |
| --- | --- | --- | --- |
| 132 | Design a new public-generic descriptor and carrier with an executable bounded contract | BP + DESC + CARR | The bounded reference codec exists and is exercised by local hostile/determinism tests; it is not yet derived from real checked HIR, and independent maintainer review of the contested classifications (see BP's "Contested classifications") has not happened — this is the review gate the contract explicitly withholds from self-approval. |
| 133 | Derive experimental generic export descriptors from checked Project/HIR facts | DESC (defines what to derive; the classifier it depends on is BP's, also unimplemented) | Entirely unaddressed: no code reads `ResolvedProgram`/`ProgramRoot` and produces a `DescriptorV1`. This is explicitly out of scope this round. |
| 134 | Implement the native generic-owned carrier and exact allocation/failure settlement | CARR (logical layer only) | Entirely unaddressed: no native C11 adapter exists. CARR's target-mapping table is naming guidance, not an implementation. |
| 135 | Implement the Core-Wasm generic-owned carrier with per-instance ownership and copy-out | CARR (logical layer only) | Entirely unaddressed, and explicitly out of this worker's file ownership (`src/wasm/**` belongs to another worker). |
| 136 | Generate and execute C and C++ callers for the real generic-owned native boundary | none directly; CARR's target-mapping table gives naming guidance | Entirely unaddressed: no generator, no execution. |
| 137 | Generate and execute a safe Rust caller for the real generic-owned boundary | none directly; CARR's target-mapping table gives naming guidance | Entirely unaddressed. |
| 138 | Generate TypeScript/JavaScript callers for the real generic-owned Wasm boundary | none directly | Entirely unaddressed. |
| 139 | Exercise candidate ABI deltas on genuinely admitted generic exports | BP (defines what "genuinely admitted" means) | Entirely unaddressed: no export is admitted yet because BP's classifier does not exist; [Public Generic Candidate Delta v1](docs/PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md) is untouched. |
| 140 | Execute the new generic callable-boundary conformance and malformed-descriptor matrix | DESC + CARR (down payment only) | The codec's own hostile decode/replay tests (`descriptor/tests.rs`, `carrier/tests.rs`) cover truncation, trailing bytes, reordering, unknown schema, oversized length claims, and cross-paired trusted values at the single-implementation reference-codec level. They are **not** the cross-language, cross-engine conformance matrix #140 asks for — that requires the generated consumers from #136-#138/#156-#159, none of which exist. |
| 141 | Prepare and record the explicit generic-owned API support/publication decision | BP (freeze/change procedure only) | Untouched by design: PG-9 is a standing human decision per [the milestone document](docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md#standing-support-and-publication-decision), and this round explicitly does not move it. |

## Pack 2 — #149-#166 (granular implementation spine)

| # | Title | Owning spec(s) | Remaining |
| --- | --- | --- | --- |
| 149 | Epic: Authenticated Public Generic Descriptor and Carrier v1 | BP + DESC + CARR (frame only) | The epic's delivery map (#150-#166) is 3/17 touched by a spec and 0/17 fully closed; see rows below. |
| 150 | Freeze the first Public Generic Boundary Profile v1 | **BP — this is BP's issue** | BP's admission predicate, bounds table, and IN/DEFERRED/EXCLUDED table are frozen. **Not done:** the pure classifier function, the closed refusal enum wired to real diagnostics (only reserved, unallocated `SPX-PG6xx` codes are documented), and every positive/negative/first-over-bound test #150 requires against real HIR — all explicitly deferred to the next round. |
| 151 | Implement Public Generic Descriptor v1 | **DESC — partially** | The wire format, canonical bytes, `encode`/`decode`, and diagnostics (`SPX-PG701`-`704`) are implemented and tested. **Not done:** deriving a `DescriptorV1` from a real checked program (`ResolvedProgram`, `ProgramRoot`, candidate context) — the codec's `InstanceBinding`/digest fields are hand-constructed fixtures in every test. |
| 152 | Add independent descriptor verification and trusted replay | **DESC — partially** | `replay()` and its byte-exact preimage comparison are implemented and hostile-tested. **Not done:** replay against a descriptor independently rederived from real checked HIR rather than a second hand-built fixture. |
| 153 | Define Public Generic Logical Carrier v1 and ownership state machine | **CARR — this is CARR's issue** | The state machine (`CarrierState`/`Event`/`HandleLedger`), phase ledger, sticky settlement, and exact-reverse release-order verification are specified and tested as pure logic. **Not done:** binding the state machine to a real settlement plan derived from checked cleanup facts (this round's fixtures hand-construct obligation orders rather than deriving them via `public_generic_settlement::plan`). |
| 154 | Implement the native C11 provider and carrier adapter | CARR (logical layer only) | Entirely unaddressed. |
| 155 | Implement the Core Wasm provider and carrier adapter | CARR (logical layer only) | Entirely unaddressed; also out of this worker's file ownership. |
| 156 | Generate and execute a Rust public-generic calling consumer | none | Entirely unaddressed. |
| 157 | Generate and execute a TypeScript/Wasm public-generic calling consumer | none | Entirely unaddressed. |
| 158 | Generate and execute a C11 public-generic calling consumer | none | Entirely unaddressed. |
| 159 | Generate and execute a C++17 move-only public-generic consumer | none | Entirely unaddressed. |
| 160 | Add the cross-language hostile descriptor and carrier replay corpus | DESC + CARR (single-language down payment) | Same status as #140 above: local Rust-only hostile coverage exists; the shared cross-language corpus does not. |
| 161 | Integrate a real public-generic signature into candidate delta and compatibility | BP (defines the signature that would be integrated) | Entirely unaddressed; depends on #150's classifier. |
| 162 | Prove cross-engine allocation, transfer, copy-out, and failure settlement | CARR (logical rules only) | Entirely unaddressed: no engine executes anything here. |
| 163 | Refresh Linux, macOS, and Windows hosted evidence for the expanded milestone corpus | none | Entirely unaddressed; explicitly local-only evidence this round. |
| 164 | Run exact-head release-candidate convergence and freeze the evidence record | none | Entirely unaddressed. |
| 165 | Make the explicit PG-9 support and publication decision | none (BP explicitly declines to move it) | Untouched by design — human decision. |
| 166 | Build the Everyday Agent end-to-end validation product | none | Entirely unaddressed. |

## Pack 3 — #170-#175 (descriptor/carrier pair)

| # | Title | Owning spec(s) | Remaining |
| --- | --- | --- | --- |
| 170 | Define Public Generic Descriptor v1 for exported concrete generic instances | **DESC — this is DESC's issue, alongside #151** | Same status as #151. One acceptance gap worth flagging explicitly: #170 asks the descriptor to "represent... ordered arguments, records/variants, owned leaves" as apparently separate wire-visible structure; this round's wire format carries only the canonical `term` (which recursively encodes ordered arguments) and `instance_digest` (which commits to the field tree and owned-leaf paths), not a separately flattened arguments/fields/owned-leaves list a codegen consumer could read without also implementing the grammar's term parser. DESC's own document does not promise the flattened form. **This is a real, not-yet-resolved gap between #170's literal wording and what this round built — flagged here rather than silently narrowed.** |
| 171 | Define Public Generic Carrier and Calling Convention v1 | **CARR — this is CARR's issue, alongside #153** | Same status as #153. #171 additionally asks for "Option/Result, and nested concrete generic values" representation; per BP's frozen IN/DEFERRED/EXCLUDED table, `Option`/`Result` are excluded from v1 admission entirely, so CARR defines no representation for them — this is consistent with BP, not an oversight, but is a real narrowing of #171's literal wording that a reviewer should confirm. |
| 172 | Generate and execute Rust, TypeScript/Wasm, C11, and C++ public-generic calling consumers | none | Entirely unaddressed. |
| 173 | Add hostile replay for public-generic descriptor, carrier, and runtime associations | DESC + CARR (single-language down payment) | Same status as #140/#160: local reference-codec hostile tests exist; cross-consumer/runtime-association replay does not. |
| 174 | Execute public-generic allocation, transfer, and failure settlement on all admitted backends | CARR (logical rules only) | Entirely unaddressed. |
| 175 | Complete the public-generic hosted matrix and record the explicit PG-9 support decision | none | Entirely unaddressed. |

## Summary

- **3 of 34 issues** (#150, #153, #170/#171 as one pair — call it 3 distinct
  contracts: BP, DESC, CARR) are the ones this round's specs actually own and
  substantially address at the specification level, plus a reference codec
  for the two that need a wire format.
- **0 of 34 issues** are closeable: every one of them requires either the
  BP classifier (not built), a physical native/Wasm adapter (not built,
  and Wasm explicitly out of this worker's ownership), a generated consumer
  (not built), or hosted CI evidence (not run).
- The highest-leverage unblocking fact for the other 31 issues is that the
  scope conflict named in this worker's assignment is now resolved in one
  place (BP's IN/DEFERRED/EXCLUDED table) instead of three overlapping,
  partially contradictory issue packs — that is what "freeze the contract"
  means for this round, not "implement the epic."
- Two real, not-invented gaps are flagged above rather than silently
  resolved: #170's flattened-argument/field wire representation, and the
  DEFERRED-vs-EXCLUDED classification of generic variants and public
  generic function templates (see BP's "Contested classifications"
  section). Both need independent review, not another pass by this worker.
