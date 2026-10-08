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
candidate archive. Provider billing receipts are never inferred.

Plan without a model request:

```sh
python3 benchmarks/webapp-tokens-v2/codex_campaign.py plan \
  --base-ref 3660e0daf9335dc9d6dc27949a43cd40dff7326c \
  --compiler-source-ref 3660e0daf9335dc9d6dc27949a43cd40dff7326c \
  --semaprax-bin /absolute/path/to/semaprax \
  --tokenizer-dir /absolute/path/to/tokenizer-prefix \
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
servers. The qualified reference receipt is retained and checked for both
arms at 912/912; it is evidence for qualification only and is never treated
as a live-agent result.
