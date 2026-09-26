# Changelog

Project log follows a compact [Keep a Changelog](https://keepachangelog.com/)
format: `Unreleased` then release buckets, grouped by impact.

> For full historical detail of every entry, refer to
> [docs/CHANGELOG-ARCHIVE.md](docs/CHANGELOG-ARCHIVE.md).

## Unreleased

- Restore source-locked test coverage after recent HIR, graph, scaffold, and
  workspace-graph splits. The tests now bind the new submodule text or verify
  its quality-route classification; the existing coverage threshold is not
  raised.

- Continue the concise documentation rewrite through the language tour,
  development guide, agent protocols, project manifests, and design decisions.
  Clarify that the workspace generation pivot is atomic visibility, not by
  itself power-loss durability, and update the historical Graphify ADR to the
  current Graft navigation workflow.

- Compose the local, caller-authorized mirror-to-held flow with the existing
  signed resolver-cache bridge and deterministic Resolver-v2/Lock-v3 root/leaf
  replay. Cache reads are bounded, receipt-named and held nofollow reads; cache
  partial effects remain distinct from pre-effect, held-publish-uncertain and
  receipt-bearing post-commit outcomes. This is local evidence only and adds
  no hosted registry, ambient network/cache, installation or execution support.

- Rewrite the documentation entry path—overview, install, quickstart, CLI
  guide, and first contribution—in shorter task-based language for new
  developers. Preserve the tested commands, diagnostic examples, and
  versioned contracts; update the v0.6.0 release-gate status without claiming
  publication.

- Remove a stale coordinator-only ABI handoff and deduplicate the 0.3.5/0.2.0
  changelog history, retaining the full older record in the linked archive.

- Record exact-job, old-head partial hosted evidence for the in-progress
  v0.6.0 tag gate without promoting it to a signed or published release.

- Compose caller-authorized bounded mirror bytes through signed Registry-v3
  proof, a held-generation-v3 authenticated timestamp-age anchor and one live
  Lock-v3 artifact read. Resume state is derived from the live held generation,
  not caller checkpoints; identical timestamp replay retains the original
  observation and a newer signed timestamp is the only anchor advance. v2
  history remains readable with no automatic migration. Receipt-bearing
  post-commit read or confirmed-pivot uncertainty remains distinct from
  no-effect refusal; lower held-publish uncertainty carries no receipt. This
  is local evidence only; it adds no hosted registry, resolver-cache, root,
  filesystem or execution authority.

- Allow the unchanged GEN-05B hosted generic-instance corpus enough job time;
  the v0.6.0 release gate reached its former 90-minute ceiling before the
  authored-variant and owned-result checks could finish.

- Persist and restore semantic caches for admitted Projects whose bundled
  standard-library closure exceeds the authored-source manifest limit. The
  private snapshot now validates the complete bounded source inventory before
  warm replay; no source or cache authority is widened.

- Reduce repeated catalog-normalizer control-escape work within the admitted
  Project graph budget. Fifteen independent-oracle cases pass across the
  interpreter, native C O0/O2 and Core Wasm; the exact 256-record maximum
  remains unproven under the unchanged fuel limit.

- Compose acquired exact mirror metadata through Registry Trust v2's signed
  timestamp/snapshot/publisher-manifest verifier and its existing rotation,
  revocation, expiry and checkpoint controls. The local mirror policy refuses
  a follow-up update after seven days without a checkpointed timestamp; this
  remains local proof, not hosted distribution support.

- Add a capability-explicit native HTTPS registry-mirror byte acquisition
  boundary. It accepts only caller-named origin-relative digest-bound metadata
  and artifact paths with disabled proxy discovery, credentials, redirects and
  retries; returned bytes still require the existing signed Registry-v3 and
  held-store admission flow. Local scripted transport controls are not hosted
  registry, TLS-peer, availability or production evidence.

- Steer agents to the cheapest sufficient semantic command. The repository
  guide now orders `query`, `doc`, `context`, then `graph`, with the measured
  cost of each step on the committed examples: one line per declaration,
  roughly the source bytes, a caller-chosen byte budget, and roughly forty
  times the source bytes.

- Attach an actionable hint to the `compact api-surface` refusal on projects
  without the owned-data-api.v1 profile. The `SPX-J105` diagnostic keeps its
  code and message and now points at `semaprax doc` and `semaprax query`;
  a CLI regression test pins the hint on the committed calculator project.

- Point the agent command ladder at `doc --json` for machine consumers. The
  contracted `semaprax.doc.v1` skeleton carries signatures, contracts, and
  identities at roughly the source bytes, eighteen to thirty-four times
  smaller than the full graph on the measured examples.

## 0.6.0 — 2026-09-24

- Keep the frozen private Component v7 WIT identity at `0.5.0` while the crate
  advances to `0.6.0`, and repin the version-bound public-generic Component
  known answers against independently emitted success and contract-failure
  fixtures. The four exact Component CI contract tests pass locally; hosted
  release evidence remains pending.

- Pin the Wavect GmbH release verifier to GitHub's immutable owner/repository
  OIDC subject and require matching subject and ID extensions in the verified
  Fulcio certificate. This is local offline verification, not hosted signature
  evidence until the authorized v0.6.0 tag gate publishes immutable assets.

- Observe Core Wasm record-`Bytes` copy-out only after the generated Node
  facade returns owned bytes from its settled JS arena, with strict transport
  refusal controls. Physical Wasm free, instruction-fuel parity and hosted
  target acceptance remain open.

- Project a frozen malformed-result carrier recipe through generated TypeScript
  against the compiler-produced checked Wasm provider, alongside exact
  descriptor/binding and once-only result-release controls. This is local
  compiled-provider evidence, not the full hostile or hosted R06 matrix.

- Bridge explicit held signed root/leaf generations into the existing resolver
  subject cache after fresh lock-bound artifact reads. CLI lock-bound fetch and
  the bridge share the extracted held cache writer; copied subjects carry no
  persistent signature/freshness authority and failures retain uncertain prefixes.

- Add explicit offline artifact reads from a held signed Registry-v3 generation,
  bound to exact Lock-v3 and independently admitted manifests. Reads replay
  trusted-time freshness and recheck ACTIVE before returning immutable bytes;
  no reusable bearer token, ambient path read or execution authority is added.

- Add an explicit local Registry-v3 generation store coordinating signed trust,
  full root/leaf Lock-v3 selection and exact core artifacts, with one-way v1
  migration and exact interrupted-commit recovery. Receipts remain evidence;
  no fetch/read/execution capability or hosted registry support is added.

- Add pure offline signed metadata-v2 verification for producer-backed
  Registry-v3 linked roots and leaves, exact lock/artifact checks, and a one-way
  Checkpoint-v2 protocol floor preserving prior version/digest high-water marks.
  Candidates remain non-authoritative; managed-host/fetch integration is separate.

- Add independently verified dependency-free Build-v1 leaf manifests and an
  additive producer-backed Registry-v3. A real linked root/leaf catalog now
  reproduces Lock-v3 bytes and checks exact source/report/dependency closure;
  focused tamper, yank, missing-leaf and API-claim controls pass locally.
  Existing profiles remain unchanged; trusted v3 distribution is separate.

- Add a separate private authenticated-native-allocating.v1 handoff for a
  closed checked Bytes body/callee subset. An explicit-context reservation,
  canonical lease settlement and all-leaf result preflight preserve sticky
  failure through generated C11/C++17 callers. Focused controls cover real
  allocating callees, reservation/body/postcondition refusals, oversized
  results and missing-drop/dispatch controls; the actual legacy runtime
  emitter retains its frozen bytes. General-body, cross-backend, hosted and
  public acceptance remain open.

- Add a separate private authenticated-native-moves.v1 handoff that executes
  checked flat-`Bytes` record movement bodies through generated C11/C++17
  callers. Focused physical controls cover both branches, malformed/legacy
  refusals, identity-body omission, and postcondition-failure settlement;
  allocating/status-producing bodies and public acceptance remain open.

- Add an explicit local registry trust host with held owner-private storage,
  independently pinned bootstrap, immutable signed-evidence/checkpoint and
  selected-artifact generations, one durable `ACTIVE` pivot, and exact
  fail-stop recovery. Focused local crash-point and hostile-state tests pass;
  this adds no fetch/network authority or physical power-loss claim.

- Correct catalog-normalizer JSON control escapes at `\\u0010`–`\\u001f`
  and exercise ten oracle-frozen controls across interpreter, native C and
  Core Wasm. The maximal 65,536-byte response still exhausts the unchanged
  100M-step fuel bound, so the catalog acceptance milestone remains open.

- Add a TUF-style local registry verifier with independently installed roots,
  namespace-delegated Ed25519 thresholds, dual-threshold root rotation, exact
  registry/manifest bindings, freshness, rollback and yank checks. Results are
  non-authoritative checkpoint candidates; the managed durable host above is
  separate, and trusted distribution remains open.
- Compact catalog-normalizer scalar projection helpers and avoid a repeated
  label walk when summing accepted record quantities. The 18-case focused
  interpreter/native C O0/O2/Core Wasm oracle selector passes, but the exact
  65,536-byte/256-record plain response still exhausts 100M fuel; enriched
  maximal output and the full owning gate remain unverified (#286 open).

- Add a dispatch-only Windows confinement runtime gate that requires nonzero
  execution of restricted-token child launch, job limit/membership, scratch
  DACL, descendant refusal and test-owned descendant timeout, normal/nonzero
  settlement, timeout cancellation, and filesystem-stage refusal with
  handle-count cleanup. The gate uses
  test-key-signed capsule fixture bytes (not a release trust anchor), requires
  successful `taskkill /T` and direct Cargo PID absence on
  timeout, and checks exact marker/scratch cleanup. Descendant quiescence is
  not independently enumerated. The original two-test slice passed on exact
  checkout `c6bf9902`; the historical five-test selector passed in hosted
  Windows run `35988348061` at `3d4220b6`. Current source uses the shared
  signed-capsule verifier and capsule-v1 architecture codes 3/4 for Windows,
  with a historical nine-test selector (six runtime cases plus three admission refusals).
  That selector passed on exact checkout `c608b8d8` in hosted Windows Server
  2025 run `35992373373` (9 passed, 0 failed, 0 ignored). Artifact-byte binding and production
  support remain unverified. A tenth selected Windows case now exercises a
  real inheritable broad ACE on a private parent and requires the created
  scratch DACL to stay protected with only its intended explicit ACE; the
  expanded selector passed at `f4d3291f` in hosted Windows Server 2025 run
  `35993882814` (10 passed, 0 failed, 0 ignored).

- Harden additive lock-bound offline fetch with held-directory authority,
  bounded retained inputs, private staging and no-replace cache publication.
  Failures retain stages or a published prefix for explicit reconciliation,
  without pathname rollback or a misleading success receipt; unsupported
  hosts refuse before cache effects.

- Re-pin exact Semantic Workspace Change and Operations artifact/evidence/receipt
  KATs to the expanded Project graph and serialized limits while retaining
  domain, reference, API/CLI parity, tamper, replay, budget, and stale/no-write
  assertions.

- Keep the standalone compiler archive independent of unpublished crates by
  moving the private OCI emitter and all hostile tests into one internal
  compiler module. Preserve the typed input boundary, exact artifact bytes,
  credential refusal, and unchanged no-signing/no-registry-publication scope.

- Keep the private public-generic Component runtime inside its ambient-authority
  source contract: acquire a checked-in canonical project explicitly, replay
  against pinned identities, and authenticate exact Component bytes before
  typed execution. This does not widen public Component support.

- Generate canonical descriptor-bound TypeScript/Wasm carrier frames and run
  the generated package against the compiler-owned provider lifecycle. Add the
  private `semaprax.authenticated-native-identity.v1` C profile, which rejects
  malformed or drifted frame metadata before physical work and invokes one
  compiler-checked flat owned-`Bytes` identity endpoint across C, C++, and Rust.
  These are local, unpublished profiles; broader shapes, shared-corpus and
  hosted acceptance remain open.

- Settle borrowed arguments and returned `Bytes` at the native Agent stage
  boundary, retaining sticky failure and strict post-drop receipts. Local
  controls cover success, contract failure, cancellation, omitted drops,
  malformed receipts, and duplicate calls; native instruction fuel, complete
  finalizer accounting, compound cleanup failures, and Wasm parity remain open.

- Emit the internal `public-generic-wasm-provider.v1` endpoint as a
  deterministic zero-import Core Wasm module with exact descriptor/binding
  replay, normalized artifact binding, canonical carrier SHA-256 validation,
  module-owned scratch and opaque lifecycle handles, checked HIR invocation,
  two-pass result export, and explicit release/close operations. The generated
  TypeScript runtime now delegates provider lifecycle to those exports. The
  subsequent canonical carrier migration makes that package interoperable;
  #229 and the broader #162 matrix remain open for their larger acceptance
  scope.

- Add the internal `public-generic-wasm-provider.v1` Project profile as the
  first compiler-owned target foundation for #229. Package Manifest v1 now
  selects exactly one concrete checked generic owned-record endpoint, derives
  its Public Generic Descriptor v1, independently replays it against the exact
  linked program and project revision, and binds the verified digest into
  Project Lock v1. Web, npm, native, and Agent Transport artifact routes remain
  fail-closed until the Core Wasm provider emitter exists. The #162 native
  generated-caller settlement gate now shares this compiler-derived descriptor,
  binding, instance, cleanup, and leaf identity across C, C++, Rust, and the
  executed reference provider; the endpoint body remains the explicit reversal
  fixture, so neither issue is closed by this phase.

- Make the push-driven CI and Docs workflows latest-ref only: a newer run now
  cancels its in-progress predecessor instead of spending hosted runner minutes
  completing already-superseded matrices or documentation builds.

- Extend the bounded resumable-effect source profile from one site to one to
  eight direct sequential Copy-scalar `yield` sites. The deterministic plan now
  carries ordered per-site states and yield-free resume projections; the public
  interpreter uses an opaque in-memory request/answer history, while private
  native `-O0`/`-O2` and Core Wasm runners replay the same projections. Every
  historical request is checked, and bindings commit exact arguments plus
  prior answer bits. This remains pure replay, not live-frame/liveness
  lowering: control-dependent yields, owned state, durability, scheduling and
  public target ABIs remain open. Distinct request/response assignment sites
  fail closed with `SPX-T299`; `let` and tail sites retain distinct types
  (#204). The original one-site `ResumablePlan`, exhaustive `ResumableStep`
  and start/resume functions remain source-compatible; sequential lowering and
  execution use additive plan, step and start/resume surfaces.

- Add the initial deterministic compiler-owned three-state HIR plan for the
  single-top-level, Copy-scalar `yield` slice (subsequently widened above).
  Resume state now binds the exact
  checked program, yield site, and bit-exact original arguments. Closed
  projections now isolate disconnected yielding functions while retaining the
  selected and authored-entrypoint direct-call closures; retained non-scalar,
  effectful, owned-cleanup, generic and function-reference surfaces, all
  authored nominal/authority surfaces, reachable yielding callees, retained
  incoming callers, forged projections,
  and stale bindings fail closed. The interpreter consumes the plan identities,
  while `cfg(test)`-only
  native `-O0`/`-O2` and Core Wasm runners execute independently validated
  yield-free projections, preserving exact normalized prefix/suffix arithmetic
  and pre/postcondition failures. Ordinary native/Wasm emitters still refuse
  `yields`; arbitrary NaN-payload preservation across the JavaScript Wasm test
  adapter, public continuation ABI, external-await runtime seam, durable source
  checkpoint, Agent migration, and general live-frame/control-dependent yield
  lowering are not claimed. The reference v1 journal additionally completes
  an already-observed partial turn without redispatch and never repeats cleanup for an in-memory
  replayed terminal; its unchanged wire has no durable append or cleanup
  settlement, so decoded terminal cleanup and crash recovery remain ambiguous
  and unclaimed (#204).

- Add a deterministic, non-executing cross-language benchmark reproduction
  capsule that binds exact task and adapter inventories, equivalence files,
  public and hidden trees (including empty directories), and per-language
  adapter policy. Stable descriptor-backed reads reject path swaps, and a
  matching capsule says only that scoring inputs match, never that a model or
  toolchain ran (#211).

- Compose the task-service reference application and generated service
  scaffold with `std.tracing`'s pure trace-context and secret-classification
  policy now that reachability pruning admits the dependency closure. Valid,
  malformed, and caller-classified-secret cases join the existing backend
  parity gate; no logging, span emission/export, or host authority is added
  (#194).

- Record the first hosted Windows compilation evidence for the production
  provisioner confinement module. The successful Windows Server 2025 job
  compiled the `cfg(windows)` code at the exact recorded revision but selected
  no confinement test function, so runtime confinement evidence remains open
  (#236).

- Add an offline, unavailable-only admission gate for future external coding-
  agent baselines. It binds the owner-pinned task inventory bytes and Zero
  source revision to closed toolchain, interface, model, and reviewed-port
  provenance without running a model or treating provenance as execution
  evidence (#107).

- Carry exact empty `Bytes` arguments through source-native Core Wasm Agent
  stages using the existing named slice/range/copy ownership path, with
  interpreter, native `-O0`/`-O2`, and Core Wasm parity plus a retained
  malformed-carrier refusal (#143).

- Harden generated package preview verification around a flat physical-file
  inventory and private verified snapshots before optional npm or Cargo dry-
  runs. Package publication, signing, registry credentials, and support-policy
  promotion remain outside this tool (#145).

- Harden the paired-agent pilot's stale-recovery metric so only an exact,
  well-formed conditional write to the drifted source can count as recovery,
  and only the gateway's exact stale-precondition refusal can count as a
  rejected stale write. Reads, unrelated paths, malformed commands, and other
  failures now fail closed instead of producing optimistic evidence. Fresh
  cohorts now have a digest-bound reviewer packet and exact 18-tuple audit:
  direct treatment labels are withheld, task content remains visible, and
  hostile or drifted evidence fails closed without backfilling the historical
  cohort (#105).

- Preserve `i64::MIN` when source-native Agent stage arguments are synthesized
  for Core Wasm and native C11 execution. A four-leg regression now exercises
  the value through the interpreter, native `-O0`, native `-O2`, and Core Wasm,
  while empty byte literals retain their stable refusal (#182).

- Add a binding-first Lean-kernel certificate recheck that re-derives the
  source, exact obligation selector, Lean document, and Wasm artifact before
  consulting caller-supplied kernel authority, then requires exact recorded
  axiom results. Kernel-confirmed evidence can now be associated with one exact
  retained Project `ProgramRoot` and appended to its assurance manifest through
  an opaque verified token; sibling projects, altered source, subset
  certificates, and replayed revisions fail before attachment. No process,
  filesystem, network, or tool-discovery authority is added to the compiler
  (#186).

- Preserve every failing real-distribution role in the provisioned Linux
  doctor diagnostics instead of stopping at Node's first failure. Hosted run
  35472257722 remains red (12/13 in both suites): Clang passed, Node received
  `SIGSEGV`, and Rust's exact termination awaits the next instrumented run;
  confinement policy and WP-05 status are unchanged (#61).

- Track AArch64 Linux doctor confinement separately with a dispatch-only native
  Arm runner and an exact 24-case plan. The committed evidence is limited to a
  historical local Docker Desktop Arm VM run (24/26); the two real-distribution
  fixtures remain excluded for missing selector/bundle preconditions, and no
  hosted, current-head, or physical-device support claim is made (#279).

- Compose `std.tracing` trace-context field admission with the existing
  `std.log.redact` six-flag caller classification, preserving
  malformed-context refusal and left-to-right lazy evaluation. It does not
  inspect bytes or tracestate and remains a pure policy layer with no span
  generator, exporter, sink, or transport authority (#193).

- Version and digest the public-generic hostile corpus, pinning 17 shared case
  outcomes plus exact bytes for 9 structured-descriptor and 6
  malformed-trusted cases in one deterministic manifest while correcting
  stale evidence counts. Persistent compiled source mutants now prove that
  five previously untested descriptor-envelope refusals are individually live
  in the generated Rust, shared C11/C++17, and TypeScript consumers, and that
  weakening each branch reaches the provider before clean settlement.
  Exact-current-head hosted evidence, carrier-side consumer hostility, and real
  generated endpoints remain outstanding (#173).

- Drive the private frozen iterative Agent loop through the existing sealed
  interpreter, native C11 `-O0`/`-O2`, and Core Wasm stage backends. The local
  parity gate covers proposal admission, fresh consumed grants, injected binary
  reads, continued and terminal reductions, cancellation, malformed proposals,
  lifecycle ceilings, and semantic evidence settlement while leaving production
  wrappers interpreter-only. It also fixes canonical lexical selection of ten-
  plus synthesized Wasm projection exports without reordering driver, decode, or
  cleanup plans. Native/Wasm interpreter-fuel and hosted evidence remain open
  (#182).

- Extend the source-live repair preview with an explicitly selected OpenCode
  Provider-Adapter route, durable V2 receipts, a restart-stable absolute
  deadline, and checkpoint identity bound to the chosen executable and
  scratch path. Terminal recovery now replays recorded provider bytes without
  pre-deriving fixture diagnostics or redispatching, binds corrective turns to
  the actual canonical prior effect, and rejects tampered settled responses.
  A public embedding seam can now inject one opaque, host-selected,
  fixed-buffer candidate-test observer: its canonical result is bound to the
  exact candidate/base/source/capability, settled as typed feedback for a later
  provider turn, and replayed without redispatch. Executable bytes and metadata
  are bound, run/export scratch state is cleaned before reuse, and foreign or
  tainted observations fail closed. The ordinary CLI still grants no test or
  publication authority; the scripted V1 seam stays frozen for offline
  regression coverage (#116).

- Add whole-line UTF-8 validation and checked nonnegative `i64` total helpers
  to the catalog-normalizer example, with malformed-scalar, boundary-total,
  backend-parity, and mutation-control regressions. This is a
  bounded foundation tranche; the complete record parser, canonical writer,
  duplicate handling, and independent oracle remain open (#124).

- Have tag release jobs produce pinned GitHub build-provenance attestations
  for each platform archive and keylessly sign the final aggregate provenance
  with pinned cosign tooling. Release publication now carries those bundles
  beside the manifest and provenance, while policy and tests distinguish the
  GitHub OIDC `sub` claim from the Fulcio workflow-URL certificate identity.
  A bounded offline verifier now closes the Sigstore v0.3 framing, GitHub
  workflow-v1 predicate structure, exact archive inventory, and aggregate
  manifest/provenance/claim bindings before invoking an explicit verifier
  capability. `SigstoreOfflineVerifier` supplies the standalone CLI's default
  pure implementation, checking certificate-chain and pinned identity,
  DSSE/message signatures, signed time, and transparency-log evidence against
  exact caller-supplied historical trusted-root bytes. An embedding host can
  still inject another capability. Verification performs no network or root
  refresh and does not establish current revocation state, publication,
  reproducibility, or support. No signed hosted release is claimed yet (#168).

- Stop Universal Semantic Transaction v1 from refusing every project with a
  commented bundled dependency (#274). `ProjectCandidate::apply`'s
  `materialize` step re-derived *every* source in the revision through the
  comment-dropping canonical formatter, so the comment-free precondition had
  to span the complete workspace -- including compiler-bundled dependency
  source (`std.auth` carries 253 comment lines, `std.jobs` 34) that no project
  can edit, making `SPX-G525` unsatisfiable by construction rather than by
  authoring. `materialize` now preserves an untouched program's exact base
  bytes, and the new `src/project/semantic_transaction/canonical_sources.rs`
  enforces comment-free canonical source differentially, of exactly the
  sources a transaction rewrites or drops. A comment in a rewritten source is
  still refused with `SPX-G525`, and the `SPX-G525` regression that proves it
  is unchanged.

- Run a real Lean kernel against the `proof_export` obligation export for the
  first time (#186). The module shipped with no `LeanKernel` implementation and
  no evidence Lean accepts its generated proofs -- every test replayed
  synthesized output. `scripts/lean-export-gate.py` is that implementation,
  deliberately outside the crate so the compiler gains no ambient process
  authority: it confirms the host runs the pinned `leanprover/lean4:v4.34.0`
  (the same pin `proofs/kernel0-lean` uses), checks the committed golden
  document plus two seeded variants, and compares each result byte-for-byte
  against transcripts committed under `src/proof_export/testdata/`, so recorded
  evidence cannot silently go stale. Results: `omega` discharges both the
  checked-range obligation and the postcondition, axiom-clean and
  byte-reproducible across runs; a seeded `sorry` is refused; and a vacuously
  weakened conclusion is *accepted* by the kernel with an axiom set cleaner
  than the honest proof's -- recorded as data, because it is precisely why a
  certificate binds `lean_source_sha256` and `verify_certificate_against_source`
  re-renders the document from source instead of trusting the embedded bytes.
  Fixes one real fail-open the real kernel exposed: Lean 4.34.0 prints
  ``declaration uses `sorry` `` with backticks, so both single-quoted spellings
  `kernel_report::parse` was written against (from synthesized fixtures) were
  dead against the very toolchain the module pins, leaving only the `sorryAx`
  clause load-bearing; quoting is now normalized. All kernel evidence is
  **local-host only** -- hosted CI provisions no Lean toolchain, the gate is not
  in `scripts/quality.sh` and not in `release-gate`'s blocker set, and every
  certificate now carries that as an explicit nonclaim.

- Narrow `ProjectFrontendCache`'s AST-level invalidation (`src/project/incremental.rs`):
  a provider module's own changed/added/removed source still invalidates its
  frontend cache entry, but an unrelated consumer that only imports from that
  provider no longer loses its entry through the old transitive reverse-import
  closure. Parsing and canonicalizing one file is a pure function of that
  file's own bytes, so a reused entry is bit-identical to a fresh reparse
  regardless of what any provider did, and every cross-module check (import
  stub validation, the checked-HIR cache's own exact `synthetic`-equality
  gate) still reruns unconditionally against the current build's sources, so
  a provider's exported-surface change is still caught -- narrowing this set
  only decides which unaffected files skip a redundant reparse. On
  `examples/calculator-project`, a provider body edit went from 0 modules
  cloned / all 3 reparsed (80 AST nodes) to 2 modules cloned / 1 reparsed
  (32 AST nodes), matching the existing local-body-edit case instead of being
  its expensive opposite (#130, #131).

- Add `scripts/generated-package-release.py` (`prepare`/`check`), a
  release-preparation and dry-run-only layer around the existing Project v8
  `owned-data-api.v1` generated npm/Rust packages: closed-inventory,
  secret/local-path, and no-private-dependency admission; deterministic
  README/LICENSE/checksum-manifest wrapping; and a `check` step that only
  ever runs `npm pack --dry-run`/`cargo publish --dry-run` (never
  `--publish`, never near a live registry credential). Prepares (but does not
  make) the maintainer publication decision drafted in
  `docs/GENERATED-PACKAGE-PUBLICATION-DECISION-DRAFT-V1.md`; nothing is
  published by this change (#145).

- Refuse a bundled dependency member that names an authored type at the Useful
  Data workspace linker boundary, by name, rather than admitting it into a
  declaration set that deliberately holds no authored type and letting it
  surface much later as `inline-array slot references an unknown type` inside
  inline-array capacity analysis. Drop such a member from the retained
  dependency inventory when nothing reaches it, so a bundled package may still
  declare a generic record. This restores `semaprax new --template service`,
  which the `record Secret<T>` added to the bundled `std.auth` had broken for
  every scaffolded project (#268).

- Execute the unchanged native reference provider inside freestanding Core Wasm
  at O0/O2 under both V8 tiers, with in-module allocation, ownership registries,
  multi-call copy-in/export/release and no host imports. Add a bounded private
  transport, exact native/corpus comparison, allocator self-tests, hostile-range
  and lifecycle tests, deterministic artifacts, canonical replay, and compiled
  semantic mutation controls for #162. This is a C11-reference fixture, not a
  Semaprax-generated generic endpoint, public Wasm ABI, PG-7 completion or hosted
  promotion. Actual Rust-renderer equality remains a separately selected gate.

- Enforce single-owner, non-reentrant native reference-provider admission with
  one atomic owner/entry word; retain ownership until the last provider closes,
  and isolate failure injection, traces and sticky diagnostics per caller thread.
  Reject misuse without touching caller output/owning aliases or reporting false
  zero-resource counters. Extend #162 with deterministic pthread misuse/handoff,
  exact/+1 thread-identity admission, sanitizer, replay and mutation gates. This
  enforces the existing synchronous restriction, not concurrent execution,
  compiled generic-provider support, PG-7 completion or hosted promotion.

- Harden the host-owned TypeScript/Wasm reference caller: authenticate immutable
  module-byte snapshots, reject unverifiable precompiled modules, isolate private
  descriptor/binding authority, enforce one in-flight owner and exact framed
  payload bounds, preflight complete result frames, and propagate primary plus
  secondary cleanup failures. Extend #162 with 108 settlement cases, 32 host
  regression groups, 13 authenticated module subjects, two V8 Wasm execution
  tiers, fresh C/C++ semantic comparison, strict replay and twelve type-checked
  behavioral mutants. Add actual-generator byte-equality gates without claiming
  those Rust gates have run, compiled-provider support, PG-7 completion or hosted
  promotion. No existing native ABI or public rejection profile is widened.

- Propagate explicit native release failures through C11/C++17 calling consumers
  without exposing a decoded success or overwriting earlier failures. Add
  checked close with retained ownership, reverse caller rollback, bounded
  encoding/export/decode, and per-invocation injection isolation. Implement
  corresponding Rust settlement and fallible codecs, with separate Cargo
  regressions. Extend #162 by 112 shared consumer cases, independent caller
  counters, replay evidence and eleven compiled semantic negative controls;
  actual generator/Rust execution remains a distinct gate, and full PG-7,
  compiled Wasm and hosted promotion are not claimed.

- Return real native input/result release failures without overwriting an earlier
  call's settlement. Extend #162 with actual per-leaf result allocation/copy and
  export preflight failures, pre-rollback byte checks, exact 16 MiB success,
  compound reverse cleanup, and 72 pinned physical-phase cases with a third
  independently replayed evidence artifact. Legacy logical trace bytes and
  public ABI declarations remain unchanged; full PG-7 and generated-consumer
  failure propagation are not claimed.

- Prevent stale native public-generic handles from reviving when heap addresses
  are reused; validate provider identities before access/close and pin bounded,
  non-recycled identities plus exact/+1 live-resource admission. Isolate
  settlement between sibling prepared inputs so an earlier success cannot mask
  a later failure as success-without-result. Extend #162 with lifecycle/recreation,
  identity-exhaustion, 8,192-call stress and paired replay evidence; public ABI
  signatures and unsupported/unpublished status remain unchanged.

- Share one bounded, digest-pinned public-generic settlement manifest across the
  existing model/native harnesses; preserve primary failures before cleanup and
  reverse every native result rollback. Add per-case native observations, real
  allocation/compound-failure regressions, retained trace expectations, and
  portable sanitizer/replay evidence. This advances the fixture subset of #162;
  it does not close PG-7 or widen public ownership support.

## 0.5.0 — 2026-09-14

- Bind effect-free retained source job handlers to persisted deployment
  descriptors before claim, using the existing durable job runtime and
  checked interpreter; refuse stale roots and handler substitutions (#192).

- Complete the ten-row feature-composition inventory, exact ownership-profile
  refusal regression, and provisioned strict differential campaign route
  with explicit CI selections and nonzero-case enforcement (#103).

- Add exact SDK-envelope byte reservations to durable retries with frozen V2
  journal preservation, canonical V3 recovery, and post-poll cancellation
  checks (#179). Tighten provider conformance around request admission,
  completion, final bytes, and usage regressions with corpus V2 (#181).

- Add a checked two-call offline repair loop and private CLI demo with
  bounded candidate edits, diagnostic feedback, read-only review evidence,
  and terminal journal replay (#116). Extend imported owned cursor failure
  composition with exact Wasm cleanup-order and sticky-status controls (#103).

- Account compact workspace validation clones before allocation and schedule
  the final uncached graph phase by temporary HIR overhead without raising
  its cap (#124). Add decoded ID bounds and allocation-free token equality
  to the catalog helper application with cross-backend oracle checks.

- Add generic durable retry/failover with acknowledged attempt journals,
  exact retained execution/schema/model bindings, non-widening source and
  deployment limits, request-seed checks, and conservative recovery (#179).
  This is local host composition; checkpoints do not prove provider identity
  or billing.

- Add V6 durable source-model quote accounting with nonrefundable observed and
  unknown usage, absolute deadlines, cumulative migration carry, and optional
  request/response byte ceilings (#113). Retain observed quote overages in
  ordinary source-model admission as well.

- Retain exact generic model request/response reservations in the additive
  priced I/O envelope, cross-bind successor carry, and reject noncanonical
  recovery requests (#113). Add store-backed typed source-model execution with
  acknowledged intents, exact raw settlements, and no uncertain redispatch
  (#177). Release full sequential workspace HIR after compact validation facts
  and selected output carriers are extracted (#124).

- Pair generic live work reservations with durable monetary accounting (#113),
  and enforce opt-in source-model ceilings before adapter construction (#177,
  #179). Add a final uncached graph retry that charges retained output vectors
  while preserving earlier successful budget receipts (#124).

- Bind typed live model operations to deployment selection and commit their
  redacted attempt evidence in an additive execution root (#177). Propagate
  remaining host deadlines and distinguish reserved, observed and unknown
  provider charges in a separate Runtime v1 receipt (#113).

- Add counterbalanced pilot scheduling, isolated MCP tools for both comparison
  lanes, retained candidate source bytes and exact transport archives (#105).
  Trial capture remains separate from eligible observations and review.

- Preserve pre-dispatch OpenCode cancellation as a zero-call cancellation
  failure (#113). A changed journal or claimed dispatch without a receipt
  remains a model failure; unresolved attempts cannot become clean refusals.

- Add direct owned String variant payloads (#216), with canonical String
  lifecycle replay, own/borrow matching and backend cleanup. Scalar-match guard
  and arm temporaries settle through exact cleanup regions. Generic String
  substitutions and nested owned-record variant payloads remain restricted.

- Add explicit Rust-host Argon2id password hashing, authenticated sessions and
  signup/login/logout composition (#191). Persist job cancellation and recurring
  schedule advancement with checked arithmetic and replay evidence (#192).

- Add checked-source HTTPS POST across explicit provider, native C11 and
  Core-Wasm/npm fixture paths (#193), with bounded body/response bytes, explicit
  destination authorization and no automatic retry or redirect for POST.

- Add an explicit native HTTPS transport for provider adapters (#181), with
  injected credentials, bounded buffered responses and conservative dispatch
  uncertainty. Extend compact task context with ordered seeds, revision binding
  and selectable token accounting (#197). Add a host-driven single-job
  checkpoint runtime and evidence-based recovery (#192).

- Drive bounded retries and ordered failover through injected provider adapters
  (#179), preserving charged attempts and stopping on uncertain outcomes.
  Add host-transport protocol adapters for Responses and Messages (#181).
  Recheck cancellation and deadlines when streaming settlement returns.
  Add an eighth cross-language task family exercising owned byte mapping and
  hidden-oracle rejection in Rust, TypeScript and SEMAPRAX (#106).

- Import bounded provider invoice evidence through explicit verifiers and retain
  reconciliation results (#180). Enforce source usage consistency and complete
  audit-view disclosure, payload association and truthful privacy claims; add
  canonical audit replay and direct receipt emission from live run evidence.

- Decode canonical model receipts with strict bounds and enrich actual generic
  and source attempt journals using retained root, request and host metadata (#180).
  Join explicit adapter observations and provider usage without inventing
  missing measurements; preserve failure and unresolved lifecycle evidence.

- Capture ordered provider adapter attempts and replay retained chunks against
  actual generic/source runtime schemas (#178/#180). Bind generic settlements
  to validated journal requests and responses; compare provider token and cost
  observations independently during invoice reconciliation.

- Validate streamed nested Proposal fields, variant cases, exact scalars and
  text/byte bounds from compiled type tables before final decode (#178).
  Expose read-only grammar states and bounded work counters; extend the
  generated TypeScript/Python/Rust client harness with adversarial chunking.

- Add versioned selective-dictionary model-text projections to CLI, retained
  service and compatibility negotiation (#201). Connect streaming proposals to
  Direct Runtime v2 typed effects and reject mismatched compiled schema envelopes
  while streaming (#178); nested semantic admission remains in the full decoder.

- Expose compact projections through CLI replay and retained service/MCP routes,
  with explicit format/profile negotiation and offline model-token measurement
  tooling (#201). Introduce Rust embedding API v2 for mandatory analysis/execution input caps, add
  cooperative request cancellation, and verify opaque session release (#203).

- Add authoritative task-context, Project API, candidate-diff, and Agent graph
  compact profiles with regeneration-bound replay (#201). Extend Rust embedding
  negotiation, cancellation, and the external consumer (#203). Connect the
  source Proposal grammar to incremental decoding and an explicit per-attempt
  provider adapter factory with bounded iterative context (#178).

- Connect the streaming Proposal decoder to the provider adapter and generic
  live-kernel seams (#178), and compose token/cost/call policy with the live
  work-budget hook (#179). Extend the embedding facade with opaque in-memory
  Project sessions, atomic refresh, semantic query, and candidate replay (#203).

- Add journal-derived model-call receipts (#180) for generic live runs and
  authenticated source checkpoints, including exact replay and Audit Capsule
  object references. Unrecorded timing and billing stay unknown. Harden enriched
  receipt replay against contradictory response states and decode refusals.
  Extend the Rust source embedding facade (#203) with bounded context v1/v2
  queries and explicitly authorized, cancellable, fuel-bounded interpretation.

- Implement additive Project Assurance Manifest v1 (#214): a canonical,
  integrity-bound envelope bound to one retained Project, workspace
  revision, ProgramRoot, and complete ordered source inventory. The profile
  deduplicates shared entry/public/test HIR obligations, records explicit
  unselected coverage, and admits only held `forbid_reaches` architecture laws;
  the existing single-file Assurance Manifest v1 bytes remain unchanged.

- Add additive source-journal I/O v5 accounting and private CLI config/receipt
  v3 (#113). Authenticated attempt rows reserve exact prompt bytes and bounded
  response capacity cumulatively; recovery and compatible migration retain
  reservations without profile conversion. Legacy source journal and CLI
  profiles remain unchanged. Add #103's result-allocation rejection case with
  ordered input release in interpreter/Wasm and status/resource parity in
  native C11 O0/O2, including a wrong-order oracle control.

- Extend #113's priced adapter regressions with exact retained retry I/O,
  invalid quote preflight, and deadlines reached at journal acknowledgements.
  Add #103's imported `std.bytes` view-composition oracle across the Project
  interpreter, native C11 O0/O2, and Core Wasm, including temporary cleanup.

- Add an explicit priced source-journal v4 route and private CLI config/receipt
  v2 (#113). Integer currency-bound reservations remain separate from work
  quotas and provider observations; replay retains unknown exposure and refuses
  quote drift. Add the private CLI's one-hop priced-to-priced migration carry,
  preserving cumulative reservations, observations, overage, and global money
  ordinals under a non-widening compatible quote. Legacy source profiles remain
  unchanged; unsupported profile conversion is refused.

- Construct ordinary imported function stubs from signatures without cloning
  discarded bodies (#124). Preserve fitting graph receipts and add an uncached
  fallback charging retained HIR plus the peak of sequential synthetic ASTs,
  within the existing builder limit; cached frontends retain summed charges.

- Add opt-in, bounded current-thread workflow stage observations and an offline
  campaign runner; compare exact cold/warm frontend products and prepared
  traced/untraced products before timing. Measurements remain local evidence,
  separate from canonical artifacts and authority (Refs #85).

- Pin cumulative durable source reservation boundaries at zero, exact, and
  one-unit-over ceilings; terminal recovery retains accounting with zero
  new proposal, model, or effect dispatches (Refs #113).

- Exercise eight borrowed byte-view call compositions across interpreter,
  C11 O0/O2 and Core Wasm, including offset-sensitive forwarded views,
  comparator rejection and the stable escaping-view diagnostic (Refs #103).

- Validate bounded Descriptor-v1 frames and versions in generated calling
  consumers before exact pairing and provider admission; exercise canonical
  shared mutations and byte-identical malformed configured descriptors in
  Rust, C11, C++17, and TypeScript/Wasm (Refs #173).
- Add independent sensor-conjunction and stable three-job ordering benchmark
  families with candidate-preserving hidden overlays and three-port negative
  controls (Refs #106).

- Add private `semaprax-full source-live run|resume|migrate` commands
  (#113/#116), with held-directory checkpoints, exclusive writers, bounded
  explicit task/read inputs and retained-Project bindings. A migration carries
  the predecessor clock floor and nonrefundable work into one claimed
  destination. Recorded-provider tests cover run, recovery and checked migration;
  monetary pricing and the actual source repair workflow remain separate.

- Extend the catalog-normalizer source application with bounded JSONL record
  counting and line/body limits (#124). Seven application cases execute on
  the interpreter, C11 at both optimization levels and Core Wasm; a terminal
  newline counting mutation is rejected. Full record normalization remains.

- Reduce the last-resort workspace graph construction estimate (#124) by
  charging the largest sequential temporary import clone once. Imported
  stubs release their discarded contract-vector buffers. Earlier accepted
  receipts and the 18 MiB cap are preserved; core retries stay bounded.

- Add a fifth held-out cross-language benchmark family for half-open booking
  conflicts (#106). Rust, TypeScript and SEMAPRAX share candidates across
  public and hidden runs; an inclusive-end mutation passes public cases and
  fails hidden adjacency cases on each port. Actual coding-agent trials remain.

- Migrate suspended source-mode agents through checked retained-Project A→B→C
  handoffs (#115), preserving schema provenance and cumulative charged work.
  Acknowledged migration results resume at Observe; initialization and completed
  effects do not repeat. Lost acknowledgements, cancellation, deadline drift,
  and malformed recovered State fail closed under explicit host-store freshness.

- Add private filesystem v3 checked atomic writes (#228), with inspectable
  Published, NotPublished, and Uncertain outcomes and an exhaustive std.fs
  variant wrapper. Interpreter, native C11 and Core Wasm preserve the separate
  callback failure channel and legacy v2 behavior. Graph v46 binds the new
  operation; compact conformance fixtures keep the existing construction limit.

- Add a fourth cross-language benchmark family for a multi-module invoice
  calculation (#106). Candidate-preserving hidden entry modules distinguish
  whole-subtotal rounding from per-item rounding in Rust, TypeScript, and
  SEMAPRAX Project execution. This adds corpus coverage, not coding-agent trials.

- Separate native public-generic allocation accounting from the 16 MiB
  logical carrier limit (#250), allowing metadata and overlapping full input
  and result payloads. Exercise exact-byte boundaries across local native,
  interpreter, and Wasm fixtures, with bounded allocation failure and cleanup.

- Pin Cargo, rustc, and rustdoc in the generated Rust consumer's MSRV gate
  (#226). Selecting Cargo alone had allowed the ambient newer compiler to
  satisfy the check. Native allocator capacity remains tracked in #250.

- Cover drop-free nested variant payloads through interaction-schema derivation,
  canonical decoding, and hostile nested-field refusal (#216). Correct the
  proposed checked-write taxonomy to distinguish proven non-publication from
  phase-ambiguous legacy I/O errors (#228); missing-parent provider regressions
  preserve the current fail-stop operation.

- Add a separate structured-envelope validation task to the cross-language
  corpus (#106), with candidate-preserving hidden tests and a negative control
  for wrong error precedence and removed visible assertions. The additive
  SEMAPRAX Project adapter leaves the original pilot and task routes intact.

- Connect source checkpoint execution to the checked iterative driver (#113).
  Journal v2 reserves fresh fuel on replay, shares one model-budget ledger,
  persists optional usage and terminal evidence, refuses uncertain redispatch,
  and retains partial failure evidence. The OpenCode durable source borrows
  that ledger; source migration and a durable CLI remain pending.

- Exercise lazy boolean operands containing checked division failure through
  the differential corpus (#103), including interpreter, native O0/O2, and
  Core-Wasm observations with explicit lane results.

- Derive Assurance Manifest result-ownership and resource-cleanup obligations
  from independently revalidated HIR (#214), and keep candidate summaries
  aligned. Architecture-law derivation and workspace binding remain pending.

- Complete the documented-limit decision for compiler capacity (#241): name
  the distinct checked-cache ceiling in SPX-G256, pin its inclusive boundary,
  and state graph/replay limits and the source-versus-runtime byte-copy guard.

- Run TypeScript/Wasm consumer Node entry points with relative fixture paths
  to avoid Windows extended-path main-module resolution failures. Complete
  private OpenCode/source-journal documentation metadata and catalog entries.

- Include Cargo example targets in the CI unit shard, preserving exhaustive
  workspace inventory and refusal of unknown target kinds.

- Add an explicit OpenCode proposal-attempt checkpoint boundary over the same
  source journal and accounting ledger (#113). It validates bound context and
  phase before charging, acknowledges intent before transport, and persists
  the outcome before exposing response text. Full source replay remains pending.

- Add bounded source checkpoint primitives with strict causal validation,
  poisoned writes after acknowledgement loss, and restoration through the
  existing charge/deadline ledger (#113). Source runtime replay, provider
  receipt persistence and migration integration remain pending.

- Derive cached Project Agent interaction facts from retained authenticated
  source programs, fixing false SPX-G564 failures without reparsing (#85).
  Repair the embedding-example index and Clippy CI blockers, and provision
  pinned Clippy for the public-generic consumer job.

- Provision pinned TypeScript 5.8.3 for the public-generic hosted job and
  fail closed on missing consumer tools; record platform/toolchain identities
  with separate Unix and Windows preflights (#163). Fresh hosted evidence
  remains pending.

- Check the OpenCode source route's shared deadline around deterministic stages,
  proposal admission, effect dispatch and result publication (#113), preserving
  earlier selected failures. Durable source failure evidence remains pending.

- Require explicit cumulative reservations for OpenCode source attempts and
  retain their bounded usage observations across malformed retries and failures
  (#113). Check the shared absolute deadline at settlement and later kernel
  boundaries; source recovery and migration accounting remain separate work.

- Connect one explicitly configured free OpenCode provider to the existing
  source-feedback driver, preserving canonical proposal admission (#112).
  Bound Unix process output/cancellation, bind real CLI receipts, preserve
  reported usage and redact provider error categories. The fixed local live
  smoke reached Complete; broader hosted/provider support remains separate.

- Reuse exact retained source ASTs during cached Project finalization and
  prelude-bound revision replay (#85). Remove nine hidden public parser calls
  from unchanged calculator builds in both cache modes, preserving revision
  hashes and admission checks; no timing improvement is claimed.

- Add explicit untraced prepared Project execution with unchanged traced
  behavior, fuel, cancellation and revision replacement. Add matching cold
  and prepared benchmark products and truthful platform-specific memory
  observations (#85); no new performance measurements are claimed.
- Persist migrated live-kernel handoffs, state and destination journals in one
  bounded canonical checkpoint before dispatch, with recovery and store-failure
  regressions (#115). Checked source migration and cumulative-chain integration
  remain open.
- Add a non-editing OpenCode availability smoke and bind archived event streams
  to the matching exported session and frozen prompt (#105/#112). This is
  provider availability evidence, not a coding-agent trial or live-driver adapter.

- Correct the Workspace Semantic Graph and Context/Impact/Review workspace
  limit projections to report the enforced 18 MiB builder ceiling from one
  renderer (#248). The 16 MiB analysis/cache ceilings remain separate. Re-pin
  exact artifact hashes after verifying that restoring only the old limit
  and dependent digests reproduces every previous known answer.
- Repair standard-library conformance registration for guarded logging and
  `std.metrics`, and synchronize the generated auth/TOML catalogs (#102).
- Exercise frozen execution evidence around a live fixture invocation and
  clarify that terminal journal replay preserves its case and carrier digest,
  not the original carrier payload (#108).
- Add a standalone Rust consumer for the public check/format/graph embedding
  facade, with its own offline lockfile and explicit checkout-only scope (#203).

- Rewrite the `SPX-H006` cleanup-replay path-budget diagnostic's message to
  name the actual cost driver instead of only the budget it exceeded. The
  previous wording ("cleanup replay path bound exceeds the global path
  budget") told an author nothing about *why*: the real driver is
  combinatorial multiplication of independently-combined branch outcomes
  within one function (2^N terminal paths for N such branches), not raw
  branch count, so the natural fix an author reaches for on reading the old
  message - splitting into helper functions - does not help unless it breaks
  the combination. The new message states the measured path count and budget,
  names the combinatorial driver, and names an actionable remedy (make the
  branches mutually exclusive, or combine their results across separate
  calls). The diagnostic code is unchanged; per issue #241, neither
  `SPX-G171`'s workspace-graph byte budget nor `SPX-H006`'s path budget was
  raised, because no session has evidence that a higher value keeps the
  workspace graph or the semantic cache finite - both remain documented,
  regression-pinned limits in the completion matrix rather than raised
  ceilings. A boundary fixture pinning the exact measured terminal-path count
  at the crossover (98,300 paths for 15 independent branch terms, not the
  naive `2^15 = 32,768` estimate the previous fixture comment assumed without
  checking) replaces the earlier code-only assertion, plus a dedicated
  regression that fails if the message regresses to the old cause-free
  wording.

- Freeze the Public Generic Boundary Profile, Descriptor and Carrier v1, and
  add a reference codec for the descriptor and carrier wire formats. The same
  callable generic boundary had been described three times by three
  overlapping issue packs, with a real scope conflict between a minimal
  one-owned-`Bytes` slice and a contract admitting nested finite records. One
  admission profile now settles it with an explicit in, deferred and excluded
  table, so downstream implementation work has a single contract to build
  against rather than three. Two classifications - owned generic variants and
  public generic templates - are recorded as contested and awaiting review
  rather than silently decided.

- Specify and implement the Live Invocation Contract v1: invocation identity
  that survives retry, resume and recovery, a causal journal with
  table-driven validation, and a provider-independent `model.invoke` effect
  with an explicit capability requirement, a closed failure domain and
  cancellation checkpoints. Replay makes zero dispatches, and an unrecorded
  result can never be reconstructed by hashing because the journal carries
  response bytes rather than only a digest. Fixture transport only: no
  provider binding, no deployment wiring and no live model call.

- Introduce Assurance Manifest v1, a deterministic per-obligation manifest
  bound to exact source bytes that fails closed on drift. Obligation identity
  keys to a declaration's persistent stable id rather than a byte offset, so
  it survives a pure formatting change, and the assurance lattice is a
  genuine partial order rather than a single ranking - compiler-proved,
  SMT-proved and model-checked are deliberately incomparable. External
  records let later formal-method backends contribute without this module
  changing, so a simple candidate summary never waits on formal proof.

- Freeze the catalog-normalizer acceptance application and an independent
  oracle: 25 requirement ids, 51 known-answer cases split into published and
  hidden, and six runnable negative controls that each provably diverge from
  the correct output. The hidden-case boundary is a review policy rather than
  cryptographic isolation, and the specification says so plainly instead of
  implying a guarantee the repository cannot enforce.

- Preserve comments and unrelated bytes in the v2 `ReplaceExpression` route.
  The transaction previously rejected any workspace containing a comment
  before it even selected an expression, because the shared candidate rebuild
  always reprinted through the comment-oblivious canonical formatter. The
  edited file may now carry comments, preserved by a span-scoped splice that
  is independently reparsed and required to match byte for byte; a comment
  overlapping the edited span is refused rather than silently relocated.
  Every other source keeps the exact comment-free requirement.

- Resolve `core.option` in the `useful-text-consumer.v1` linker, which never
  seeded the compiler prelude its Useful Data sibling relies on, so any
  `match byte_get(...)` in a Project-linked text package failed validation.
  The profile's public signature restrictions are unchanged and pinned by a
  test.

- Bind release publication to the exact-tag gate explicitly, and add a
  reconciliation check for release claims. This caught a live defect: the
  README claimed v0.4.1 was the published tag while citing v0.4.0's date,
  commit and anchor.

- Connect an external coding-agent runner to the existing comparison ledger,
  reusing its plan, trial, ledger, observation and audit schemas unmodified.
  A candidate cannot escape its sandbox, write the finalized ledger, or claim
  its own acceptance: a backend that reports every criterion passed while
  leaving the work undone is still scored as failed. Offline fixture backend
  only; the paid paired pilot needs an approved model budget.

- Correct a stale claim in the doctor provisioner specification, which stated
  the provisioned gate had never executed when nine real runs exist, the most
  recent of which failed.

- Give the MSRV shards the same time budget as the identical `verify-tests`
  command. Both run `scripts/ci-msrv.py --shard`, but the MSRV job had 40
  minutes against that job's 90 while compiling the same workspace on an
  older toolchain. The `integration-1` shard was cut off mid-test on two
  runs while finishing in about 35 minutes on others, so its result reported
  the runner's speed rather than the shard's outcome - and a cancelled
  blocker fails the release gate exactly like a real failure. The coverage
  guarantees are unchanged: the contract still forbids `continue-on-error`,
  `--no-fail-fast`, `--exclude`, `--skip` and caching, and still requires
  the full check and every shard.

## 0.4.1 — 2026-09-11

- Record cross-platform hosted evidence for the public generic ownership
  milestone. The `public-generic-ownership-milestone` job is green on
  `ubuntu-latest`, `macos-latest` and `windows-latest` for implementation
  commit `2ef043ba1b989f49b256e456f71fb6e89068bf33` in [run 34594793245](https://github.com/wavect/semaprax/actions/runs/34594793245), with each leg's log showing all four consumer
  toolchains exercised rather than skipped. Gate PG-8 and the four gates
  whose corpus that job runs - the type grammar, template and ordered
  argument identities, the compatibility rules and the candidate delta -
  move to `Hosted green`. PG-5, PG-6 and PG-7 stay open because the corpus
  contains none of what they ask for, and PG-9, the support and publication
  decision, stays open: public generic ownership remains unsupported and
  unpublished.

- Fix a temporary-path collision between two copies of the same owned-data
  native test helper. `owned_vec_bytes_runtime/native.rs` is
  `#[path]`-included by two modules of one test binary, so it is compiled
  twice and each copy owns a separate case counter starting at zero - while
  both named their scratch files from the process id and that counter alone.
  Two tests in the same process therefore shared a `.c` path and a binary
  path, which is exactly what the hosted shard reported twice: one run could
  not execute a binary a concurrent `clang` still held open (`ETXTBSY`), the
  next could not read a `.c` file the other copy's cleanup had already
  removed. The name now includes the module path, so the two copies cannot
  collide. The generated public generic consumer gate, which also compiles
  and then executes, retries a launch refused with `ETXTBSY` rather than
  adding another way for a foreign fork to redden an unrelated shard.

- Make the generated C and C++ public generic consumers build on Windows.
  The hosted leg found two things a Unix-only run could not: the UCRT marks
  the standard `fopen` deprecated in favour of its own `fopen_s`, which a
  `-Werror` build turns into an error, and a compiled consumer needs the
  host's executable name because Windows resolves a bare `consumer` by
  looking for `consumer.exe`. The generated sources now opt out of that
  deprecation, and the gate names the executable per host.
  The C++ consumer also drops `std::span`, which it used only to carry a
  pointer and a length together: a pointer and a size are the same
  information, so the generated artifact no longer needs a C++20 library
  header and builds under C++17. The compile check now fails on a warning
  or an error in the toolchain's output rather than on any output at all,
  because "compiles warning-free" is the claim; a linker note about
  something else is not evidence about the generated code.

- Make generated public generic consumers independent of the checkout that
  produced them. The fixed part of each consumer is an included template,
  and a Windows checkout delivers `.txt` with CRLF, so every placeholder
  whose match included its newline silently stopped substituting: the
  generated files kept their literal placeholders and lost both their type
  declarations and their embedded metadata. Templates are now normalized at
  generation time, `.gitattributes` pins `.txt` to LF like every other
  embedded text asset, and the generator itself asserts that no generated
  file carries a carriage return or a surviving placeholder — so this class
  fails where it happens rather than as a downstream expectation on one
  platform. A generator that only worked because of a checkout setting was
  not the deterministic generator the milestone claims.

- Guard the Windows checkout of the public generic ownership milestone job.
  Its first hosted run failed on `windows-latest` before any gate executed:
  the checkout itself cannot write this repository's retained evidence paths
  without `core.longpaths`, which every other Windows job already sets. The
  cross-platform leg was therefore reporting a checkout limit, not a
  milestone result.

- Add Public Generic Candidate Delta v1, milestone gate PG-4: a
  candidate-bound delta that describes the selected exports of an immutable
  Project candidate's base and final revisions over the public generic
  grammar, classifies the pair with the PG-3 rules, and binds the candidate
  digest, both Project and workspace revisions, both graph digests and a
  domain-separated facts digest. Inclusion is grammar-strict, so an entry in
  the delta is exactly a candidate public generic signature: no admitted
  export has one today, which makes the generic fixture all-excluded with
  the closed reason `borrowed_byte_view` - and the gate asserts its rendered
  bytes carry no template identity, instance term, record identity, or even
  the grammar's instance sigil. The substantive evidence rides on a Project
  v9 record-returning export, where a real `add_record_field` change is
  `breaking` with the finding on the record that changed and ordered
  arguments, substituted fields and owned leaves retained across mutation,
  recovery-capsule restoration and byte-exact independent replay
  (`SPX-PG301`-`SPX-PG303`).
  The grammar now also exposes `Rejection::ALL` and `Rejection::of`, so a
  consumer recovers a typed reason from the owning artifact instead of
  re-spelling its message, and the compatibility report exposes its digest.

- Add Public Generic Settlement Obligations v1: for one owned admitted
  instance parameter, which owned leaves a boundary is accountable for, in
  which order, how each is discharged, and what is released when a transfer
  fails part way - each fact bound to the compiler's own cleanup facts
  rather than derived beside them. The grammar's owned-leaf paths must equal
  the inventory's structural leaf order; each obligation is paired with its
  liveness flag in flag order and carries that flag's checked drop
  lifecycle, because an obligation whose flag does not match it has no
  stated way to be discharged; and the transfer unit is the cleanup plan's
  single whole live owned place, since reading leaves out of the plan would
  invent a per-leaf transfer the compiler never performs. Release order is
  the exact reverse of the canonical order, and every disagreement is a
  refusal (`SPX-PG501`, `SPX-PG502`) rather than a sort or a repair.
  This is the specification half of milestone gate PG-7, which stays open:
  nothing here allocates, transfers, releases or observes a runtime, and
  there is no public generic boundary to exercise on any engine.

- Wire the public generic ownership milestone corpus into CI as the
  `public-generic-ownership-milestone` job on `ubuntu-latest`,
  `macos-latest` and `windows-latest`, and declare it a release blocker so
  it cannot be satisfied vacuously. It runs the grammar, template-identity,
  compatibility and metadata-consumer projections, the separation gate, the
  frozen Project v9/v11 descriptor refusals of a selected generic result,
  and the four-language consumer gate, and it resolves and prints the
  consumer toolchains each host actually has - a language whose toolchain is
  absent is skipped, which is a narrower run rather than a pass.
  Milestone gate PG-8 stays open: the harness exists, the hosted evidence
  does not, and the gate moves only when the owning document records a run
  and job for an exact implementation commit.

- Add Public Generic Metadata Consumers v1: a Rust, TypeScript/Wasm, C11
  and C++ consumer generated from a candidate public generic surface, plus
  the length-framed canonical metadata format they read. Before a foreign
  toolchain can call a public generic export, several of them have to agree
  on what its types are and refuse everything else - and that is
  falsifiable without a calling convention. All four consumers are compiled
  warning-free and run for real against nine hostile documents, and each
  must report the same closed refusal (`malformed`, `term`, `mismatch`) as
  the others and as the Rust reference reader; the gate fails when no
  toolchain was available rather than passing silently. Identifiers come
  from term bytes rather than display names, nested instances are declared
  before their holders so C and C++ never see an incomplete type, and
  regeneration is byte-identical.
  This closes the grammar half of milestone gates PG-5 and PG-6; both stay
  open for their descriptor half, because no public generic descriptor,
  carrier or calling convention exists. No generated consumer receives a
  SEMAPRAX value, allocates one, frees one, or links against anything, and
  every generated file says so in its own banner.

- Add Public Generic Compatibility v1, gate PG-3 of the public generic
  ownership milestone: candidate surfaces over the type grammar, and a
  closed thirteen-reason classification of one ordered pair of them. The
  rules are deliberately stricter than source compatibility, because a
  foreign consumer reads the whole substituted field tree and owned-leaf
  shape of what it receives: adding a Copy field to a nested reachable
  record is breaking even though no canonical term, parameter position or
  owned-leaf path moves, and it is reported once on the record that changed
  rather than on every position mentioning it. Presentation is never
  compatibility - renaming records, type parameters, fields, exports and
  parameters yields `unchanged` with an identical surface digest - and a
  parameter's value identity is excluded for the same reason the repository
  excludes it elsewhere: a revision-scoped fact must not move a verdict.
  A signature position that is not a data type gets its own closed
  `view:` vocabulary instead of widening the grammar. Both artifacts are
  canonical JSON with directional byte-exact replay
  (`SPX-PG201`-`SPX-PG204`), and both record that no semantic-version,
  support, publication or runtime conclusion follows from a verdict.
  A candidate surface is a description, not an admission: no public generic
  signature is admitted, and local evidence is the only evidence.

- Add Public Generic Type Grammar v1, gates PG-1 and PG-2 of the public
  generic ownership milestone: a versioned, target-neutral term for the
  types a public generic surface could name, plus the explicit template and
  ordered argument identities derived from it. Identities are
  length-prefixed in bytes, so an identity holding the grammar's own
  punctuation still round trips and two distinct types can never render
  alike; digests are computed over persistent identities, declared arity and
  ordered parameter positions, so a record, parameter or field display
  rename changes nothing while argument permutation, duplication or
  substitution changes the instance identity and omission is an
  `arity_mismatch` refusal. The vocabulary is closed to the eight Copy
  scalars, direct `Bytes` and fully concrete authored records; the other
  twelve reasons reject, bounds refuse instead of truncating, and replay is
  byte-exact against an independent recomputation. It is deliberately not
  the compiler's internal, unversioned `identity_key` spelling.
  Local evidence only: no hosted run, no descriptor, carrier, package or
  consumer selects the grammar, and public generic ownership remains
  unsupported and unpublished.

- Make public generic ownership a separate milestone instead of a side
  effect of the internal generic closure. The new
  [Public Generic Ownership milestone](docs/PUBLIC-GENERIC-OWNERSHIP-MILESTONE-V1.md)
  owns nine prerequisite gates - a versioned target-neutral type grammar,
  explicit template and ordered argument identities, semantic compatibility
  rules, candidate ABI-delta evidence, generated Rust/TypeScript-Wasm/C/C++
  consumers, hostile metadata replay, owned allocation and failure
  settlement, cross-platform hosted evidence, and the explicit support and
  publication decision - plus separation invariants and an executable
  separation gate. The gate selects a generic template, a concrete
  generic-instance result and an owned generic parameter through the
  canonical ABI report, C header emission and the public scalar Wasm export
  edge, and pins each closed refusal, so an internal admission that starts
  producing a public generic surface reddens the build rather than becoming
  a silent public claim. Public generic ownership remains unsupported and
  unpublished; no grammar, descriptor, carrier or consumer is added here.

- Add effect-free listing cursors to `std.fs`, so the canonical
  immediate-name listing `list` already returns can be walked without a
  provider or any further authority: entry counting, per-entry span offsets,
  and an entry-name validity predicate rejecting an empty name, an embedded
  separator or NUL byte, and the two traversal names. No function gained an
  effect and the module's capability list is unchanged; recursive traversal,
  streaming and richer metadata remain Missing.

- Add the bundled `std.env.policy` package: a portable environment-variable
  name predicate, a value predicate, and a `NAME=VALUE` assignment cursor, all
  effect-free offset computations over borrowed bytes. It is a sibling package
  rather than more of `std.env` because the completion gate requires every
  `std.env` function to declare `process.environment.read`, and these functions
  read no environment - declaring that effect to satisfy a gate would claim a
  capability they never exercise.

- Add effect-free settlement and argument policy to `std.process`:
  classification of an existing settlement value as a normal exit or a signal
  termination - the only two kinds the termination encoding defines, per the
  `low2-kind-0-exit-u32-or-1-signal` graph fact - exit-code extraction defined
  only where the process exited normally, and argument-vector admissibility
  over borrowed bytes. A deadline expiry is deliberately not a settlement
  class: it is a call-level failure that prevents an output from existing, so
  classifying an unused bit pattern as one would invent a value no provider
  emits. Only `std.process.run` still carries `process.execute`; the new
  functions perform nothing and grant nothing, and broader physical-provider
  and general process support remain open.

- Add a checked lifecycle policy to `std.agent`: an admitted stage-transition
  predicate total over the lifecycle vocabulary, a terminality predicate
  consistent with it, and a deterministic bounded retry policy whose backoff is
  a pure function of the attempt count. These describe checked meaning only:
  no clock is read, no stage is executed, and the package gains no capability,
  effect or permit. Native and Wasm execution of these ordinary library
  functions still does not execute Agent stages.

- Add quoted-key validation and value cursors to `std.data.toml`, closing the
  key half of its recorded gap. `basic_quoted_key_end` admits the TOML
  basic-string escapes and rejects an unterminated string, a raw control byte
  and any other escape; `literal_quoted_key_end` admits `'...'` with no escapes;
  `key_end` dispatches the bare, basic-quoted and literal-quoted forms; and
  `value_start`, `value_content_end` and `value_end` bound a value quote-aware,
  so a `#` inside quotes does not open a comment and trailing spaces and tabs
  are excluded. Every scanner is an allocation-free offset computation and
  reports failure through the `byte_len + 1 + offset` sentinel the
  `std.data.json` family already uses for arbitrary-start scanners. Values stay
  uninterpreted bytes; tables, decoding, typed values and encoding remain
  Missing. Bare-key admission walks bytes directly instead of testing a growing
  `byte_range` sub-slice: that shape is admitted by the checker and executes on
  the interpreter and native C11, but emits a corrupt slice carrier on the Core
  Wasm lane, filed as issue #100 with a single-variable reproduction.

- Add `decoded_token_eq` to `std.data.json.dec`: two JSON string tokens in one
  input compared by their decoded bytes through the existing pull surface, with
  no buffer, so `"a\u0062"` and `"ab"` are equal keys and `"\n"` equals
  `"\u000a"`. Tokens of different decoded length are unequal without decoding
  either fully. Duplicate-key detection over a whole document still needs the
  object walk in `std.data.json.doc` and remains Missing.

- Add the bundled `std.encoding.base64` package,
  [Base64 v1](docs/BASE64-V1.md): pull-based padded standard Base64 encoding
  over a borrowed byte view. `len` gives the padded output length and `byte`
  gives its byte at one index, both computed from the input alone with no
  buffer, so a caller writes the digits into capacity it already owns. It is a
  sibling package because `std.encoding` sits on the default Project v1 route,
  which admits only Copy scalar boundaries and rejects a borrowed view.
  Decoding of padded input, streaming, and URL-safe or unpadded alphabets
  remain Missing.

- Add `wrapping_mul` to `std.num.overflow`, closing the wrapping-multiplication
  gap its own required scope recorded as Missing. The result is exact
  two's-complement wrapping for every operand pair, including `i64::MIN * -1`,
  `i64::MIN * i64::MIN` and `i64::MAX * 2`, and it never evaluates an
  overflowing intermediate under the language's checked arithmetic.

- Add a failure-mask discipline to `std.test`, so a nonzero test result names
  the failing check instead of merely being nonzero: `bit_for` is the bit an
  indexed case owns, `bit_is_set` reports membership, `record_failure`
  accumulates one case's verdict, and `first_failure` and `failure_count`
  report the lowest failing index and the number of failures. Indexes run 0 to
  62, `record_failure` refuses a bit another case already claimed, and the
  accumulated mask is monotonic, so two cases cannot silently share a bit and
  hide one another. The package's own conformance and examples modules now use
  the discipline they document.

- Add quote-aware field cursors to `std.data.csv`: `field_end` stops at the
  first comma outside quotes, `field_start` opens the next field,
  `field_is_quoted` reports the quoted form, and `content_start` and
  `content_end` bound a field's content excluding its surrounding quotes. A
  `""` inside a quoted field is one escaped quote that never ends the field and
  stays in the content bytes, so decoding remains the caller's step. Empty
  records, empty quoted fields and consecutive commas yield exact offsets, and
  a walk terminates because the last field's `field_start` is the record
  length. The offsets compose with the `std.io.lines` record content and the
  `std.bytes` trimming offsets, and the package conformance adds a `cursors`
  bit to its failure mask on the interpreter, native C11 `-O0`/`-O2` and Core
  Wasm. Typed fields, decoded content, dialects, streaming and writing remain
  Missing.

- Add explicit level filtering to `std.log` under the existing
  [Log Writer v1](docs/LOG-WRITER-V1.md) contract: `level_enabled` compares a
  level against a threshold on the existing 0-5 scale, `event_admitted` is the
  borrowed observer that is true only when the event both passes the threshold
  and fits the writer's live capacity, `discard_event` is the named drop path
  that consumes the event and returns the Writer untouched, and
  `append_event_if` writes a passing event exactly as `append_event` does.
  Capacity is required only for an event that is actually written, so a small
  buffer with a high threshold is a valid composition rather than a contract
  failure, and a filtered event releases its name and message bytes through
  ordinary lexical cleanup instead of being buffered. Three named cases join
  the logger corpus as individual bounded projects on the interpreter, native
  C11 `-O0`/`-O2` and repeated Core Wasm. No sink, queue, timestamp source,
  redaction or concurrency is added.

- Add span cursors to `std.bytes`: `is_space`, `trim_start`, `trim_end` and
  `is_blank` for ASCII whitespace, and `field_end`, `field_start` and
  `field_count` for delimiter-separated fields. Field walking preserves empty
  fields, so `a,,b` is three fields and a trailing delimiter opens one final
  empty field, and every operation is a borrowed-view offset computation with
  no allocation and no copy. The offsets compose directly with the
  `std.io.lines` line content, so a bounded caller can walk a delimited record
  and trim each field without a buffer. Whitespace is exactly space, tab,
  carriage return and line feed; no Unicode whitespace class, quoting or
  escaping policy is implied.

- Add field padding to `std.format` under the existing
  [Format Writer v1](docs/FORMAT-WRITER-V1.md) contract: `pad_len` is the field
  width actually written (never narrower than the content), `append_fill`
  writes one repeated byte, `append_str_left` writes left-aligned text and
  `append_usize_right` right-aligned decimals. Each padded operation
  preflights the whole field rather than only its content, so a buffer that
  could hold the content but not its padding fails before any byte is written,
  and content longer than the field is written in full rather than truncated.
  Five named cases join the existing corpus as individual bounded projects on
  the interpreter, native C11 `-O0`/`-O2` and repeated Core Wasm, and four
  further short or forged-output cases reject padded writes with the exact
  `requires`-false status. Alignment is byte alignment and the fill byte
  carries no character or locale policy; general format strings, arbitrary
  alignment modes, grouping separators and floating-point rendering remain
  Missing.

- Add the bundled `std.path.normalize` package,
  [Path Normalization v1](docs/PATH-NORMALIZATION-V1.md): lexical normalization
  of the typed `Path` values `std.path.value` owns. `normalized_len` and
  `normalized_byte` give the exact normalized length and each byte of a
  borrowed view with no buffer, and `into(borrow Path, own Bytes) -> Path`
  writes the normalized form into caller-supplied capacity after an exact
  preflight while preserving the borrowed input. A separator run collapses, `.`
  vanishes, `..` cancels the nearest retained segment, an uncancelled `..` is
  kept for a relative path and dropped at an absolute root, a trailing
  separator is removed, and an empty result is `.` or `/`, so a normalized
  Path is never zero bytes. Retention is decided without a stack, as the
  clamped maximum prefix sum of a forward walk that scores `..` as `+1` and an
  ordinary segment as `-1`. Ten named cases run as individual bounded projects
  on the interpreter, native C11 `-O0`/`-O2` and repeated Core Wasm, with
  hostile cases rejecting a short buffer, a forged Path and out-of-range
  offsets, and a graph check replaying the projection and its per-shape
  cleanup-schema selection. No public export, descriptor, platform conversion
  or filesystem authority is added, and `std.path.value` stays byte-identical.

- Add the bundled `std.io.lines` package, [IO Lines v1](docs/IO-LINES-V1.md):
  bounded line processing over the unchanged `std.io` Reader and Writer
  cursors. `line_end`, `line_terminated` and
  `line_content_len` observe a borrowed byte view; `reader_line_len` and
  `reader_line_complete` observe a borrowed Reader; `reader_line_into` copies
  one line's content into caller-supplied Writer capacity after an exact
  preflight; `reader_next_line` is the consuming transition past the line and
  its terminator. One line feed terminates a line, whose content excludes that
  byte and one immediately preceding carriage return, so LF and CRLF inputs
  yield identical content and a bare carriage return stays content. Eight named
  cases run as individual bounded projects on the interpreter, native C11
  `-O0`/`-O2` and repeated Core Wasm, and nine hostile cases reject short
  capacity, forged cursors and out-of-range view offsets with the exact
  `requires`-false status. A graph check pins the selected cleanup schema per
  shape: v5 for the copy and the record observers, v2 for the pure view
  helpers, with the caller's Writer as the copy's one owned parameter. No public export, descriptor, stream, standard
  stream or host authority is added, and existing cursor signatures, contracts
  and identities are unchanged.

- Add the private owned iterator payload profile for `Iter<Bytes>` and
  `IterStep<Bytes>`. Prelude v8, Graph v45, and CleanupPlan v13 bind the exact
  detached-prefix item/rest transfer and preserve prior scalar iterator,
  vector, and cache bytes. Focused library, interpreter/native C11 `-O0`/`-O2`,
  workspace, and Core Wasm checks pass, including graph/Prelude and cache
  replay, exact failure settlement, and malformed-provider output rejection.
  Hosted promotion, public generic iterator ABI, and broader iterator scope
  remain pending. Also close the omitted Graph v44 process-profile rejection
  in frozen evidence admission without changing serialized graph bytes.

- Extend `std.test.bytes` with reusable named snapshot fixtures and Copy
  comparison results containing equality, lengths, and the relative first
  difference. Borrowed fixtures and Reader cursors remain unchanged, with no
  new allocation or filesystem authority. Focused source-package and bundled
  consumer cases pass locally across the interpreter, C11 O0/O2, and repeated
  Core Wasm, including invalid-cursor cleanup.

- Add authenticated Project-linked State migration and workspace typed binding.
  A single declared or explicitly imported migration function extends the
  checked role closure; additive migration root v3 binds its exact source
  association while legacy root v1/v2 bytes remain unchanged. Durable recovery
  reconstructs the closure without evaluating migration again, preserves
  cumulative usage, and skips destination initialization. Focused local linked
  migration, selection refusal, recovery, and workspace currentness cases pass.

- Add the private `std.agent` source package and the linked Project lifecycle
  boundary. Task, Context, Observation, and Outcome records plus checked
  lifecycle and outcome helpers compose as ordinary source data; the linked
  path derives the Proposal schema from the retained Project closure and binds
  it to the additive typed iterative runtime while preserving the existing
  scalar/public package bytes and direct Runtime v2 binder. All six linked-role
  cases pass locally, and the four standard-library testing cases pass across
  interpreter, native C11 `-O0`/`-O2`, and Core Wasm, including epoch boundary
  cases and the byte-assertion regression. Linked migration, native/Wasm
  Agent-stage execution, live providers, hosted support, and full `std.agent`
  completion remain open.

- Add the sibling private `std.test.bytes` assertion package while preserving
  the existing scalar `std.test` facade and public descriptor. Exact slice and
  Reader-suffix comparisons, cursor validation, unchanged Reader positions, and
  failure-bit wrappers use the private `useful-data.v2` profile with no exports
  and exact `std.io` plus `std.test` dependencies. Execution and invalid-cursor
  gates pass locally; richer fixtures, property tests, fuzzing, and snapshots
  remain open.

- Record the private `std.process` bounded process slice. Example, conformance,
  and bundled-consumer commands pass locally on the interpreter, native C11
  `-O0`/`-O2`, and Core Wasm. Five focused physical Darwin provider cases also
  pass, covering the registered local provider path. Linux physical-provider,
  hosted, public, and broader process support remain open; this does not promote
  the full process profile.

- Record the nested record-match entry/result phase ownership fix. The named
  `nested_record_match_entry_and_result_phases_settle_across_engines` gate now
  passes interpreter, native, and Core Wasm success and postcondition-failure
  cases while preserving canonical cleanup vectors.

- Add the private `std.env` environment snapshot package and Project
  `environment-io.v1` composition. The bounded operation vocabulary, immutable
  UTF-8 snapshot constructor, capability checks, graph facts, package metadata,
  catalogs, and source links are in place. Four focused carrier cases cover
  empty, success, failure, and malformed Wasm paths; fourteen selected library
  checks cover the snapshot and environment admission surfaces, including the
  provider constructor on Node. The named gates
  `environment_manifest_is_canonical_and_authority_is_closed` and
  `environment_package_executes_all_functions_with_injected_snapshot` pass:
  the two Project commands pass on the
  interpreter, native C11, and repeated Core Wasm, including the bundled
  consumer; five focused useful-data environment checks also pass. The package
  is local/private evidence only, with no hosted CI or public ABI claim.

- Add the private `std.log` structured JSON-lines Writer. Its owned `Event`
  carries level, sequence, name, and message fields; append preflights level,
  UTF-8, and complete caller-owned Writer capacity before emitting one exact
  line. It composes bundled JSON UTF-8, JSON writing, and IO helpers without
  hidden allocation or ambient effects. The canonical package and fifteen
  expanded fixtures pass on interpreter, C11 `-O0`/`-O2`, and repeated Core
  Wasm with exact zero-to-three live Bytes bounds. Nine invalid-event/output
  cases pass twice with exact contract status. A direct ASCII validation path
  keeps the 300-byte message within unchanged interpreter fuel; the existing
  UTF-8 package conformance passes on all three backends. Catalog, metadata,
  formatting, links, and module-size checks pass. Broader Everyday logging
  remains Partial; no public ABI or hosted support is promoted.

- Add the private `std.format` Writer append slice: `append_str`,
  `append_i64`, `append_usize`, and `append_bool` preflight exact caller-owned
  `std.io.Writer` capacity and return the advanced writer. Checked digit,
  decimal-byte, and length helpers supply deterministic output without hidden
  allocation or effects under the private `useful-data.v2` profile; the
  package has no public exports. Local verification now passes seven
  owned-function-import unit tests,
  including borrowed-`str` and ordinary owned-byte-record positives plus
  non-byte-record refusal; eight individually runnable named SPX tests pass on
  the interpreter, native C11 `-O0`/`-O2`, and repeated Core Wasm with a strict
  two-entry byte arena (the pure helper case uses zero allocation). Five
  short/forged-output preflight cases pass twice with exact `requires`-false
  status through the bundled `std.format` consumer and transitive `std.io`;
  metadata and catalog regeneration pass. Named tests run individually
  because the per-function static allocation limit is unchanged. General
  format strings and floating-point rendering remain outside this slice, with
  no hosted or production claim.

- Add allocation-free JSON cursor adapters: `decode_into` and `quoted_into`
  consume an owned `Writer` after exact capacity preflight while borrowing a
  `Reader`, and `count_into` renders a `usize` value into that writer. The
  additive Project v16 `useful-data.v2` profile keeps private owned cursor
  composition separate from the frozen Useful Data v1 byte-export boundary.
  Legacy byte-only consumers retain their public boundary while unused newer
  dependency members stay outside the linked program.
  The standalone decoder/writer corpus passes on the interpreter, native C11
  `-O0`/`-O2`, and repeated Core Wasm, including a 300-byte decoded input; six
  malformed-input, insufficient-capacity, and forged-cursor contract-rejection
  cases pass. The named Project v16 gate
  `profile_admission::project_v16_json_cursor_public_facade_replays_and_executes`
  passes deterministic npm reconstruction, replay, and Node execution.
  The named cross-package gate
  `private_json_cursor_roundtrip_executes_across_project_backends` passes the
  local interpreter entry and repeated test, native C11 `-O0`/`-O2`, and
  repeated Core Wasm with a strict two-entry arena under the unchanged 16 MiB
  budget. These are local observations and do not claim hosted or public
  support.

- Refine workspace build-memory prebounds using bounded dependency identities
  and proven AST storage facts. Earlier successful budget receipts and fixed
  limits remain unchanged; a refused production build may retry once with the
  tighter bound after discarding its partial core and staged cache entries.
  Explicit smaller limits and nested budgets keep their original single-attempt
  behavior. JSON conformance scratch directories now remain distinct when
  decoder and writer checks run concurrently.

- Extend private `std.fs` with typed metadata, canonical immediate directory
  listing, directory creation/removal, and atomic file replacement through
  explicit providers. Project v15 and Graph v42 preserve the v1 profiles.
  Typed commands run on the interpreter, C11 O0/O2 and Core Wasm; malformed
  directory results fail before owned publication. Unix providers retain a
  directory descriptor and use same-parent rename for atomic replacement,
  without a durability claim. Private owned-input calls can return checked
  Copy-only records such as FileInfo; public ABI boundaries stay unchanged.

- Add bounded filesystem reads and create-new writes through explicit providers,
  with source-authored `std.fs` composition of Path, Reader and Writer, bundled
  dependencies, Graph v41 replay, and the private Project v14 execution profile.
  Interpreter, C11 O0/O2 and Core Wasm cover typed operations, byte/operation
  limits, failure priority, repeated calls and owned-result cleanup. The Unix
  provider retains a directory descriptor and rejects symlink traversal; writes
  never overwrite and do not promise rollback of physical effects. Broader
  filesystem facilities and cross-platform promotion remain open.

- Add the source-authored `std.path.value` owned Path library with checked
  logical prefixes, lexical queries, consuming parent traversal and joins into
  caller-supplied buffers. Bundled dependency composition preserves the original
  `std.path` byte helpers and adds no filesystem authority or public nominal ABI.
  Shared-loan replay now authenticates completion of synchronous borrowed calls
  used directly as contract roots, preserving previously accepted plan bytes.

- Add source-authored `std.io` Reader/Writer cursors over caller-owned Bytes,
  checked bounds, consuming transitions and bundled dependency use. Internal
  Project calls now compose explicit owned-record signatures, and empty-export
  owned-data libraries run in both manifest layouts without a public descriptor.
  Independent cleanup replay and interpreter/native/Wasm lanes retain result
  transfers before arm cleanup. Focused tests cover binary roundtrips, contract
  failures, borrowed-owner escape, forged identities and missing transfers;
  generated catalogs now include the record declarations.

- Preserve both immutable workspace generations through typed Agent migration,
  durable recovery and chained migration using additive provenance receipts.
  Recovery rechecks compiler-owned bindings and exact receipts; current-run
  paths refuse stale destinations before host or store work. Focused local
  tests cover recovered A→B→C chains, forged receipts, stale destinations and
  preserved terminal failures after checkpoint acknowledgement loss.

- Bind acyclic, iterative, and typed Agent runtimes to exact immutable semantic
  service generations through additive workspace execution receipts. Replaying
  a receipt reselects compiler-owned state; current execution rejects drift
  before host calls while historical bindings retain their original generation.
  Join only actual producer evidence and preserve typed durable checkpoint
  replay. Twelve focused execution-root tests pass locally, including the new
  V1/V2/V3, forgery, refresh, and zero-host replay cases.
- Complete private native-builder cleanup test visitors for the additive
  iterator renewal transitions; the private test target compiles locally.

- Add private one/two-parameter generic iterator operations with ordered Copy
  substitutions, explicit argument permutation, and authored map/filter/fold.
  Conditional same-owner Vec updates inside consuming loops use additive
  CleanupPlan v12 reservation/renewal facts and Graph v40; ordinary loops keep
  v11/v39 semantics. Replay rejects missing renewal facts and schema downgrades.
  Native layout discovery now includes retained concrete function bodies.
  Focused local runtime and projection evidence is recorded in
  [Generic Iterator Operations v1](docs/GENERIC-ITERATOR-OPERATIONS-V1.md);
  public generic ABI and hosted promotion remain separate.

- Separate cross-platform Rust build validation from focused runtime evidence
  in CI, retaining both as release blockers. Run the complete MSRV check once
  across its four test shards and remove two identical generic-lane test
  repetitions. Cache the pinned mdBook tool and upload the book only for Pages
  deployment. Hosted timing and platform validation remain pending.

- Add private consuming `for own` traversal over scalar iterators, including
  generic callbacks and same-owner vector accumulation. The hidden Step
  protocol preserves exact loop ownership with additive CleanupPlan v11 and
  Graph v39. Fix native conditional owner materialization and Wasm borrowed
  remainder/aliased-move handling. Focused interpreter, C11 O0/O2, Core-Wasm,
  graph and ProgramRoot replay checks pass locally; hosted promotion is pending.

- Compose private generic iterator helpers with scalar callbacks and step
  reconstruction. Source and HIR retain scoped `Iter<T>`/`IterStep<T>` ownership;
  existing Prelude v7, CleanupPlan v10, and Graph v38 remain authoritative.
  The eight-scalar runtime corpus passes interpreter, C11 O0/O2, and Core Wasm,
  including callback contract failure and repeated settlement. Graph and
  ProgramRoot reject forged instance/scoped identities and changed source.
  Public iterator ABI and consuming loops remain separate work.
- Repair the prior head's CI failures by consolidating Closure test visitors,
  keeping production iterator/prelude helpers before test modules, and
  completing closure/iterator documentation metadata and catalog entries.

- Repair local-only iterator step construction across prelude selection, graph
  classification, cleanup case-state replay, backend runtime activation, and
  canonical owning matches. Add repeated cross-engine regressions for bound
  and direct `Done` constructors; preserve legacy graph and prelude selection.
  Retain complete iterator declaration and cleanup facts in both workspace
  linkers, restore Box's frozen prelude slice, and verify ProgramRoot replay.

- Add the private Owning Iterators v1 implementation tranche for scalar
  `Iter<T>`/`IterStep<T>` and consuming `vec_into_iter`/`iter_next`, with
  Prelude v7, CleanupPlan v10, Graph v38, and ProgramRoot binding. Local
  interpreter and C11 O0/O2 observations cover all eight scalar types, order,
  empty/exhaustion, early drop, `Done`/`Yield` reconstruction, contracts,
  private returns, and forged native cursors. The same corpus passes Core Wasm with exact scope settlement; hosted promotion and the
  broader iterator, owning-payload, lazy-adapter, and public-ABI work remain
  pending.

- Repair Closure exhaustiveness in projection and semantic test traversals and
  native-builder mutable AST traversal, plus narrowly mechanical `-D warnings`
  hygiene exposed by the cancelled CI run. Hosted revalidation remains pending.

- Add scalar closure construction inside generic collection functions and bounded
  loops, with concrete instance identity remapping, independent scoped HIR
  validation, and source-only template body facts in Graph v37/ProgramRoot.
  Preserve per-iteration snapshots and collection settlement across interpreter,
  native C11, and Core Wasm; extend native Rust builder accounting while keeping
  its public callable boundary closed.

- Add private scalar snapshot Closures v1: exact AST/HIR cache carriers,
  Graph v37 and SemanticProgram v5/ProgramRoot replay, and local interpreter,
  C11 O0/O2, and Core-Wasm evidence for snapshot timing and captured generic
  Vec map/filter/fold composition. Owning captures, public callable ABI, and
  hosted promotion remain pending.

- Add private noncapturing Function Values v1 with checked declaration-identity
  references and indirect invocation, Graph v36 projection, and retained
  SemanticProgram v3 callable closures. Public ABI and hosted promotion remain
  outside this additive profile.

- Add the private generic-collection callback profile for Function Values v2;
  focused collection execution and ownership checks remain pending.

- Extend argument inference through nested omitted calls and generic callers, retaining scoped symbolic forwarding identities, independent bounded evidence, exact graph/root replay and ordinary evaluation-once ownership settlement.

- Extend private generic argument inference to complete ordered vectors and bounded expression type evidence, with independent source/HIR derivation and unchanged concrete instance, ownership and cleanup admission.

- Add durable Agent migration handoffs, trusted-store destination recovery and repeated revision chains with cumulative call, byte, stage and fuel accounting; preserve frozen operation checkpoint v2 bytes and bind additive migrated evidence to the handoff.

- Add private `Vec<Bytes>` push, replacement, reserve, clear and lexical cleanup across checked source/HIR, interpreter, C11 and Core Wasm. Mutations stage vector and payload owners together and fail before transfer; successful replacement drops the old payload once. Prelude v6 and explicit v2 host imports bind the new meaning, including graph and ProgramRoot replay. Scalar storage and prior prelude contracts remain frozen; focused local evidence is separate from hosted and public promotion.

- Add private owned `Box<Bytes>` allocation and consuming extraction across source/HIR, interpreter, native C11 and Core Wasm. Prelude v5 binds the additive contract; v2 Wasm imports prevent a legacy scalar host from silently leaking the payload. Allocation refusal keeps the staged Bytes owner live until ordinary cleanup, independently replayed before lowering. Focused local probes cover success, contract failure, allocation refusal, repeated settlement and frozen scalar compatibility; hosted and public promotion remain pending.

- Added exact argument-directed generic inference at monomorphic call sites,
  preserving explicit concrete HIR instances and ownership transfer boundaries.
  Seven language checks, private ProgramRoot replay and all-eight-scalar runtime
  success/failure settlement pass locally.
- Added consuming State migration from actual durable Suspend evidence into a
  differently rooted retained Agent, through a pure checked function replayed
  twice. Destination execution skips initialize, binds fresh authorizations and
  retains prior call, byte, stage and fuel charges, including failed migration
  fuel reservations. Focused unit and joined-runtime checks pass locally.
- Fixed YAML interpretation of unquoted Rust test-prefix selectors in CI run
  steps; GitHub had rejected the workflow before creating jobs. Added focused
  inference and migration selectors. Hosted execution remains unobserved.

- Added trusted-store checkpoints around each typed iterative effect, with
  persisted stage fuel reservations, intent/observation/transition generations,
  exact call and byte accounting, fresh-authorized recovery, and fail-closed
  uncertain delivery. Terminal failure survives a lost final store acknowledgement.
  Four execution and eight hostile decoder checks pass locally.

- Added private authored generic variants with one owned Bytes case and all
  eight Copy substitutions, including owning match/branch/call composition.
  Native selected-case destructuring now precedes arm construction, with
  outgoing ownership transferred only after the result exists. The 18-profile
  runtime corpus passes on interpreter, native O0/O2 and Core Wasm.
  Exact private variant/collection signatures now survive HIR linking; attempted
  public owning results reach the unchanged Scalar Export Profile and reject
  with its SPX-W115 diagnostic instead of the earlier private-linker SPX-H006.

- Added a deployed typed scalar operation registry with exact argument/result
  contracts, per-turn authorization, and call/byte ceilings. Direct Runtime v2
  compiles retained Agent source into this iterative product and joins its actual
  execution to deployment, instance and evidence roots. Five registry checks and
  the three-turn/two-operation Runtime integration pass locally.

- Added private generic Box/Vec functions over all eight Copy scalars, with
  exact source/HIR materialization, graph and ProgramRoot replay, and runtime
  success/failure settlement on interpreter, native O0/O2 and Core Wasm.
  Native intrinsic arguments now stage through the canonical transfer boundary;
  owned collection parameters reference their live cleanup slots. Interpreter
  report replay recognizes only the finite existing Box/Vec status tables.
- Added bounded iterative Agent Step execution with fresh per-turn authorization,
  cancellation and budget ceilings, exact source-Agent binding, and immutable
  invocation-bound evidence. Joined roots associate retained ProgramRoot v1-v3,
  deployment, invocation and actual one-pass or iterative execution. Iterative
  execution obeys both deployed turn and call ceilings. Copy-only Observation
  results use a narrow retained-call extension. Focused local checks pass;
  typed multi-effect checkpoints and migration remain follow-on work.
- Made the generated Proposal-client execution gate portable across Linux,
  macOS and Windows with exact UTF-8/LF output and provisioned TypeScript JS
  execution through Node. All three clients compile and execute locally on
  macOS; the added blocking CI matrix awaits exact-head hosted evidence.

- Added structural nested generic record composition and multiple owning
  parameters, with explicit reconstruction into different nominal result types.
  Source and HIR validate substituted fields, recursive patterns and complete
  ownership; nested update construction and replay select cleanup v9 after
  concrete substitution. Native owning branches now emit their canonical join
  and staged-call transfers. Focused all-eight-scalar runtime corpora pass on
  interpreter, C11 O0/O2 and Core Wasm, including both branches, nested owner
  replacement, contract and second-argument failure, repeated execution,
  allocation settlement and no additional aggregate memory.copy.

- Added explicit generic argument permutations, repetition and concrete
  substitutions with independent source/HIR proof, cycle rejection and the
  existing 256-instance closure bound. Additive Graph v35 binds symbolic
  caller-parameter/concrete mappings by authenticated expression paths;
  identity-only programs retain v34 bytes. ProgramRoot and bounded context
  expose the same mappings. Focused source, hostile graph, workspace replay
  and interpreter/native O0/O2/Core-Wasm checks pass.

- Extended generic Result propagation to `Result<T, Bytes>` for all eight Copy
  success types. Cleanup consumes the conditional owner without inventing a
  success cleanup slot; interpreter, native and Wasm preserve scalar Ok values
  and owned Err settlement. Forty focused cross-engine success/failure profiles
  pass, with forged empty-case flags and missing residual transitions rejected.

- Added generic `Result<Bytes, E>` relay, explicit forwarding and postfix `?`
  for every Copy error scalar plus `Bytes`. Independent HIR proofs validate
  unused substitutions without requiring a discovered call instance. Existing
  conditional cleanup handles empty scalar-error ownership paths; native
  residual return now preserves those scalar values before publishing the tag.
  Graph v34 and ProgramRoot bind concrete variant and residual facts across
  private function boundaries. Focused local runtime evidence covers 45
  success/failure profiles on interpreter, C O0/O2 and Core-Wasm; public generic
  descriptors remain closed. This begins GEN-06, not the end of the full goal.

- Completed Graph-v34 type facts for concrete generic instance signatures and
  bodies, including template-only context selection. Frozen legacy graph
  collection remains unchanged. Added focused hosted compatibility selectors
  for existing Component byte known answers and closed public mappings.

- Added Graph v34 concrete generic-instance ownership, revision-bound semantic
  identities, forwarding facts and exact source replay, with independently
  selected cleanup schemas preserving existing CleanupPlan bytes. An additive
  SemanticProgram v2 node binds linked generic closures into ProgramRoot;
  frozen graph consumers and public ABI descriptors retain their prior
  contracts. The independent `GEN-05B generic instance semantic closure` Linux
  job combines the eight-scalar flat and
  nested corpus, graph/schema hostility, expression composition and scalar
  cross-package execution. Focused graph and workspace replay checks pass;
  the bounded GEN-05B/GEN-05C Linux tranche passed for implementation commit
  `c27d06f0cf74749804237a43cc71c248b319cfe0` in
  [CI run 34058787739, job 101555489228](https://github.com/wavect/semaprax/actions/runs/34058787739/job/101555489228).
  This documentation-only successor records that implementation result and
  does not claim a new test run, full-CI passage, or a public generic ABI.
  Semantic instance identities survive
  comment-only edits while ProgramRoot still binds exact source. Preserved
  existing workspace known answers and ownership/range diagnostics.

- Added a source-native Agent-to-Lifecycle v1 bridge and executable generated
  Proposal-client evidence. One checked `.spx` Agent is selected by stable
  identity, lowered through the frozen AgentDefinition-v1 compiler, and bound
  to the existing one-pass lifecycle; replay now requires both exact lifecycle
  bytes and the semantic source revision, so role-body drift fails closed.
  Focused regressions cover completion, refusal, injected-effect failure,
  missing-Agent/incompatible-role cases, stale source, and fail-first oversized or
  malformed replay selectors. A named Linux step also materializes generated
  record and variant clients in isolated temporary projects, strict-compiles
  TypeScript 5.8.3, byte-compiles Python, builds Rust offline with a private
  target, executes all three, and submits their exact integer/UTF-8/case output
  through the canonical decoder. This changes no frozen Agent or Runtime wire
  and adds no provider, tool, filesystem, publication, or ambient authority.

- Extended Exact Program Context v2 with candidate-safe ProgramRoot-v3 refresh.
  A host-authenticated successor context is independently replayed against a
  separately compiler-admitted candidate Project; successor external facts are
  freshly supplied and replayed rather than implicitly copied from the current
  generation. The persistent service selects the active
  workspace/v3 root first, stages the complete candidate generation, cache,
  indexes, unchanged refresh-v1 receipt, and history entry, then adopts them
  together. Stale selectors, cross-paired facts, invalid source, or replay
  failure preserve the active generation and history; old snapshots remain
  exact. This adds no wire, filesystem acquisition, execution, commit, or
  publication authority.

## 0.4.0 — 2026-09-06

- Added Universal Semantic Transaction v2 for one exact, authority-free
  `ReplaceExpression` over an authenticated revision-scoped body-expression
  identity, including explicit monomorphic `main`. Validation rebuilds the
  complete Project Candidate, preserves exact source bytes outside the selected
  span, and emits deterministic separately versioned result/evidence. The
  persistent service adds ordinary and ProgramRoot-v2/v3 validation/replay,
  with exact selection before parsing/history and no replay history append;
  `change preview ... replace-expression` returns exact core output or the
  Candidate structural diff without writes. V1 transaction and CLI bytes are
  unchanged; contract/implicit/generic/synthetic/imported editing, composition,
  commit, publication, and authority remain unavailable.

- Added the product/package contract for Owned Bounded Box v1 and the
  alloc-tier `std.mem` package. Compiler-owned `Box<T>` is limited to the eight
  explicit Copy scalars with `new`, synchronous `get`, consuming `into_inner`,
  one unique non-Copy owner, a 4,096-live-allocation bound, and sticky
  allocation refusal. Additive prelude v4 is Box-selected while v1-v3 and
  authored inline `record Box<T>` programs remain frozen. `std.mem` contains
  exactly three authenticated aliases, explicit 3-by-8 conformance,
  scalar-only example/test results, and no public exports. Focused local
  package, catalog, interpreter, native C11, Core-Wasm, cleanup-replay, and
  hostile-carrier evidence passes; contract-failure cleanup, owned payloads,
  allocator interfaces, public generic ABI, regions, arenas, ARC/shared ownership,
  Iterator integration, hosted evidence, and production support remain open.

- Specified Owned Bounded Vec For Traversal v1: the source form
  `for item in values { body }` accepts one simple immutable `Vec<T>` binding
  over the existing eight Copy scalars, snapshots its length once, visits
  indices in ascending order, freezes the source, and discards each body
  result. Resolver lowering reuses the existing len/get/while HIR, so this adds
  no stable identity, schema, prelude or backend operation, standard-library
  declaration, or public ABI. Focused local language and all-engine runtime
  selectors pass; hosted evidence remains required before promotion. The
  lowering has no origin marker, so ordinary HIR-node validation applies
  instead of a traversal-specific canonical-shape rule. This is not Iterator
  support: objects, `next`,
  adapters, closures, associated types, lifetime inference, consuming
  traversal, owned elements, and `std.iter` remain open.

- Added bounded acyclic generic-to-generic forwarding between already-admitted
  templates. A direct call must pass the callee exactly the caller-owned type-
  parameter vector in declaration order; each concrete caller instance derives
  the deterministic transitive callee-instance closure, bounded at 256 entries. The existing
  direct-scalar and one-owner-identical-result relay profiles, Graph v14, and
  CleanupPlan v2/v5/v7 remain the limits. Focused evidence covers
  chained source/HIR identities in authored FIFO order,
  interpreter/native C11 `-O0`/`-O2`/Core-Wasm settlement, concrete
  non-identity/permutation/cycle rejection, and missing/reordered/forged HIR
  instance rejection.
  This adds no inference, constraints, construction, projection, variants,
  resources, effects, package signature, or public generic ABI.

- Added the exact flat generic owned-record expression-composition tranche.
  One owning parameter returns the identical record while all eight explicit
  Copy substitutions exercise Copy-field projection, top-level immutable
  update, `match borrow` returning a bound Copy field, and `match own`
  reconstructing the same owner. Focused local evidence covers update and
  reconstruction failure settlement, hostile HIR/backend mutation replay,
  repeated interpreter, native C11 `-O0`/`-O2`, and Core-Wasm execution, and
  no added aggregate `memory.copy` against the direct-relay baseline. Generic
  variants, nested expression-result composition, standalone constructors,
  consuming projections, public generic ABI, and Graph/schema widening remain
  closed or unclaimed.

- Added a dedicated Linux CI step for the additive nested generic-owned relay
  and identity-forwarding tranche. It names the exact source/HIR boundary,
  transitive-instance hostility, all-engine settlement, and scalar-only Project
  dependency selectors, and removes their duplicate invocations from the
  adjacent generic-owned step. New locally passing hostility regressions cover
  all eight Copy scalars through a three-template nested relay, forged HIR
  carrier and cleanup vectors, source vector changes and cycles, and the exact
  256/+1 instance-closure bound. The named step is hosted green in CI run
  34048713967, Ubuntu job 101528399406; the older run 34031917437 remains the
  evidence for the pre-nested-relay corpus.

- Re-derived the offline doctor carrier ceiling from measured distributions.
  `DOCTOR_OFFLINE_INPUT_MAX_BYTES` was 536,870,912 bytes, and on a hosted
  `ubuntu-24.04` runner the loader closures of Node v22.23.2 and Rust 1.88.0
  alone encode to 462,424,370 of them, 86% of the ceiling, leaving 74,446,542
  bytes for a whole Clang role. No official LLVM release that runs on 24.04 is
  that small: clang 9.0.1 reaches a 568,339,434-byte carrier, 14.0.0 reaches
  618,411,514 and 17.0.6 reaches 652,142,493, and Ubuntu's own clang-18 closure
  is about 713,000,000. Since `render_rows` admits only Node 22 or newer and
  Rust 1.88 or newer, the two non-Clang roles cannot shrink, so the two
  real-distribution lifecycle fixtures could not be satisfied by any current
  real distribution set. The ceiling is now 1,073,741,824 bytes: 1.65 times the
  measured clang-17 three-role carrier, 1.51 times Ubuntu's clang-18 closure,
  and still 6.25% of a hosted runner's 16 GB. It stays a hard bound and remains
  a resource bound rather than an authority boundary, since seals, digests, the
  release signature, the ELF contract and the closed inventory decide admission
  and none of them depend on size. The delegated cgroup-v2 scope's `memory.max`
  moves with it, from 2 GiB to 4 GiB, and is now derived rather than
  coincidental: an admitted carrier of N bytes costs 2N of unswappable
  residency inside that scope -- the worker's whole-carrier snapshot, which the
  root plan borrows and so cannot release before the tool children run, plus
  the page-rounded tmpfs root written out of it -- while `memory.swap.max` is 0
  and `memory.oom.group` is 1, so an overshoot kills the whole scope instead of
  refusing cleanly. The signed capsule's `MAX_ARTIFACT_BYTES`, the release
  directory's `MAX_ARTIFACT_BYTES` and the signed store's `MAX_FILE_BYTES` are
  held equal to the new ceiling because they bound the same bundle and request
  bytes and the smallest of them is always the effective limit. The derivation
  is recorded beside the constant and in Doctor Sealed Input v1.

- Fixed the offline doctor collector retaining its whole-carrier snapshot
  across the blocking collect. Every fact the collector keeps is already an
  owned copy by that point, and the launcher and provisioner both release
  theirs before creating a child, so the omission left three whole carriers
  resident in the confined scope at once instead of two.

- Fixed the provisioned Linux doctor gate failing before any lifecycle fixture
  ran, with `input permissions or link count are unsafe`. Cargo uplifts each
  binary out of `deps/` as a hard link, so the artifact in the target root has
  link count 2, and `semaprax-doctor-release` refuses an input a second name
  can still reach. The workflow now copies the four images to private
  single-link files and asserts the link count; the packager's check is
  unchanged. The gate also carries all three real roles again, which the
  re-derived ceiling admits.

- Added `std.data.json.dec`, the seventh JSON sibling package, which expands
  JSON string escapes. All eight simple escapes, `\uXXXX`, and surrogate pairs
  decode to their exact UTF-8 bytes; a lone or unpaired surrogate, an unknown
  escape letter, a raw control byte, and an unterminated string are rejected at
  the offset of the backslash or byte that opened them, in the family's
  `usize` result encoding. `decoded_len`/`decoded_size` size the output,
  `token_end`/`emit_len`/`emit_at` stream the decoded bytes with no allocation,
  and `decoded_eq` fills one owned bounded byte buffer of a fixed 256-byte
  capacity through the loop-carried same-owner replacement and compares it to a
  caller-supplied view. The conformance module runs on the interpreter, clang
  C11 `-O0`/`-O2`, and Core Wasm, where the standard-library closure now
  supplies the `env.spx_bytes_zeroed`/`env.spx_bytes_set` host-arena imports and
  asserts that the package's module reaches them, that the arena holds at most
  one live entry, and that it is empty after each of four re-entries. Three
  admission facts were measured rather than assumed and are recorded in the
  specifications: `owned-data-api.v1` is the only project profile that admits
  both a borrowed byte view and the buffer, the buffer must stay out of the
  entry program because a public web build containing it is `SPX-W115`, and a
  `Bytes` value does not cross a module boundary (`SPX-G172`), so a
  caller-provided output buffer is not expressible. The package is committed at
  17,620 B against a measured 18,480 B admitted / 18,653 B first `SPX-G171`;
  `decoded_at` and `decoded_same` were written, measured over that bound, and
  cut.

- Added the bounded ScalarV1 internal flat generic-owned body path. Admission is
  based on the exact reachable ResolvedProgram rather than source or dependency
  provenance; callable and selected-public signatures remain value-scalar. One
  exact project-local Subject-v3 dependency provides cross-package evidence by
  retaining the concrete generic owned-byte record composition internally while
  exposing exactly one no-argument `i64` function. Focused local evidence covers
  exact Report-v2/Subject-v3 resolution, linked-HIR identity and
  cleanup, Project check and repeated entry/test, native C11 `-O0`/`-O2`,
  Core-Wasm, the unchanged scalar Web build, `SPX-W115` public-selection escape
  and `SPX-J123` dependency tamper. This evidence is unhosted. Manifest, report,
  Project, public descriptor, Wasm and package-evidence schemas remain frozen;
  no generic record, owner or aggregate signature crosses a call, package or public
  boundary, and no acquisition, publication or production support follows.

- Extended the bounded concrete generic owned-record relay from its flat
  carrier to any acyclic authored-record template tree within the existing
  64-level, 256-owned-leaf, and 4,096-field work limits. Each template has one
  `own` parameter and an identical return type, requires explicit Copy-scalar
  arguments, and preserves exact template/instance identity and recursive
  ownership through source verification and independent HIR validation.
  Focused local evidence exercises `Box<Pair<Bytes, T>>` and
  `Pair<Box<Bytes>, T>` across all eight scalars in source/HIR, including
  representative source and forged-HIR rejection. The representative
  `bool`/`i64` runtime instances cover success plus
  requires/ensures/staged-call failure settlement on the interpreter, native
  C11 `-O0`/`-O2`, and Core-Wasm.
  Graph v14 and CleanupPlan v7 remain unchanged. The pre-nested-relay
  generic-owned corpus is hosted green in CI run 34031917437, Ubuntu job
  101482963175; this additive nested-relay evidence remains local until its own
  pushed run. Nested-nonflat multiple-owner or non-identical-return
  composition, Project/package/public ABI exposure, and production support
  remain closed; the legacy flat generic-function admission is unchanged.

- Admitted the loop-carried owned byte buffer fill. `bytes_set` gains one
  same-owner replacement shape, `buffer = bytes_set(buffer, index, value)`,
  whose assignment target and buffer operand are the same `let mut` binding:
  the call moves the single owner out and the assignment publishes the returned
  owner back, so exactly one generation is live at every point. It is the only
  `bytes_set` a bounded `while` body admits, so a loop can fill a buffer it did
  not allocate; `bytes_zeroed` stays outside the loop and is still rejected
  there by both the byte-family rule (`SPX-T252`) and the owned byte allocation
  rule (`SPX-T267`). Every other named buffer operand remains `SPX-T271`, a
  borrowed view live across the replacement remains `SPX-T265`, and independent
  HIR validation re-derives the same fact so hostile HIR that swaps two
  replacements' targets fails closed with `SPX-H006`. The fill and a store one
  element past the capacity execute on the reference interpreter, native C11,
  and Core Wasm under Node against a one-entry owned-byte arena across four
  re-entries with no linear-memory copy or growth, and the out-of-range store
  selects the identical `semaprax.byte-buffer.v1` code 1 status on all three
  engines before the owner transfer commits. Capacity growth stays out of
  scope: the allocation capacity is still a literal.

- Added the bounded AGENT-04 generated Proposal-to-Runtime v1 compatibility
  adapter. One exact checked Proposal record or Copy-scalar variant now passes
  the existing decoder before its complete canonical bytes, including the
  terminal LF, become the escaped message of the frozen Runtime v1 final
  action. The adapter performs no case or field translation, reaches no host,
  and cannot select a Runtime tool; `SPX-G578` rejects cross-pair and complete
  escaped-action-bound failures while Proposal `SPX-G550`/`SPX-G551` remain
  unchanged. AgentDefinition, AgentGraph, Proposal Schema, and every Runtime v1
  schema, API, digest and known answer remain frozen. Direct provider Proposal
  input, Runtime tool-action/schema generation, broader Proposal, public ABI,
  and Runtime v2 support remain open.

- Added authority-free Exact Program Context v2, which independently replays
  exact context v1, Contracts and Tests Facts v1, and ProgramRoot v3 before
  requiring the enriched workspace and v3-root selectors across typed query,
  transaction, service, and history paths. The same appended facts descriptor
  is retained in memory; query/result, transaction/evidence, history, service
  receipt, exact-context-v1, and ProgramRoot wires remain unchanged. Exact
  transaction history keeps the authenticated default base workspace identity,
  selector and replay failures append nothing, and exact service refresh plus
  candidate ProgramRoot v3 remain unavailable.

- Added the authority-free Contracts and Tests Facts v1 association and
  ProgramRoot v3. The standalone fact bundle binds one admitted Project,
  semantic Graph, and legacy workspace revision to stable-ID-sorted function
  and function-template rows with ordered checked `requires`/`ensures` facts,
  plus only the declared test `main` and ordinary executable named tests. Its
  closed document explicitly denies coverage, execution, result, and source
  authority claims. ProgramRoot v3 freshly retains all eleven ProgramRoot-v2
  descriptors and three unbound relationships, then appends only the fact
  schema/digest/byte-count descriptor. Canonical Semantic Workspace v1 and
  ProgramRoot v1/v2 identities and bytes remain unchanged. This is inventory
  and association only: contract proof, coverage, test execution, runtime
  roots and authority remain absent; the separate Exact Program Context v2
  entry above records the later typed selector integration.

- Extended the additive Exact Program Context v1 lifecycle through exact query
  replay, transaction replay, and persistent-service history selection. Every
  exact route requires both the enriched workspace revision and ProgramRoot-v2
  digest, retains the selected `ProgramRootV2` only on typed in-memory results,
  and fails closed on stale, reminted, cross-paired, or mutated inputs. Exact
  transaction history records the authenticated default Project-derived base
  workspace identity; read-only replay appends no entry. Universal Semantic
  Query v1, Universal Semantic Transaction v1, service-history v1, receipt,
  ProgramRoot v1/v2, canonical-workspace v1, and other legacy wire bytes remain
  unchanged. Candidate ProgramRoot-v2 derivation and exact refresh remain
  unavailable pending candidate-safe Project Lock replay.

- Corrected the cleanup-plan storage ownership rule so the Owned Bounded Vec
  v1 loop-carried fill is reachable from source, and added
  `examples/vector-stats-project`, the first example that accumulates a
  **variable** number of scalar values and filters them. A storage now belongs
  to the cleanup region that first introduced it. It was previously re-homed
  into whichever region asked for it second, so `values = vec_push<T>(values,
  value)` inside a bounded `while` moved the enclosing binding's slot into the
  loop body's own region: the body's scope exit finalized the vector on every
  iteration, one linearized body pass no longer preserved owned liveness, and
  every such program fail-closed with `SPX-H006` even though source
  verification, HIR validation and independent cleanup replay all admitted it.
  The example initializes one `Vec<i64>` outside a bounded loop whose body
  pushes a computed number of readings, then walks it back through `vec_len`
  and `vec_get` summing only the readings over a threshold. Its gate drives the
  accumulating function at seven element counts and three thresholds and runs
  both the entry and the conformance module on the interpreter, native C11 at
  `-O0` and `-O2`, and Core Wasm under Node against a one-generation host
  arena; the owned-data harness carries the same shape as a focused language
  regression.

- Admitted a computed `usize` element index for the Owned Bounded Byte Buffer
  v1 `bytes_set` store, so a value can be written at an offset a scan
  discovers. The allocation capacity is still one literal at the
  `bytes_zeroed` site (`SPX-T271`), and a literal index at or above that
  capacity, or any index into an empty buffer, stays `SPX-T272`; only what the
  compiler cannot decide moved to run time. `bytes_set` therefore became the
  first fallible compiler-owned byte operation: it carries an ordinary
  `PropagatedCall` status source whose failure is selected **before** the owner
  transfer commits, so an out-of-range store writes nothing, is not a backend
  accident, and leaves the buffer in the canonical call-argument slot that the
  exit's single `core.bytes.drop` finalizer already owns. Independent replay
  re-derives the same ordering. The reference interpreter, native C11 through
  the new `spx_bytes_set_check_v1`, and internal Core-Wasm through a generated
  comparison against the carrier's byte length all select the identical
  `semaprax.byte-buffer.v1` code 1 adapter status; the Web wrapper reserves
  internal value 16 for it. The deferred-owner-commit decision the bounded Vec
  lane introduced for `vec_push` now lives in one `cleanup_plan::deferred_commit`
  module that both operations share. The Core-Wasm host import keeps its own gate, now
  unreachable from admitted source. Local focused evidence covers a computed
  in-range fill and a computed out-of-range store on all three engines,
  including a one-entry Core-Wasm arena that still balances across four failing
  invocations. Elements wider than one byte, a loop-driven fill, a computed
  capacity, and a public FFI layout stay open and unclaimed.

- Restored the cross-platform CI gates after the owned algebra expansion by
  refreshing the checked Core-Wasm, component, artifact-DAG, browser project
  graph, and macOS provider symbol KATs; aligning Project admission regressions
  with the newly supported generic and byte-bearing shapes; and keeping the
  POSIX native network fixtures Unix-only. The TCP deadline regression now
  constrains the client send buffer so it cannot spuriously complete before the
  peer-side timeout is exercised, and newly enabled strict Clippy lints are
  clean.

- Executed the VS Code Extension Host execution evidence v2 gate for the first
  time, on exact subject `3fccd30b861d48c9d404eb2698fa2eff510569af` in a locally
  provisioned Visual Studio Code 1.136.1 on Darwin arm64 with Node v24.3.0. The
  97 standalone controller cases and the single Extension Host scenario passed,
  producing bundle
  `29660fc88dac4d3bd11098f7facfe1bd05fda23b378d519d9590f028a3fdc7dd`, whose
  envelope and four artifacts are archived under `docs/evidence/`. The run found
  two defects the mock-backed suites could not: `runCandidateTests` and
  `suggestHoleFill` awaited non-modal notifications that resolve only when a
  human dismisses the toast, hanging the command and holding the busy latch;
  and the host assertion for a cleared diagnostic expected `undefined` from
  `DiagnosticCollection.get`, which the real host never returns for an absent
  URI. Those commands now fire the notices without awaiting, and absence is
  observed through `has`. This is one local product on one platform; VSIX or
  Marketplace packaging, manual UI, accessibility, minimum-version, remote,
  hosted and cross-platform evidence remain open, and the run covers its exact
  subject rather than any later head.

- Added internal Owned Bounded Vec v1 across source, HIR, Graph, cleanup-plan
  replay, the interpreter, native C11, and Core Wasm. `Vec<T>` and eight explicit
  generic intrinsics admit exactly the eight Copy scalars, any `usize` capacity
  expression with a hard runtime maximum of 8192 and sticky code 3 on dynamic
  overflow/allocation failure, consuming push, deterministic exact reserve,
  indexed set, capacity-retaining clear, borrowed length/capacity/get, exact
  same-owner reopening, and deterministic settlement on all three engines.
  Reserve uses `max(old_capacity, len + additional)`, set reuses bounds status
  code 2, and all three new mutators consume and return the one owner. Additive
  compiler prelude v3 is selected only when one of those operations is used;
  frozen prelude v1/v2 contract bytes remain unchanged, and collision-free
  legacy Vec programs keep selecting their prior bindings. The three new
  intrinsic names and core identities are now reserved language vocabulary. The
  alloc-tier `std.collections` package now authors the exact eight
  authenticated aliases, an explicit eight-scalar conformance module, bundled
  dependency metadata, generated catalogs, focused local Project/package
  evidence, and no public exports. Owned/aggregate elements, inference,
  iterators, public generic ABI, hosted support, and broader collections remain
  closed.

- Admitted the exact internal Owned Bounded Byte Buffer v1 write-once profile
  on Core-Wasm. The frozen host-arena imports allocate a literal-bounded zeroed
  `Bytes` value and mutate the same opaque token at literal indices; focused
  local Node evidence covers deterministic valid modules, three writes and
  reads, repeated success and contract-failure settlement/re-entry at one live
  arena entry, and absence of `memory.copy` and `memory.grow`. Source/HIR and
  cleanup hostiles preserve exact capacity, callee, transfer and call-commit
  authority. The public byte adapter remains rejected with `SPX-W115`; this
  adds no loops, growth, wider elements, Project/public ABI, `std.*`, browser,
  hosted, or cross-platform support.

- Added one exact cross-file Project product gate for an internal concrete
  generic owned record. The retained Project keeps `Pair<Bytes, bool>` inside
  its linked closure while its frozen v8 descriptor remains scalar-only; the
  gate repeatedly executes Project entry and test functions, generated native
  C11 at `-O0`/`-O2`, and an external Node consumer calling the generated
  npm/Core-Wasm scalar API for empty and nonempty inputs. This does not expose
  a generic record, widen v8/v9/v11, or add a public generic ABI or Project v14.

- Added `agent_lifecycle::durable`, the Agent Checkpoint v1 revision-bound
  durable slice over Agent Lifecycle v1. `bind_durable_agent` anchors one
  checked module, one AgentDeployment v1 bound product and one caller-supplied
  policy epoch, and every checkpoint generation binds nine recomputed facts -
  the policy epoch, the semantic definition, deployment and bound-product
  digests, the State role identity, the proposal-grammar digest, the lifecycle
  digest, the exact module source digest and the caller's task digest - so
  definition, deployment, source or state-schema drift and a revoked epoch each
  reject a stale checkpoint by its own reason, re-running no stage and writing
  no generation. A durable run splits the lifecycle at its single external
  boundary: it commits an intent before crossing it and a settled observation
  after it, so a crash between the two leaves delivery uncertain. An uncertain
  operation is never retried automatically - it ends in a terminal `unknown`
  state, or the host reconciles it with a settled observation or an
  abandonment - and a read that reports failure is treated as uncertain too,
  because a reported failure is not evidence of non-occurrence. Neither budget
  ledger is refunded: the effect grant is consumed at the intent whatever the
  operation's fate, and re-executing the deterministic prefix on a resume spends
  interpreter fuel from the same remaining total.
- A resumed run cannot forge an authorization. A checkpoint carries no
  `Authorized`, no grant seal and no state carrier, only digests, and there is
  no decoder from checkpoint bytes back to a retained value; a resume recomputes
  the state from the caller's task and mints a fresh grant through the crate's
  single mint site before comparing the derived operation identity against the
  journal. Checkpoint bytes are self-verifying - closed key set, strictly
  advancing journal ranks, recomputed chain link, program counter agreeing with
  the journal, and exact canonical re-rendering - so a partially written
  generation fails closed with `SPX-G573` rather than being adopted, and
  atomicity itself stays the caller's `CheckpointStore` contract. Caller inputs
  are retained as digests only; the settled observation is the sole retained
  external datum, and `Retention::ObservationDigestOnly` redacts that too. The
  checkpoint is deliberately unauthenticated and republishes that dependence in
  its own nonclaims, alongside the standing provider-billing and
  external-exactly-once nonclaims. The `agent_runtime_v1` harness gains
  `agent_checkpoint_v1` covering crash injection at all five boundaries with
  their recoveries and total boundary-crossing counts, the drift and
  caller-input rejections, seven malformed-document rejections, the redaction
  sentinels, and an atomic write-and-rename store against a contract-violating
  torn one; the crate-internal gate additionally proves that an internally
  consistent rechained journal forgery still mints nothing. No CLI surface was
  added: `semaprax agent` still refuses `resume` and `reconcile`.

- Added `agent_lifecycle`, the Agent Lifecycle v1 compiler and runner: it binds
  an AgentDefinition's four deterministic operation identities - `initialize`,
  `observe`, `authorize`, `reduce` - to actual verified `.spx` functions in the
  same HIR ordinary execution uses, and executes one acyclic lifecycle over
  them through `interpreter::retained_call`. Binding validates each stage's
  parameter count, parameter ownership modes against the AgentGraph v1
  relationships, declared effects, role types, the derived two-case
  grant/refusal decision variant, and the acyclic and uniquely ordered stage
  graph, and it rejects an unresolved identity, an incompatible signature, an
  incorrect ownership mode, a declared effect on a deterministic stage, an
  unadmitted proposal field representation, and an unadmitted decision shape
  with an `SPX-G570` diagnostic naming the exact failing field - all before any
  host work is reachable. The authorizing transition mints `Authorized`, an
  opaque one-use value bound to the exact lifecycle policy digest, the
  identity-keyed encoding of the state carrier, the exact proposal document
  bytes, the grant case identity, and the seal the program itself constructed.
  Its fields are private to one module, it derives nothing and has no public
  constructor, it is consumed by move at the effect boundary, and the crate's
  single mint site is reachable only from the function that runs the validated
  authorize stage - which requires an `AuthorizeStage` only the stage binder
  builds, requires the retained product to name that exact validated function,
  and mints only on the validated grant case. A proposal, an observation and a
  reduction therefore have no route to one. Spending an authorization
  independently recomputes its binding from the state and proposal presented,
  so a substituted state or proposal is `SPX-G571` before the injected read
  operation is called. `propose` is a scripted offline document admitted only
  through Proposal Schema v1, and `execute` is one explicitly injected
  `AgentReadOperation` and nothing else. The six terminal conditions -
  `completed`, `rejected`, `model_failed`, `effect_failed`, `cancelled`,
  `budget_exhausted` - each produce a canonical
  `semaprax.agent-lifecycle-evidence.v1` document carrying identities, outcomes
  and cleanup-event counts but no payload bytes, byte-identical on replay.
  Stage values stay inside the retained seam's closed vocabulary, so `string`
  cannot cross a stage boundary and the Proposal role crosses as its exact
  ordered scalar projection instead. The frozen AgentDefinition, AgentGraph and
  Runtime v1 profile digests are re-asserted unchanged; `AgentDefinition` gains
  two additive read-only accessors and no other change. Iterative `AgentStep`
  execution, typed language effects beyond the injected read, durable
  checkpoint/resume, and a CLI surface remain Missing.

- Re-derived the `SPX-G171` identity term. The builder pre-bound charged
  every declaration identity slot the longest identity in scope times 64, a
  factor whose enumeration of retained resolver structures overlapped the
  occurrences the slot count already enumerates, so each slot was billed for
  the whole resolver twice. Measured with a counting global allocator around
  the core build, lengthening every identity in a package by one byte raises
  the heap that build retains by 0.87 to 1.11 bytes per identity slot
  (`std.test` 460 over 416 slots, `std.core` 1,378 over 1,385, `std.bytes`
  2,080 over 2,378, `std.data.json.token` 2,409 over 2,755), so the factor is
  now 16: eight retained structures, each able to hold the identity as a map
  key and as a value. The structural factor stays 24 against a measured 3.8,
  because the sixteen it is built from is a compile-time bundle assertion
  rather than an estimate, and the string factor stays 64 because that term is
  1.2% to 2.5% of the estimate. Padding the six `std.data.json.*` packages
  until `SPX-G171` fires now admits 19.7 KB to 22.1 KB of total package source
  where the same measurement gave 12.3 KB to 14.2 KB, so `std.data.json.doc`
  has headroom again. Nothing about the retained-memory bound is relaxed: the
  pre-bound and the structures the core build actually retains are reserved
  against the same 16 MiB budget and either overflow is still `SPX-G171`.
  Whole-document KATs that embed `used_builder_bytes` were re-pinned;
  rendering the same workspace document under both factors and diffing it
  field by field shows `used_builder_bytes` and the digest over it as the only
  changes. Standard Library v1 also records two limits that were written down
  nowhere: `match` is not admitted in a `while` body (`SPX-T252`) outside a
  two-arm `Option` match on `byte_get`, and replay path counts multiply across
  sequential statements while only summing across `match` arms, so packing
  small tables into one function can trip `SPX-H006` where splitting them
  does not.

- Added `interpreter::retained_call`, the retained multi-argument call seam
  Reference Interpreter v1 was missing: `prepare_resolved_zero_arg_i64` is
  zero-argument and additionally requires `entry_id == program.entrypoint`, so
  it can run only `main`; `interpret` re-reads and re-verifies source on every
  call; `ArgumentValue` is scalar-only; and `evaluate_resolved_owned_data` is
  restricted to a borrowed `Slice<u8>` entry with bytes-shaped results.
  `prepare_retained_call` admits ONE explicitly identified function - the
  entrypoint or any other - through the interpreter's own admitted-function map
  and closure scan, restricts its signature to a closed value vocabulary, and
  retains the authority-free dispatch index; `evaluate_retained_call` then
  invokes that product repeatedly, reading no source, re-resolving nothing, and
  re-running neither `hir::validate` nor the closure scan, re-checking only
  that the retained vector positions still name the same identities and that
  the admitted signature is unchanged. Arguments and results are the
  monomorphic scalar record/variant subset of Agent Proposal Schema v1 the
  interpreter actually executes: `bool`, `i32`, `i64`, `u8`, `usize`, owned
  `Bytes`, and bounded acyclic records, classes, and owned-byte variants over
  exactly those leaves; `string`, `char`, `f32`, `f64`, borrowed carriers, and
  generic shapes fail closed with a located `SPX-F102` diagnostic in the
  existing closed reason vocabulary rather than widening the cleanup shape the
  shared machinery and the native and Wasm backends own. Execution enters
  through the same evaluator call frame, a staged owned `Bytes` argument is
  charged against the same verified byte-data capacity a `bytes_copy` consumes,
  a Copy carrier is harvested by reference while an owned one must be uniquely
  held, and result leaves settle in declared field order without being sorted
  or repaired. The frozen zero-argument entrypoint product keeps its exact
  admission and known answers; both products now share one owner for the
  retained dispatch index. `string` fields, admitted by the proposal schema but
  by no interpreter record or variant classifier, remain Missing here.

- Added `std.data.json.doc`, the structural document layer of the bounded JSON
  slice: the object and array grammar driven by six modes over a base-2
  container stack held in one `i64`, an explicit `depth_limit` clamped to 32
  open containers and rejected at the offset of the container that exceeds it,
  a closer that does not match the innermost container rejected at its own
  offset, and trailing-byte rejection that reports the first trailing
  non-whitespace byte. `document_end` scans one value, `whole_end` requires
  only whitespace after it, and `is_document` is the whole-input form; the
  RFC 8259 number grammar, the exact `true`/`false`/`null` words, and string
  framing that rejects raw `0x00`-`0x1F` and an unterminated string are scanned
  in the same pass, allocation-free, returning only scalars in the family's
  end-offset/rejection encoding. Escape-character and surrogate validity stay
  with `std.data.json`, raw UTF-8 with `std.data.json.utf8`, and duplicate keys
  are accepted rather than rejected - all three stated in
  [Bounded JSON Scanner v1](docs/BOUNDED-JSON-SCANNER-V1.md) rather than left to
  inference, because the `SPX-G171` pre-bound admits 12,216 B of this package's
  source and rejects 12,292 B. The package deliberately depends on nothing: a
  `[dependencies]` edge on `std.data.json` alone puts it over that bound, since
  vendored dependency source is charged in full against the consumer.

- Extended the bounded JSON slice from one package to five sibling packages,
  because the Workspace Semantic Graph pre-bound is charged against a whole
  package - library, examples, and conformance modules - and no single library
  module can hold the slice. `std.data.json.token` adds the RFC 8259 number
  grammar as composable integer, fraction, and exponent scanners, `true`,
  `false`, and `null` recognition, and exact `i64` decoding that refuses
  overflow and refuses a fraction or exponent rather than rounding it, so
  `-9223372036854775808` decodes exactly and no value passes through an `f64`.
  `std.data.json.utf8` adds raw-byte UTF-8 validation that rejects invalid lead
  bytes, missing or malformed continuations, overlong encodings, raw
  surrogates, and scalars above `U+10FFFF`. `std.data.json.write` and
  `std.data.json.digits` add a pull-based, buffer-free writer: the exact length
  and each byte of a quoted JSON string, and the exact decimal bytes of any
  `i64` plus the literal words. All four carry the existing scanner's `usize`
  end-offset/rejection encoding and pass interpreter, native C11 at O0 and O2,
  and Core Wasm conformance. Two new gate cases link the siblings from one
  consumer. Structural document validation - nesting limit, trailing-byte
  rejection, and duplicate-key policy - decoded strings, an owned document
  tree, and an output buffer remain Missing.

- Extended concrete generic owned-`Bytes` records across the complete direct
  Copy-scalar set: `i64`, `i32`, `u8`, `usize`, `char`, `f32`, `f64`, and
  `bool`. Source verification, resolved HIR, cleanup inventory/replay,
  Native64/Wasm32 layout and native/Core-Wasm lowering retain the exact
  owner-and-index substitution. Concrete generic immutable updates now retain
  unchanged fields, replace owned fields left to right, and settle both partial
  construction and partial update failures without replacing the selected
  status. Focused local interpreter, Clang O0/O2 and Node/Core-Wasm evidence
  covers borrow-then-own execution, update execution, both partial failures,
  repeated entry, one-live-owner capacity, and hostile plan mutation. Explicitly
  instantiated generic functions can now consume and return the admitted flat
  owned-record template, with exact instance dispatch and failure settlement in
  the interpreter, native C11, and Core-Wasm. Real cross-file Project linking,
  semantic-recipe replay, candidate rename/recovery, and ABI-delta replay retain
  the internal generic identity while frozen public descriptors stay closed.
  Bounded acyclic concrete nesting now composes those records as
  `Box<Pair<Bytes, bool>>`, `Pair<Box<Bytes>, i64>`, and a two-owned-leaf
  instance. One global worklist authenticates depth, owned-leaf, and field-work
  limits; recursive cleanup retains complete stable field paths; Native64 and
  Wasm32 layouts independently replay complete concrete identities; and local
  interpreter, native C11, and Core-Wasm evidence covers whole moves, exact
  owner capacity, partial-prefix settlement, and repeated recovery. Generic
  variants, classes, resources, general nested storage, public generic ABIs,
  and hosted execution remain unchanged or unclaimed; the required Linux steps
  are authored but have not yet produced hosted evidence.

- Added a bounded authored generic owned-variant path for exact
  `Either<Bytes, i64>` and `Either<i64, Bytes>`-equivalent instances. Source,
  HIR, conditional cleanup, layout, interpreter, native C11, and Core-Wasm
  retain owner/index substitution and one authenticated owned case. Local
  evidence covers opposite live-case vectors, inactive cases, borrow then own,
  partial construction, owned-arm failure, exact semantic status, repeated
  recovery, zero native leaks, and rejection of shallow native/Wasm copies. A
  partial-construction fix now stores each completed Wasm field before
  evaluating the next initializer and still publishes the tag last. Hostile
  index/type layout drift is rejected. The compiler-owned two-sided `Result`
  is handled by a separate exact profile; nested/resource variants, public
  ABIs, and hosted promotion remain closed or unclaimed.

- Proved an exact authored two-owned-branch generic variant shape with two
  parameters, `[Bytes, Bytes]` arguments, and two owned cases. Existing
  conditional cleanup represents both case-qualified owners without a schema
  change. Local interpreter, C11 `-O0`/`-O2`, and Core-Wasm evidence covers
  both constructors, borrow/own matching, dynamic parameter/result/call
  transfer, branch-specific partial construction and arm failure, precondition
  settlement, repeated recovery, tight capacity, zero native leaks, invalid
  carriers/tags, and native/Wasm shallow-copy rejection. Hostile replay changes
  inactive liveness, case authentication, and guarded finalizers in both
  directions. Compiler-owned `Result<Bytes, Bytes>` is admitted separately
  below; broader multi-case generic sums, Project/public ABIs, and hosted
  promotion remain closed or unclaimed.

- Admitted the exact compiler-owned `Result<Bytes, Bytes>` instance for
  ordinary internal construction, explicit own/borrow matching, parameters,
  results, and calls. Its authenticated prelude identities reuse the proven
  conditional two-branch cleanup machinery. Focused local interpreter, native
  C11 `-O0`/`-O2`, and Core-Wasm evidence covers both active branches,
  dynamic forwarding, staged-call and arm-failure settlement, capacity-one
  execution, hostile cleanup-plan mutation, invalid tags, tag-last result
  publication, and native/Wasm shallow-copy rejection. The same exact internal
  instance now supports `Result<Bytes, Bytes> -> Result<Bytes, Bytes>` postfix
  `?`: the operand evaluates once, Ok moves its selected payload, Err transfers
  the complete residual, and ownership-only replay joins preserve shared
  postconditions, sticky failure, and both guarded finalizers. Focused local
  interpreter, native C11 `-O0`/`-O2`, and Core-Wasm evidence covers both
  branches and re-entry. Mixed/general/nested and generic-function owned
  propagation, Project/public ABIs, and hosted promotion remain closed or
  unclaimed.

- Bounded native C11 name resolution by the same aggregate operation deadline
  as the rest of the operation. A numeric endpoint is answered under
  `AI_NUMERICHOST` with no name service, no budget and no worker; a name is
  resolved on a POSIX-thread worker the emitted translation unit owns, waits
  only for what is left of the deadline, and leaves an abandoned worker
  registered, joined and freed by the next reap or by settlement rather than
  detached — the same model as the Rust `SystemResolver`, and the same
  non-claim: this bounds waiting, not `getaddrinfo`, which POSIX cannot cancel.
  A host may now select a shorter native deadline by defining
  `SPX_NETWORK_OPERATION_DEADLINE_MILLIS_V1`, clamped to the fixed maximum.
  Four executed native gates drive an injected 300 ms name service, a spent
  budget, an always-`EINTR` read and an interrupted-then-successful read
  through the emitted text in milliseconds, over loopback and an `AF_UNIX`
  socket pair with no outbound traffic; an always-interrupted whole program
  still selects `semaprax.network.v1` code 5 under a 250 ms deadline. The
  `_WIN32` branch is now cross-compiled where a Windows toolchain exists and
  keeps the inline resolver; Windows execution is explicitly scoped out rather
  than claimed.

- Added deterministic grammar-driven differential compiler tests with shrinking
  as a module of the existing `scalar_status_backend_equivalence` harness. A
  seed names one module in the admitted scalar subset — nested operands,
  parameterized helper calls, branches, bounded loops, mutation, contracts,
  checked failure and lazy evaluation — and every lane answers in one closed
  vocabulary: canonical parse-format-parse stability, graph identity,
  verifier/HIR agreement, the reference interpreter, native C11 at O0 and O2,
  and Core-Wasm on Node. Sixteen fixed seeds run in PR CI; a larger bounded
  campaign runs separately. A disagreement is classified, minimized by a
  structure-preserving shrinker, and rendered with the seed, source, compiler
  commit, toolchain identities, exact commands, expected and observed outcomes
  and the minimized module. Unsupported profiles and absent provisioned tools
  are explicit `Unavailable` outcomes, never parity passes, and failure
  injection proves the checker notices a wrong backend value, an incorrect
  failure selection and an abort. Generated modules declare no effects and no
  `unsafe`, so no fuzzer-generated file executes with ambient network,
  filesystem, process or signing authority. Owned cleanup and task schedules
  remain out of this first tranche.

- Added the bounded AGENT-04 interaction-contract slice. Source-owned Agents
  now derive exact Proposal and Observation grammars from checked same-module
  record or variant declarations, including closed bounded decoders and a
  deterministic provider-neutral JSON Schema/TypeScript/Python/Rust Proposal
  client bundle. Project revisions retain independently replayed contract facts
  and expose them through the existing AgentDefinitions node, ProgramRoot,
  typed query, and semantic service without changing closed transport schemas
  or legacy agent-free and explicit-association bytes. Generated client source
  is structurally verified but is not claimed as compiled, packaged, or run;
  cross-module role resolution remains follow-up work.

- Fixed `semaprax run` refusing bounded Copy record construction that `check`
  verifies and that the native C11 and Core-Wasm backends execute (#75). The
  single-file interpreter route was the only route asking
  `record_construction_is_admitted` for the narrower owned-byte answer, so
  reading `p.x` was admitted while building the `p` it reads from was not. The
  admission predicate is now the single bounded acyclic classifier every route
  already shared, and the parameter that split them is gone. Field Mutation v1
  stores now replace one direct scalar field instead of the whole binding —
  the previous store left a later projected read failing closed on `SPX-F105`
  — and a shared Copy carrier is copied before the store, so a sibling binding
  keeps the value the other two backends give it. Record shapes still outside
  the profile keep their exact closed reason at admission: a record-typed
  callee signature is `unsupported_callee`, and `with` over a Copy record is
  `record_update`.

- Added the bounded AGENT-03 language-native source Agent slice. A closed
  `.spx` declaration with explicit identities and fixed deterministic/model/
  effect roles now parses, round-trips canonically, lowers through the unchanged
  AgentDefinition v1 compiler, and retains byte-identical AgentDefinition,
  AgentGraph, and Runtime Profile products on the admitted Project revision.
  Default canonical workspace derivation populates its existing
  AgentDefinitions segment, with the same node selected through ProgramRoot,
  exact context, a typed in-memory query, and the persistent service. The three
  focused compatibility/hostile/integration cases pass locally; role execution,
  provider authority, opaque authorization, and durability remain absent.

- Fixed the Workspace Semantic Graph `builder_bytes` pre-bound charging every
  imported function's contract and body as resolved structure in the importing
  module. The synthetic projection retains an import as a stub — rewritten
  signature, no contract, default body — so a conformance module that imports
  everything its library exports was costing a second complete copy of that
  library, expanded by the structural factor. Imports are now charged as the
  stub they become, plus the largest single transient provider clone at the raw
  AST rate. `SPX-G171` still refuses before mutation and the estimate remains an
  upper bound of resolver memory. Documented the practical authoring limits,
  including the `SPX-H006` cleanup-replay path budget, in
  [Standard Library v1](docs/STANDARD-LIBRARY-V1.md).
- Added ProgramRoot v2 and Exact Program Context v1. The versioned root retains
  the enriched canonical workspace's nine v1 segments, appends exact interface/
  artifact and Project Lock association descriptors, and explicitly binds the
  distinct default Project root. Additive service, query, transaction, and
  structural-diff entry points select the same dual-keyed v2 root in memory
  without changing legacy wire bytes. Exact candidate-root derivation and
  refresh remain fail-closed pending candidate-safe lock replay.

- Added locally exercised SEG-02 input bundles for exact source-interface and
  pathless generated-artifact facts plus exact Project Lock v1 association.
  Both freshly replay their existing owning compiler artifacts, retain bounded
  content identities without authority, and preserve ProgramRoot v1 and every
  legacy Project/Image/Graph/lock byte. Versioned exact-root integration remains
  explicit follow-up work.
- Added the provisioned Linux offline doctor lifecycle gate: one dispatch-only
  workflow, `scripts/doctor-provisioned-linux-gate.py`, and
  [Provisioned Linux gate v1](docs/DOCTOR-PROVISIONED-LINUX-GATE-V1.md). The
  gate asserts host, kernel, delegated cgroup-v2, held-image, release and
  trust-anchor preconditions before any test runs, selects the twenty-six
  existing ignored lifecycle fixtures exactly and serially, binds commit,
  platform, tools, fixture identity, bounded capture and final cgroup
  emptiness into one evidence document, and keeps failure selection sticky
  over cleanup. Absent provisioning is a failure, never a skip: unmet or
  unobservable preconditions, still-ignored tests, an unmatched filter and a
  narrowed selection are each rejected, and `--self-test` drives that logic
  with synthetic inputs on any host. **The gate itself has never been
  executed**; no provisioned Linux x86-64 host exists, no completion row
  moves, and the private provisioner stays out of every ordinary CLI route.

- Added the SEG-02 explicit AgentDefinition association bridge. A bounded,
  stable-ID-ordered set of exact compiler-admitted definition/graph/profile
  bundles can populate the existing Canonical Workspace AgentDefinitions node
  for one exact Project revision and flows into ProgramRoot automatically.
  Default empty derivation remains byte-compatible; the new route explicitly
  does not claim `.spx` Agent syntax, intrinsic Project ownership, execution,
  or authority. Its three focused Workspace-harness cases pass locally.

- Added the SEG-02 ProgramRoot v1 foundation as a small segmented,
  content-addressed projection of Canonical Semantic Workspace Revision v1.
  Nine independently digested descriptors bind the existing typed node bytes,
  while DeploymentRoot, InstanceRoot, and EvidenceRoot remain typed unbound
  placeholders. Existing canonical workspace, Project, Image, and Graph bytes
  and identities are unchanged. The 33-case combined focused gate and strict
  all-target clippy pass locally.

- Added the Project-v13 native C11 HTTPS lane. Generated commands link through
  a narrow libcurl route, embed a pinned 146-certificate Mozilla trust bundle,
  require hostname/certificate validation with TLS 1.2 or 1.3, negotiate
  HTTP/2 with HTTP/1.1 fallback, bound redirects/connections/headers/body and
  time, suppress ambient proxies, canonicalize the complete response, and
  settle before publication. A real encrypted loopback gate and an opt-in
  public-PKI smoke exercise the generated executable.

- Added a locked Project-v13 HTTPS browser fixture and provisioned Chromium CI
  gate. It executes the real generated npm/Wasm package, verifies exact
  fixture-v3 output, one-shot invocation and tampered-Wasm rejection, and
  proves every browser request stays on the loopback harness origin.

- Added a descriptor-derived safe C++17 adapter for Project-v9 flat owned
  records. Its thread-bound noncopyable client preflights borrowed inputs,
  authenticates private carrier values, copies and settles the sole byte
  handle, closes the provider context, and only then returns a value-only C++
  record. A real separately compiled C11 provider and C++17 consumer execute at
  O0/O2. The C header now preserves its C11 static-array minimum while using
  C++-legal syntax when included from C++.

- Added `std.data.json`, the first JSON facility a SEMAPRAX program can call.
  [Bounded JSON Scanner v1](docs/BOUNDED-JSON-SCANNER-V1.md) specifies a pure,
  allocation-free JSON string-token scanner over `borrow Slice<u8>`:
  whitespace skipping, escape classification, `\uXXXX` code-unit decoding,
  strict surrogate-pair rules that reject a lone or unpaired surrogate, RFC
  8259 control-byte rejection, and the byte offset of the first rejection
  carried in the same `usize` result. Number and literal tokens, structural
  document validation, UTF-8 validation, decoded strings, an owned document
  tree, and a writer stay Missing: the `SPX-G171` workspace-graph pre-bound
  admits roughly 4.7 KiB of library source for a package of this density, and
  rejects a consumer that links two such packages, so the remaining scope
  needs that bound raised rather than more library code.

- Added invocation-owned HTTPS work to the bounded structured-task runtime.
  Providers now settle exactly once across success, HTTP failure,
  cancellation, panic, deadline expiry, and registration rejection; results
  publish only after settlement, while started blocking I/O drains and late
  responses are discarded.

- Added authored Universal Semantic Query v1 checked-fact projections for one
  expression's ownership/loan facts and for direct retained-HIR declaration
  consumers. Both are exact revision-bound canonical requests with replay,
  paging/walk/output bounds, and explicit static-analysis and export-visibility
  nonclaims; the focused evidence passes locally.

- Extended Universal Semantic Transaction v1 with typed `AddDeclaration`.
  It authenticates the exact anchor-module path, source digest, and ordered
  declaration identities, rejects Project-wide identity reuse, delegates the
  closed function/record/variant constructor to Project Candidate, and proves
  one source insertion with unrelated bytes preserved. The read-only
  `change preview add-declaration` adapter and focused regression evidence pass
  locally.

- Added the Project-v13 HTTPS Core-Wasm and npm lanes. The new replayable
  `semaprax.project-npm-build.v12` package authenticates one
  `spx_https_get_v1` import, a distinct HTTP status marker, owned response
  carriers, fixture-v3 authority, and success-only output; the generated
  package executes the committed HTTPS project under Node without sockets.

- Added frozen Project Manifest v13 with the exact `https-command-io.v1`
  profile and `network.http` authority. Its authenticated `network-run` route
  replays fixture v3, while Project v12 and all prior manifest bytes remain
  unchanged.

- Repaired the VS Code Extension Host inventory and widened its scenario. The
  host test asserted exactly 28 contributed commands while the manifest
  contributes 37, so a provisioned run failed before its workflow. The
  assertion is now the exact 37-command inventory in manifest order, every
  contributed command must be registered, no registered `semaprax.` command may
  be undeclared, and the forbidden authority-bearing list is checked against
  both contribution and registration. The scenario additionally covers
  check-on-save positions past a supplementary character, retention of previous
  diagnostics across an unclassifiable run, project-routed navigation across all
  three calculator sources, and the dirty-buffer and project-rename refusals.
  The provisioned runner's node-controller inventory, recorded inputs, and
  command count were updated to match. The Extension Host gate itself was not
  run for this change: no Visual Studio Code product is installed here.

- Routed VS Code declaration navigation, callers, code lenses, and ownership
  through the project that owns the saved file, resolved exactly as
  check-on-save resolves its subject. A module with `use` imports has no
  standalone meaning (`SPX-G172`), so those commands previously failed on every
  importing file, including the shipped calculator project. The
  `semaprax.project-query.v1` result is parsed with its per-match `path` and
  `source_revision`, matches outside the project root are dropped, and a
  selection opens the authenticated file the match was found in. Safe rename
  reports that a project-owned file belongs to the saved-source session's
  replay-checked typed intent rather than to a standalone patch, and the
  module-only `doc` and `graph` routes name their boundary.

- Made VS Code MCP frame assembly linear in received bytes. The receive path
  concatenated the whole retained response with every incoming chunk and
  rescanned it from byte zero, so a legal response fragmented into 1 KiB pieces
  copied about 2.1 GB for 2 MiB received. Retained fragments are now counted for
  the cap check and copied once into the frame they complete. The byte cap,
  strict UTF-8, CR rejection, response-identity validation, serial request
  semantics and terminal failure are unchanged.

- Fixed VS Code editor ranges mixing three coordinate systems. The compiler's
  UTF-8 byte spans are now translated against the exact saved source into
  zero-based UTF-16 positions that may cross lines, through one mapper
  (`editors/vscode/positions.js`) shared by diagnostics, declaration
  navigation, and code lenses. Torn, reversed, and out-of-range offsets fall
  back to the reported line and column instead of underlining the wrong text,
  and a document changed while the compiler ran receives no positions at all.

- Fixed the VS Code check-on-save adapter reporting a clean project from
  output it could not read. `check --json` output is now classified into
  diagnostics, the verified record, and malformed lines, and the exit status is
  validated against them; a killed child, a foreign status, an unparsed line, an
  error with status 0, or a verified record with status 1 is a check failure
  that retains the previously published diagnostics instead of clearing them.
- Made the `Release gate` CI aggregate fail closed. It ran under
  `if: ${{ success() }}`, which skips the job whenever a blocker did not
  succeed, and GitHub scores a skipped check run as a satisfied required status
  check. It now always runs and asserts every `needs` result through
  `scripts/ci-required-checks.py`, rejecting failed, skipped, cancelled,
  malformed, vacuously empty, and foreign-commit inputs; a local test drives
  those cases and a second derives the blocker inventory from the workflow so a
  new job cannot escape the aggregate. Added
  [Required CI checks v1](docs/CI-REQUIRED-CHECKS-V1.md) recording the observed
  unprotected `main`, the branch-scope, bypass and recovery policy, and the
  exact ruleset request. The ruleset is a proposal: no repository setting,
  ruleset, membership, credential or branch permission was changed, and no
  required check is in force.
- Added Persistent Semantic Workspace Service MCP v1 through
  `semaprax service <project> --mcp`. Its closed seven-tool MCP catalogue exposes
  protocol/status, universal, retained-index, and bounded history queries,
  transaction validation, and caller-owned refresh through one process-retained
  service generation.
  Startup is the only host-path boundary; MCP tools add no authority and frozen
  Project Agent Transport v5 bytes and MCP catalogue remain unchanged.
- Added Persistent Semantic Workspace Service Transport v1 and the
  `semaprax service <project>` adapter. One authenticated Project and one
  incremental semantic service remain alive across bounded JSON-RPC-lines
  universal-query, retained-index-query, transaction-validation, and
  caller-owned refresh requests; failed or stale refresh rolls back before the
  in-memory generation/cache/index CAS. Four focused Workspace-harness cases
  pass locally.
  This is a single-client local stdio process, not MCP/LSP, a socket, daemon,
  durable/shared state, source writer, execution service, or authority broker;
  frozen Project Agent Transport v5 remains separate and unchanged.
- Extended Universal Semantic Transaction v1 with typed whole-function
  `ReplaceBlock`. It binds an exact old source block, delegates the replacement
  expression to complete ProjectCandidate validation, proves byte-identical
  source outside the selected span, and exact-replays the ordinary transaction
  artifacts without commit authority. The focused transaction/composition
  selection passes 10 cases locally; nested expression replacement,
  multi-operation transactions, and ReplaceBlock composition remain open.
- Added refresh-atomic retained semantic-service indexes for tests covering a
  stable declaration and functions that can reach a named effect. Canonical,
  revision-bound query/results are bounded, deterministic, replayable, and
  available through both the service core and `workspace/index-query`.
  Persistent-core focused evidence passes five cases locally.
- Extended `semaprax review` with the closed Project form
  `review <project> <transaction.json> [--evidence]`. Default output is the
  exact concise semantic review; evidence is displayed only when requested.
  The existing source-patch form remains unchanged, no files are written, and
  the focused three-case CLI gate passes locally.

- Fixed the macOS held-Git process boundary to preseed CoreFoundation's
  user-text-encoding key with the process UID and fixed encoding fields. This
  prevents CoreFoundation from reading the user-home encoding file or rewriting
  the child environment, while the exact environment assertion remains closed.

- Added Signed Minimum Literals v1: `-9223372036854775808` and
  `-2147483648i32` are ordinary literals in expressions and match patterns, so
  boundary constants no longer need a `-MAX - 1` workaround. The magnitude
  survives tokenization as its own token and is claimed only by a directly
  applied unary `-`, so tokenization stays context-free and subtraction is
  unchanged; the bare magnitude, a parenthesized one, and `MIN - 1` all keep
  the stable located `SPX-P003` rejection. `-MIN` and `MIN / -1` still select
  checked arithmetic failures, and the native C11 backend now spells both
  signed minimums as a negated maximum less one so no C literal names an
  unrepresentable magnitude.
- Fixed `run <file> --json` publishing human diagnostics on stderr when its
  preliminary load or verification failed. Parse errors, unreadable inputs, and
  type/effect/ownership failures now emit the same diagnostic records on stdout
  that `check --json` does, including for the bounded stdout profile. Human
  mode and every execution envelope are unchanged.

- Added `semaprax.network-fixture.v3` as an ordered, bounded HTTPS
  request/response replay carrier. V1 and v2 reject the new member, URL or
  response mismatches do not consume queue entries, and hosted `https_get`
  conformance now uses the serialized carrier shared with generated lanes.
- Added Installed Fix Plan v1. `semaprax fix --plan` returns the exact bounded,
  version-bound one-operation installed catalog, while `semaprax fix <file>
  assign-function-id <automatic-function-id> --plan` wraps the existing
  source-authenticated `SPX-S103` Diagnostic Repair discovery in a canonical,
  replayable plan. Five focused semantic-harness cases pass locally. Planning
  does not select a persistent ID, instantiate or apply a patch, write source,
  rank repairs, or grant host/publication
  authority; existing `repairs`, `repair`, and Diagnostic Repair bytes remain
  unchanged.

- Admitted a bounded internal concrete generic owned-byte record slice. Exact
  authored instances such as `Pair<Bytes, bool>` now substitute fields through
  source verification, HIR facts, cleanup inventory and replay, interpreter,
  Native64 C11, and Wasm32 lowering. Focused local evidence covers borrow then
  own, repeated success, and failure after owner creation across interpreter,
  C11 `-O0`/`-O2`, and Node/Core-Wasm; stable diagnostics keep nested generic,
  class, variant, and non-Copy record shapes closed. This record slice does not
  authorize prelude carriers; exact `Result<Bytes, Bytes>` support was added
  later under the owned-variant contract. It does not expose a Project, C,
  Rust, WIT, Component, or package ABI.
- Named the exact `Option<Bytes>` and `Result<Bytes, i64>` tags in the public
  Project-v8 C11 header. A separately compiled C consumer now executes all four
  cases at O0/O2, proves inactive cases grant no handle authority, and copies,
  drops, and stale-rejects each active byte handle before context closure.
- Exposed the shared descriptor-derived C11 provider header for Project-v10
  owned UTF-8. A separately compiled consumer links against the actual provider
  at O0/O2 and copies, drops, and closes an exact-length result containing both
  embedded NUL and multibyte UTF-8, without exposing native String layout.
- Added Installed Diagnostics v1: an offline build-time scan embeds the exact
  static `SPX-*` token inventory from compiler and workspace-member Rust
  sources while separately reporting unresolved dynamic diagnostic
  constructor sites. Authority-free library constructors expose bounded,
  canonical, compiler-version-bound catalogue and per-code explanation
  artifacts with exact replay; `semaprax explain` prints the exact concise or
  JSON explanation. Five focused projections-harness cases pass locally.
  Static presence is not runtime reachability, message or repair inference,
  binary attestation, or a stable cross-version
  registry, and legacy diagnostic bytes remain unchanged.
- Added a descriptor-derived C11 header for the Project-v11 nested
  owned-record provider. Its fixed-width leaf carrier preserves full
  descriptor occurrence order without exposing native record layout. A
  separate C consumer links and runs at O0/O2, copies and settles two distinct
  byte owners, rejects both duplicate drops, and closes their shared context.
- Added a descriptor-derived low-level C11 header for the Project-v9 flat
  owned-record provider. It publishes fixed-width carrier field counts,
  ordinals and kinds without exposing native record layout. A separate C
  consumer compiles, links and runs with the actual provider at O0/O2 while
  checking poison preservation, scalar decoding, byte copy/drop, stale-handle
  rejection, and context closure.
- Added source-level `https_get` through an explicit `network.http` capability,
  a closed `semaprax.http.v1` status domain, canonical bytes consumable by
  `std.http`, and a reusable authenticated HTTP/1.1/2 host provider; added
  `net_tls_accept` for source-controlled server-side TLS without changing the
  existing network status domains or prior HIR cache tags.
- Added a pure C11 consumer gate for the authenticated Project-v8 owned-data
  package. The generated provider and a translation unit using only the public
  C header compile separately, link, and execute at O0/O2 while checking
  invalid-bool output poison, owned-byte copy/drop, stale-handle rejection, and
  context closure. This establishes a local linkable C ABI slice without
  promoting Objective-C, cross-platform support, distribution, or compatibility.
- Added the locally exercised Universal Semantic Transaction Composition v1
  core. It derives a bounded canonical workspace structural diff, rebases one
  validated display rename onto an exact independently admitted revision, and
  merges two distinct-target sibling renames in an explicit order through the
  existing Project Candidate conflict and replay machinery. The five focused
  integration cases pass locally. Four focused Workspace-harness cases also
  prove that the read-only CLI forms print the exact core structural-diff,
  rebase, and merge reports; no general multi-operation
  transaction, behavioral equivalence, source commit, transport, or authority
  is claimed, and all Semantic Transaction v1 bytes remain unchanged.
- Added Installed Agent Guidance v1: six deterministic, version-matched,
  authority-free skill documents are available through `skills get`, and
  `query --capabilities` reports the exact installed five-operation Universal
  Semantic Query catalogue with no host grants. Core and CLI return identical
  canonical, bounded, digest-bound artifacts assembled only from embedded
  compiler resources. The focused projections integration gate passes 5/5;
  no binary attestation, live discovery, skill execution,
  host authority, registry, network, MCP/LSP, or legacy-query change is claimed.
- Added exact `semaprax help diagnostic <SPX-code>` lookup and the bounded
  `semaprax help diagnostic codes` inventory. A generated JSON companion keeps
  the CLI response identical in meaning to the compiler-checked quick
  reference's correction table, and the documentation gate now requires every
  marked failing example to have indexed help. The guarded `SPX-T208` answer
  is 111 bytes and 32 lexical units versus 2,513 bytes and 916 units for the
  complete diagnostic index.
- Added the locally exercised Universal Semantic Workflow CLI v1 read-only
  adapter. Five Project-only `query` subcommands print exact
  Universal Semantic Query results, while `change preview rename-display-name`
  prints the exact Universal Semantic Transaction result or evidence. Each
  one-shot invocation retains and finally rechecks one authenticated Project;
  no source write, commit, persistent transport, or MCP/LSP route is added, and
  frozen Project Agent Transport v5 remains unchanged. The five focused
  integration cases pass locally.
- Added the authority-free, transport-neutral Universal Semantic Query v1
  core. Five exact canonical, workspace-revision-bound operations cover bounded
  declaration paging, symbol, context, impact, and truthful transaction
  eligibility; results bind Project, image, canonical component, payload, and
  request identities and support fresh exact replay. The six focused integration
  regressions pass locally. No wire, CLI, MCP, LSP, mutation,
  publication, or frozen Project Agent Transport v5 change is claimed.
- Added exact `semaprax help language <topic>` lookup and the bounded
  `semaprax help language topics` selector list. An installed compiler can now
  return one compiler-checked section of the agent quick reference without
  transferring the whole card; every topic is guarded at more than five times
  smaller in both bytes and repository lexical units. The `scalars` result is
  793 bytes and 296 units versus 26,140 bytes and 7,418 units for the full card.
- Added a hosted Network Services v1 extension with Rustls-authenticated TLS
  clients, explicit listener/accept lifecycle, deterministic fixture v2, five
  effect-gated source operations, and a bounded real structured-task runtime.
- Restored the frozen legacy Project-v3 Wasm byte projection by keeping the
  data-export profile's unused `spx_contract_fail` import at its historical
  void signature. Data exports continue to publish typed arithmetic, contract,
  and byte-range failures through status globals; the ordinary entry wrapper
  retains its status-carrying failure import. The byte-capacity unit module now
  names its test submodule path explicitly so both its library owner and the
  consolidated cleanup harness remain resolvable by `cargo fmt --all`.
- Added an explicit reusable native-host HTTPS client with bounded redirects,
  keep-alive pooling, HTTP/1.1 and HTTP/2 negotiation, stable typed responses,
  body limits, and an opt-in public-PKI smoke; the TCP provider can now accept
  server-side TLS with caller-installed Rustls certificate policy.
- Added the developer-preview `network-command-io.v1` Project v12 profile,
  native and deterministic fixture-only npm/Web build lanes, the bounded
  `semaprax network-run --fixture` command, and a committed HTTP Project
  fixture. Browser and npm packages receive no ambient or real socket
  authority.
- Added Graph v32/v33 for exact composition of owned-variant conditional
  cleanup with unprojected or stable-field-projected Shared Loan Plan facts.
  Semantic Workspace now preserves those source schemas, unknown spellings and
  evidence flows remain closed, and the formerly ignored checked-HIR cache
  regression executes cold/warm Graph v32 accounting and replay.
- Added scoped `semaprax help shapes <kind|stable-id|path#stable-id>` backed by
  a generated JSON companion to the unchanged Markdown catalog. Exact IDs can
  be source-disambiguated, and a kind returns its smallest canonical exemplar
  instead of every declaration. The guarded `calculator.add` result is 114
  bytes and 33 lexical units versus 22,888 bytes and 7,571 units for the full
  catalog, while every kind exemplar stays within 512 bytes and 128 units.
- Added the authority-free, transport-neutral Persistent Incremental Semantic
  Workspace Service v1 core. A process can retain one immutable Project,
  Canonical Semantic Workspace Revision, Semantic Workspace Image, and semantic
  cache generation; serve revision-bound bounded snapshot queries; stage a
  complete source-exact incremental successor; and install it through one
  expected-current in-memory generation/cache compare-and-swap. Transaction
  validation returns Universal Semantic Transaction artifacts without adopting
  their candidate. The three focused lifecycle regressions pass locally; no
  wire, CLI, MCP, LSP, disk persistence, build, execution, commit, publication,
  or broad operation-algebra claim is added.
- Corrected the pathless public C-artifact projection after entry and public
  target closures were separated: native inspection now emits the independently
  admitted entry-plus-export program, so unsupported owned exports remain
  explicit header exclusions instead of failing because they are absent from
  the executable entry-only closure. Native useful-data, language-I/O, and
  line-I/O command products likewise emit that public closure so the
  manifest-selected command and its exact permits remain present. Entry
  execution semantics remain unchanged.
- Admitted explicit nongeneric resource-free record and variant type imports
  containing owned `Bytes` in owned Project profiles. Stable-ID HIR and cleanup
  plans now survive that module boundary, and an owning nominal function can
  move with one exact type import; imported callables exposing owned nominal
  arguments, borrowed storage, resources, generics, cycles, runtime execution,
  and public carrier widening remain closed. Promoted the focused cleanup
  dependency image/transport corpus into the ordinary suite.
- Promoted the focused nominal record/variant rename regressions into the
  ordinary candidate suite after correcting an invalid shadowed pattern
  fixture; the corpus now executes owning and generic nominal renames, stable
  member identities, immutable rejection, recovery, conservative rebase, and
  the existing transport surface; runtime and hosted gates remain unrun.
- Hardened `std.http.status_code` so an HTTP/1.x status code is accepted only
  when the required space before the reason phrase is present; truncated lines
  such as `HTTP/1.1 299` now return `-1` consistently on the interpreter,
  native C11, and Core Wasm lanes.
- Added `semaprax help shapes`, which prints a generated language shapes
  catalog: every declaration of every committed example with its `@id` and
  canonical header, rendered through the `semaprax doc` model and pinned by
  the projections harness, so the bundled reference of admitted shapes is
  derived from the graph rather than hand-written.
- The VS Code adapter adds `Safe Rename by Stable ID` (authors the semantic
  rename patch, shows `impact`, applies through the replay-checked `patch`
  route), `Show Cleanup Plan` (the module graph's canonical cleanup plan for a
  function), and `Run Agent Transcript` (trace, evidence, or receipt of a
  scripted `agent run`), all bounded like check-on-save.
- `semaprax doctor` is now admitted by the standalone compiler: the pure
  offline-profile admission and version-policy module moved from the private
  toolchain into the root crate (`src/doctor.rs`), so both binaries print the
  same report and the guided help pages are identical; only the settled
  provisioner-observation renderer remains in the toolchain, and only
  `build --target rust` still needs the private host.
- Added exact offline `semaprax help library <module|name|stable-id>` lookup,
  returning only the generated dependency row, required profile, signature,
  effects, and contracts while preserving the full catalog bytes. The guarded
  `std.core.compare` lookup stays within 512 bytes and 128 lexical units and is
  more than 50 times smaller than the 22,076-byte, 6,662-unit catalog in both
  measures.
- The VS Code adapter adds `Show Ownership, Contracts, and Effects` (the
  compiler's bounded `context` facets for a chosen callable) and `Inspect Agent
  Definition` (`agent inspect` over the saved AgentDefinition v1 file), both
  bounded like check-on-save and covered by `test/navigation.test.js`.
- Added `semaprax agent run <definition.json> <task.json> <transcript.json>
  [--evidence|--trace]` and `semaprax agent replay ... <evidence.json>`: a
  scripted transcript of provider responses and tool results drives the
  bounded Agent Runtime v1 through the definition's derived profile with no
  transport, clock, or tool authority, so runs are deterministic and a replay
  proves an evidence capsule byte for byte (`SPX-V221` malformed transcript,
  `SPX-V222` replay mismatch). `resume` and `reconcile` stay unadmitted.
- Added the bounded, authority-free Universal Semantic Transaction v1 kernel
  over Canonical Semantic Workspace Revision v1, with one typed
  `RenameDisplayName` operation, exact base/old-name preconditions,
  deterministic intent/impact/review/result/evidence, fresh exact replay, and
  a locally passed focused Project-candidate gate. The additive slice does not
  alter legacy Project, workspace, Image, Semantic Change, or Candidate bytes.
- The bundled standard-library catalog (`semaprax help library`,
  `std/catalog.json`) is now rendered from the `semaprax doc` documentation
  model, with every signature cross-checked against the source text and each
  declaration's leading comments carried as its description.
- Added `semaprax add <dir>|semaprax.toml <package> <range>`, which appends
  one byte-sorted `[dependencies]` row to a Package Manifest v1 table manifest
  and rewrites it canonically only after the result re-parses (`SPX-J127` for
  frozen layouts and duplicates), and `semaprax fetch <cache-dir>
  <subject.json>...`, which replays Subject-v3 envelopes and files them into
  the resolver's content-addressed cache by digest with one receipt line
  (`SPX-J128` on tampering or collisions), with no registry or network access.
- The VS Code adapter navigates by meaning: `Go to Declaration by Stable ID`,
  `Show Callers of a Declaration`, and `Show Module Documentation` run the
  selected compiler's read-only `query --json` and `doc` over the saved active
  file, and code lenses show each declaration's `@id`, effects, and contract
  counts (`semaprax.codeLens`). The `doc` and `query` JSON projections now
  carry each declaration's and member's `location`.
- Fixed Project owned-API target preparation to retain a distinct,
  already-admitted entry-plus-export HIR closure without changing entry-only
  execution or cleanup semantics, and updated both typed-expression schema
  gates for the two newly admitted numeric-to-String operation alternatives.
- Updated cleanup and protocol regression fixtures for aggregate-equality
  rejection, stable-ID native contract details, and the precise scalar-profile
  signature diagnostic while preserving owned-result and lazy-temporary
  cleanup coverage.
- Kept the freestanding C profile libc-free after native contract details
  gained argument rendering by replacing hosted formatting with a bounded
  byte copy and eliding per-call formatting, refreshed the affected native
  known answers, and made the depth-512 HIR oracle independent of the parser's
  depth-128 source admission boundary.
- Added the locally exercised Canonical Semantic Workspace Revision v1
  foundation: one immutable authority-free object derived from an admitted
  Project, with nine typed node projections, distinct semantic, source
  projection, manifest, and dependency-closure digests, one composite revision,
  and exact fresh replay. Existing Project, managed Workspace, and Semantic
  Workspace Image v1 schemas, bytes, and revision identities remain unchanged;
  universal transactions, full semantic coverage, persistent service, `.spx`
  agent syntax, and publication authority remain separate work.
- Added Bounded Language Network I/O v1: six compiler-owned, effect-gated TCP
  client operations (`net_connect`, `net_send`, owned `net_recv`,
  transcript-streaming `net_stream_stdout`, bounded-readiness `net_wait`,
  `net_close`) with the closed `semaprax.network.v1` status domain, a
  `NetworkV1` operation profile, invocation-scoped handles, a deterministic
  `semaprax.network-fixture.v1` provider, a real `TcpNetworkProvider` for the
  hosted interpreter seam, native C11 POSIX/Winsock lowering, and Core Wasm
  closed `env` imports with fixture injection; plus pure `std.net`,
  `std.http`, and `std.async` helper packages and the `net_http_get` example.
- Fixed the native C11 borrowed-view context extension: the `call_depth`
  field added to `spx_context` had silently detached the text anchors that
  splice in `borrowed_str_depth`, so every native program with a
  `borrow Slice<u8>` or `borrow str` parameter failed to compile; the anchors
  now target the current struct and the emitter asserts that they matched.
- Added `semaprax query <file|project> [filters] [--json]`, a read-only
  declaration search over a checked module or every authenticated Project
  source with `--kind`, `--name`, `--id`, `--effect`, `--calls`, and
  `--called-by` filters. Project results name owning paths and exact revisions,
  and use the retained semantic graph for cross-file call predicates; unknown
  kinds or identities fail closed with `SPX-V211`/`SPX-V212`.
- `semaprax context <file|project> <stable-id>` now accepts a Project directory
  or manifest and renders a compact authenticated, bounded cross-file context
  without requiring agents to discover and reopen one source. The calculator
  gate caps it at 2 KiB, 600 lexical units, and one sixth of the full graph.
- Added the `semaprax package report|lock|resolve` namespace, each subcommand
  exactly its long-form offline package route.
- Split CLI option parsing and single-file execution/reporting out of the
  command dispatcher, bringing `src/cli_driver.rs` below the module-size limit
  while retaining the moved surfaces in the source-locked doctor contract.
- Added `semaprax verify`, one verb for every independent evidence verifier:
  the capsule's `schema` selects the owning route (semantic patch evidence
  v1/v2, workspace patch evidence, semantic workspace change, structural
  change, and operations evidence, agent graph bundles, and project images)
  and the receipt is that route's own bytes; unrecognized or unreadable
  capsules fail closed with `SPX-V201`/`SPX-V202` before any verifier runs.
- Added `semaprax agent inspect <definition.json> [--profile]`, which compiles
  a canonical AgentDefinition v1 and prints its AgentGraph v1 or its Agent
  Runtime Profile v1 projection; the other lifecycle verbs stay unadmitted.
  The guided help gains an `Agents` group and lists `verify` under `Change by
  meaning`, with shapes shortened to hold the 2048-byte bound.
- Added `semaprax doc <file> [--json]`, the documentation projection of one
  checked module: a Markdown page or one `semaprax.doc.v1` document of every
  declaration's identity, canonical signature, ownership modes, effects,
  contracts, members, and leading `//` comments, bound to the graph revision.
  The projections harness proves the page and `semaprax graph` name the same
  declarations at the same revision on every committed example. The guided
  help lists it under `Inspect meaning`; several guide summaries were
  shortened to keep both capability pages under the 2048-byte bound.
- The canonical graph-operational agent workflow now uses compact candidate
  source-review, function-summary, and impact-summary projections while full
  candidate and semantic-delta replay stays inside the verifier. Its regression
  rejects the older full-report routes and enforces aggregate protocol and
  review-material byte/lexical-unit ceilings, preventing silent context-cost
  regressions in the fixed twelve-step application change. Immutable review
  data is transferred once and reused across the expected conflict; recovery
  replay proves the candidate and source review stayed exact without two
  duplicate protocol responses.
- Project-link diagnostics now name and locate the exact entry, test, or
  provider module responsible for missing or invalid `main` declarations and
  distinguish capability exclusions from declaration-shape exclusions.
- CLI and parser diagnostic gaps now retain stable codes and actionable source
  anchors: missing `fmt` inputs use `SPX-I001`/`SPX-J102`, unknown `context`
  symbols use `SPX-G404`, bare identifier statements point at their token,
  UTF-8 BOMs name their removal, and semantic-review patch decoding shares the
  ordinary `SPX-I202` wording.
- Improved newcomer diagnostics: statement-position `if` failures now name
  the mandatory `else` and discard form, immutable parameters show the
  mutable-copy repair, and unknown bundled standard-library functions name
  their manifest dependency and offline catalog route.
- Added reserved `string_from_i64` and `string_from_usize` operations with
  canonical decimal spelling across the interpreter, native C11, and Core
  Wasm lanes, allowing computed integers to be printed without handwritten
  digit tables.
- Malformed three-token help requests now identify the unexpected extra
  operand instead of misreporting `help` as an unknown command. CLI report and
  analysis option parsers also live in a bounded driver submodule, keeping the
  shared dispatcher comfortably below its recorded module-size cap.
- Build target errors now use input- and toolchain-specific catalogs; scoped
  help and the CLI guide document the Web-compatible `wasm` alias, `-o` /
  `--output`, target and destination defaults, and structured `build --json`
  results. Profile Web collisions report Web-specific `SPX-I307`, concurrent
  project native builds have one create-new winner, and command-profile
  `run` output explains that it executes the entry while built adapters invoke
  the manifest command.
- Single-file native and Web/Wasm builds now require fresh destinations.
  Native output is reserved before compiler invocation and Web output uses an
  atomic create-new directory, so existing sources/artifacts and concurrent
  winners are never overwritten or merged; invalid parents report `SPX-I301`
  and existing destinations report `SPX-I307`.
- Native contract-failure stderr now includes the canonical clause, persistent
  function identity, and declaration-ordered observed arguments, matching the
  interpreter's human repair detail while preserving the normalized status
  and exit code.
- Reference-interpreter frames now use indexed binding slots instead of
  reverse linear scans. Interned `ValueId` handles also make executed `let`
  binding insertion allocation-free after HIR construction; scalar reads keep
  their existing by-value copy path. The runtime loop is retained in the
  Criterion interpreter benchmark as a regression gate.
- Single-file `semaprax run` now executes `app.main` through the bounded
  reference interpreter, accepts `--json`, `--max-steps`, and `--max-bytes`,
  and automatically admits the exact bounded stdout-transcript profile. The
  former generated C11 behavior remains available explicitly as `--native`.
- Generated native executables now guard the same 256-frame call-depth bound
  used by the reference interpreter. Deep and mutual recursion return the
  deterministic `semaprax.runtime.v1/1` capacity outcome with visible stderr
  and exit 73 instead of terminating through an empty-stderr stack signal.
- Web packages now preserve semantic status domains for byte-range failures
  and checked i32, u8, and usize arithmetic. Generated JavaScript exposes the
  same frozen `{schema, domain_id, code}` observation for these failures as
  the interpreter and native lanes instead of mislabeling or bare trapping.
- Native builds now accept legal scalar self-comparisons such as `x == x`
  under the generated C warning policy, including the idiomatic floating-point
  NaN test shape, while retaining `-Werror` for actionable generator warnings.
- Fixed right-nested i32 arithmetic in the scalar Core-Wasm emitter by keeping
  the outer widened operand on the Wasm value stack until the nested operand
  finishes. Nested literal and function-call expressions now match native and
  interpreter results at multiple depths.
- Fixed the scalar Core-Wasm local layout so i32, u8, and usize arithmetic
  scratch locals follow function parameters instead of aliasing parameters or
  user `let` bindings. Parameterized narrow-integer programs now validate and
  agree with the reference interpreter.
- The CLI now rejects single-file semantic-patch no-ops and read-only sources
  before staging, admits Context byte budgets only from the viable 2048-byte
  envelope floor, and refuses `fmt` source, manifest, or project-directory
  symlink/reparse aliases with `SPX-J102`.

- Cleanup replay now validates long scalar checked-status sequences through
  bounded status-source/edge/exit summaries instead of cloning every failed
  path's successful prefix, eliminating quadratic replay memory and preflight
  rejection for large straight-line functions.

- Cleanup replay now recognizes decision-only CFGs with no cleanup state and
  validates them structurally without enumerating every lazy-boolean outcome,
  so ordinary long `&&`/`||` chains no longer hit the path budget.

- Source verification now skips lazy-branch snapshots for ordinary binary
  operators, tracks whether a scope has local borrows before liveness work,
  and indexes declared record fields once, removing quadratic rescans from
  wide scalar blocks and record literals.

- Formatter capacity accounting now measures all expression-subtree lengths
  during one canonical traversal instead of re-rendering every subtree, making
  revision hashing linear in expression size while preserving exact bytes.

- Cleanup replay's preflight now accounts for every constructor-field
  continuation, so wide record and variant literals no longer exhaust an
  underestimated per-function skeleton budget.

- Corrected the installed language card: `while` repetition is controlled by
  the re-evaluated condition, while the required body tail is discarded; the
  card now names the Copy-scalar and aggregate restrictions behind `SPX-T252`.
- Misspelled variant constructors and patterns now suggest the nearest unique
  case and preserve the nominal type after a bad constructor, suppressing the
  downstream unknown-binding and incompatible-match cascade.
- String ordering now emits only `SPX-T250`, without a duplicate numeric
  `SPX-T208`; aggregate equality in contracts now fails at the clause with
  located `SPX-T207` and scalar-field/match guidance before backend lowering.

- Nominal aggregate-valued `match` arms now fail during source verification
  with a located `SPX-T258` and an `if`/scalar-extraction remedy. `check`,
  native, and Wasm agree before unsupported record/variant lowering begins.

- Added a deterministic 128-level source-nesting limit. Deep balanced or
  truncated delimiters, unary chains, and expression trees now fail with a
  located `SPX-P207` and extraction help instead of overflowing the Rust
  runtime stack in front-end commands.

- Empty function identities now fail at source verification with located
  `SPX-S102` suffix guidance, and the 4,096-function byte-data capacity limit
  uses located `SPX-T270` with the bound and a module-splitting remedy instead
  of an unlocated internal replay diagnostic.

- `semaprax check` now resolves and validates HIR, cleanup plans, and replay
  budgets after source verification, matching the verdict used by `graph`,
  `run`, and both build targets instead of accepting backend-invalid source.

- Agent Context v2 now projects `while` statements in modern byte-data
  function bodies, so every declaration in `examples/text_analytics.spx` is
  available through the bounded context route when the full graph succeeds.

- Pinned source-verifier rejection of unsuffixed `i64` literals mixed with
  `usize`, `u8`, or `i32` arithmetic. `SPX-T208` now has explicit regression
  coverage for the suffix help before HIR or a backend can observe the input.

- Rejected a trailing semicolon after a block's value with the existing
  `SPX-P106` expression-statement diagnostic. Canonical formatting no longer
  accepts and silently removes that source token.
- Offline language/library help now shows the compiler-bundled dependency
  route and each `std.*` package's required consumer profile. `semaprax new`
  writes the extensible table scaffold and the calculator demonstrates a
  stable-ID import from `src/core.spx`. Table-manifest structural diagnostics
  report independent failures together and point to both scaffold routes.

- Project command operands now accept shell-natural `.` and `..` spellings,
  including `--manifest-path`, consistently across check, run, test, lock,
  build, and new-project verification. Manifest-declared source paths retain
  their strict no-alias rule.

- Scalar Project v1 now retains module-local record declarations through HIR
  replay, while aggregate function boundaries fail at their declaration with
  actionable `SPX-G174` guidance. The language card and generated project
  agent guide now state that boundary explicitly.

- Workspace declaration replay now retains classes as `Class` rather than
  `Record`, includes their owned methods in the independent fact map, and names
  the first differing stable identity in genuine `SPX-G173` disagreements.
  Class-bearing projects can therefore reach their ordinary execution lanes.

- Source byte-data capacity verification borrows the ordinary-function index
  instead of cloning its full `BTreeMap` once per function, and rejects the
  4,096-function bound at the start of declaration verification with the first
  excess declaration's source span.

- Canonical formatting now emits comment hooks for variant cases, variant
  payload fields, resource lifecycle items, and their closing braces. Comments
  in those bodies survive exactly once and formatting is a fixed point.

- `semaprax patch` rejects every surplus positional argument or unknown option
  with usage status 2 before reading or rewriting the source, matching the
  strict `impact` and `review` command grammars.

- `fmt --check` reports the first differing line and documents compact
  single-line `match` projection; `resolve` validates its target before
  dependency availability, and `new .` now explains that an explicit project
  name is required.
- CLI success and usage surfaces are now consistent for automation: successful
  `check --json` emits a verified envelope, `fmt --check` accepts flag-first
  order, `lock`/`resolve` print one recovery hint, and bare native-callable
  output names resolve against the current directory.
- `test` treats missing operands as domain failures rather than usage errors,
  `--max-steps` uses one closed usage-error range, and failed project summaries
  distinguish `main` from their consistently counted named cases.
- Added a replayable Agent Payment Graph and unified injected-host harness that
  compiles one AgentDefinition into Runtime v1, binds an independently admitted
  Economic Agent Policy, and carries a completed canonical Payment Intent into
  the existing approval/custody/broadcast/reconciliation state machine. The
  graph binds all three semantic digests and grants no model, wallet, signing,
  network, journal, approval, custody, or publication authority.
- Added exact project dependency inputs: scalar Package Manifest projects can
  replay and link a complete project-local Subject-v3 SEMAPRAX closure across
  check, test, run, analysis, and build routes, while generated Native Rust SDK
  packages carry exact Cargo versions/features and deterministic crate
  re-exports. An offline executable consumer now proves that a non-allowlisted
  crate can implement a typed `import rust fn` adapter and return its result
  through a SEMAPRAX export. Both routes remain bounded, authenticated, and
  free of implicit registry or network authority.
- Deepened four existing standard-library packages without widening ambient
  authority: `std.time` rounds durations upward and measures elapsed
  milliseconds, `std.path`
  exposes parent and extension boundaries, `std.data.csv` validates quote
  placement in complete records, and `std.data.toml` locates assignment
  delimiters outside simple quoted keys and comments. Each addition is covered
  by the package examples and conformance cases on every standard-library
  backend.
- Added `std.data.toml` as the fourth executable `portable`-tier package, with
  allocation-free bare-key validation, blank/comment line recognition, and
  first assignment-delimiter location over borrowed bytes. Full keys, values,
  tables, decoding, validation, and encoding remain Missing.
- Added `std.path` as the third executable `portable`-tier package, with
  allocation-free inspection of canonical slash-separated path bytes for
  absoluteness, trailing separators, nonempty segment counts, and filename
  position. Typed values, normalization, traversal policy, safe joining, and
  host-platform conversion remain Missing.
- Added the second executable `portable`-tier package, `std.url`, with ASCII
  scheme and unreserved-byte classification plus percent-triplet validation
  and decoding. The package depends on `std.encoding`; a consumer that names
  only `std.url` receives both bundled sources through deterministic transitive
  closure, with no acquisition authority.
- Added the first executable `test`-tier package, `std.test`, with scalar
  equality predicates and deterministic 0/1 or caller-selected failure status
  helpers for the current Project return-code test model. It is portable and
  compiler-bundled; rich diagnostics, fixtures, property tests, fuzzing, and
  snapshots remain Missing.
- Added the first executable `portable`-tier data-format package,
  `std.data.csv`, with allocation-free single-record field counting that
  respects quoted commas and escaped quotes, plus balanced-quote validation.
  It runs on every standard-library backend and is available through the
  closed compiler-bundled dependency inventory; streaming, typed fields,
  dialects, and writing remain Missing.
- Added the eighth executable `core`-tier package, `std.time`, with
  nonnegative millisecond conversion and decomposition, deadline comparison,
  remaining-duration calculation, and saturating addition. The package is
  effect-free and portable; clock reads, sleeps, timers, and other
  authority-bearing time operations remain explicitly Missing.
- Added the seventh executable `core`-tier package, `std.random`, with a pure
  Park–Miller generator, total seed normalization, bounded deterministic
  advancement, and range sampling. Secure randomness remains unavailable
  without a future explicit capability boundary; the deterministic package
  runs on the interpreter, native C11, and Core Wasm lanes and is included in
  the closed compiler-bundled dependency inventory.
- `semaprax test` names a `test_`-prefixed function that is not a case (it
  takes parameters, does not return `i64`, or has no explicit `@id`) with a
  `note:` line on stderr instead of skipping it silently; the JSON envelope is
  unchanged. The generated `AGENTS.md` now explains the test module and named
  cases, so the scaffold capsule digests change.
- `semaprax project-scaffold --layout tables` emits a new project whose
  `semaprax.toml` uses the extensible `semaprax.manifest.v1` table layout,
  under a new additive capsule schema `semaprax.project-scaffold.v3`
  ([Public Project Scaffold Capsule v3](docs/PROJECT-SCAFFOLD-V3.md)). The
  default (`--layout frozen`) is byte-for-byte the shipped v2 capsule with the
  frozen `semaprax.project.v1` manifest, so no existing scaffold byte or digest
  moves. The calculator's table layout adds a separate core module and imports
  its exported function by stable identity; both layouts lower to the same
  Project v1 contract. `tests/project.rs::scaffold` and `::scaffold_cli` pin it.

- Added the sixth executable `core`-tier package, `std.encoding`, with bounded
  ASCII-byte classification, hexadecimal value conversion, byte-pair decoding,
  lowercase/uppercase hex digit encoding, standard Base64 digit conversion,
  and unpadded four-digit decoding to a 24-bit value. Its contracted examples and
  conformance suite run on the interpreter, native C11, and Core Wasm lanes,
  and package manifests can import the compiler-bundled module at version
  `0.1.0` without acquisition authority.
- Extended `std.bytes` with length-aware suffix matching over borrowed byte
  slices, including empty, exact, proper-suffix, prefix-only, and longer-suffix
  conformance cases on every listed backend.
- Extended `std.text` with exact, length-aware borrowed UTF-8 equality and
  direct empty, Unicode, embedded-NUL, equal, and unequal interpreter,
  native-C11, and Core-Wasm assertions.
- Project manifests can now link the closed compiler-bundled `std.*` inventory
  at version `0.1.0` without cache, network, or acquisition authority. Exact,
  tilde, and caret ranges are checked, transitive standard dependencies are
  expanded deterministically, and one Project can link multiple packages by
  stable identity; ordinary resolved-package builds remain open.
- The Useful Text workspace boundary now links exact non-escaping `borrow str`
  providers across files, enabling the new partial `std.text` package with
  byte-length, emptiness, prefix, and substring operations across interpreter,
  native C11, and Core Wasm conformance lanes.
- The full toolchain's held-parent `new` authority now admits the exact
  six-file library scaffold as well as the calculator, preserving fixed
  inventories, create-new writes, no-replace publication, and post-publication
  authentication.

- `semaprax lock` and `semaprax resolve` accept `[<dir>|semaprax.toml]`,
  resolving directory arguments to `<dir>/semaprax.toml` and defaulting to
  `./semaprax.toml` when omitted, matching `check`, `fmt`, `run`, `test`, and
  `build`. Outside a project without arguments, both commands attach the
  standard missing-manifest guidance hint. `tests/project.rs::project_lock_v1`
  and `tests/project.rs::dependency_resolution_v1` pin the ergonomics.

- Canonical project comments now remain valid inputs to Project semantic-graph
  construction and persisted frontend-cache replay, while the parsed semantic
  program remains comment-free. Project-format and install-guide fixtures use
  native path spelling on every host and explicitly create the required lock
  before executing a freshly formatted project.

- `semaprax lock` gains `--emit-interface` and `--compare-interface
  <baseline.json>`: a fine-grained per-export compatibility for Project v1
  scalar packages. `--emit-interface` prints the scalar WIT interface
  descriptor to store as a baseline; `--compare-interface` diffs the current
  project against it and names exactly which export was added or removed, or had
  its parameter or result type change, exiting nonzero on a breaking change. It
  is purely additive and does not touch the `semaprax.lock` format; the coarse
  `--compare` stays. `tests/project.rs::project_lock_v1` and the
  `scalar_wit_compare` unit tests pin it.


- The `useful-data.v1` byte-data profile admits `requires` and `ensures`
  contracts throughout its function inventory. The Core-Wasm data emitter
  walks contracts with bodies and lets a false contract publish status 9 or
  10 through the data status global, the generated facade already maps those
  to `SemapraxDataError` in the `semaprax.contract.v1` domain, and the npm
  semantic recipe renders contract lines so replay binds them byte for byte.
  Functions with effects are still rejected. `std.bytes` ships on the profile
  with byte conversion, guarded indexing, search, counting, ASCII
  classification, equality and prefix tests, and endian reads, all
  contracted and run on every lane; the catalogs are regenerated.
- `semaprax test` runs every `fn test_<name>() -> i64` of the manifest-declared
  test module as a named case after `main`, each with its own step budget, and
  names each failing case with its outcome (`failed
  calculator.tests.test_add: returned 2`) followed by `project tests failed: K
  of N in <module>` and a `help` line; a failing `main` is reported the same
  way instead of the bare `project tests failed with result N`. The
  `semaprax.project-execution.v1` test envelope gains an always-present
  additive `cases` array under the unchanged schema string and payload-digest
  domain, and `project::verify_execution_envelope` verifies it. Entry envelopes
  and the passing-test line without cases are unchanged.
  [Project Test Cases v1](docs/PROJECT-TEST-CASES-V1.md) owns the rule and
  `tests/project.rs::developer_loop` pins it.

- A violated `requires` or `ensures` under `semaprax run` or `semaprax test`
  now names the failing function's stable id, the clause kind and source text,
  and the call's argument values, both as two indented lines after the
  unchanged language-status line and as an additive `failure` member of the
  `language_failure` outcome. The interpreter records the detail at the failing
  frame (`src/interpreter/failure_detail.rs`); the status object, cleanup, and
  exit status are untouched, and the native path is unchanged. The legacy
  resolved-entry evaluator moved verbatim into `src/interpreter/resolved_case.rs`
  so a named function can be evaluated without being the entrypoint.

- `semaprax check` on a project whose `use` names a module no listed source
  declares keeps `SPX-G172` and its message and adds a `help` line: it names
  the unlisted `.spx` file that declares the module and the `sources` key in
  `semaprax.toml`, or says that no listed file declares the module. The hint is
  added by the project loader in `src/project/source_hint.rs`, so human and
  JSON diagnostics agree; [Project Manifest v1](docs/PROJECT-MANIFEST-V1.md)
  owns the rule and `tests/project_cli_v1.rs` pins it.
- `scripts/quality.sh changed` routes CLI and editor changes narrowly. Paths
  under `src/cli/` and `src/bin/`, plus `src/cli_driver.rs` and
  `src/main.rs`, classify as `cli-surface` and append a `test-cli` gate that
  runs the CLI harnesses of the standalone package and the full toolchain;
  paths under `editors/` classify as `editor-adapter` and append `test-editor`,
  which runs the extension's `node --test` suite and the documentation
  harness. Both follow the fixed `changed` gates in one order, and the
  executor rejects a repeated or reordered surface gate. Other paths route as
  before, `full`'s gate list is unchanged, and the plan schema stays
  `semaprax.quality-route.v2`; `tests/quality_routing.rs` pins the routes and
  the executor's dispatch.

- `AGENTS.md` forbids pointing a worktree's Cargo `target-dir` at another
  worktree or at any path a different checkout's tests depend on, and the
  development guide gives the private `CARGO_TARGET_DIR`,
  `CARGO_INCREMENTAL=0`, and `CARGO_PROFILE_TEST_DEBUG=0` setup with the disk
  a full gate needs.

- The VS Code extension checks on save. Saving a `.spx` file or
  `semaprax.toml` runs the user-selected `semaprax.compilerPath` binary as
  `check <nearest semaprax.toml or file> --json`, maps each JSON diagnostic to
  an editor diagnostic (`code: message` plus the help on a new line, range
  from the reported line and column), and clears entries the re-check no
  longer reports. `SEMAPRAX: Check Project` runs the same check explicitly and
  names the setting to fill when none is set; the machine setting
  `semaprax.checkOnSave` (default `true`) turns the save trigger off. The
  child is spawned without a shell, capped at 4 MiB of output and 30 seconds,
  and writes nothing. Activation adds `onLanguage:semaprax` and the new
  command; `editors/vscode/diagnostics.js` holds the pure logic and
  `test/diagnostics.test.js` covers it.
- The `SPX-J121` build rejection for a manifest with `[dependencies]` now points
  at `semaprax resolve --write` to resolve and pin the dependency graph, instead
  of claiming no resolution route exists; only a build that links resolved
  dependencies is still unimplemented. The agent quick reference gains a short
  locking-and-dependencies note covering `lock` and `resolve`.

- `semaprax resolve` gains `--write` and `--verify`: `--write` pins the
  resolution evidence to `semaprax.resolution-<target>.json` beside the
  manifest, and `--verify` re-resolves and confirms that pin still holds byte
  for byte, failing closed with `SPX-J126` when the cache no longer produces the
  recorded selection. Because resolution is deterministic the pin is a stable
  per-target dependency lockfile a CI job can check.
  `tests/project.rs::dependency_resolution_v1` pins it.

- `semaprax lock <manifest> --compare <baseline.lock>` classifies a project's
  current interface against a baseline `semaprax.lock` and prints a
  `semaprax.project-lock-compatibility.v1` verdict, exiting nonzero on a
  breaking change so a CI gate fails. A removed export, a widened required
  capability, a removed target, a changed package name or contract, or a
  changed interface descriptor digest with the same export set are breaking; an
  added export, a narrowed capability, an added target are not; a pure display
  rename or a version-only change is not. It is a coarse project-level
  counterpart to the offline Compatibility Evidence; `tests/project.rs::project_lock_v1`
  pins it.

- `semaprax resolve <manifest> --target <native64|wasm32> --cache <dir>`
  resolves a project's `[dependencies]` against a local content-addressed cache
  of Subject-v3 envelopes and prints the offline resolver's evidence
  ([Project Dependency Resolution v1](docs/PROJECT-DEPENDENCY-RESOLUTION-V1.md)).
  It selects one version per package that satisfies the manifest ranges and
  their transitive requirements, deterministically and per target, reading the
  cache as an explicit effect with no registry, acquisition, or build. Cache
  files are named by their subject digest, so a misfiled subject is rejected;
  `SPX-J126` covers missing dependencies, a target outside the matrix, and
  cache faults. Manifest dependency names are now dotted lowercase package
  identities matching the resolver, so `examples.meaning` is admitted;
  `tests/project.rs::dependency_resolution_v1` is the gate.

- Cross-platform CI now preserves the exact verifier hint while accepting
  native line endings in CLI help, and deep standalone-String Wasm planning
  uses narrow recursive walkers so the contracted nesting depth fits the
  default macOS test stack. Supply-chain command dispatch also moved into its
  audited submodule to restore the CLI driver's source-size budget.

- The help catalog test's dispatcher inventory lists the `lock` command that
  landed with the deterministic `semaprax.lock`, so the catalog and dispatcher
  closure check passes again in both executables.

- `semaprax help library` prints the generated standard-library catalog, the
  fourth `help` shape beside `help language`, so an installed compiler lists
  every `std.*` function and contract offline.
- Project-shape rejections now carry their fix: a Project v1 manifest whose six
  lines are missing, extra, or unterminated lists the exact lines in order, an
  `entry` that names a module without `main` explains the `entry` key, and an
  unknown function with no near name shows the `use function @id(…) from
  module as name;` import line. `tests/project/manifest_hints.rs` pins the
  manifest and CLI cases.
- `semaprax fmt` accepts a project directory or `semaprax.toml`, like `check`,
  `run`, `test`, and `build`: it formats every `sources` entry in manifest
  order through the comment-preserving projection, parses every file before
  writing any, and with `--check` prints one `<path> is not canonically
  formatted` line per drifting file. Previously `fmt .` failed with `cannot
  read .: Is a directory`. `tests/projections/fmt_comments.rs` and
  `tests/project_cli_v1.rs` pin it.

- `semaprax new --template library` creates the scaffold's library template
  in the standalone compiler; the success line is now `created <template>
  project <destination>`, and an unknown template is rejected with `expected
  calculator or library`. The full toolchain's `new` still publishes only the
  calculator inventory and refuses `library` with a message naming the
  standalone route. `tests/project/new_cli.rs` pins the library project's
  bytes and that it checks, tests, runs, and is canonical.

- `semaprax patch`, `patch-with-evidence`, and `patch-with-evidence-v2` keep
  the file's `//` comments: the patched candidate is parsed with its comments
  and rendered through the same projection as `fmt`, so a comment above a
  renamed function stays above it and the patched file passes `fmt --check`.
  Graph revisions and evidence digests are unchanged. [Canonical comments
  v1](docs/CANONICAL-COMMENTS-V1.md) lists the preserved routes;
  `tests/semantic/patch.rs` pins the regression.

- `semaprax lock <manifest>` renders the deterministic `semaprax.lock` beside a
  project ([Project Lock v1](docs/PROJECT-LOCK-V1.md)): the canonical manifest
  and its contract, the project revision as the program root, every source
  file's revision and digest, the retained interface descriptor digest, the
  declared target matrix, required capabilities, the compiler, and the
  resolution policy. `--write` persists it atomically and `--verify` re-renders
  and compares bytes, failing closed with `SPX-J123` when a source, manifest,
  or compiler drifts and naming the drifted fields. Like every package
  operation the lock is explicit and never touched by `check`;
  `tests/project.rs::project_lock_v1` is the gate.

- `semaprax fmt` keeps `//` comments. The lexer records each comment's
  position, and the canonical formatter prints it above the item it preceded
  or right after the item it followed, at that item's depth; formatting is
  idempotent and a comment-free file formats to the same bytes as before.
  Only `fmt` restores comments; `patch` and other rewriting transactions still
  emit comment-free text. [Canonical comments v1](docs/CANONICAL-COMMENTS-V1.md)
  owns the placement rules, `src/format/comments.rs` and
  `tests/projections/fmt_comments.rs` pin them, and the formatter's capacity
  accounting moved verbatim into `src/format/capacity.rs`.

- Every generated project now carries an `AGENTS.md`: the commands to check,
  test, run, format, and build it, and the rules that differ from other
  languages, written for coding agents and people alike. The scaffold capsule
  becomes `semaprax.project-scaffold.v2` with five files and a new digest
  domain, the generated `README.md` points at `AGENTS.md` and uses directory
  operands, and both `new` routes publish the same five files. The library
  template gains the same file. The `_v1` API names are unchanged; the schema
  string and digest domain are v2.
  [Public Project Scaffold Capsule v2](docs/PROJECT-SCAFFOLD-V2.md) owns the
  contract.

- `semaprax new <destination>` now works on the standalone compiler. It
  derives the same calculator template as the full toolchain, refuses anything
  but a fresh destination under an existing real directory, writes every file
  with create-new semantics, reads the files back, authenticates the project,
  and prints the same success line. The full toolchain keeps its held-parent
  staged publication; [Standalone project creation v1](docs/NEW-PROJECT-STANDALONE-V1.md)
  owns the bounded route and its non-claims, `tests/project/new_cli.rs` pins
  it, and the install guide, quickstart, and README no longer require the
  full toolchain to create a project.
- `semaprax.toml` gains one extensible table layout, `semaprax.manifest.v1`
  ([Package Manifest v1](docs/PACKAGE-MANIFEST-V1.md)): `[package]`,
  `[modules]`, `[exports]`, `[command]`, `[capabilities]`, `[dependencies]`,
  and `[targets]` tables lower onto the frozen Project v1-v11 profile
  contracts, so every project route, descriptor, and generated artifact is
  unchanged and only the manifest bytes differ. Reserved and unknown tables or
  keys reject with `SPX-J120`, a declared dependency fails every build closed
  with `SPX-J121`, a build target outside `[targets] matrix` rejects with
  `SPX-J122`, and a non-canonical manifest names its first differing line.
  The frozen layouts remain admitted byte-for-byte;
  `tests/project.rs::package_manifest_v1` is the gate.

- The standard-library contract and the agent quick reference explain how a
  Project consumes a `std.*` module today, by vendoring its library file and
  importing by `@id`; the gate vendors every package into a fresh project and
  runs its examples and conformance there.

- `std.num` gains checked `pow`, `isqrt`, `digit_count`, `is_power_of_two`,
  `log2_floor`, and `log10_floor`, each with contracts and conformance checks
  on every lane; the catalogs are regenerated.

- `semaprax project-scaffold --template library` prints a library package in
  the standard-library shape: `src/lib.spx` with one contracted function,
  `src/examples.spx` as the entry, and `src/tests.spx` as the conformance
  suite, all checked and tested at derivation and replayable only as the
  library template. The calculator capsule bytes are unchanged, and the
  private `new` still admits only the calculator inventory.

- Added the standard-library contract, [Standard Library v1](docs/STANDARD-LIBRARY-V1.md),
  and its first `core`-tier packages under `std/`: `std.core` (ordering as
  `-1`/`0`/`1`, extrema, clamping, range membership, `bool` conversions and
  connectives), `std.num` (sign, absolute value, parity, Euclidean division and
  remainder, greatest common divisor), and `std.num.overflow` (overflow
  predicates and wrapping and saturating arithmetic). Each package is a
  Project whose entry is its examples module and whose test module is its
  conformance suite; `tests/project.rs::standard_library` runs both on the
  interpreter, native C11 at O0/O2, and Core Wasm under Node, checks
  identities and conformance coverage, and generates the human
  [catalog](docs/STANDARD-LIBRARY-CATALOG.md) and `std/catalog.json`.

- The Workspace Semantic Graph builder pre-bound now charges structural bytes,
  string contents, and per-shape identity slots separately instead of the
  `Try` and string rates for every node. The 16 MiB budget and every reported
  field are unchanged, but `used_builder_bytes` values move, so the frozen
  known-answer digests in the workspace change, operations, structural-change,
  and analysis tests, the workspace CLI harness, and the browser fixture's
  `project_graph_digest` answers were re-pinned. Before the split a 4.9 KiB
  module of twenty scalar functions was rejected with `SPX-G171`; the `std/`
  packages are the regression.

- The workspace CLI tests that assert exact command usages now read them from
  `semaprax help all`, the exhaustive catalog, instead of the guided one-screen
  `--help` page that replaced it.
- `check`, `run`, `test`, and `build` invoked with no input outside a project
  now attach a hint to the unchanged `SPX-J102` missing-manifest diagnostic
  naming the three admitted inputs: a `.spx` file, a project directory, or
  running from inside a project. An explicitly named manifest is unchanged.

- `semaprax help language` prints the compiler-checked agent quick reference
  byte for byte from the installed binary, so an agent or developer without the
  source checkout can read the admitted shapes, the diagnostics foreign habits
  trigger, and their fixes offline. Both help harnesses pin the bytes and the
  guided page lists the form.

- The VS Code extension now contributes the `.spx` language declaratively:
  a TextMate grammar and language configuration give highlighting, `//`
  comment toggling, bracket matching, and auto-closing pairs without starting a
  session or running code. The documentation gate checks that the grammar
  names every keyword the parser recognises and every literal suffix.

- Checking one module of a multi-file project on its own now explains the
  next step: `SPX-G172` (the module imports other modules) and `SPX-T105`
  (the module has no `main`) keep their codes and messages and gain a hint
  naming `semaprax check <project-dir>`.

- `check`, `run`, `test`, and `build` accept a project directory as their
  positional operand and select the `semaprax.toml` inside it, so
  `semaprax check examples/calculator-project` and `semaprax run .` work
  without naming the manifest. Inert `.` components are removed before the
  manifest is authenticated; `--manifest-path` is still taken literally, and a
  directory without a manifest reports the ordinary `SPX-J102` for that path
  instead of an unreadable directory. Scoped help and the catalog show `<dir>`.

- A source file that does not start with its `module` line now carries a fix
  hint under the unchanged `SPX-P104` ``expected `module` `` diagnostic,
  naming the `module dotted.name;` header a pasted or truncated file is
  missing.

- `semaprax --help`, `help`, `-h`, and the empty invocation now print a guided
  one-screen overview: the commands for writing, checking, running, inspecting,
  and changing programs, grouped by task with a one-line purpose each, bounded
  to 2048 bytes and filtered by the executable's capability class. The former
  7 KB exhaustive page moved to the new `semaprax help all` form with its bytes
  otherwise unchanged; scoped help, typo guidance, and recovery hints are
  unchanged. [Guided CLI Help v4](docs/CLI-HELP-V4.md) owns the contract and
  the standalone and full-toolchain help harnesses pin it.
- A borrowed view taken directly from a string literal, an array literal, or a
  call result (`str_as_bytes("hi")`, `array_as_slice([1u8])`) now names the
  `let` binding step in its `SPX-T266` help, and the README routes coding agents
  to the agent quick reference.

- More first-attempt habits now carry their fix: `struct`/`enum`/`pub`/`const`
  declarations, a missing trailing `,` after the last field or match arm,
  `x += 1`, a missing `->` result type or a `()` unit type, an uninitialised
  `let`, `=` in an `if` condition, `a[0]` indexing, `Some(1)`/`None` shorthand,
  a method call on a `string`, byte, record, or variant value, and foreign type
  names such as `String`, `int`, `double`, `boolean`, or `Vec`. Codes and
  messages are unchanged; `tests/language/foreign_syntax_hints.rs` and
  `tests/language/verifier_hints.rs` pin each case. Type syntax parsing moved
  verbatim into `src/parser/types.rs` to keep the grammar root under budget.

- An owned `string` or byte value passed to a user function's `borrow str` or
  `borrow Slice<u8>` parameter now names the view conversion in its `SPX-T205`
  help, the parser's expression-statement hint shows the admitted `let _ = …;`
  discard, and the agent quick reference explains that a failing project test
  reports only its return value.

- Verifier diagnostics now carry fix hints for the type-level habits an agent
  brings from other languages: an unknown function names the nearest declared
  or compiler-owned function (`did you mean `string_len`?`) or, for the
  print family, the one admitted output route; a generic call or generic type
  without explicit type arguments shows the `id<i64>(…)` and
  `Option<i64>::Some { value: … }` shapes; an unsuffixed integer literal against
  a `usize`, `i32`, or `u8` operand names the suffix to write; and an owned
  `string` handed to `str_as_bytes`, a byte operation, or `stdout_write` names
  the `string_as_str`/`str_as_bytes` conversion. Codes, messages, and spans are
  unchanged, and the iterative verifier and the test-only oracle share the
  helpers in `src/source_verify/hints.rs`; `tests/language/verifier_hints.rs`
  pins each case and the no-hint baselines.
- Fixed the stale descendant expectation in
  `tests/language/generic_records.rs`. Admitting concrete generic owned
  variants made `Maybe<Bytes>` a legal standalone owned value, so the
  descendant of `own Box<Maybe<Bytes>>` stopped reporting `SPX-T268` on its
  own. The shape itself stayed closed throughout - the record carrier reports
  `SPX-T223` in source verification and resolved-HIR classification returns
  `OutsideProfile`, matching the identical case already pinned in
  `tests/owned_data/nested_generic_owned_record_frontend_hir.rs` - so no
  admission changed. The test now asserts carrier closure, pins the standalone
  variant admission that explains the descendant's silence, and adds two
  descendant cases that must still report `SPX-T268`: a variant without a
  persistent `@id` under the same carrier, and an admitted owned variant stored
  as a declared record field.

## Earlier releases

The detailed 0.3.5, 0.2.0, and 0.1.0 history is preserved in the [changelog archive](docs/CHANGELOG-ARCHIVE.md).
