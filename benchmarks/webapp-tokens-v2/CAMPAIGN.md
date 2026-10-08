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
  --round 2 \
  --base-ref c50c3bd7623354840504687d64260edb8ff3895f \
  --compiler-source-ref 60002439e1b651ebbfbf3a887f12a20c927d720f \
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
servers. The retained r6 reference receipt is checked for both arms at 912/912.
Its SEMAPRAX reference was compiled from source
`60002439e1b651ebbfbf3a887f12a20c927d720f`; this is qualification evidence,
never a live-agent result. Before a paid request, the harness verifies the local
Codex controls, Node, Playwright package, and Chromium executable. It snapshots
the full transitive acceptance source closure and runs that snapshot. Seed
hashes and bytes come from `--base-ref`; candidate writes are confined to the
new candidate root and rechecked before acceptance, archival, and cleanup.
