# Public Generic Release-Candidate Evidence v1

Audience: maintainers evaluating issue #164 ("Run exact-head release-candidate
convergence and freeze the evidence record") and anyone reviewing PG-9
(issues #165 / #141).

Status: **partial, and deliberately not a candidate freeze.** Sections A
(contract inventory) and B (implementation map) of #164 are assembled here
from the checked-out tree alone. No candidate SHA is frozen and no local gate
was run to produce Sections A/B. **Revised 2026-09-19** (this issue's own
audit pass): the widened milestone job *has* since executed hosted, at commit
`7def8fb1a727787f989428d77eea603ffd0513cf` — [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593),
all three OS jobs `success` — found by checking `gh run list`/`gh api` job
conclusions rather than only aggregate run conclusions. This is real hosted
evidence, not a frozen #164 candidate: see the corrected §2.16 and §3 below.
Section 3 lists every #164 requirement that remains open.

In plain terms: the recorded runs are useful history, but they do not freeze a release candidate.

**Second revision, 2026-09-19:** [run 35407101886](https://github.com/wavect/semaprax/actions/runs/35407101886)
ran at earlier ancestor `3548dc9af5d5c576c884a83a82024891d950e4fe`, not current
`main`. Its milestone job succeeded on all three hosts although the aggregate
run was cancelled by an unrelated superseded job. It was the first hosted
`--lib public_generic_abi` selector run (307 passed); see corrected §2.16 and
§3 item 4 for the complete citation and counts.

## What this document is, and is not

This document assembles the two evidence sections of #164 that are
**offline-assemblable from the checked-out tree alone**: Section A (contract
inventory) and Section B (exact implementation map). It was produced entirely
by reading source, docs, and fixtures already committed to this working
copy — **no `cargo` command was run to produce it**, and no GitHub Actions run
was triggered or inspected beyond what the cited documents already record.

**This document is explicitly NOT the #164 frozen evidence record.** #164 asks
for eight sections (A through H), a frozen candidate SHA, fresh local gate
execution, and a fresh hosted CI run at that exact SHA. This document supplies
only A and B. [Section 3](#3-what-164-still-requires-explicitly-open) lists,
without hedging, everything #164 still requires and why none of it is
satisfied here. In particular:

- **No candidate SHA is frozen by this document.** This repository is a
  **shared checkout**: other agents commit to it concurrently, and `HEAD`
  moved at least once while this document was being written (see
  [Section 3](#3-what-164-still-requires-explicitly-open)). Any commit hash
  named below is cited as "the commit a specific fact was verified against,"
  never as a frozen release candidate.
- **No hosted CI evidence is claimed here beyond what
  [PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md](PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md)
  now records, as of its own 2026-09-19 corrections.** When this document was
  first assembled, those two documents recorded exactly one hosted run,
  [run 34594793245](https://github.com/wavect/semaprax/actions/runs/34594793245)
  at implementation commit `2ef043ba1b989f49b256e456f71fb6e89068bf33`, which
  predates all PG-5/PG-6/PG-7 calling-consumer, hostile-carrier, and
  settlement code. **That is no longer the only hosted run on record**: this
  issue's own audit found [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593)
  at commit `7def8fb1a727787f989428d77eea603ffd0513cf`, whose three
  `public-generic-ownership-milestone` jobs (105871581195, 105871581214,
  105871581165) are all `success` and cover the complete widened corpus, and
  a same-day follow-up pass found a **third** hosted run,
  [run 35407101886](https://github.com/wavect/semaprax/actions/runs/35407101886)
  at commit `3548dc9af5d5c576c884a83a82024891d950e4fe` (an ancestor of
  `7def8fb1`, not the current head), whose "Public generic ownership
  milestone" job is `success` on all three of ubuntu-latest, macos-latest,
  and windows-latest (ubuntu-latest job id `105799024571`), while that run's
  own **aggregate** conclusion is `cancelled` — see the milestone document's
  own "Reverified 2026-09-19" addendum and this document's corrected §2.16
  for the full citation of both runs. This is real hosted evidence, but
  neither `7def8fb1` nor `3548dc9a` is a frozen #164 candidate (see §3,
  revised).
  Every other identifier, status, and test count below not tied to one of
  these three runs is **local, proof-only, or source-inspection evidence**,
  and is labelled that way throughout. Nothing here upgrades any of that to
  current-head, physical-device, or production support.
- **No test was executed to produce this document.** Every test count cited
  below was a verbatim quotation from an existing, already-committed document,
  cited by file and line, never a number this document's author measured.
  Two of those quoted figures were later found stale by measuring them:
  `public_generic_native_adapter_v1` is **55 cases**, not 16, and
  `public_generic_wasm_adapter_v1` is **25**, not 15. The source document has
  been corrected; the lesson is that quoting a figure faithfully does not make
  the figure true.
- **Line-number citations into `PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md`
  below may be imprecise as of 2026-09-19.** That document grew from 697 to
  840 lines during this issue's own audit pass (new PG-8 hosted-run evidence,
  corrected gate statuses, and a new "Reverified 2026-09-19" addendum). The
  citations most load-bearing for #164 (the PG-8/hosted-run ones) were
  re-verified and corrected against the new line numbers; citations to
  unrelated content (predecessor-format rejections, #229's exact lines, the
  candidate-delta bullet) were not individually re-walked line-by-line. If a
  cited range looks off, search the target document for the quoted phrase
  rather than trusting the line number alone.

## 1. Section A — Contract inventory

Every row cites the exact file and line this session read to verify it. An
entry marked `unverified` was not confirmed against source in this session;
its reason is stated instead of a guess.

### 1.1 Core contracts

| # | Contract | Exact identifier(s) | Verified at |
| --- | --- | --- | --- |
| 1 | Public Generic Boundary Profile v1 | `semaprax.public-generic-boundary-profile.v1` | `docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md:52` |
| 2 | Public Generic Type Grammar v1 | schema `semaprax.public-generic-type-grammar.v1`; term digest domain `semaprax.public-generic-type-grammar.v1.term\0`; template digest domain `semaprax.public-generic-type-grammar.v1.template\0`; instance digest domain `semaprax.public-generic-type-grammar.v1.instance\0` | `docs/PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md:30-33` |
| 3 | Public Generic Descriptor v1 | schema `semaprax.public-generic-descriptor.v1`; identity digest domain `semaprax.public-generic-descriptor.v1.identity\0` | `docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md:42-43` |
| 4 | Public Generic Logical Carrier v1 | schema `semaprax.public-generic-carrier.v1`; binding digest domain `semaprax.public-generic-carrier.v1.binding\0` | `docs/PUBLIC-GENERIC-CARRIER-V1.md:45-46`; schema constant also defined at `src/public_generic_abi/carrier.rs:31` (`CARRIER_SCHEMA`) |

### 1.2 Physical adapter ABIs

| # | Contract | Exact identifier(s) | Verified at |
| --- | --- | --- | --- |
| 5 | Native C11 adapter ABI | Header `spx_pg_v1.h`, generated verbatim into every emitted provider translation unit (never hand-edited per provider); binding schema `semaprax.public-generic-native-adapter.v1`; binding digest domain `semaprax.public-generic-native-adapter.v1.binding\0` | Header: `src/public_generic_abi/native/spx_pg_v1.h:1-9` (banner: "Native C11 physical adapter ABI for Public Generic Carrier v1 (issue #154)"); `HEADER_V1` constant `include_str!`s it at `src/public_generic_abi/native/template.rs:19`; schema/domain: `src/public_generic_abi/native/binding.rs:20,23` |
| 6 | Core Wasm adapter ABI | Binding schema `semaprax.public-generic-wasm-adapter.v1`; binding digest domain `semaprax.public-generic-wasm-adapter.v1.binding\0` | `src/public_generic_abi/wasm/binding.rs:19,21`. **Caveat, load-bearing**: this is the *logical/binding* schema only. No compiled `.wasm` artifact implements *this specific* adapter's `open`/`input_prepare`/`call`/`result_export`/`release` ABI — `public_generic_abi::wasm::provider::WasmProvider` is still an in-process Rust struct (`src/public_generic_abi/wasm/provider.rs:85-86` names its bound endpoint a "fixture"), stated as open blocker #229 in `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:554-566` (line numbers as of 2026-09-19; see the disclaimer above). **Correction, 2026-09-26 (issue #287 audit):** a genuinely different, genuinely compiled closed Core Wasm provider artifact implementing the same open/input_prepare/call/result_export/release shape (plus value_release/provider_close) now exists elsewhere in this repository (`src/wasm/public_generic_provider`, selected by the `public-generic-wasm-provider.v1` profile; see PUBLIC-GENERIC-WASM-PROVIDER-TARGET-V1.md and the correction in PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md near its #229 bullet); this row's own binding schema and `WasmProvider` remain unchanged by it. |

### 1.3 Provider binding format(s)

| # | Contract | Exact identifier(s) | Verified at |
| --- | --- | --- | --- |
| 7a | Logical carrier binding (target-neutral) | `CarrierBindingV1`, binds descriptor identity + `TargetProfile` + carrier schema version + opaque `runtime_identity` | `src/public_generic_abi/carrier.rs:372-376`; described in prose at `src/public_generic_abi/wasm/binding.rs:5-8` and `src/public_generic_abi/native/binding.rs:5-8` |
| 7b | Native provider binding (physical) | `NativeProviderBindingV1`, schema `semaprax.public-generic-native-adapter.v1`; diagnostics `SPX-PG901` (malformed) / `SPX-PG902` (replay mismatch) | `src/public_generic_abi/native/binding.rs:20-28` |
| 7c | Wasm provider binding (physical) | `WasmProviderBindingV1`, schema `semaprax.public-generic-wasm-adapter.v1`; diagnostics `SPX-PG910` (malformed) / `SPX-PG911` (replay mismatch) | `src/public_generic_abi/wasm/binding.rs:19-29` |

### 1.4 Calling consumers (four languages)

All four share one hostile-corpus manifest (§1.5) and one canonical
descriptor/binding trust model. Hosted job-level evidence exists for the
widened calling-consumer scope at prior commits (see §2.16), but no hosted run
at the exact current checkout head is recorded; this remains pending for the
release-candidate freeze.

| # | Contract | Generator (module) | Status quoted at | Verified at |
| --- | --- | --- | --- | --- |
| 8 | Rust calling consumer (issue #156) | `semaprax::public_generic_consumer::rust_calling`, `generate_rust_calling_consumer(...)` | "unsupported and unpublished" (revised 2026-09-19: hosted evidence now exists, run 35433295593 at `7def8fb1…`, not yet a #164 frozen candidate) | `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:201-217`; module at `src/public_generic_consumer/rust_calling.rs` |
| 9 | TypeScript/Wasm calling consumer (issue #157) | `semaprax::public_generic_consumer::typescript_calling`, `generate_typescript_calling_consumer(...)` | "unsupported and unpublished" (revised 2026-09-19: hosted evidence now exists, run 35433295593 at `7def8fb1…`, not yet a #164 frozen candidate) | `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:473-489`; module at `src/public_generic_consumer/typescript_calling.rs` |
| 10 | C11 calling consumer (issue #158) | `semaprax::public_generic_consumer::c_calling`, `generate_c_calling_consumer(...)` | "unsupported and unpublished" (revised 2026-09-19: hosted evidence now exists, run 35433295593 at `7def8fb1…`, not yet a #164 frozen candidate) | `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:333-349`; module at `src/public_generic_consumer/c_calling.rs` |
| 11 | C++17 calling consumer (issue #159) | `semaprax::public_generic_consumer::cxx_calling`, `generate_cxx_calling_consumer(...)`, thin wrapper reusing `c_calling`'s files byte-for-byte | "unsupported and unpublished" (revised 2026-09-19: hosted evidence now exists, run 35433295593 at `7def8fb1…`, not yet a #164 frozen candidate) | `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:656-671`; module at `src/public_generic_consumer/cxx_calling.rs` |

Metadata-only consumer format shared by all four (a distinct, narrower,
**hosted-green** artifact — see §1.5):

| Layer | Identifier |
| --- | --- |
| Metadata format | `semaprax.public-generic-consumer-metadata.v1` |
| Magic prefix | `spxpgcm1;` |
| Languages | `rust`, `typescript`, `c`, `cxx` |

Verified at `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:62-64`.

### 1.5 Hostile corpus version(s)

The composed hostile corpus has the versioned schema
`semaprax.public-generic-hostile-corpus.v1` and the pinned outcome-manifest digest
`sha256:79ae641c8b3274fcb87e8cd314f028052f6cdcd5ccdea66f0ae524ccca89008f`,
defined by `tests/support/public_generic_hostile_corpus.rs` and checked by its
`versioned_manifest_digest_is_stable` test. For PG-6's descriptor/carrier
portion, it consists of two related, separately-scoped artifacts; the older
metadata-format corpus remains a separate comparison row below:

| Artifact | Scope | Case count | Verified at |
| --- | --- | --- | --- |
| Shared calling-consumer hostile corpus (issue #160, extended by #173) | Cross-checks Rust/C11/C++17/TypeScript-Wasm calling consumers against one manifest | 27 cases (`EXPECTED` table) | `tests/support/public_generic_hostile_corpus.rs`; case list in prose at `docs/PUBLIC-GENERIC-CONSUMERS-V1.md#shared-hostile-corpus-issue-160` |
| Reference-codec hostile replay (issue #173's own two closed gaps) | `descriptor.rs`/`carrier.rs`/`carrier/frame.rs`/`native/binding.rs`/`wasm/binding.rs` invalid-UTF-8 and unrecognized-`LeafKind`-tag cases | 6 cases | `tests/projections/public_generic_descriptor_carrier_hostile_replay.rs` (named at `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:990-1003`) |
| Metadata-format hostile corpus (grammar half, hosted green) | Nine hostile documents against the four *metadata* consumers | 9 documents | `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:169-176` |

The schema and digest bind the canonical baseline, all 27 shared case ids and
outcomes, the nine structured descriptor mutations, and all six
malformed-trusted mutations (including the latter groups' expected
replay/decode outcomes and exact byte digests), plus exact bytes for seven
malformed and three valid result-carrier cases. The native and Wasm harnesses'
driver-local recipes for the 27 shared cases are not encoded in this manifest;
their agreement remains execution evidence. A changed bound field must
deliberately mint a new corpus version or update the pinned known-answer with
review.

### 1.6 Settlement corpus version

| Layer | Identifier | Verified at |
| --- | --- | --- |
| Settlement corpus schema | `semaprax.public-generic-settlement-corpus.v1` | `tests/fixtures/public-generic-settlement-v1/cases.json` — `"schema":"semaprax.public-generic-settlement-corpus.v1"` (trailing field of the one-line canonical JSON document) |
| Admitted profile | `flat-owned-bytes-reference-fixture.v1` | same file — `"profile":"flat-owned-bytes-reference-fixture.v1"` |
| Settlement plan schema (the specification the corpus executes against) | `semaprax.public-generic-settlement-plan.v1` | `docs/PUBLIC-GENERIC-SETTLEMENT-V1.md:27` |
| Case count | 22 cases: 7 base shapes, 14 individual logical-failure labels, 1 compound execution/cleanup-failure case | Counted directly from the `cases` array of `tests/fixtures/public-generic-settlement-v1/cases.json` (22), agreeing with the "Manifest contract" section of `docs/PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md` |
| Cross-engine comparison module | `public_generic_abi::carrier::settlement_corpus`, `semaprax.public-generic-settlement-corpus.v1` (module-level identifier reused) | `src/public_generic_abi/carrier/settlement_corpus.rs`; test count quoted at `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:1097-1101`: "13 passed, 0 failed (6 cross-engine agreement cases, 5 `should_panic` negative controls … and 2 settlement-manifest cases)" — **quoted, not re-run in this session** |

Related, additive settlement-continuation schemas found by direct grep (not
independently confirmed beyond the string match — listed for completeness,
each `unverified` beyond its literal existence):

`semaprax.public-generic-consumer-settlement-corpus.v1`,
`semaprax.public-generic-consumer-settlement-evidence.v1`,
`semaprax.public-generic-native-lifecycle-corpus.v1`,
`semaprax.public-generic-native-lifecycle-evidence.v1`,
`semaprax.public-generic-native-result-phase-corpus.v1`,
`semaprax.public-generic-native-result-phase-evidence.v1`,
`semaprax.public-generic-native-thread-admission-corpus.v1`,
`semaprax.public-generic-native-thread-admission-evidence.v1`,
`semaprax.public-generic-settlement-evidence.v1`,
`semaprax.public-generic-typescript-settlement-corpus.v1`,
`semaprax.public-generic-typescript-settlement-evidence.v1`
(found via `rg -n "semaprax\.public-generic" docs/*.md`, backing scripts under
`scripts/public_generic_*.py` referenced in the CI job, §2.6).

### 1.7 Compatibility / candidate delta versions affected

| Layer | Identifier | Verified at |
| --- | --- | --- |
| Candidate surface | `semaprax.public-generic-candidate-surface.v1` | `docs/PUBLIC-GENERIC-COMPATIBILITY-V1.md:17` |
| Compatibility comparison | `semaprax.public-generic-compatibility.v1` | `docs/PUBLIC-GENERIC-COMPATIBILITY-V1.md:18` |
| Candidate-ABI delta report | `semaprax.project-candidate-public-generic-delta.v1` | `docs/PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md:21` |
| Candidate-ABI delta verification record | `semaprax.project-candidate-public-generic-delta-verification.v1` | `docs/PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md:22` |
| Facts digest domain | `semaprax.candidate-public-generic-delta.facts.v1\0` | `docs/PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md:25` |
| Report digest domain | `semaprax.candidate-public-generic-delta.report.v1\0` | `docs/PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md:26` |

Two version literals exist **only as hostile-test fixtures**, not as real
successor specifications — recorded here so nobody mistakes them for an
actual v2:

- `semaprax.public-generic-boundary-profile.v2` — `src/public_generic_abi/descriptor/tests.rs:194`, a `stale_profile` mutation used by a rejection test.
- `semaprax.public-generic-type-grammar.v2` — `src/public_generic_abi/descriptor/tests.rs:200`, a `stale_grammar` mutation used by a rejection test.

### 1.8 Predecessor formats explicitly unchanged

Per `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:345-359` ("Recorded
public-boundary rejections") and `:55-57`/`:333-335` ("Separation invariants"):

| Predecessor projection | Recorded outcome for a generic surface | Verified at |
| --- | --- | --- |
| Project v8/v9/v11 bytes | Reinterpreted by nothing here; a public generic surface requires new versioned descriptor/carrier artifacts, never reused Project bytes | `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:55-57` |
| Canonical ABI report | A generic function excluded, reason `generic_function`; a monomorphic function returning a concrete generic instance excluded, reason `unsupported_result_type` | `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:352-353` |
| C header emission | Same two selections excluded with the same closed reasons | `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:354` |
| Project v9/v11 public API descriptors | A selected generic result rejected before descriptor or target generation | `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:355` |
| Wasm scalar exports | A generic or generic-returning export rejected with `SPX-W115` | `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:356` |

This session did not re-run the tests backing this table (no `cargo`
permitted); the table above is a direct quotation of the milestone document's
own recorded rejections, not independently re-verified execution.

### 1.9 Diagnostic ranges (cross-reference for Section B)

Verified by grep across the specification documents and the two binding
modules:

| Range | Owner | Verified at |
| --- | --- | --- |
| `SPX-PG101`-`SPX-PG104` | Type grammar parse/injectivity failures | `docs/PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md` (occurrences found by `rg`; `SPX-PG103` cited in prose at line 90) |
| `SPX-PG201`-`SPX-PG204` | Compatibility / candidate-surface selection failures | `docs/PUBLIC-GENERIC-COMPATIBILITY-V1.md` |
| `SPX-PG301`-`SPX-PG303` | Candidate delta failures | `docs/PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md` |
| `SPX-PG401`, `SPX-PG402` | Consumer metadata-format refusal / bound | `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:160` |
| `SPX-PG501`-`SPX-PG503` | Settlement obligations refusals (`SPX-PG503` split from `SPX-PG502` per issue #231) | `docs/PUBLIC-GENERIC-SETTLEMENT-V1.md` (range use); split noted in prose |
| `SPX-PG6xx` | Boundary profile classifier refusals | `docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md:57` |
| `SPX-PG7xx` | Descriptor refusals | `docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md:46` |
| `SPX-PG8xx` | Carrier (logical) refusals | `docs/PUBLIC-GENERIC-CARRIER-V1.md:49` |
| `SPX-PG901`, `SPX-PG902` | Native provider binding malformed / replay mismatch | `src/public_generic_abi/native/binding.rs:25,28` |
| `SPX-PG910`, `SPX-PG911` | Wasm provider binding malformed / replay mismatch | `src/public_generic_abi/wasm/binding.rs:26,29` |
| `SPX-PG701`, `SPX-PG703` | Native adapter physical-ABI status constants restating descriptor diagnostics | `src/public_generic_abi/native/spx_pg_v1.h` (comments above `SPX_PG_STATUS_MALFORMED_DESCRIPTOR` / `SPX_PG_STATUS_DESCRIPTOR_REPLAY_MISMATCH`) |

## 2. Section B — Exact implementation map

For each stable artifact: owning specification, owning code module, generator/
verifier entry points, focused test selector, canonical golden fixture,
diagnostic/reason range, CI job/step, and support/publication standing.

### 2.1 Public Generic Type Grammar v1 (PG-1, PG-2)

- **Owning specification**: `docs/PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md`
- **Owning code module**: `public_generic_type` (grammar projection); referenced from `docs/PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md:14` ("`public_generic_type` projects one already-checked `ResolvedType`…")
- **Generator/verifier entry points**: term/template/instance projection and parse functions in the `public_generic_type` module (not independently re-enumerated by symbol in this session; the module's own doc names it authoritative)
- **Focused test selector**: covered by the milestone corpus step `cargo test --locked -p semaprax --test projections public_generic_consumers -- --nocapture` (`.github/workflows/ci.yml:447`, quoted verbatim) for the grammar-consuming half; grammar-only unit tests live alongside the module
- **Canonical golden fixture**: none named as a standalone fixture file; canonical bytes are derived, not stored
- **Diagnostic/reason range**: `SPX-PG101`-`SPX-PG104` (§1.9); closed-reason table at `docs/PUBLIC-GENERIC-TYPE-GRAMMAR-V1.md:56-72`
- **CI job/step**: `public-generic-ownership-milestone` job, "Four-language metadata consumers and hostile replay" step (`.github/workflows/ci.yml:446-447`)
- **Support/publication standing**: **Hosted green**, run 34594793245, at commit `2ef043ba1b989f49b256e456f71fb6e89068bf33` — `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:77-78,454-456`

### 2.2 Public Generic Compatibility v1 (PG-3)

- **Owning specification**: `docs/PUBLIC-GENERIC-COMPATIBILITY-V1.md`
- **Owning code module**: candidate-surface/compatibility comparison logic behind `semaprax.public-generic-candidate-surface.v1` / `semaprax.public-generic-compatibility.v1` (module path not independently traced by symbol in this session beyond the doc's own identifiers)
- **Focused test selector**: not independently re-derived; milestone doc records this gate hosted green in the same run as PG-1/PG-2 (`docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:456`)
- **Diagnostic/reason range**: `SPX-PG201`-`SPX-PG204`, and `SPX-PG201` specifically for selection failures (`docs/PUBLIC-GENERIC-COMPATIBILITY-V1.md:26-28`)
- **CI job/step**: `public-generic-ownership-milestone` job (same job as §2.1; no separately named step)
- **Support/publication standing**: Hosted green, same run/commit as PG-1/PG-2 — `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:79,456`

### 2.3 Public Generic Candidate Delta v1 (PG-4)

- **Owning specification**: `docs/PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md`
- **Owning code module**: `ProjectCandidate::public_generic_delta(expected_candidate)`, and `public_generic_delta_with_boundary_subjects` for the explicit-subject route — `docs/PUBLIC-GENERIC-CANDIDATE-DELTA-V1.md:15-16`, milestone doc `:372`
- **Focused test selector**: `cargo test --locked -p semaprax --test project_candidate public_generic_delta` (`.github/workflows/ci.yml`, quoted verbatim from the step preceding line 447); two additional exact-name regressions in the same job step block: `cargo test --locked -p semaprax --test project flat_owned_record_api::frozen_v9_descriptor_rejects_an_admitted_concrete_generic_result -- --exact` and the analogous `nested_owned_record_api::…v11…` case (`.github/workflows/ci.yml`, lines immediately preceding 446)
- **Diagnostic/reason range**: `SPX-PG301`-`SPX-PG303`
- **CI job/step**: `public-generic-ownership-milestone` job, step preceding "Four-language metadata consumers and hostile replay"
- **Support/publication standing**: Hosted green, same run/commit as PG-1-PG-3, **only** for the zero-subject / explicitly-named-subject route; the manifest-profile route "describes nothing generic" by construction — `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:204-211,372`

### 2.4 Public Generic Boundary Profile v1

- **Owning specification**: `docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md`
- **Owning code module**: `src/public_generic_abi/classifier.rs` (pure classifier over checked HIR, issue #150's implementation half), test module `src/public_generic_abi/classifier/tests.rs`
- **Generator/verifier entry points**: the classifier itself (admission predicate) — `docs/PUBLIC-GENERIC-BOUNDARY-PROFILE-V1.md:6-7`
- **Focused test selector**: not independently re-derived beyond the module path; exercised transitively by the descriptor producer and by `cargo test --locked -p semaprax --lib public_generic_abi` (`.github/workflows/ci.yml`, "Public generic ABI carrier, settlement corpus and WIT projection" step, quoted verbatim)
- **Diagnostic/reason range**: `SPX-PG6xx`
- **CI job/step**: `public-generic-ownership-milestone` job, "Public generic ABI carrier, settlement corpus and WIT projection" step
- **Support/publication standing**: No PG gate names this document by itself as hosted green (it feeds PG-4/PG-5/PG-6/PG-7); classifier code exists and is exercised locally

### 2.5 Public Generic Descriptor v1

- **Owning specification**: `docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md`
- **Owning code module**: `src/public_generic_abi/descriptor.rs`, submodules `descriptor/producer.rs` (real-HIR producer, `generate_public_generic_descriptor`, issue #151), `descriptor/verify.rs`, `descriptor/tests.rs`, `descriptor/fuzz.rs`
- **Generator/verifier entry points**: `producer::generate_public_generic_descriptor` — `docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md:11-12`
- **Focused test selector**: `cargo test --locked -p semaprax --lib public_generic_abi` (library-wide selector covering descriptor unit tests); hostile-replay-specific: `cargo test --locked -p semaprax --test projections public_generic_descriptor_carrier_hostile_replay` (`.github/workflows/ci.yml`, "Callable-boundary corpus, generated consumers, settlement and hostile replay" step, quoted verbatim)
- **Canonical golden fixture**: `BASELINE_DESCRIPTOR_BYTES` used by the shared hostile corpus (issue #160/#173) — named in `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:844`
- **Diagnostic/reason range**: `SPX-PG7xx` (`docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md:46,205,290`)
- **CI job/step**: `public-generic-ownership-milestone` job, "Public generic ABI carrier, settlement corpus and WIT projection" and "Callable-boundary corpus, generated consumers, settlement and hostile replay" steps
- **Support/publication standing**: Local evidence only; frozen wire-format specification with a reference codec — `docs/PUBLIC-GENERIC-DESCRIPTOR-V1.md:5-6`; part of PG-5/PG-6, "Implemented, local evidence" — `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:82,459`

### 2.6 Public Generic Logical Carrier v1

- **Owning specification**: `docs/PUBLIC-GENERIC-CARRIER-V1.md`
- **Owning code module**: `src/public_generic_abi/carrier.rs`, submodules `carrier/frame.rs` (`LogicalCarrierFrame`, `CarrierFrameBinding`), `carrier/machine.rs` (call-machine orchestration), `carrier/trace.rs` (normalized trace vocabulary), `carrier/tests.rs`, `carrier/fuzz.rs`, `carrier/settlement_corpus.rs` (cross-engine comparison, issue #162)
- **Generator/verifier entry points**: `CarrierBindingV1` codec (`src/public_generic_abi/carrier.rs:372-376`); `CARRIER_SCHEMA` constant at `src/public_generic_abi/carrier.rs:31`
- **Focused test selector**: `cargo test --locked -p semaprax --lib public_generic_abi` (library-wide); cross-engine comparison specifically: `cargo test --locked -p semaprax --lib public_generic_abi::carrier::settlement_corpus` (quoted result "13 passed, 0 failed" at `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:1097-1101`, **not re-run this session**)
- **Diagnostic/reason range**: `SPX-PG8xx` (logical); physical adapters extend with `SPX-PG901`/`SPX-PG902` (native) and `SPX-PG910`/`SPX-PG911` (Wasm)
- **CI job/step**: `public-generic-ownership-milestone` job, "Public generic ABI carrier, settlement corpus and WIT projection" step
- **Support/publication standing**: Local evidence only for the physical adapters — `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:82-83`; the logical layer defines "no physical target mapping" itself (`docs/PUBLIC-GENERIC-CARRIER-V1.md:18-20`)

#### 2.6.1 Native C11 physical adapter (issue #154)

- **Owning code module**: `src/public_generic_abi/native.rs`, `native/binding.rs`, `native/template.rs` (`HEADER_V1` = `spx_pg_v1.h`), `native/provider_body.c`, `native/spx_pg_v1.h`
- **Fixture endpoint caveat**: `src/public_generic_abi/native.rs:20-30` and `native/provider_body.c:18-26` both state directly that "the bound endpoint is a fixture: it reverses each owned leaf's bytes," and that deriving a real checked-program endpoint requires codegen wiring that "remains unimplemented and is out of this adapter's own scope"
- **Focused test selector**: `cargo test --locked -p semaprax --test public_generic_native_adapter_v1` (`.github/workflows/ci.yml:469`, quoted verbatim); aggregate entry point `sh tests/public_generic_native_adapter_v1/run_all_four_callers.sh`
- **Canonical golden fixture**: `tests/public_generic_native_adapter_v1/fixture.rs`, `probe.c`
- **Support/publication standing**: Local evidence only

#### 2.6.2 Core Wasm physical adapter (issue #155)

- **Owning code module**: `src/public_generic_abi/wasm.rs`, `wasm/binding.rs`, `wasm/provider.rs`, `wasm/memory.rs`, `wasm/registry.rs`, `wasm/probe.rs`, `wasm/reverse_probe.mjs`
- **Fixture endpoint caveat**: `src/public_generic_abi/wasm/provider.rs:85-86` names `FIXTURE_ENDPOINT_EXPORT_NAME = "spx_pg_wasm_endpoint_reverse_bytes_v1"` and calls it "the one fixture endpoint this round's adapter binds"; no compiled `.wasm` implements *this specific* `WasmProvider` protocol (issue #229's own open/closed tracking status is unaffected, `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:554-566`, line numbers as of 2026-09-19; see the disclaimer above). **Correction, 2026-09-26**: a differently-scoped, genuinely compiled closed Core Wasm provider artifact exists elsewhere in the repository since (`src/wasm/public_generic_provider`); see the same correction referenced in row 6 of §1.2 above.
- **Focused test selector**: `cargo test --locked -p semaprax --test public_generic_wasm_adapter_v1` (`.github/workflows/ci.yml:470`, quoted verbatim)
- **Canonical golden fixture**: `tests/public_generic_wasm_adapter_v1/reference_wasm_module.rs` — explicitly a "hand-assembled, committed test-only stand-in," not a build of `src/public_generic_abi/wasm/**` — `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:170-177` (line numbers as of 2026-09-19; see the disclaimer above)
- **Support/publication standing**: Local evidence only

#### 2.6.3 Reference interpreter physical adapter (issue #162)

- **Owning code module**: `src/public_generic_abi/interpreter.rs`, `interpreter/tests.rs`
- **Fixture endpoint caveat**: `src/public_generic_abi/interpreter.rs:97` — `FIXTURE_ENDPOINT_EXPORT_NAME = "spx_pg_interpreter_endpoint_reverse_bytes_v1"`, "the one fixture endpoint this adapter binds"
- **Support/publication standing**: Local evidence only; used only inside the in-process cross-engine `settlement_corpus` comparison (§2.6)

### 2.7 Rust calling consumer (issue #156)

- **Owning specification**: `docs/PUBLIC-GENERIC-CONSUMERS-V1.md#rust-calling-consumer-issue-156`
- **Owning code module**: `src/public_generic_consumer/rust_calling.rs`, `rust_calling/render.rs`, `rust_calling/tests.rs`
- **Generator entry point**: `generate_rust_calling_consumer(descriptor_bytes, binding, input, output)` — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:215-216`
- **Focused test selector**: `tests/public_generic_native_adapter_v1/rust_calling_consumer.rs`; commands quoted at `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:294-296`: `cargo generate-lockfile`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo test --locked -- --test-threads=1` **inside the generated crate**, executed by the harness, not by this session
- **Canonical golden fixture**: generated crate's own `tests/round_trip.rs`, seven tests quoted at `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:298-309`
- **CI job/step**: `public-generic-ownership-milestone` job, "Callable-boundary corpus, generated consumers, settlement and hostile replay" step (via `public_generic_native_adapter_v1`)
- **Support/publication standing**: Local, proof-only evidence — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:205`

### 2.8 C11 calling consumer (issue #158)

- **Owning specification**: `docs/PUBLIC-GENERIC-CONSUMERS-V1.md#c11-calling-consumer-issue-158`
- **Owning code module**: `src/public_generic_consumer/c_calling.rs`, `c_calling/render.rs`, `c_calling/tests.rs`
- **Generator entry point**: `generate_c_calling_consumer(descriptor_bytes, binding, input, output)`
- **Focused test selector**: `tests/public_generic_native_adapter_v1/c_calling_consumer.rs`; build flags quoted at `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:423-427`: `-std=c11 -Wall -Wextra -Werror`, both `-O0`/`-O2`; ignored sanitizer variant `provisioned_c_calling_consumer_asan_ubsan`
- **Canonical golden fixture**: generated `round_trip.c`
- **Support/publication standing**: Local, proof-only evidence — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:332`

### 2.9 C++17 calling consumer (issue #159)

- **Owning specification**: `docs/PUBLIC-GENERIC-CONSUMERS-V1.md#c17-calling-consumer-issue-159`
- **Owning code module**: `src/public_generic_consumer/cxx_calling.rs`, `cxx_calling/render.rs`, `cxx_calling/tests.rs` — calls `c_calling::generate_c_calling_consumer` directly and reuses its four files byte-for-byte (test: `reuses_the_c_calling_consumer_files_byte_for_byte`)
- **Focused test selector**: `tests/public_generic_native_adapter_v1/cxx_calling_consumer.rs`; ignored sanitizer variant `provisioned_cxx_calling_consumer_asan_ubsan`
- **Canonical golden fixture**: generated `test/round_trip.cpp`
- **Support/publication standing**: Local, proof-only evidence — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:644`

### 2.10 TypeScript/Wasm calling consumer (issue #157)

- **Owning specification**: `docs/PUBLIC-GENERIC-CONSUMERS-V1.md#typescriptwasm-calling-consumer-issue-157`
- **Owning code module**: `src/public_generic_consumer/typescript_calling.rs`, `typescript_calling/render.rs`, `typescript_calling/tests.rs`
- **Generator entry point**: `generate_typescript_calling_consumer(descriptor_bytes, binding, input, output)`
- **Focused test selector**: `tests/public_generic_wasm_adapter_v1/typescript_calling_consumer.rs`; toolchain pin `tsc` 5.8.3 — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:594-596`
- **Canonical golden fixture**: `tests/public_generic_wasm_adapter_v1/reference_wasm_module.rs` (same test-only stand-in named in §2.6.2)
- **Support/publication standing**: Local, proof-only evidence — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:465-466`; the document's own "load-bearing honest limitation" states no compiled `.wasm` implements the full provider ABI (`docs/PUBLIC-GENERIC-CONSUMERS-V1.md:474-489`)

### 2.11 Shared hostile corpus (issue #160, #173)

- **Owning specification**: `docs/PUBLIC-GENERIC-CONSUMERS-V1.md#shared-hostile-corpus-issue-160`
- **Owning code module**: `tests/support/public_generic_hostile_corpus.rs` (pure data, `#[path]`-included into both harnesses below)
- **Generator/verifier entry points**: `tests/public_generic_native_adapter_v1/shared_hostile_corpus.rs` (Rust/C11/C++17), `tests/public_generic_wasm_adapter_v1/shared_hostile_corpus.rs` (TypeScript/Wasm)
- **Focused test selector**: `cargo test --locked -p semaprax --test public_generic_native_adapter_v1` and `--test public_generic_wasm_adapter_v1` (same selectors as §2.7-2.10; the shared-corpus tests live inside these binaries)
- **Canonical golden fixture**: `EXPECTED` table in `tests/support/public_generic_hostile_corpus.rs` (27 shared cases); the separate malformed-trusted descriptor manifest has 6 cases
- **CI job/step**: `public-generic-ownership-milestone` job, "Callable-boundary corpus, generated consumers, settlement and hostile replay" step
- **Support/publication standing**: Hosted job-level evidence exists at prior commits, with local proof details; no exact-current-head run is recorded — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:180-190`

### 2.12 Public Generic Settlement Obligations v1 (PG-7, specification half)

- **Owning specification**: `docs/PUBLIC-GENERIC-SETTLEMENT-V1.md`
- **Owning code module**: settlement-plan derivation bound to `semaprax.public-generic-settlement-plan.v1` (module path not independently re-traced beyond the doc's own identifier in this session)
- **Diagnostic/reason range**: `SPX-PG501` (empty-plan refusal), `SPX-PG502` (inventory disagreement), `SPX-PG503` (transfer-unit disagreement, split from `SPX-PG502` per issue #231) — `docs/PUBLIC-GENERIC-SETTLEMENT-V1.md`
- **Support/publication standing**: Implemented bounded projection, local evidence, no hosted run — `docs/PUBLIC-GENERIC-SETTLEMENT-V1.md:3-8`

### 2.13 Cross-engine settlement corpus (issue #162, execution half of PG-7)

- **Owning specification**: `docs/PUBLIC-GENERIC-CONSUMERS-V1.md#cross-engine-settlement-corpus-issue-162`, `docs/PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md`
- **Owning code module**: `src/public_generic_abi/carrier/settlement_corpus.rs`; native-side manifest loader `tests/support/public_generic_settlement_manifest.rs`; native fixtures via `native::template::render_reference_provider`
- **Canonical golden fixture**: `tests/fixtures/public-generic-settlement-v1/cases.json` (§1.6)
- **Focused test selector**: `cargo test --locked -p semaprax --lib public_generic_abi::carrier::settlement_corpus` (quoted result "13 passed, 0 failed" — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:1097-1101`, not re-run this session)
- **CI job/step**: `public-generic-ownership-milestone` job, "Native settlement sanitizer evidence and independent replay" step (Linux-only, `.github/workflows/ci.yml`, `runner.os == 'Linux'` guard), which additionally invokes `scripts/public_generic_settlement_evidence.py`, `scripts/public_generic_consumer_settlement.py`, `scripts/public_generic_consumer_mutations.py`, `scripts/public_generic_settlement_threads.py`, `scripts/public_generic_settlement_thread_mutations.py`, `scripts/public_generic_typescript_settlement.py`, `scripts/public_generic_typescript_mutations.py`, all quoted verbatim from the workflow file
- **Support/publication standing**: Local, in-process, proof-only evidence covering only interpreter vs. Wasm-model (native C11 is explicitly out of scope for this specific corpus mechanism — proven separately) — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:1054-1059,1077-1082`

### 2.14 Aggregate execution entry point (issue #172)

- **Owning code module**: `tests/public_generic_native_adapter_v1/run_all_four_callers.sh`
- **What it runs**: `cargo test --test public_generic_native_adapter_v1` then `cargo test --test public_generic_wasm_adapter_v1`, printing one `AGGREGATE <test> PASS|FAIL|SKIPPED` line per caller
- **Support/publication standing**: Local, proof-only evidence; "never claims parity across the four" — `docs/PUBLIC-GENERIC-CONSUMERS-V1.md:1031-1037`

### 2.15 Compiled C11-reference-inside-Core-Wasm continuation (issue #162)

- **Owning specification**: `docs/PUBLIC-GENERIC-SETTLEMENT-CORPUS-V1.md#compiled-c11-reference-provider-inside-core-wasm-issue-162`
- **Focused test selector**: `cargo test --locked -p semaprax --test public_generic_wasm_adapter_v1 compiled_provider::actual_native_renderer_executes_in_core_wasm -- --ignored --exact` (`.github/workflows/ci.yml`, "Compiled C11-reference provider inside Core Wasm" step, Linux-only, requires `wasm-ld`/`lld`)
- **Owning code module**: `tests/public_generic_wasm_adapter_v1/compiled_provider.rs`, `compiled_reference_endpoint.rs`, `compiled_reference_endpoint.mjs`; driven by `scripts/public_generic_compiled_wasm.py`
- **Support/publication standing**: Explicitly stated to supersede only "a blanket assertion that no compiled reference provider lifecycle is exercised" — does **not** supersede issue #229's compiled public-generic provider ABI requirement — `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md:677-690`

### 2.16 The `public-generic-ownership-milestone` CI job itself (PG-8)

- **Owning specification**: `docs/CI-REQUIRED-CHECKS-V1.md` (job listed at line 156 of that file: `public-generic-ownership-milestone` / "Public generic ownership milestone (ubuntu-latest \| macos-latest \| windows-latest)" / weight 3)
- **Owning workflow file**: `.github/workflows/ci.yml`, job definition starting at line 253 (`public-generic-ownership-milestone:`), matrix `[ubuntu-latest, macos-latest, windows-latest]`, `timeout-minutes: 240`
- **Steps, as they exist in source at the commit named in §3** (quoted, not executed): "Allow Windows to check out long evidence paths"; a candidate-delta/frozen-descriptor-rejection step; "Four-language metadata consumers and hostile replay" (`public_generic_consumers`); "Public generic ABI carrier, settlement corpus and WIT projection" (`--lib public_generic_abi`); "Callable-boundary corpus, generated consumers, settlement and hostile replay" (`public_generic_native_adapter_v1`, `public_generic_wasm_adapter_v1`, `public_generic_descriptor_carrier_hostile_replay`); "Native settlement sanitizer evidence and independent replay" (Linux-only); "Compiled C11-reference provider inside Core Wasm (not public generic admission)" (Linux-only)
- **Support/publication standing (revised 2026-09-19)**: two hosted runs are now on record for this job. Run `34594793245` at commit `2ef043ba…` exercised only the "Four-language metadata consumers and hostile replay" step (that step is, as of the commit below, still unchanged and still invokes only `cargo test --locked -p semaprax --test projections public_generic_consumers`). **This issue's own audit found [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593) at commit `7def8fb1a727787f989428d77eea603ffd0513cf`**, whose three `public-generic-ownership-milestone` jobs (105871581195 ubuntu-latest, 105871581214 macos-latest, 105871581165 windows-latest) are all `success`, with every step listed above — including the two Linux-only sanitizer/compiled-Wasm steps — passing on the ubuntu-latest leg. The workflow run's own aggregate conclusion is `failure`, from unrelated jobs (verified by name: `Rust tests macos-latest`, `STD-08 bundled library depth`, `Public Native Rust SDK v1`, `Rust 1.88 minimum`), which is why no prior document had surfaced it — job-level conclusions were not checked, only run-level ones. `7def8fb1` is 18 commits behind the head this finding was made against (`e0c268b378ecf9279f1968ee706ea990b0da18d1`); that range was diffed against every public-generic path and contains exactly one touching commit (`11f07087`, an unrelated `std.export.policy` CI addition), so the result is representative of current `main` but is **not** a #164 frozen candidate.
- **Second revision, 2026-09-19 (same-day follow-up pass)**: a third hosted run is now on record, [run 35407101886](https://github.com/wavect/semaprax/actions/runs/35407101886) (workflow `CI`), at commit `3548dc9af5d5c576c884a83a82024891d950e4fe` — an **ancestor** of `7def8fb1a727787f989428d77eea603ffd0513cf` above, and **not** the current head of `main`; state this as "at commit `3548dc9a`," never as "current head" or "hosted at head." The run's own **aggregate** conclusion is `cancelled` (an unrelated superseded job, not this milestone job); its "Public generic ownership milestone" job concluded `success` on all three of ubuntu-latest, macos-latest, and windows-latest — the ubuntu-latest job's database id is `105799024571`. That job genuinely executed real, non-zero-count test selectors, not an empty-filter false pass: `cargo test --locked -p semaprax --test public_generic_native_adapter_v1` → **52 passed, 0 failed, 3 ignored, 0 filtered out**; `cargo test --locked -p semaprax --test public_generic_wasm_adapter_v1` → **24 passed, 0 failed, 1 ignored, 0 filtered out**; `cargo test --locked -p semaprax --test projections public_generic_descriptor_carrier_hostile_replay` → **6 passed, 0 failed** — the six named tests `carrier_binding_decode_rejects_invalid_utf8_in_the_runtime_identity_field`, `carrier_frame_parse_bounded_rejects_an_unrecognized_leaf_kind_variant_tag`, `carrier_frame_parse_bounded_rejects_invalid_utf8_in_a_leaf_path`, `descriptor_decode_rejects_invalid_utf8_in_the_export_id_field`, `native_provider_binding_decode_rejects_invalid_utf8_in_the_exported_endpoint_symbol_field`, `wasm_provider_binding_decode_rejects_invalid_utf8_in_the_exported_endpoint_export_name_field`; `cargo test --locked -p semaprax --test projections public_generic_ownership_milestone` → **19 passed**; and `cargo test --locked -p semaprax --lib public_generic_abi` → **307 passed** — the first hosted execution anywhere on record of that library selector, exactly the gap issue #140's own audit found (`grep public_generic_abi .github/workflows/*.yml` returned only unrelated hits before `3548dc9a` added it). `git merge-base --is-ancestor` confirms commit `6d1289b9` (issue #173's descriptor-envelope work) and commit `8802d013` (issue #140's max-bounds work) are both ancestors of `3548dc9a`, so this run's success covers the corpus both issues added. This is the same generalizable lesson the first revision above already recorded, in the mirror-image direction: a `cancelled` aggregate, like a `failure` aggregate, says nothing about whether the milestone job inside it completed and passed — only job-level conclusions do. This does not change #164's open status: no SHA-freeze protocol ran around `3548dc9a` either, and it sits well behind the current head of `main` just as `7def8fb1` does.

### 2.17 Completion matrix cross-reference

- `docs/COMPLETION-MATRIX.md:107-116` names the public generic ABI programme as a separate milestone with eight prerequisite gates and a distinct PG-9 decision gate, and states "No public generic signature, descriptor, carrier … [is admitted]" — corroborates the milestone document's own standing decision (§1, §3).

## 3. What #164 still requires (explicitly open)

This section exists per this task's hard constraint: nothing above may be
mistaken for the frozen #164 record. The following remain **entirely open**:

1. **No candidate SHA is frozen.** This session observed `HEAD` at
   `3548dc9af5d5c576c884a83a82024891d950e4fe`
   ("`ci(public-generic): select the ABI library module no hosted run ever
   ran`", committed 2026-09-19T01:48:28+02:00) while reading the CI workflow
   file for §2.16 — but this is a **shared checkout**: other agents commit to
   `main` concurrently, `git status`/`git log` were not re-run after every
   read in this session, and no fast-forward-only update, uncommitted-changes
   check, or SHA-freeze step (#164 steps 1-2) was performed. Do not treat this
   hash as a candidate; it is only "the commit some specific fact in this
   document was checked against."
2. **No local focused-or-full gate was executed in this session.** Hard
   constraint #3 for this task forbade running `cargo`. Every test count
   above was a **quotation of an existing document**, not a result this
   session measured. #164's Section C ("Local evidence") is therefore not
   produced here at all. Note that two quoted figures were subsequently
   measured and found stale — the native adapter harness is 55 cases and the
   Wasm adapter harness 25, against the quoted 16 and 15 — so a quotation in
   this document establishes only what another document claimed, never what a
   run would report.
3. **Revised 2026-09-19 by a later #164 audit session (this one), which was
   permitted to run `gh` and `cargo`.** The claim above — "the only hosted run
   in evidence anywhere in this repository's docs remains run `34594793245`"
   — was accurate for the session that wrote it, which was not permitted to
   inspect GitHub Actions. It is no longer accurate: this session ran `gh run
   list`/`gh api` job-level (not just run-level) queries and found
   [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593)
   at commit `7def8fb1a727787f989428d77eea603ffd0513cf`, whose three
   `public-generic-ownership-milestone` jobs (105871581195 ubuntu-latest,
   105871581214 macos-latest, 105871581165 windows-latest) are all `success`,
   covering PG-5/PG-6/PG-7's calling-consumer, hostile-carrier, and
   settlement-execution work that run `34594793245` predates. No *new* run
   was triggered — this is an existing, already-completed run that no
   document had previously cross-referenced by job ID rather than by the
   run's own (misleading, because it aggregates unrelated failing jobs)
   `failure` conclusion. §164's Section D ("Hosted evidence") is still not
   fully produced — no candidate-SHA freeze protocol ran around this commit,
   and `7def8fb1` sits 18 commits behind the head this finding was checked
   against (`e0c268b378ecf9279f1968ee706ea990b0da18d1`; the range was diffed
   against every public-generic path and contains exactly one unrelated
   touching commit, `11f07087`). A genuine, verifiable hosted run now exists
   for this scope; it is simply not yet the exact-head frozen candidate
   Section D requires. See §2.16 (revised) and the milestone document's own
   "Reverified 2026-09-19" addendum for the full citation.
4. **A second, same-day follow-up pass (still this #164 audit) found a
   third hosted run, at a different, earlier prior head.**
   [Run 35407101886](https://github.com/wavect/semaprax/actions/runs/35407101886)
   (workflow `CI`) executed at commit
   `3548dc9af5d5c576c884a83a82024891d950e4fe` — an ancestor of
   `7def8fb1a727787f989428d77eea603ffd0513cf` (item 3 above), and **not** the
   current head of `main`. The run's aggregate conclusion is `cancelled` (an
   unrelated superseded job, not this milestone job); its "Public generic
   ownership milestone" job concluded `success` on all three of
   ubuntu-latest, macos-latest, and windows-latest (ubuntu-latest job
   database id `105799024571`), executing real non-zero-count selectors:
   `--test public_generic_native_adapter_v1` → 52 passed/0 failed/3 ignored;
   `--test public_generic_wasm_adapter_v1` → 24 passed/0 failed/1 ignored;
   `--test projections public_generic_descriptor_carrier_hostile_replay` → 6
   passed/0 failed; `--test projections public_generic_ownership_milestone`
   → 19 passed; and `--lib public_generic_abi` → 307 passed — the first
   hosted execution on record of that selector, closing the exact gap issue
   #140's own audit named ("no workflow file in the repository selected the
   `public_generic_abi` library module on any host"). `git merge-base
   --is-ancestor` also confirms commits `6d1289b9` (#173) and `8802d013`
   (#140) are both ancestors of `3548dc9a`, so this run's success covers the
   corpus both issues added. This does **not** change #164's open status:
   no SHA-freeze protocol ran around `3548dc9a` either, and it sits well
   behind the current head of `main` just as `7def8fb1` does. See the
   corrected §2.16 for the full citation.
5. **No artifact inventory (Section E) was built.** No descriptor bytes,
   provider bindings, native/Wasm artifacts, or generated-package outputs
   were built, hashed, or sized in this session (that would require running
   the generators/compilers, which requires `cargo`/`clang`/`tsc`, none of
   which this session was permitted to invoke).
6. **No compatibility audit (Section F) or security/trust-boundary audit
   (Section G) was freshly executed.** Both would require running the
   relevant test suites; this session only read the specifications and
   existing test *names* cited in §2.
7. **Nothing in this document is itself a support or publication decision.**
   PG-9 was later decided (2026-09-19, in
   `docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md`'s PG-9 decision record and
   its "PG-9 decision recorded" addendum, by the named maintainer) as
   `unsupported`/`unpublished` — the same conservative option this document's
   evidence already pointed to. Nothing here advances PG-5, PG-6, PG-7, or
   PG-8 beyond their currently-recorded hosted/local evidence states, and
   nothing here was an input the maintainer needed beyond what the milestone
   document itself already cited.
8. **The two structural blockers under PG-5/PG-6/PG-7 are unresolved and
   unaddressed by this document**: (a) every physical adapter (interpreter,
   native, Wasm) still binds a fixture endpoint, not a function body
   codegenned from a real admitted public-generic `.spx` export
   (`src/public_generic_abi/native.rs:20-30`,
   `wasm/provider.rs:85-86`, `interpreter.rs:97`,
   `native/provider_body.c:18-26`, all confirmed live in §2.6.1-2.6.3); (b) no
   compiled `.wasm` artifact implements the Core Wasm provider ABI (issue
   #229, open).
9. **A genuine drift was found (2026-09-18) and is recorded here rather than
   silently corrected — and then partly resolved by hosted evidence
   (2026-09-19).** The milestone document's PG-8 note (as it read on
   2026-09-18, since reworded) stated the CI job's calling-consumer step is
   "unchanged" and still invokes only the metadata-consumer test. §2.16 above
   (revised) shows the **currently checked-out** `.github/workflows/ci.yml`
   already contains additional steps (native/Wasm adapter tests,
   hostile-replay projection test, sanitizer/settlement Python scripts, the
   compiled-Wasm step) that postdate that 2026-09-18 note —
   `git log --oneline -- .github/workflows/ci.yml` shows commits `c841b9b8`,
   `862f886a`, `4d02a4fa`/`684588d6`, and `3548dc9a` landing after it. **At
   the time this was written, none of that was hosted evidence** — it showed
   only that the CI *definition* had moved, not that any hosted run had
   executed the widened job. **That has since changed**: this issue's own
   audit on 2026-09-19 found [run 35433295593](https://github.com/wavect/semaprax/actions/runs/35433295593)
   at commit `7def8fb1a727787f989428d77eea603ffd0513cf`, whose three
   `public-generic-ownership-milestone` jobs are all `success`, covering
   every one of those additional steps. This closes the specific evidentiary
   gap the #163 → #164 dependency chain describes, though it does not close
   #163 or #164 themselves: no SHA-freeze protocol ran, and `7def8fb1` is 18
   commits behind the head this finding was checked against (diffed clean for
   public-generic scope except one unrelated commit, `11f07087`). See the
   milestone document's own "Reverified 2026-09-19" addendum for the full
   citation.
10. **The release-candidate decision packet template** (`Candidate SHA:`,
   `Contracts/versions:`, `Hosted run/jobs:`, …) that #164 asks maintainers be
   handed is not filled in here — filling it now, with an unfrozen SHA and no
   fresh hosted run, would be exactly the "invent or imply a hosted run"
   failure this task's hard constraints forbid.

## 4. Items recorded as `unverified`

| Item | Reason |
| --- | --- |
| MSRV 1.88 pin for the generated Rust consumer building on a provisioned 1.88 toolchain *in a hosted run* | The workflow does provision it: `.github/workflows/ci.yml`'s "Install the generated Rust consumer's declared MSRV toolchain" step runs `rustup toolchain install "1.88"` so that `generated_rust_calling_consumer_builds_and_runs_on_the_declared_msrv_toolchain` resolves it through `rustup which cargo --toolchain 1.88`. What is unverified is only that a *hosted run has executed that step*, which is the same open item as every other row in §3 |
| `HEAD` as a stable subject | This is a shared checkout; `HEAD` moved during this session's own reads (§3, item 1). Any single SHA cited elsewhere in this document names only what a specific fact was checked against, never a frozen candidate |
| PG-8's hosted run job IDs mapping to the *current* job definition | **Resolved 2026-09-19**, no longer unverified: jobs 103248047092/103248046648/103248046983 map to the job as it existed at commit `2ef043ba…` (the narrower grammar/metadata scope), but jobs 105871581195/105871581214/105871581165 (run 35433295593, commit `7def8fb1…`) map to the widened job as it exists in source today, and all three are `success` (§2.16, §3 item 3, revised). A third run, 35407101886 at commit `3548dc9a…` (ubuntu-latest job `105799024571`), is likewise `success` on all three hosts and additionally exercises the `--lib public_generic_abi` selector (§2.16, §3 item 4) |

## 5. Nonclaims

This document admits no syntax, executes no code, builds no artifact, and
grants no filesystem, process, network, publication, or signing authority. It
is not the #164 evidence packet, not a hosted-CI claim, not a candidate
freeze, and not a PG-9 decision input beyond what it explicitly cites from
already-existing, already-committed documents. Public generic ownership
remains unsupported and unpublished, unchanged by anything in this document.
