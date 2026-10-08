# TeamDesk webapp-v2 matched campaign

`codex_campaign.py` is the matched Codex harness for the TeamDesk Enterprise
benchmark. It exposes only `SPEC.md` and the launch contract to each isolated
agent seed. The acceptance runner remains in the harness checkout and requires
the candidate to pass all 912 independent obligations before an attempt is
accepted.

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

`authored_source_recount.py` produces a separate, hash-bound
`semaprax.authored-source-recount.v1` sidecar from existing measured
per-file proxy metrics. It partitions final inventory tokens into authored
source, proven generated output, dependency locks, and unresolved files.
Generated exclusions need closed recipe/entrypoint chains ending in classified
authored source. The output retains the legacy total and tokenizer binding and
sets both authorship verification and ratio eligibility to false.
