# TeamDesk webapp-v2 matched campaign

`codex_campaign.py` is the matched Codex harness for the TeamDesk Enterprise
benchmark. It exposes only `SPEC.md` and the launch contract to each isolated
agent seed. The acceptance runner remains in the harness checkout and requires
the candidate to pass all 912 independent obligations before an attempt is
accepted.

## Optional pinned TypeScript tooling for future campaigns

New campaigns may opt into `--typescript-bootstrap-receipt /absolute/path/receipt.json`
on both `plan` and `run`. Without this flag the original matched prompts and
dependency provisioning remain unchanged. This protocol does not retrofit,
retry, or exclude any round9 trial; its offline installation failures and costs
remain evidence about that campaign's provisioning, not intrinsic language cost.

Prepare a new bundle from an already retained, locked, 912/912 TypeScript
reference dependency tree, without npm installation or cache reconstruction:

```sh
python3 benchmarks/webapp-tokens-v2/typescript_bootstrap.py \
  --reference /absolute/path/to/qualified/typescript \
  --qualification-summary /absolute/path/to/reference-summary.json \
  --node-binary /absolute/path/to/qualified/node \
  --destination /absolute/path/to/new-dependency-tooling
```

The receipt binds the reference package and lock hashes to its qualified report,
the installed lock versions, current dependency bytes, executable modes and
internal relative links, both bootstrap/copy module hashes, and exact Node binary,
version set, architecture and host platform. It supplies only `node_modules`.
It records the inspected dependency tree's present inventory; it does not claim
historical dependency byte identity from the older qualification report.
The reference package scripts, lockfile, application, server, UI and tests are
never copied into a candidate. Agents author their own package.json,
configuration, scripts and complete implementation.

The retained qualified toolchain currently contains React/React DOM 18.3.1,
React Router DOM 6.30.6, TypeScript 5.9.3, Vite 5.4.21, React Vite plugin 4.7.0,
Node types 22.20.5, React types 18.3.31 and React DOM types 18.3.7.
The prompt names the exact receipt versions and local `node_modules/.bin/tsc`
and `node_modules/.bin/vite`; it requires no npm metadata or network fetch.
The independent acceptance browser remains the separate pinned harness tool.

Before any paid TypeScript request, the harness rechecks runtime, platform,
qualification, receipt and dependency hashes, copies the closed tree into a
private writable candidate, rechecks source and copy, and repeats disk admission
at the same 5 GiB floor. It never hardlinks or links to a cache or reference.
Each serial attempt has one private copy; normal workspace cleanup removes it.
The retained ready tree is approximately 72 MiB, so budget for both the immutable
bundle and the active private copy before launch, separately from the floor.
Candidate edits to its own dependencies are allowed and cannot modify the
bundle. Dependency directories remain excluded from authored-source proxies.

Campaign and trial records retain supplied-tool receipt/copy fingerprints and
provisioning wall time,
module hashes, package versions and application/manifest-not-supplied flags.
They record fixed prompt byte counts and the tooling paragraph hash separately.
These are supplied harness context, not verified authored output or hidden
request-token composition: stable/hidden context tokens remain null, calibration
is separate and is never subtracted, and all observed request usage and costs
are retained. No savings or comparative result is promised by this provisioning
change. Actual sandbox bootstrap checks and the new regression cases must pass
in the final verification batch before a future campaign uses it.

The campaign is pinned to five trials per arm, alternating SEMAPRAX and
TypeScript, with `gpt-6.1-sol` at medium effort. Calibration is a separate
record and is never subtracted from trial accounting. Every attempted trial,
including failed or rejected attempts, remains in `results.json` with its
transcript, task-owned rollout trace, conditional list-price estimate, and
candidate archive. The summary reports raw and legacy-net input, authored
source, conditional estimated cost per accepted task, and agent and acceptance
wall time separately. Provider billing receipts are never inferred.

Plan without a model request:

`plan` and `run` accept `--round N` as a positive integer campaign identity;
it defaults to `1` and is recorded in `campaign.json`, `results.json`, and the
final command summary. Use a fresh artifact path for each round.

```sh
python3 benchmarks/webapp-tokens-v2/codex_campaign.py plan \
  --round 3 \
  --base-ref 3fcdf034a5e5da57c10845a8074c06f2862816e1 \
  --compiler-source-ref aae2719e29df37438e55bf52b00da3d7954bbd7f \
  --semaprax-bin /absolute/path/to/semaprax \
  --tokenizer-dir /absolute/path/to/tokenizer-prefix \
  --playwright-root /absolute/path/with-pinned-playwright \
  --artifacts /absolute/path/to/new-artifacts \
  --model gpt-6.1-sol --effort medium --trials-per-arm 5 \
  --timeout-seconds 1800
```

After reviewing the plan and supplying the exact compiler binary, the live
command is the same invocation with `run` and
`--acknowledge-paid-attempts`. The live command was not run while developing
this harness.

The acceptance gate uses Node 24+, the pinned Playwright 1.62.0 Chromium, a
fresh evidence directory for every attempt, and loopback-only application
servers. The retained r8 reference receipt is checked for both arms at 912/912.
Its SEMAPRAX reference was compiled from source
`e045527a611a048515349ab2f970043a5d33b185`; this is qualification evidence,
never a live-agent result. Before a paid request, the harness verifies the local
Codex controls, Node, Playwright package, and Chromium executable. It snapshots
the full transitive acceptance source closure and runs that snapshot. Seed
hashes and bytes come from `--base-ref`; candidate writes are confined to the
new candidate root and rechecked before acceptance, archival, and cleanup.

## Separate rescoring

`codex_rescore.py` is an offline follow-up only after a terminal receipt binds
a finalized ten-trial campaign. A fully launched campaign that is marked interrupted for retained resource contamination may be separately rescored only when its terminal receipt binds that interruption, every trial identity, an empty unlaunched order, and the actual process exit; the original results and contamination assessment remain archival evidence. It requires the original results and
artifact hashes, the exact compiler binary/source SHA, a newly qualified gate
source and receipt, a clarification document, and a fresh output path. It
copies each closed candidate archive into a separate writable directory and
writes a versioned sidecar; it never rewrites original results, costs,
transcripts, prompts, or candidate archives. `--validate-sidecar` rechecks the
original result hash, new gate hashes, clarification hash, and all ten unique
arm/number identities. This sidecar is new acceptance evidence, not a claim
that the frozen original gate or paid wall time changed.

Reference qualification and archival rescoring use the same minimal immutable
runner snapshot: frozen SPEC plus the twelve acceptance runtime files. The
acceptance CONTRACT prose is stored separately as qualified, hash-bound
metadata so it cannot change the runner inventory reported by the gate.

Offline rescoring accepts `--jobs 1` (default) or `--jobs 2`, with isolated
candidate/evidence directories and original trial order retained. Its worker
count and elapsed rescore time are separate from original agent and scoring
wall times.

The browser gate recognizes enum filters by an exact field-labelled control or
the legacy field-qualified clear option. It still checks every enum value
against displayed rows and resets the selected filter; a generic unrelated
`All` control is not accepted.
Permission actions may be links or buttons with one exact accessible action name. Form locators accept admitted field-label prefixes on native input, select, or textarea controls so navigation labels cannot collide with fields.

TypeScript replay that needs excluded dependencies requires an explicit
`--dependency-receipt` using `semaprax.rescore.dependencies.v1`. It binds
each `typescript-01` through `typescript-05` bundle to the archived
`package.json`, optional lockfile, and a closed hash/mode/link inventory
limited to `node_modules` and `.cache`. The rescorer copies that inventory
into the separate candidate, records the receipt and copy fingerprints, and
rechecks both bundle and source inventories. A missing bundle is unscorable
offline infrastructure, not an application rejection; recovery provenance and
an explicit false historical-byte-identity field do not claim original-runtime
equivalence.

The dependency inventory/copy contract is shared in `dependency_bundle.py`;
existing rescore receipts keep their original semantics. Newly created rescore
sidecars also retain a hash-bound snapshot of that module beside the gate.
Older sidecars do not gain a retrospective provenance claim.

`authored_source_recount.py` produces a separate, hash-bound
`semaprax.authored-source-recount.v1` sidecar from existing measured
per-file proxy metrics. It partitions final inventory tokens into authored
source, proven generated output, dependency locks, and unresolved files.
Generated exclusions need closed recipe/entrypoint chains ending in classified
authored source. The output retains the legacy total and tokenizer binding and
sets both authorship verification and ratio eligibility to false.
