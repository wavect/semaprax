# ADR 0003: Maintained generated-package support for owned-data-api.v1 (Rust)

Status: scope decision accepted on 2026-09-19; reaffirmed on 2026-09-30 for
issue #325. Ordinary owned-data Rust only; npm maintenance deferred.
Generated packages remain unpublished; formal public promotion remains open.
Audience: maintainers and generated-package contributors.

## Current maintainer decision

**Decision, 2026-09-30: retain the accepted Rust-only scope and the eight
recorded answers.** The first maintained generated-package route remains the
ordinary `owned-data-api.v1` Rust SDK for `semaprax.project.v8`. npm maintenance
is deferred to a separate ADR with its own identity, version, target/runtime,
evidence and support decisions. A passing npm consumer test does not make npm
a maintained package. This reaffirms the existing policy; it neither expands
support nor reopens excluded profiles.

The package identity, preview-version policy and five maintained target
boundaries are recorded below. The fixed `0.1.0` is an unpublished preview
version, not a registry SemVer compatibility promise. Real version assignment
is required before any separately authorized publication, with the exact API
descriptor digest retained alongside the version. Publisher identity and the
choice of a reference package versus per-project registry names remain **not
yet selected**; they are future publication decisions, not blockers to this
Rust-only maintenance decision.

This resolves the pending-confirmation status carried from
[#290's latest decision request](https://github.com/wavect/semaprax/issues/290#issuecomment-5862069497)
into [#325](https://github.com/wavect/semaprax/issues/325). Those requests
postdate the accepted 2026-09-19 answers but did not revoke them. This current
record supersedes their pending status, not their revision-bound evidence.
The reconciled source baseline is `wavect/v070` at
`1ae35afe83f62f405b979f57ac90772cfec9daca`; this documentation update is not a
new compiler, archive-consumer or MSRV execution receipt.

**Decision-ticket closure is separate from support promotion.** No new hosted
run is required solely to close #325 under its hosted-only waiver. That waiver
does not choose a support policy, turn local evidence into hosted evidence,
or waive the existing CI, ownership, refusal and evidence requirements for a
future public/current-head support claim. Answers 5 and 7 remain the promotion
and ongoing-regression requirements; neither job isolation nor the existing
archive/MSRV machinery needs rebuilding for this decision ticket.

No registry publication, publisher credentials, signing, release provenance,
release/tag creation, external deployment or paid model/training calls are
authorized by this decision. Source-authority, resource, ownership, oracle and
refusal requirements remain unchanged.

## Context

Issue #145 asked for one reproducible generated-package consumer route. It
excluded publishing, signing, and the separate public generic ABI in #144.
The support decision requires a scoped package, an executable consumer gate,
and version/target claims backed by clean-install evidence.

The release-preparation script from commits `2ae8a968`, `88d826b0`, and
`e0c7f192` checks package bytes, checksums, source association, and absence of
local paths or private crates. It has no live publish path, refuses
`--publish`, and refuses publish-shaped credentials. A local macOS arm64 run
on 2026-09-19 passed:

```sh
python3 scripts/test-generated-package-release.py
# 21 passed, no skips; real npm and Cargo dry-run tools were available
```

The earlier [decision draft](../GENERATED-PACKAGE-PUBLICATION-DECISION-DRAFT-V1.md)
remains background. This ADR records the decision and its dated evidence;
later support claims must be checked against current gates, not inferred from
historical execution snapshots. The publication draft is not the authority for
the accepted maintenance scope; this ADR is.

## Decision

Choose one initial profile: the Rust package for `owned-data-api.v1`.

| Field | Value |
| --- | --- |
| Project schema / profile | `semaprax.project.v8` / `owned-data-api.v1` |
| Generated Rust package identifier | `semaprax.native-rust-owned-data-sdk.v1` |
| Generated crate name / preview version | `semaprax-generated-native-rust-owned-data-sdk` / `0.1.0` (unpublished preview only; see Support matrix) |
| Owning specification | [docs/PUBLIC-OWNED-DATA-API-V1.md](../PUBLIC-OWNED-DATA-API-V1.md) |
| npm package for the same profile | **Maintenance deferred; separate ADR required** (execution evidence retained below) |

The release-preparation script already handles this profile's fixed file and
archive inventory. It is distinct from the general scalar Native Rust SDK
and from the excluded generic-ABI packages. Project v9-v11 package
promotion also remains outside this Project-v8 decision.

Both genuine Rust archive consumption and genuine compiler-built npm
pack/install/execute evidence are retained below. The synthetic npm fixture
remains useful wrapper coverage, but is no longer the only npm execution
evidence. npm is still deferred by the accepted scope decision, not by an
assertion that its genuine consumer has never run. Do not promote npm by
borrowing Rust's evidence or by treating npm execution as a maintenance promise.

The general scalar SDK has a different package inventory and evidence. It
needs a separate support decision rather than being folded into this one.

## Support matrix

| Claim | Exact value | Notes |
| --- | --- | --- |
| Maintained target boundary (5, fixed) | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc` | Per `docs/NATIVE-RUST-INTEROP-V1.md`'s "narrower five targets" rule, shared by the owned-data SDK's compiled-ABI admission (`docs/PUBLIC-OWNED-DATA-API-V1.md`). `aarch64-pc-windows-msvc` is explicitly excluded: its archive tool plan is not frozen. Musl, GNU-Windows, x32, and big-endian configurations are rejected before staging, not silently accepted. |
| Generated crate declared MSRV | `rust-version = "1.85"` | Literal in every generated `Cargo.toml` (`crates/semaprax-native-rust-owned-data-package/src/render.rs:26`). The genuine archive consumer passed on exactly Cargo/rustc 1.85.0 at `0cdd26d312fd65653d24a46098195484720c78e9`, locally on macOS arm64; see the dated evidence below. This is the generated crate's consumer proof, not repository compiler MSRV or other-target evidence. |
| Repository harness toolchains | Dedicated `native-rust-owned-data-sdk-v1` job: Rust `1.88`; `verify-tests` shard: Rust `1.97.1` | These are the compiler/harness toolchains pinned in `.github/workflows/ci.yml`, not the generated crate's `1.85.0` consumer claim. The dedicated three-OS job explicitly executes the ignored archive-consumer gate; answer 6's job isolation is implemented. |
| Package version scheme | Unpublished preview: fixed `0.1.0`; before publication: real, incrementing SemVer plus exact API descriptor digest | Keep `OWNED_CRATE_VERSION` and byte-frozen previews unchanged for local generation. The digest binds the exact API, not registry version ordering. Answer 2 already requires real version assignment before any publish; release-name allocation, version assignment and compatibility/release review belong to that separately authorized publication work. |
| Execution coverage, not a blanket support receipt | Prior-revision hosted Rust archive-consumer passes on macOS, Windows and Ubuntu; exact `1.85.0` consumer pass locally on `aarch64-apple-darwin` | See retained evidence rows 15-16. The five target triples are the unchanged maintenance/admission boundary, not five measured MSRV passes. No other-target MSRV, current-head hosted-green or whole-run-green claim is inferred. |
| npm package | Maintenance deferred | Genuine local execution is recorded in row 14; support requires its own separate ADR. |

## Evidence

These are retained execution records, not new runs performed by the
2026-09-30 decision update. Rows 1-13 preserve the original 2026-09-19 review
and its follow-up snapshots, including their then-current gaps. References in
those rows to "this session", "current HEAD", missing hosted evidence or
unimplemented job isolation refer to those historical snapshots, not today's
status. Rows 14-16 record later evidence; the current decision and support
matrix above own the present policy/status. Local, proof-only and prior-head
evidence must never be described as hosted, current-head or production support.

| # | Claim | Evidence | Kind |
| - | --- | --- | --- |
| 1 | Publish is refused unconditionally; no live-publish code path exists; credential-shaped env vars block both subcommands before any file is touched | `python3 scripts/test-generated-package-release.py` -> 21/21 passed, run this session on this macOS arm64 host at this checkout | **Local, current session, this host.** Not hosted CI; not a registry test. |
| 2 | `npm pack --dry-run` and `cargo publish --dry-run` independently confirm packaging validity/refusal using real tools | Same 21-test run: both `test_real_npm_pack_dry_run_succeeds_and_writes_nothing` and `test_real_cargo_publish_dry_run_is_refused_by_cargo_itself` executed (not skipped) against this machine's real `npm` (nvm, Node 24.3.0) and `cargo` (Homebrew) | **Local, current session, real tools, dry-run only.** No network call, no registry write; not evidence of a registry accepting the package. |
| 3 | Generated packages are deterministic (identical SHA-256 across two independent generate-and-package runs) and different source produces different digests | Described in issue comment for `e0c7f192`: "two generate-and-package runs from identical `.spx` source produce an identical SHA-256, and the calculator and callback programs produce different ones," with a stated negative control (flipped assertion fails) | **Local evidence at a prior commit (`e0c7f192`), not independently re-run this session.** Not hosted; not re-verified at the current checkout. |
| 4 | The owned-data-api.v1 Rust SDK's own test harness (`tests/public_native_rust_owned_data_sdk_v1.rs`) exists, is unconditionally selected (no env-var gate), and is wired into a hosted CI matrix | Confirmed this session via `python3 scripts/ci-msrv.py --plan-only`: the target lands in `verify-tests`'s `integration-3` shard, run across `ubuntu-latest`/`macos-latest`/`windows-latest` | **Structural fact about CI configuration, verified this session by reading, not by a passing hosted run.** See row 6 for why no recent green run exists. |
| 5 | The owning specification's own claimed test evidence for this exact harness | `docs/PUBLIC-OWNED-DATA-API-V1.md`: "Scoped local execution of `public_native_rust_owned_data_sdk_v1` on Linux AArch64/Rust 1.88/Clang 14 passes nine selected tests... These results do not establish shared-context safety, full native support, hosted promotion or physical allocator settlement." | **Local evidence, as the spec itself already states.** Predates, or does not claim, the CI wiring found in row 4. |
| 6 | Whether a recent hosted CI run has actually exercised this harness successfully | Checked this session via `gh run list --workflow=ci.yml --limit 100`: **zero** of the last 100 completed CI workflow runs had conclusion `success` (mostly `cancelled`, superseded by rapid pushes from concurrent lanes, or `failure`). Inspected one representative recent run (commit `019fd931db`, 2026-09-19) job-by-job: every `verify-tests (integration-2)` and `(integration-3)` job (the shards containing this profile's and its sibling packages' tests) failed on all three OSes, but the failure that stopped each shard (`component_runtime_ci_contract`/`standalone_runner_is_pinned_private_and_outside_the_root_workspace`) is in an unrelated subsystem and occurs *before* the shard reaches `public_native_rust_owned_data_sdk_v1`, whose result is therefore unknown, not failing | **No current hosted evidence.** There is presently no green baseline run, at or near HEAD, that this ADR can point to for this profile's own harness. This is the most important honest gap in this table. |
| 7 | The separate general native-rust-interop-v1 profile's dedicated hosted job (`native-rust-sdk-v1`, NOT owned-data-api.v1) | Same commit `019fd931db`: `Public Native Rust SDK v1 (windows-latest)` completed **success**; `(ubuntu-latest)` and `(macos-latest)` completed **failure** in `public_native_rust_sdk_ci_contract` (an inventory-count meta-test, e.g. `left: 14, right: 10`) before the substantive `public_native_rust_sdk_v1` test target ran at all on those two OSes | **Hosted evidence, at a prior commit (`019fd931db`, not current HEAD), for a different profile than the one recommended here, 1-of-3 OSes fully green.** Cited only to show CI health context; does not back any claim about owned-data-api.v1. |
| 8 | External consumer execution against the packaged owned-data-api.v1 artifacts via a path dependency | `tests/release_archive_product_v1/owned_frame.rs`, invoked from `tests/release_archive_product_v1.rs:77` | The two tests that call it are `#[ignore]`d, requiring "an actual unpacked native release in absolute `SEMAPRAX_RELEASE_ROOT` and its exact `SEMAPRAX_RELEASE_COMMIT` label" (or additionally NODE/CLANG/SEMAPRAX_ARCHIVER/CARGO). **No evidence found or produced this session that this has been run recently, hosted or local; it is a manual, release-time procedure, not a routine gate.** |
| 9 | A packaged-**tarball** (not path-dependency) Rust consumer round trip, per issue #145 step 3's explicit requirement | `packaged_safe_package_builds_offline_and_fail_stops_on_unsettled_handles` (`tests/public_native_rust_owned_data_sdk_v1.rs`) now packages the generated owned-data SDK offline, extracts the actual `.crate`, compares its descriptor and package manifest to the generated source-derived bytes, and builds/runs a fresh locked consumer against only that extraction. The general SDK test remains separate. | **Local gate added at the current checkout, but not executed in this follow-up.** It needs the dedicated job's explicit Clang/archiver wiring or an equivalent local invocation; until it has run, this is executable coverage rather than clean-install evidence. |
| 10 | A packaged-tarball npm consumer round trip | `scripts/test-generated-package-release.py::test_real_npm_tarball_consumer_installs_verifies_and_imports_offline` packs a real `.tgz`, validates every archive member, structurally binds npm lockfile v3 to the sole file-tarball dependency, installs with offline `npm ci --ignore-scripts`, verifies the installed inventory and bytes, and imports the package with Node. Hostile regressions prove a substring-only lockfile reference and installed byte substitution are refused before install/import respectively. The genuinely compiler-built package lanes remain `tests/frame_payload_product_v1/npm_installation.rs:60` and `tests/image_packaged_typescript_workflow_v1.rs:715`. | **Local, current-checkout real-tool evidence for a synthetic compiler-shaped fixture.** The full Python suite passed 54/54 on this host. This is not hosted evidence, does not execute the fixture's fake Wasm, and does not replace the two ignored compiler-generated-product gates. |
| 11 | Manual validation against a genuinely compiler-built package | Issue comment for `2ae8a968`: "validated by hand against a genuinely compiler-built package (`examples/frame-payload-project` via `target/debug/semaprax-full`)" | **Local, one-off, prior commit, by hand.** Not re-run this session; not a repeatable gate. |
| 12 | Row 7's exact `left: 14, right: 10` failure, re-run against current HEAD | Reproduced locally, then fixed in commit `7ef1ada2`: `public_native_rust_sdk_ci_contract` had drifted on two checks -- a pinned Cargo-invocation count stale since `e0c7f192` added four more calls, and an overbroad private-dependency ban that flagged the acyclic `semaprax-oci-package` leaf crate added by `f4d9eba4`. Before the fix (this session, this checkout): 5 passed, 2 failed, matching row 7's cited failure exactly. After: `cargo check --manifest-path examples/calculator-rust/Cargo.toml` exit 0; `public_native_rust_sdk_ci_contract` 7 passed, 0 failed; `public_native_rust_sdk_v1` (env guard armed) 10 passed, 0 failed in 395.40s -- not the 0.01s degraded no-op a missing guard would produce | **Local, current session, this host, current HEAD.** Removes one concrete, previously-hosted-observed cause of red on the *native-rust-sdk-v1* job's ubuntu-latest/macos-latest legs -- a different profile than the one this ADR recommends -- but has not itself been observed green in hosted CI yet (see row 13). Does not touch owned-data-api.v1's own harness or evidence. |
| 13 | Whether `public_native_rust_owned_data_sdk_v1` -- this ADR's actually-recommended profile's own harness -- passes at all, and whether it has a hosted run at or after this session's fix | Run to completion locally this session, this checkout, at HEAD (`0346ae19`, after both `7ef1ada2` and the row-12 write-up): `cargo test --locked --offline -p semaprax --test public_native_rust_owned_data_sdk_v1 -- --test-threads=1 --nocapture` -> **11 passed, 0 failed, finished in 193.49s** (unconditional harness, no env-var gate to arm). Separately, `gh run list --workflow=ci.yml` checked live during this session: recent runs were queued, in-progress, or cancelled by a subsequent push before completing; none observed to reach a `success` conclusion at or after `7ef1ada2` during this session | **A genuine local pass, current session, this host, current HEAD -- and still, by this ADR's own rule, not the evidence answer 5 asks for.** It shows the harness is not currently broken on at least one machine/toolchain, which is worth recording, but a local pass is explicitly **not sufficient** for any support claim per the Maintainer decision: only a specific, recent, confirmed-green **hosted** run across the pinned three-OS matrix satisfies it, and none exists. Answer 6 (isolate the harness into its own CI job so an unrelated failure cannot hide its result) also remains unimplemented. |
| 14 | Whether the npm route's own genuinely-compiler-built consumer lane (row 10's caveat: "The genuinely compiler-built package lanes remain ... `npm_installation.rs:60`") actually runs, and whether it withstands the same class of hostile-archive/byte-binding regressions row 9/10 already prove for the Rust and synthetic-npm routes | Issue #290 (P3) session, commit `77d68e49`, this host (macOS arm64, `rustc`/`cargo` 1.98.0, Node 22.12.0, npm 11.6.2, TypeScript 5.8.3 via a pinned local install, Python 3.12.12): `cargo test --locked --offline -p semaprax --test frame_payload_product_v1 npm_installation::installed_owned_npm_package_resolves_and_runs_without_compiler -- --ignored --exact --nocapture` -> **1 passed, 0 failed, finished in 119.49s**, for both the baseline and display-renamed frame projects, exercising the real compiler-produced six-file package end to end (`npm pack`, offline `npm ci`, Node import against real corpus/adversarial data, strict TypeScript). The test now also applies four hostile-byte regressions directly to this genuine package rather than the row-10 synthetic fixture: (a) a tampered payload file and (b) a swapped/tampered `app.wasm` are both refused by the release-preparation script's own preview digest check before packing; (c) a path-traversing archive member is refused by the script's own `_verify_npm_tarball_payload` byte-binding, reused unchanged against a hostile derivative of the real packed tarball; (d) a tarball substituted after the lockfile recorded its integrity is refused by npm's own subresource-integrity check (`EINTEGRITY`) once a fresh cache forces it to re-verify from disk -- a cache already warmed by the earlier lock-only install of the genuine tarball was found, this session, to mask the same substitution instead of catching it, so the test now uses a cache the genuine artifact has never touched for this specific check. | **A genuine local pass, current session, this host, current HEAD, against the actual compiler-generated npm package (not a synthetic fixture).** It answers row 10's open caveat for this one lane: the genuinely-compiler-built npm route is not merely present in source, it runs, and the same hostile-guard classes already proven for the Rust route (tamper, archive-member admission, integrity binding) hold for it too. It does **not** establish hosted CI evidence (this target is not selected into any hosted job today -- confirmed this session by grepping `.github/workflows/ci.yml` and `scripts/ci-msrv.py` for `frame_payload_product_v1`/`npm_installation`, with no hit; `#[ignore]`d tests only run where a job explicitly passes `--ignored`, which no job does for this target), an exact-MSRV build (this ADR's MSRV questions are scoped to the Rust package; no npm-side MSRV claim is made or implied), or any package-identity/version/support decision for npm, which answer 8 already reserves for a separate ADR. Companion `tests/image_packaged_typescript_workflow_v1.rs:715` (a different, `@semaprax/agent-workflow` profile) is out of this scope and was not touched or run. |
| 15 | Genuine Rust archive/extraction/clean-consumer and hostile-byte checks executed in the isolated three-OS job | [2026-09-24 retained report](https://github.com/wavect/semaprax/issues/290#issuecomment-5806865400), exact source `a7cad038bdf09fca02b818c8753def23a1f943db`, CI run `35948261395`: macOS job `107471367738`, Windows job `107471367761`, Ubuntu job `107471367800`; each reports **9 passed, 0 failed, 0 ignored, 2 filtered**, including the genuine archive consumer. | **Hosted, prior revision, Rust route only.** Not exact 1.85.0, npm execution, a current-head receipt or a whole-CI-run success. Supersedes the historical absence of hosted Rust consumer evidence in rows 6/9/13. |
| 16 | Exact generated Rust consumer MSRV | [2026-09-28 local evidence](#local-exact-msrv-evidence-2026-09-28) and [retained issue report](https://github.com/wavect/semaprax/issues/290#issuecomment-5862069497): source `0cdd26d312fd65653d24a46098195484720c78e9`, actual Cargo/rustc **1.85.0**, host `aarch64-apple-darwin`; genuine archive consumer **1/1**, configuration negative controls **3/3**. | **Local, unsigned, prior revision.** The exact tool pair applies only to the extracted consumer and settlement exercises, not compiler generation or archive packaging. No remaining technical MSRV blocker for that scoped gate; no other-target or release-provenance claim. |

**Current reading of the retained evidence:** row 14 closes the genuine npm
execution gap that row 10 identified; rows 15-16 close the specific historical
Rust hosted/archive and local exact-MSRV gaps. The npm route remains local-only
in the cited record and unmaintained under this ADR. Its future ADR may cite
that genuine execution without inheriting Rust's support decision. The current
confirmation for the Rust-only decision is recorded above; neither old
"pending" wording nor the absence of a new hosted run reopens #325.

## Conditions before any support claim

- **Rebuild-and-revalidate cadence.** Dependency, toolchain or compiler-output
  changes require fresh `prepare` + `check` and actual execution of the owning
  consumer/settlement gates at that revision. Preserve the existing required
  CI/release gates; a historical pass or a wired-but-unexecuted test is not a
  new receipt. The #325 closure waiver is not a blanket promotion waiver.
- **Version honesty.** The fixed `0.1.0` names unpublished previews only. Keep
  the exact descriptor digest and byte binding; do not infer compatibility
  from the preview version. Answer 2's real-SemVer requirement is already
  decided, but its publication-time implementation is neither authorized nor
  required by this decision ticket.
- **CI isolation and ongoing regression.** Retain the existing dedicated
  `native-rust-owned-data-sdk-v1` job, explicit ignored-test selection and
  fail-closed controls. Answer 6 is implemented, not an open request to add
  `--no-fail-fast`; answer 7 still requires a CI gate rather than manual-only
  release checks.
- **Toolchain and target honesty.** The generated crate declares
  `rust-version = "1.85"`. Exact Cargo/rustc 1.85.0 passed for the extracted
  consumer on macOS arm64 at the cited revision. Other targets, regeneration
  changes and the repository compiler require their own evidence; do not
  extrapolate that local consumer result to them.
- **Publication remains separate.** Any future registry release needs explicit
  publisher/name/version, compatibility and release authorization. The proposed
  rollback policy is to supersede a broken version, not mutate its bytes; it
  remains part of the separately reviewed publication draft.
- **Credentials, signing and provenance remain separate.** No npmjs.org scope,
  crates.io publisher, `NPM_TOKEN`, `CARGO_REGISTRY_TOKEN`, signing identity or
  release provenance is selected here. Preview checksums are integrity values,
  not signatures, provenance attestations or authority to publish.

## What is explicitly NOT proposed

- **No registry write of any kind**, in any mode. `scripts/generated-package-release.py`
  has no code path that performs one; this ADR does not ask for one to be
  added.
- **No signing.** This decision grants no generated-package signing authority.
  [Release Signing Policy v1](../RELEASE-SIGNING-POLICY-V1.md) remains separate;
  this package remains unsigned and its checksum manifest is an integrity
  value, not a signature or provenance claim.
- **No release or tag creation.** This is release-preparation tooling, not a
  release step.
- **No generic-ABI packages.** Those remain gated on SPX-AI-042 (#144) per
  issue #145's own scope boundary; nothing here changes which Project profile
  is admitted or grants any generic package new authority.
- **No npm package promotion** in this round, for the reasons given in
  Decision.
- **No registry-consumer or new current-head execution claim.** Genuine
  clean-consumer evidence now exists (rows 14-16); it is no longer an
  uncollected technical prerequisite. Local `.crate`/`.tgz` consumption is
  not a registry install, and the historical release-root path dependency
  in row 8 remains a separate, weaker route. This decision does not rerun
  any consumer or manufacture a hosted, signed or published receipt.

## Questions recorded for the scope decision

Historical questions from 2026-09-19, retained with their answers below.
References to missing evidence describe that review date; the current decision
and later evidence above supersede those status observations, not the choices.

1. Approve owned-data-api.v1's Rust package as the first maintained
   generated-package route, with npm explicitly deferred? (yes/no)
2. Should a real, incrementing semver scheme replace the current fixed
   `0.1.0` literal before any publish is considered, given that compatibility
   currently rides on the descriptor digest instead? (require-real-semver /
   accept-fixed-version-plus-digest)
3. Should the generated crate's `rust-version = "1.85"` MSRV claim stand as
   is, or must it first be confirmed by an actual 1.85.0 end-to-end build
   before being called supported? (accept-as-is / require-1.85.0-build)
4. Which exact registry publisher identity (crates.io account/org and
   npmjs.org scope, if npm is later added) would own this package's name?
   (name a value, or "not yet")
5. Should `public_native_rust_owned_data_sdk_v1` be required to show a
   recent, specific, confirmed-green hosted run before this route is trusted
   for any support claim, given none currently exists? (yes/no)
6. Should the `verify-tests` shard be changed so that one target's failure
   does not prevent sibling targets in the same shard from running and
   reporting their own result? (yes/no)
7. Is a manual, `#[ignore]`-gated consumer check run by a human before each
   release acceptable ongoing box-3 evidence, or must an unconditional CI
   gate replace it first? (manual-acceptable / require-ci-gate)
8. Should npm support for the same profile be folded into this ADR once its
   `#[ignore]`d tests are unblocked, or should it get its own separate ADR
   with its own evidence table? (extend-this-adr / separate-adr)

## Recorded answers

Recorded 2026-09-19. The maintainer approved the scope and delegated the eight
choices above. These answers require evidence before any support claim.
They remain the accepted choices after the 2026-09-30 reaffirmation. The
following rationale and follow-up paragraphs are historical, not open tasks
for #325; the ticket's hosted-only closure waiver does not repeal answers 5/7.

1. **Yes — Rust only, npm deferred.** A narrow route that is fully evidenced is
   worth more than a wide one that is half evidenced. A support claim is very
   cheap to make and very expensive to withdraw: once consumers depend on it,
   retracting it breaks them, so the first maintained route should be the one we
   are most certain of.
2. **require-real-semver, before any publish** (not before this ADR stands). The
   descriptor digest is the real compatibility signal *inside* this project, but
   no registry consumer can see it: Cargo's resolver, lockfiles and downstream
   automation all assume the version string orders releases and carries
   compatibility meaning. A fixed `0.1.0` tells every one of those tools
   something false. It is also self-defeating in practice — registries make
   versions immutable, so a second publish at `0.1.0` is simply refused. Keep
   the digest, and record it in metadata *alongside* a real version.
3. **require-1.85.0-build.** An unverified MSRV is a support claim with no
   evidence, which is the exact thing this repository's documentation invariant
   forbids. MSRV is load-bearing for consumers who pin toolchains, it is cheap
   to verify and expensive to be wrong about. Note this cannot be discharged on
   the current dev host (Homebrew Rust, no `rustup`), so it is a gate, not a box
   to tick today.
4. **Not yet.** No publish is proposed, and naming a registry identity now would
   imply an intent to publish that this ADR explicitly does not carry. This must
   be answered before any publish, and by a human.
5. **Yes — a specific, recent, confirmed-green hosted run is required before any
   support claim.** This is the load-bearing answer. The harness is genuinely
   wired into `verify-tests` across three operating systems and has still never
   been observed to pass. Treating wiring as evidence would set the worst
   available precedent for a language whose entire pitch is "meaning in,
   verified machine code out". Credibility here is the product.
6. **Yes in goal, but NOT by `--no-fail-fast` — isolate the harness into its
   own job instead.** The diagnosis holds: `cargo test` stops at the first
   failing test binary, so every later target in the shard never runs, and that
   is precisely why question 5 has no evidence to point at. Fail-fast inside a
   shard destroys information — it converts "we do not know" into something
   easily misread as "it failed".

   **The obvious remedy is the wrong one, and this ADR initially reached for
   it.** `tests/ci_msrv_sharding_contract.rs` forbids `--no-fail-fast` twice, as
   an "MSRV coverage bypass" (line 170) and a "Rust shard bypass" (line 216).
   That ban is deliberate and, on inspection, correct: `scripts/ci-msrv.py`
   already forces `--test-threads=1` for the Project/npm shards because those
   fixtures "share process/filesystem resources". In a suite with shared global
   state, continuing past a failure produces a cascade of *misattributed*
   failures — noise that reads like signal and is worse than a single honest
   stop.

   Note also that the flag lives in `scripts/ci-msrv.py` while the ban is
   enforced against the workflow YAML text, so adding it there would have
   slipped past the guard while violating its intent — the same silent-contract-
   weakening this repository's prohibited-shortcuts list names.

   The correct change is **isolation by job, not by flag**: give
   `public_native_rust_owned_data_sdk_v1` its own CI job so its result is
   independent of unrelated subsystems, leaving fail-fast intact within each
   shard. Same goal, no cascade, no contract reversal.
7. **require-ci-gate.** A manual, `#[ignore]`-gated check run by a human before
   each release is the kind of gate that silently stops happening, and nothing
   detects that it stopped. This repository already holds the stronger line
   elsewhere — no feature is implemented without the completion matrix's
   executable gate — and box 3 should not be the exception.
8. **separate-adr.** Each support claim stays bound to its own evidence table.
   Folding npm into this document later would let it inherit Rust's evidence by
   adjacency, which is the same conflation this ADR exists to prevent.

**Consequence for issue #145.** Box 1 has its *decision* half. Its *gate* half,
and boxes 2 and 3, remain open on evidence rather than on judgment, and answer 6
names the concrete change that unblocks collecting it.

**Follow-up session, same date.** Rows 12-13 above record a second pass: the
specific hosted-CI failure row 7 pointed at was reproduced locally and fixed
(commit `7ef1ada2`), removing one concrete cause of red for the *general*
native-rust-interop-v1 profile's CI job. That fix does not touch
owned-data-api.v1 or advance answer 5's bar for it. This session also
confirmed live that this repository's CI queue was, at the time, cycling
through queued/cancelled runs from concurrent pushes without a completed run
at or after the fix commit -- direct, current confirmation of row 6's
"rapid pushes from concurrent lanes" finding, not merely a historical one.
Boxes 1's gate half, 2, and 3 remain open: they need (a) answer 6's job
isolation implemented, and (b) a resulting hosted, confirmed-green run of
`public_native_rust_owned_data_sdk_v1` specifically -- neither of which a
local session can supply on its own.

## Rejected alternatives

### Recommend Rust and npm together

Rejected as the current support scope. The original review lacked genuine
npm execution, but that technical reason is superseded by row 14's actual
compiler-built pack/install/run evidence. The accepted decision still defers
npm maintenance to its own ADR (answer 8). Execution success alone does not
select npm package identity/version policy, supported runtimes or maintenance
obligations, and does not authorize a combined Rust/npm support promise.

### Recommend the general native-rust-interop-v1 SDK instead of owned-data-api.v1

Rejected because, while that profile has stronger hosted-CI and
tarball-consumer coverage (rows 7 and 9), the existing release-preparation
tool (`scripts/generated-package-release.py`) was built specifically around
owned-data-api.v1's file inventory, and the issue asks for one deliberately
chosen profile. A maintainer who wants the scalar SDK promoted too should
treat that as a separate, independently evidenced decision rather than
folding two different generated-package identities into one support claim.

### Treat the existing decision draft as sufficient and skip a formal ADR

Rejected because the issue's own guardrails ask that a design like this be
"submitted for independent maintainer review" in a form a maintainer can
accept or reject cleanly, and this repository's house convention for that is
an ADR under `docs/decisions/`, not a standalone spec-shaped draft. The draft
remains useful background detail and is retained, not deleted.

## Local exact-MSRV evidence, 2026-09-28

At clean source commit `0cdd26d312fd65653d24a46098195484720c78e9`, the genuine
`packaged_safe_package_builds_offline_and_fail_stops_on_unsettled_handles`
gate passed **1/1**, with no skipped selected test, in **186.41 seconds**.
Compiler generation and archive packaging used the current toolchain; only
the independently extracted generated SDK consumer and settlement exercises
used exact Cargo/rustc **1.85.0**, host `aarch64-apple-darwin`. The selected
compiler, disabled wrappers and fixed host target were checked by three
configuration tests, which also passed **3/3**.

The gate retained the real preview and archive byte-binding checks, hostile
traversal/link/duplicate/dependency/target controls, and executable substitutions
refused before Cargo/build/consumer entry. The fresh external consumer ran
locked and offline, printed `42`, preserved its lockfile, and retained the
existing ownership/cleanup fail-stop assertions. Generator, package and
consumer targets were separate temporary directories.

To reproduce after building the owning harness with the current toolchain,
set `SEMAPRAX_NATIVE_RUST_CONSUMER_CARGO` and
`SEMAPRAX_NATIVE_RUST_CONSUMER_RUSTC` to actual absolute 1.85.0 binaries, then
select that exact ignored gate explicitly. Invalid or incomplete tool pairs
refuse without fallback. These overrides apply only to the consumer route.

This is local, unsigned evidence. It does not create release provenance or
publish a package. Non-main hosted evidence follows the maintainer's waiver
when hosted execution is the only missing item. The confirmation requested
in the later issue discussion is resolved by the
[current maintainer decision](#current-maintainer-decision): retain ordinary
owned-data Rust-only maintenance, defer npm to its own ADR, and leave future
publication and public promotion separately gated. No technical MSRV or
hosted-only blocker remains for closing the decision-only #325.
