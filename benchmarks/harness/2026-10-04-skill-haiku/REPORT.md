# Harness benchmark report: 2026-10-04 reuse-skill matched trial, claude-haiku-4-5 via loopback shim

Contract `semaprax.harness-benchmark.v1`. Corpus digest `sha256:96af8435a9c706d0ff169106edc8be2056bed99274e3659dd6d3c48b8bed6acf`. Baseline profile `native-only`. Trials per cell: 1 cold + 0 warm.

## Labels

- Measurement unit: `byte-v1` (UTF-8 bytes of the final model-visible envelope). No named tokenizer is available on this machine, so **no token counts are reported** and no byte figure is a token or a billing figure.
- Tested local support: cells marked `ok` below, run on this one machine (Darwin 25.5.0 arm64).
- Protocol-only hosted support (Jev, remote gateways): **not benchmarked**; protocol contract tests only.
- Untested platforms: Linux and Windows. Nothing here supports a claim for them.
- Historical evidence (earlier HP lane reports, ADR 0001, the #309 case study) is cited for context only and is not mixed into these numbers.
- Zero observed failures is not proof of safety: the corpus is small and seeded.

## Per-profile results (all applicable cells)

| profile | cells ok/failed/untested | accepted (n) | Wilson 95% | fact retention | false neg. | visible bytes | incurred | bytes / accepted | warm p50/p95 ms | cold p50/p95 ms |
|---|---|---|---|---|---|---|---|---|---|---|
| native+skill | 10/0/0 | 0.40 (10) | 0.1682-0.6873 | 0.4118 | 0 | 29201 | 6920 | 7300.25 | n/a/n/a | 115/619 |
| native-only | 19/0/0 | 0.47 (19) | 0.2733-0.6829 | 0.4815 | 0 | 46316 | 0 | 5146.2222 | n/a/n/a | 37/908 |

Rows cover different task sets (a profile only runs tasks it can change); compare profiles only through the matched table below.

## Matched comparison against the baseline (same task and trial)

| profile | matched cells | accepted delta / cell | visible bytes delta / cell | latency ms delta / cell |
|---|---|---|---|---|
| native+skill | 10 | 0.0 | 692.1 | -29.7 |

Negative visible-bytes delta = fewer bytes than the baseline. A combined profile is an ablation of its parts: its delta is not the sum of per-plugin savings, and each part is judged on the tasks it touches.

## Costs, resources and reconciliation

- `native+skill`: calls 19, retries 0, detail retrievals 0; child CPU user n/a ms / sys n/a ms; max RSS n/a KiB; disk left behind (max) 22347 bytes; observation report partial: true; reconciliation mismatches: 0.
- `native-only`: calls 24, retries 0, detail retrievals 0; child CPU user n/a ms / sys n/a ms; max RSS n/a KiB; disk left behind (max) 21549 bytes; observation report partial: true; reconciliation mismatches: 0.

## Adverse cases and negative savings (retained)

- `native+skill` on `diagnose-ledger-failure`: 10045 visible bytes vs baseline 10044 (**more**).
- `native+skill` on `diagnose-ledger-failure`: accepted 0 of 1 (n/a).
- `native+skill` on `law-sound-fix`: accepted 0 of 1 (n/a).
- `native+skill` on `repair-ledger-polyglot`: accepted 0 of 1 (n/a).
- `native+skill` on `reuse-byte-tokenizer`: 2321 visible bytes vs baseline 591 (**more**).
- `native+skill` on `reuse-byte-tokenizer`: accepted 0 of 1 (n/a).
- `native+skill` on `reuse-cross-language-add`: 3388 visible bytes vs baseline 1658 (**more**).
- `native+skill` on `reuse-cross-language-add`: accepted 0 of 1 (n/a).
- `native+skill` on `reuse-line-total-contract`: 3398 visible bytes vs baseline 1668 (**more**).
- `native+skill` on `reuse-line-total-contract`: accepted 0 of 1 (n/a).
- `native+skill` on `reuse-square-via-multiply`: 3421 visible bytes vs baseline 1691 (**more**).

## Explicit acceptance cells

Workflow-driven external context (one warm trial each; the `run` report carries bytes and provider identity, not item text, so facts are verified in the broker context for the same seed):


Adopted skill packages (`adopt --skills` plus `[skills] enabled`):

- `native+skill`: skill loaded ["reuse-before-generation"] on 4 api-reuse cell(s), prompt 1730 bytes counted as incurred cost; 6 workflow cells, 3 accepted by the compiler's checks; manifest bytes unchanged in every workflow cell: true.

## Seeded adversarial cases


## Gates (declared in docs/HARNESS-BENCHMARK-V1.md before measurement)

- **G1**: zero protected-fact loss: every seeded adversarial case detected and zero false negatives in measured cells
- **G2**: no permission widening: stored grants unchanged during cells and the widening case detected
- **G3**: no hidden command replay: every command cell shows exactly one execution and the double-execution case is detected
- **G4**: task quality: accepted-rate delta vs baseline >= -0.0 over >= 10 matched cells
- **G5**: net cost: model-visible bytes including incurred requests fall >= 20% vs baseline
- **G6**: latency: added end-to-end latency (cold and warm trials together) <= 2000 ms per cell
- **G7**: tested: the scope ran with the real tool (no untested or failed cell)

### `native+skill`

- `api_reuse`: matched 4 cells over 4 distinct task(s); eligible for auto-enable: **false**; failed: G1 (adversarial all detected: false; false negatives in scope: 0); G4 (matched cells 4; accepted delta per cell +0.0000); G5 (visible-byte reduction -123.4% over 4 matched cells); G6 (added latency +2 ms per matched cell)
- `failing_test_diagnosis`: matched 1 cells over 1 distinct task(s); eligible for auto-enable: **false**; failed: G1 (adversarial all detected: false; false negatives in scope: 0); G4 (matched cells 1; accepted delta per cell +0.0000); G5 (visible-byte reduction -0.0% over 1 matched cells); G6 (added latency -289 ms per matched cell)
- `law_effect_change`: matched 3 cells over 3 distinct task(s); eligible for auto-enable: **false**; failed: G1 (adversarial all detected: false; false negatives in scope: 0); G4 (matched cells 3; accepted delta per cell +0.0000); G5 (visible-byte reduction +0.0% over 3 matched cells); G6 (added latency +2 ms per matched cell)
- `multi_file_repair`: matched 2 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G1 (adversarial all detected: false; false negatives in scope: 0); G4 (matched cells 2; accepted delta per cell +0.0000); G5 (visible-byte reduction +0.0% over 2 matched cells); G6 (added latency -11 ms per matched cell)

## Local model pilot

Status `ran`, label `pilot-only`, model `claude-haiku-4-5` (digest remote:anthropic/claude-haiku-4-5 via claude CLI 2.1.289 (loopback shim)), 0 calls, min trials per configuration 0. small local model, fewer than 10 trials per configuration: not evidence for any default

### Skill pilot (reuse-before-generation)

Status `ran`, label `matched-trials`, task `reuse-square-via-multiply`, 20 calls. Existing API reused (check: answer contains ["multiply("]): without skill 10/10, with skill 10/10; skill prompt adds 1730 bytes. unchanged: proposals are still judged only by the workflow's compiler checks (see the native+skill workflow cells) small local model, few trials: no evidence for any default

## Recommendation

Per profile, derived from the gate verdicts above:

- `native+skill`: opt-in or disabled: no scope passes the gates.

**native-only stays the default.** No provider profile passed every gate on any measured task scope, so none is recommended for automatic use. Providers remain opt-in; an integration that failed a gate above should stay disabled for that scope.

Evidence breadth: matched cells are repeated deterministic runs of a few fixed tasks, so they establish stability of the measurement, not independent samples of the input distribution; the scope is the measured task set only (see the number of distinct tasks per scope).

Each automatic default above is tied to its task family, matched-cell count and gate verdicts in the Gates section; a scope with fewer than 10 matched cells cannot pass G4-G6.
