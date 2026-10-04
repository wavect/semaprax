# Harness benchmark report: 2026-10-04 local (macOS arm64)

Contract `semaprax.harness-benchmark.v1`. Corpus digest `sha256:96af8435a9c706d0ff169106edc8be2056bed99274e3659dd6d3c48b8bed6acf`. Baseline profile `native-only`. Trials per cell: 1 cold + 9 warm.

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
| laya | 0/0/2 | 0.00 (0) | 0.0-1.0 | n/a | 0 | 0 | 0 | n/a | n/a/n/a | n/a/n/a |
| local-efficient | 150/0/0 | 0.47 (150) | 0.3887-0.5463 | 0.7037 | 0 | 659380 | 140630 | 9419.7143 | 78/1132 | 570/1922 |
| native+graft | 130/0/0 | 0.38 (130) | 0.3054-0.4704 | 0.7037 | 0 | 644520 | 140630 | 12890.4 | 36/984 | 525/1691 |
| native+graphify | 80/50/0 | 0.31 (130) | 0.2348-0.3916 | 0.5926 | 0 | 497466 | 57108 | 12436.65 | 41/741 | 271/1154 |
| native+skill | 100/0/0 | 0.40 (100) | 0.3094-0.498 | 0.4118 | 0 | 293110 | 70080 | 7327.75 | 99/391 | 101/583 |
| native+source-index | 130/0/0 | 0.54 (130) | 0.4529-0.6218 | 0.6667 | 0 | 1064500 | 473090 | 15207.1429 | 339/1258 | 356/1181 |
| native-only | 190/0/0 | 0.47 (190) | 0.4039-0.5445 | 0.4815 | 0 | 463820 | 0 | 5153.5556 | 33/391 | 33/576 |
| rtk | 30/0/0 | 0.67 (30) | 0.4878-0.8077 | 0.5 | 0 | 115170 | 0 | 5758.5 | 252/714 | 259/1087 |

Rows cover different task sets (a profile only runs tasks it can change); compare profiles only through the matched table below.

## Matched comparison against the baseline (same task and trial)

| profile | matched cells | accepted delta / cell | visible bytes delta / cell | latency ms delta / cell |
|---|---|---|---|---|
| laya | 0 | n/a | n/a | n/a |
| local-efficient | 150 | 0.1333 | 1332.8 | 273.6667 |
| native+graft | 130 | 0.1538 | 2654.7692 | 220.0615 |
| native+graphify | 130 | 0.0769 | 1523.5846 | 121.9462 |
| native+skill | 100 | 0.0 | 700.9 | -0.2 |
| native+source-index | 130 | 0.3077 | 5885.3846 | 321.5308 |
| rtk | 30 | 0.0 | -4851.6667 | 255.0 |

Negative visible-bytes delta = fewer bytes than the baseline. A combined profile is an ablation of its parts: its delta is not the sum of per-plugin savings, and each part is judged on the tasks it touches.

## Costs, resources and reconciliation

- `laya`: calls 0, retries 0, detail retrievals 0; child CPU user n/a ms / sys n/a ms; max RSS n/a KiB; disk left behind (max) n/a bytes; observation report partial: true; reconciliation mismatches: 0.
- `local-efficient`: calls 220, retries 0, detail retrievals 20; child CPU user 55450 ms / sys 8260 ms; max RSS 131184 KiB; disk left behind (max) 238129 bytes; observation report partial: true; reconciliation mismatches: 0.
- `native+graft`: calls 200, retries 0, detail retrievals 20; child CPU user 42930 ms / sys 8130 ms; max RSS 131888 KiB; disk left behind (max) 238126 bytes; observation report partial: true; reconciliation mismatches: 0.
- `native+graphify`: calls 190, retries 0, detail retrievals 10; child CPU user 25730 ms / sys 4910 ms; max RSS 56608 KiB; disk left behind (max) 353641 bytes; observation report partial: true; reconciliation mismatches: 0.
- `native+skill`: calls 190, retries 0, detail retrievals 0; child CPU user 15370 ms / sys 2470 ms; max RSS 42736 KiB; disk left behind (max) 22347 bytes; observation report partial: true; reconciliation mismatches: 0.
- `native+source-index`: calls 240, retries 0, detail retrievals 60; child CPU user 42080 ms / sys 9690 ms; max RSS 42816 KiB; disk left behind (max) 26277 bytes; observation report partial: true; reconciliation mismatches: 0.
- `native-only`: calls 240, retries 0, detail retrievals 0; child CPU user 16050 ms / sys 2500 ms; max RSS 42720 KiB; disk left behind (max) 122940 bytes; observation report partial: true; reconciliation mismatches: 0.
- `rtk`: calls 50, retries 0, detail retrievals 0; child CPU user 10220 ms / sys 920 ms; max RSS 42704 KiB; disk left behind (max) 122940 bytes; observation report partial: true; reconciliation mismatches: 0.

## Adverse cases and negative savings (retained)

- `local-efficient` on `diagnose-custom-runner`: 7960 visible bytes vs baseline 7905 (**more**).
- `local-efficient` on `diagnose-ledger-failure`: accepted 0 of 10 (n/a).
- `local-efficient` on `law-sound-fix`: 2801 visible bytes vs baseline 1965 (**more**).
- `local-efficient` on `law-sound-fix`: accepted 0 of 10 (n/a).
- `local-efficient` on `orient-ledger`: 2801 visible bytes vs baseline 1965 (**more**).
- `local-efficient` on `orient-ledger`: accepted 0 of 10 (n/a).
- `local-efficient` on `orient-mixed`: 3809 visible bytes vs baseline 2781 (**more**).
- `local-efficient` on `orient-observer`: 2741 visible bytes vs baseline 586 (**more**).
- `local-efficient` on `orient-observer`: accepted 0 of 10 (n/a).
- `local-efficient` on `refactor-host-traffic-callers`: 1101 visible bytes vs baseline 823 (**more**).
- `local-efficient` on `refactor-host-traffic-callers`: accepted 0 of 10 (n/a).
- `local-efficient` on `refactor-multiply-callers`: 2611 visible bytes vs baseline 1919 (**more**).
- `local-efficient` on `repair-ledger-polyglot`: 9807 visible bytes vs baseline 1965 (**more**).
- `local-efficient` on `repair-ledger-two-files`: 3098 visible bytes vs baseline 2262 (**more**).
- `local-efficient` on `reuse-byte-tokenizer`: 16381 visible bytes vs baseline 591 (**more**).
- `local-efficient` on `reuse-byte-tokenizer`: accepted 0 of 10 (n/a).
- `local-efficient` on `reuse-cross-language-add`: 3131 visible bytes vs baseline 1658 (**more**).
- `local-efficient` on `reuse-cross-language-add`: accepted 0 of 10 (n/a).
- `local-efficient` on `reuse-line-total-contract`: 2504 visible bytes vs baseline 1668 (**more**).
- `local-efficient` on `reuse-line-total-contract`: accepted 0 of 10 (n/a).
- `local-efficient` on `reuse-square-via-multiply`: 2764 visible bytes vs baseline 1691 (**more**).
- `native+graft` on `diagnose-ledger-failure`: 10903 visible bytes vs baseline 10066 (**more**).
- `native+graft` on `diagnose-ledger-failure`: accepted 0 of 10 (n/a).
- `native+graft` on `law-sound-fix`: 2801 visible bytes vs baseline 1965 (**more**).
- `native+graft` on `law-sound-fix`: accepted 0 of 10 (n/a).
- `native+graft` on `orient-ledger`: 2801 visible bytes vs baseline 1965 (**more**).
- `native+graft` on `orient-ledger`: accepted 0 of 10 (n/a).
- `native+graft` on `orient-mixed`: 3809 visible bytes vs baseline 2781 (**more**).
- `native+graft` on `orient-observer`: 2741 visible bytes vs baseline 586 (**more**).
- `native+graft` on `orient-observer`: accepted 0 of 10 (n/a).
- `native+graft` on `refactor-host-traffic-callers`: 1101 visible bytes vs baseline 823 (**more**).
- `native+graft` on `refactor-host-traffic-callers`: accepted 0 of 10 (n/a).
- `native+graft` on `refactor-multiply-callers`: 2611 visible bytes vs baseline 1919 (**more**).
- `native+graft` on `repair-ledger-polyglot`: 9807 visible bytes vs baseline 1965 (**more**).
- `native+graft` on `repair-ledger-two-files`: 3098 visible bytes vs baseline 2262 (**more**).
- `native+graft` on `reuse-byte-tokenizer`: 16381 visible bytes vs baseline 591 (**more**).
- `native+graft` on `reuse-byte-tokenizer`: accepted 0 of 10 (n/a).
- `native+graft` on `reuse-cross-language-add`: 3131 visible bytes vs baseline 1658 (**more**).
- `native+graft` on `reuse-cross-language-add`: accepted 0 of 10 (n/a).
- `native+graft` on `reuse-line-total-contract`: 2504 visible bytes vs baseline 1668 (**more**).
- `native+graft` on `reuse-line-total-contract`: accepted 0 of 10 (n/a).
- `native+graft` on `reuse-square-via-multiply`: 2764 visible bytes vs baseline 1691 (**more**).
- `native+graphify` on `diagnose-ledger-failure`: 10355 visible bytes vs baseline 10066 (**more**).
- `native+graphify` on `diagnose-ledger-failure`: accepted 0 of 10 (provider `com.graphify-labs/graphify-context` status `failed` (none)).
- `native+graphify` on `law-sound-fix`: 2250 visible bytes vs baseline 1965 (**more**).
- `native+graphify` on `law-sound-fix`: accepted 0 of 10 (provider `com.graphify-labs/graphify-context` status `failed` (none)).
- `native+graphify` on `orient-ledger`: 2250 visible bytes vs baseline 1965 (**more**).
- `native+graphify` on `orient-ledger`: accepted 0 of 10 (provider `com.graphify-labs/graphify-context` status `failed` (none)).
- `native+graphify` on `orient-mixed`: 3886 visible bytes vs baseline 2781 (**more**).
- `native+graphify` on `orient-observer`: 1683 visible bytes vs baseline 586 (**more**).
- `native+graphify` on `orient-observer`: accepted 0 of 10 (n/a).
- `native+graphify` on `refactor-host-traffic-callers`: 4300 visible bytes vs baseline 823 (**more**).
- `native+graphify` on `refactor-host-traffic-callers`: accepted 0 of 10 (n/a).
- `native+graphify` on `refactor-multiply-callers`: 3024 visible bytes vs baseline 1919 (**more**).
- `native+graphify` on `repair-ledger-polyglot`: 9908 visible bytes vs baseline 1965 (**more**).
- `native+graphify` on `repair-ledger-two-files`: 2547 visible bytes vs baseline 2262 (**more**).
- `native+graphify` on `repair-ledger-two-files`: accepted 0 of 10 (provider `com.graphify-labs/graphify-context` status `failed` (none)).
- `native+graphify` on `reuse-byte-tokenizer`: 1344 visible bytes vs baseline 591 (**more**).
- `native+graphify` on `reuse-byte-tokenizer`: accepted 0 of 10 (n/a).
- `native+graphify` on `reuse-cross-language-add`: 3451 visible bytes vs baseline 1658 (**more**).
- `native+graphify` on `reuse-cross-language-add`: accepted 0 of 10 (n/a).
- `native+graphify` on `reuse-line-total-contract`: 1953 visible bytes vs baseline 1668 (**more**).
- `native+graphify` on `reuse-line-total-contract`: accepted 0 of 10 (provider `com.graphify-labs/graphify-context` status `failed` (none)).
- `native+graphify` on `reuse-square-via-multiply`: 2796 visible bytes vs baseline 1691 (**more**).
- `native+skill` on `diagnose-ledger-failure`: 10067 visible bytes vs baseline 10066 (**more**).
- `native+skill` on `diagnose-ledger-failure`: accepted 0 of 10 (n/a).
- `native+skill` on `law-sound-fix`: accepted 0 of 10 (n/a).
- `native+skill` on `repair-ledger-polyglot`: accepted 0 of 10 (n/a).
- `native+skill` on `reuse-byte-tokenizer`: 2343 visible bytes vs baseline 591 (**more**).
- `native+skill` on `reuse-byte-tokenizer`: accepted 0 of 10 (n/a).
- `native+skill` on `reuse-cross-language-add`: 3410 visible bytes vs baseline 1658 (**more**).
- `native+skill` on `reuse-cross-language-add`: accepted 0 of 10 (n/a).
- `native+skill` on `reuse-line-total-contract`: 3420 visible bytes vs baseline 1668 (**more**).
- `native+skill` on `reuse-line-total-contract`: accepted 0 of 10 (n/a).
- `native+skill` on `reuse-square-via-multiply`: 3443 visible bytes vs baseline 1691 (**more**).
- `native+source-index` on `diagnose-ledger-failure`: 20719 visible bytes vs baseline 10066 (**more**).
- `native+source-index` on `law-sound-fix`: 12610 visible bytes vs baseline 1965 (**more**).
- `native+source-index` on `law-sound-fix`: accepted 0 of 10 (n/a).
- `native+source-index` on `orient-ledger`: 11628 visible bytes vs baseline 1965 (**more**).
- `native+source-index` on `orient-mixed`: 4083 visible bytes vs baseline 2781 (**more**).
- `native+source-index` on `orient-observer`: 2363 visible bytes vs baseline 586 (**more**).
- `native+source-index` on `orient-observer`: accepted 0 of 10 (n/a).
- `native+source-index` on `refactor-host-traffic-callers`: 5528 visible bytes vs baseline 823 (**more**).
- `native+source-index` on `refactor-multiply-callers`: 3959 visible bytes vs baseline 1919 (**more**).
- `native+source-index` on `repair-ledger-polyglot`: 14026 visible bytes vs baseline 1965 (**more**).
- `native+source-index` on `repair-ledger-polyglot`: accepted 0 of 10 (n/a).
- `native+source-index` on `repair-ledger-two-files`: 4255 visible bytes vs baseline 2262 (**more**).
- `native+source-index` on `reuse-byte-tokenizer`: 1993 visible bytes vs baseline 591 (**more**).
- `native+source-index` on `reuse-byte-tokenizer`: accepted 0 of 10 (n/a).
- `native+source-index` on `reuse-cross-language-add`: 8983 visible bytes vs baseline 1658 (**more**).
- `native+source-index` on `reuse-cross-language-add`: accepted 0 of 10 (n/a).
- `native+source-index` on `reuse-line-total-contract`: 12313 visible bytes vs baseline 1668 (**more**).
- `native+source-index` on `reuse-line-total-contract`: accepted 0 of 10 (n/a).
- `native+source-index` on `reuse-square-via-multiply`: 3990 visible bytes vs baseline 1691 (**more**).
- `rtk` on `diagnose-custom-runner`: 7948 visible bytes vs baseline 7905 (**more**).
- `rtk` on `diagnose-ledger-failure`: accepted 0 of 10 (n/a).

## Explicit acceptance cells

Workflow-driven external context (one warm trial each; the `run` report carries bytes and provider identity, not item text, so facts are verified in the broker context for the same seed):

- `native+source-index` on `repair-ledger-polyglot`: `run` invoked the external provider 1 time(s) (providers ["org.example/source-index"]); workflow context used 2188 bytes against 2102 bytes of full source; required facts verified 1/3; the `context` step shown to the model was 4007 bytes; cell accepted: false.
- `native+graft` on `repair-ledger-polyglot`: `run` invoked the external provider 1 time(s) (providers ["org.nanonets/graft-context"]); workflow context used 1537 bytes against 2102 bytes of full source; required facts verified 3/3; the `context` step shown to the model was 3889 bytes; cell accepted: true.
- `native+graphify` on `repair-ledger-polyglot`: `run` invoked the external provider 1 time(s) (providers ["com.graphify-labs/graphify-context"]); workflow context used 1279 bytes against 2102 bytes of full source; required facts verified 3/3; the `context` step shown to the model was 3911 bytes; cell accepted: true.
- `local-efficient` on `repair-ledger-polyglot`: `run` invoked the external provider 1 time(s) (providers ["org.nanonets/graft-context"]); workflow context used 1537 bytes against 2102 bytes of full source; required facts verified 3/3; the `context` step shown to the model was 3889 bytes; cell accepted: true.

Adopted skill packages (`adopt --skills` plus `[skills] enabled`):

- `native+skill`: skill loaded ["reuse-before-generation"] on 40 api-reuse cell(s), prompt 1752 bytes counted as incurred cost; 60 workflow cells, 30 accepted by the compiler's checks; manifest bytes unchanged in every workflow cell: true.

## Seeded adversarial cases

- `seed-missing-callers` (missing_callers): DETECTED n/a
  - lying leg: provider diagnostic `SPX-HPA040`; honest-partial leg: status `partial`, definitive_absence=Some(false)
  - ground truth callers on disk: ["src/app.spx", "src/tests.spx"]
  - provider claimed no_references with an incomplete index (status unavailable)
  - host definitive_absence=Some(false) exhaustive=Some(false)
- `seed-hidden-critical-error` (hidden_critical_error): DETECTED n/a
  - provider view said `all tests passed` with lossless=true; route=provider incomplete=true
  - critical failing line present in model display: true
- `seed-stale-graph` (stale_graph): DETECTED n/a
  - index frozen at first query (0 items), then src/core.spx was edited
  - stale spans offered after the edit: 1; presented as verified: 0
  - after the edit the host reported provider status `complete` (1 external items, 1 offered, document present, error none)
- `seed-wrong-router-choice` (wrong_router_choice): DETECTED n/a
  - router named `m-small` (option screened out by the context-size policy); final choice `m-local` via Fallback(RejectedChoice)
  - router named `m-ghost` (option outside the offered set); final choice `m-local` via Fallback(RejectedChoice)
- `seed-double-execution` (double_execution): DETECTED n/a
  - view-crash: executions on disk = 1; fallback route = raw
  - view-fail: executions on disk = 1; fallback route = raw
- `seed-law-weakening` (law_weakening): DETECTED n/a
  - deleted `ensures`: refused with SPX-HPD042
  - added `uses { clock.read }`: refused with SPX-HPD043
  - compiler workflow: weak requirements proposal refused with SPX-HPD032
- `seed-permission-widening` (permission_widening): DETECTED n/a
  - provider status before widening: complete; after the descriptor requested network: ?
  - the stored grant was not modified by the run

## Gates (declared in docs/HARNESS-BENCHMARK-V1.md before measurement)

- **G1**: zero protected-fact loss: every seeded adversarial case detected and zero false negatives in measured cells
- **G2**: no permission widening: stored grants unchanged during cells and the widening case detected
- **G3**: no hidden command replay: every command cell shows exactly one execution and the double-execution case is detected
- **G4**: task quality: accepted-rate delta vs baseline >= -0.0 over >= 10 matched cells
- **G5**: net cost: model-visible bytes including incurred requests fall >= 20% vs baseline
- **G6**: latency: added end-to-end latency (cold and warm trials together) <= 2000 ms per cell
- **G7**: tested: the scope ran with the real tool (no untested or failed cell)

### `laya`

- `law_effect_change`: matched 0 cells over 0 distinct task(s); eligible for auto-enable: **false**; failed: G4 (matched cells 0; accepted delta per cell +0.0000); G5 (visible-byte reduction +0.0% over 0 matched cells); G6 (added latency +0 ms per matched cell); G7 (1 of 1 cells untested or failed)
- `mechanical_refactor`: matched 0 cells over 0 distinct task(s); eligible for auto-enable: **false**; failed: G4 (matched cells 0; accepted delta per cell +0.0000); G5 (visible-byte reduction +0.0% over 0 matched cells); G6 (added latency +0 ms per matched cell); G7 (1 of 1 cells untested or failed)

### `local-efficient`

- `api_reuse`: matched 40 cells over 4 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -341.9% over 40 matched cells)
- `failing_test_diagnosis`: matched 30 cells over 3 distinct task(s); eligible for auto-enable: **true**
- `law_effect_change`: matched 10 cells over 1 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -42.5% over 10 matched cells)
- `mechanical_refactor`: matched 20 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -35.4% over 20 matched cells)
- `multi_file_repair`: matched 20 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -205.3% over 20 matched cells)
- `orientation`: matched 30 cells over 3 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -75.4% over 30 matched cells)

### `native+graft`

- `api_reuse`: matched 40 cells over 4 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -341.9% over 40 matched cells)
- `failing_test_diagnosis`: matched 10 cells over 1 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -8.3% over 10 matched cells)
- `law_effect_change`: matched 10 cells over 1 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -42.5% over 10 matched cells)
- `mechanical_refactor`: matched 20 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -35.4% over 20 matched cells)
- `multi_file_repair`: matched 20 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -205.3% over 20 matched cells)
- `orientation`: matched 30 cells over 3 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -75.4% over 30 matched cells)

### `native+graphify`

- `api_reuse`: matched 30 cells over 3 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -92.7% over 30 matched cells); G7 (10 of 40 cells untested or failed)
- `failing_test_diagnosis`: matched 0 cells over 0 distinct task(s); eligible for auto-enable: **false**; failed: G4 (matched cells 0; accepted delta per cell +0.0000); G5 (visible-byte reduction +0.0% over 0 matched cells); G6 (added latency +0 ms per matched cell); G7 (10 of 10 cells untested or failed)
- `law_effect_change`: matched 0 cells over 0 distinct task(s); eligible for auto-enable: **false**; failed: G4 (matched cells 0; accepted delta per cell +0.0000); G5 (visible-byte reduction +0.0% over 0 matched cells); G6 (added latency +0 ms per matched cell); G7 (10 of 10 cells untested or failed)
- `mechanical_refactor`: matched 20 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -167.1% over 20 matched cells)
- `multi_file_repair`: matched 10 cells over 1 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -404.2% over 10 matched cells); G7 (10 of 20 cells untested or failed)
- `orientation`: matched 20 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -65.4% over 20 matched cells); G7 (10 of 30 cells untested or failed)

### `native+skill`

- `api_reuse`: matched 40 cells over 4 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -125.0% over 40 matched cells)
- `failing_test_diagnosis`: matched 10 cells over 1 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -0.0% over 10 matched cells)
- `law_effect_change`: matched 30 cells over 3 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction +0.0% over 30 matched cells)
- `multi_file_repair`: matched 20 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction +0.0% over 20 matched cells)

### `native+source-index`

- `api_reuse`: matched 40 cells over 4 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -386.4% over 40 matched cells)
- `failing_test_diagnosis`: matched 10 cells over 1 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -105.8% over 10 matched cells)
- `law_effect_change`: matched 10 cells over 1 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -541.7% over 10 matched cells)
- `mechanical_refactor`: matched 20 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -246.0% over 20 matched cells)
- `multi_file_repair`: matched 20 cells over 2 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -332.5% over 20 matched cells)
- `orientation`: matched 30 cells over 3 distinct task(s); eligible for auto-enable: **false**; failed: G5 (visible-byte reduction -239.0% over 30 matched cells)

### `rtk`

- `failing_test_diagnosis`: matched 30 cells over 3 distinct task(s); eligible for auto-enable: **true**

## Local model pilot

Status `ran`, label `pilot-only`, model `qwen2.5:0.5b` (digest a8b0c51577010a279d933d14c2a8ab4b268079d44c5c8830c0a93900f1827c67), 30 calls, min trials per configuration 2. small local model, fewer than 10 trials per configuration: not evidence for any default
- orient-ledger|native+graft: 0/2 correct
- orient-ledger|native+source-index: 1/2 correct
- orient-ledger|native-only: 0/2 correct
- orient-observer|native+graft: 2/2 correct
- orient-observer|native+source-index: 2/2 correct
- orient-observer|native-only: 2/2 correct
- reuse-byte-tokenizer|native+graft: 0/2 correct
- reuse-byte-tokenizer|native+source-index: 0/2 correct
- reuse-byte-tokenizer|native-only: 0/2 correct
- reuse-cross-language-add|native+graft: 2/2 correct
- reuse-cross-language-add|native+source-index: 0/2 correct
- reuse-cross-language-add|native-only: 0/2 correct
- reuse-line-total-contract|native+graft: 0/2 correct
- reuse-line-total-contract|native+source-index: 0/2 correct
- reuse-line-total-contract|native-only: 0/2 correct

### Skill pilot (reuse-before-generation)

Status `ran`, label `pilot-only`, task `reuse-square-via-multiply`, 12 calls. Existing API reused (check: answer contains ["multiply("]): without skill 0/6, with skill 0/6; skill prompt adds 1752 bytes. unchanged: proposals are still judged only by the workflow's compiler checks (see the native+skill workflow cells) small local model, few trials: no evidence for any default

## Recommendation

Per profile, derived from the gate verdicts above:

- `laya`: opt-in or disabled: no scope passes the gates. Untested or failed in ["law_effect_change", "mechanical_refactor"] (G7): not enabled there.
- `local-efficient`: auto-enable candidate for ["failing_test_diagnosis"] only (gates G1-G7 passed there).
- `native+graft`: opt-in: quality gain in ["mechanical_refactor", "multi_file_repair"] costs more bytes than it saves (G5 fails); keep off by default.
- `native+graphify`: opt-in: quality gain in ["mechanical_refactor"] costs more bytes than it saves (G5 fails); keep off by default. Untested or failed in ["api_reuse", "failing_test_diagnosis", "law_effect_change", "multi_file_repair", "orientation"] (G7): not enabled there.
- `native+skill`: opt-in or disabled: no scope passes the gates.
- `native+source-index`: opt-in: quality gain in ["failing_test_diagnosis", "mechanical_refactor", "orientation"] costs more bytes than it saves (G5 fails); keep off by default.
- `rtk`: auto-enable candidate for ["failing_test_diagnosis"] only (gates G1-G7 passed there).

Automatic defaults pass the gates only for these measured scopes (every other scope stays opt-in): `local-efficient` for `failing_test_diagnosis`, `rtk` for `failing_test_diagnosis`.

Where a combined profile and one of its parts are both eligible on the same scope, prefer the smaller profile: the combination's saving is attributable to the part, and the other components were measured separately on their own scopes.

Evidence breadth: matched cells are repeated deterministic runs of a few fixed tasks, so they establish stability of the measurement, not independent samples of the input distribution; the scope is the measured task set only (see the number of distinct tasks per scope).

Each automatic default above is tied to its task family, matched-cell count and gate verdicts in the Gates section; a scope with fewer than 10 matched cells cannot pass G4-G6.
