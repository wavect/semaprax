# Exact next commands (prepared, not executed)

Status: retained evidence; the recorded scope and qualification below apply.

Audience: compiler contributors, tool authors, and reviewers.

Set these inputs to actual reviewed artifacts. Do not relabel older qualification. Run one campaign at a time. The compiled source ref must name the commit that actually produced the selected binary; the seed ref must preserve the frozen public inputs. Use absolute receipt-bound runtime/tool paths and fresh artifact destinations outside the repository.

Required variables: `BENCH_REPO`, `BENCH_SEED_REF`, `BENCH_COMPILER_REF`, `BENCH_COMPILER`, `BENCH_CODEX`, `BENCH_TOKENIZER`, `BENCH_NODE`, `BENCH_NPM`, `BENCH_PLAYWRIGHT`, `BENCH_WEB_TS_RECEIPT`, `BENCH_CLI_TS_RECEIPT`, `BENCH_SHIFT_TS_RECEIPT`, `BENCH_CLI_BUILD_RECEIPT`, `BENCH_SHIFT_V6_EVIDENCE`, `BENCH_WEB_ARTIFACTS`, `BENCH_CLI_ARTIFACTS`, `BENCH_SHIFT_ARTIFACTS`.

The web adapter additionally requires the actual current both-arm912/912 producer receipt at `benchmarks/webapp-tokens-v2/acceptance/evidence/reference-r8-summary.json`, bound to the exact compiler source and binary. This audit does not update it. Current CLI full49 and ShiftSim all15 qualification are likewise unexecuted here.

Prepare each plan first; these commands have not been run:

```sh
python3 "$BENCH_REPO/benchmarks/webapp-tokens-v2/codex_campaign.py" plan \
  --repo "$BENCH_REPO" --round 10 --base-ref "$BENCH_SEED_REF" \
  --compiler-source-ref "$BENCH_COMPILER_REF" --semaprax-bin "$BENCH_COMPILER" \
  --codex-binary "$BENCH_CODEX" --tokenizer-dir "$BENCH_TOKENIZER" \
  --model gpt-6.1-sol --effort medium --trials-per-arm 5 --timeout-seconds 1800 \
  --node-binary "$BENCH_NODE" --playwright-root "$BENCH_PLAYWRIGHT" \
  --typescript-bootstrap-receipt "$BENCH_WEB_TS_RECEIPT" --artifacts "$BENCH_WEB_ARTIFACTS"

python3 "$BENCH_REPO/benchmarks/cli-tokens-v1/codex_campaign.py" plan \
  --repo "$BENCH_REPO" --base-ref "$BENCH_SEED_REF" \
  --compiler-source-ref "$BENCH_COMPILER_REF" --semaprax-bin "$BENCH_COMPILER" \
  --compiler-build-receipt "$BENCH_CLI_BUILD_RECEIPT" --selected-file-resource-v1 \
  --codex-binary "$BENCH_CODEX" --tokenizer-dir "$BENCH_TOKENIZER" \
  --model gpt-6.1-sol --effort medium --trials-per-arm 5 --timeout-seconds 1800 \
  --node-binary "$BENCH_NODE" --npm-binary "$BENCH_NPM" \
  --typescript-bootstrap-receipt "$BENCH_CLI_TS_RECEIPT" --artifacts "$BENCH_CLI_ARTIFACTS"

python3 "$BENCH_REPO/benchmarks/event-sim-tokens-v1/codex_campaign.py" plan \
  --repo "$BENCH_REPO" --round 6 --authoring-profile semaprax-project-v31-collection-record-v1 \
  --base-ref "$BENCH_SEED_REF" --compiler-source-ref "$BENCH_COMPILER_REF" \
  --semaprax-bin "$BENCH_COMPILER" --qualification-evidence "$BENCH_SHIFT_V6_EVIDENCE" \
  --codex-binary "$BENCH_CODEX" --tokenizer-dir "$BENCH_TOKENIZER" \
  --model gpt-6.1-sol --effort medium --trials-per-arm 5 --timeout-seconds 1800 \
  --node-binary "$BENCH_NODE" --npm-binary "$BENCH_NPM" \
  --typescript-bootstrap-receipt "$BENCH_SHIFT_TS_RECEIPT" --artifacts "$BENCH_SHIFT_ARTIFACTS"
```

Only after the plans and actual qualification succeed, use the identical arguments with `plan` replaced by `run` and append `--acknowledge-paid-attempts`. This preserves five attempts per arm, all failures, the complete original acceptance gates, immutable input/receipt bindings and source capture. Do not add `--max-budget-usd`: the Codex adapters explicitly refuse a purported strict monetary cap that the CLI cannot enforce.
