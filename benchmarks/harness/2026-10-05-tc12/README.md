# TC-12 paid cost-profile qualification campaign, 2026-10-05

Status: retained local evidence (macOS aarch64), explicitly authorized remote spend (USD 5.00 hard cap).
Verdict: **inconclusive for every arm; defaults unchanged** (`qualification.json`, `leave-defaults-unchanged`).

## What ran
- Model: `claude-haiku-4-5` through Claude Code CLI 2.1.289 (`claude -p --model haiku`, no tools, thinking off,
  system prompt replaced, empty cwd) behind `shim.py` (copy of the HN-17 shim; adds `max_output_tokens` ->
  `CLAUDE_CODE_MAX_OUTPUT_TOKENS`). The shim has its own ledger and never logs text.
- Provider: `adapter/` (`org.wavect/haiku-cli-shim`, a `model.generate` adapter; permission `network=[loopback:*]` only).
  It returns a typed `anthropic_messages` receipt: model, finish reason, provider usage including cache read/creation,
  `provider_cost_micros` from `total_cost_usd`. `max_output_tokens` is honoured (control reported `applied`) through
  `CLAUDE_CODE_MAX_OUTPUT_TOKENS`; the effect was checked once by hand (64-token cap gave a 64-token message).
  Ordered-v1 prompts arrive as plain concatenated text and are forwarded as is.
- Path: every trial is `path: production-harness`, `origin: real` (adopted and trusted adapter, fresh `HostModel` per trial).
- Arms (applicable to app tasks): defaults, tiers, feedback-allowance, prompt-renderer, combined. Not applicable:
  compact-skills, context-target, caveman-view, spend-ledger, routing. 12 tasks x 10 reps x 5 arms = 600 planned trials.

## Commands
```
python3 shim.py --port 11512 --ledger shim-ledger.json --calls shim-calls.jsonl --cap 5 --cwd <empty dir>
semaprax-harness adopt adapter/harness-provider.json --runtime <python3>   # then: trust org.wavect/haiku-cli-shim
OUT=<out> REPS=10 ./run.sh        # bench app run <tasks> --profile-arms defaults,tiers,feedback-allowance,prompt-renderer,combined
                                  #   --model id=claude-haiku-4-5,size=large,billed=1 --production-adapter org.wavect/haiku-cli-shim
                                  #   --production-project <project> --max-usd 5 --reps 10 ...
semaprax-harness bench app qualify <out>
```

## Spend (two independent ledgers)
- Shim ledger (`shim-ledger.json`, cumulative incl. canaries and the discarded run): **USD 3.100407**, 711 calls, 0 refused.
  Of this, the campaign proper is USD 3.0661; two canaries USD 0.0173 (`compile-repair-js`/`prompt-renderer`); the discarded
  run below USD 0.0171.
- Harness ledger of the campaign (`harness-ledger.json`): USD 4.735986, 709 calls, max call 0.8447. **It disagrees with the
  shim by design of its failure accounting, not by spend**: a call that fails without a cost report is charged its whole
  reservation (`SpendLedger::settle`). Six such host-side failures near the end (see below) were charged the full
  ceiling, which also tripped the harness cap ("halted by the spend cap") for the last two trials. Real provider cost is the
  shim figure; neither ledger exceeded USD 5.00.
- Discarded first run (`discarded-adapter-crash-run/`): my adapter crashed on `ordered-v1` input (not JSON), so the
  prompt-renderer/combined trials failed with no cost and the harness ledger charged USD 2.55 conservatively while the shim
  spent USD 0.017. Adapter fixed (`prompt_text`), run restarted from an empty out dir. Nothing from it is in any figure.

## Results (production-harness, real origin, `trials.jsonl`)
- 109 of 109 matched, fully-measured (task, rep) items accepted in every arm (acceptance 109/120 per arm counting the
  10 untested compile-repair-spx cells as not accepted; no tested trial was rejected by a grader except the infra failures below).
- Cost per accepted task, matched 109 items, micro-USD (my derivation from `trials.jsonl`; `qualification.json`
  reports null for arms with an unknown-cost trial): defaults 5132, feedback-allowance 5100, tiers 5080,
  prompt-renderer 5228, combined 5077. Best saving is 1.1 percent against the predeclared 10 percent minimum.
- First-pass (of 120): defaults 0.800, feedback 0.825, tiers 0.825, renderer 0.792, combined 0.817.
- Latency means 6.0 to 6.2 s, all well inside the 1.5x ratio.

## Honest limits
- **Inconclusive by the registry**: `compile-repair-spx` is untested in all 50 trials (no `SEMAPRAX_COMPILER` was
  configured, so the tool pin has `compiler: null`; a campaign's pins are immutable, so it could not be resumed with a compiler
  without a second campaign, which the cap did not allow).
- The last (task, rep) cell, `reuse-py-order-row` rep 9, is incomplete: defaults, tiers and feedback-allowance failed with
  `SPX-HPC022: host time budget is exhausted` (the adapter host's session time budget ran out after about 3 hours; host
  infrastructure, not a model or grader failure, but recorded as `failed` with unknown cost), prompt-renderer and combined were
  `budget_aborted` by the harness cap described above.
- Ceiling effect, as in HN-17: haiku-4-5 accepts essentially everything on this task set, so no arm can show a quality gain; cost differences are
  within about 1 percent. Even a clean cohort would be a cost no-go against the 10 percent saving gate. Single model, single task set, 10 reps.
- Provider cache states are mostly `expired`/`cold`; the Claude Code CLI reports usage that includes an extra hidden
  request (top-level usage matches billed cost; per-message usage is smaller). Top-level usage is what the receipt carries.
- No default was changed; no recommendation to promote any profile.
