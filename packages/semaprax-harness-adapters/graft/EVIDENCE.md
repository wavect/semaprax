# Graft adapter evidence (HP-06)

Local, macOS arm64, real Graft 0.18.0 through the common host. Size unit is
`byte-v1` (bytes of the harness `context --json` document, never model tokens).

## Environment

| Item | Value |
| --- | --- |
| Base | worktree `hp/hp0607` on `wavect/v090` merge `7db320ffa` (the lane commit follows it) |
| Graft | `@nanonets/graft` 0.18.0, `/Users/kevin/.nvm/versions/node/v24.3.0/bin/graft`, Node v24.3.0 |
| Graphify | `graphifyy` 0.9.25, `/Users/kevin/.local/bin/graphify`, Python 3.12 |
| Compiler | `/Users/kevin/Documents/ChatGPT/AI-Lang-v090/target/debug/semaprax` (`semaprax 0.7.0`) |
| Platform | macOS arm64 (Darwin 25.5.0); no Linux, hosted or production evidence |

## Commands

```sh
export HARNESS_GRAFT=/Users/kevin/.nvm/versions/node/v24.3.0/bin/graft HARNESS_NODE=/Users/kevin/.nvm/versions/node/v24.3.0/bin/node \
       HARNESS_GRAPHIFY=/Users/kevin/.local/bin/graphify HARNESS_PYTHON=/Users/kevin/.local/bin/python3 \
       SEMAPRAX_COMPILER=/Users/kevin/Documents/ChatGPT/AI-Lang-v090/target/debug/semaprax
CARGO_TARGET_DIR=$PWD/target cargo test --offline -p semaprax-harness --test real_tools_v1 -- --ignored --test-threads=1 graft graphify
CARGO_TARGET_DIR=$PWD/target cargo test --offline -p semaprax-harness --test real_tools_v1 -- --ignored --nocapture measure_model   # the table below
```

Every run goes through the harness CLI (`adopt --upstream`, `trust`, a project
`semaprax.harness.toml`, `resolve`, then `context --json`), the host starts the
adapter with `LaunchSpec`, and the adapter runs the real tool. The downstream
project is `crates/semaprax-harness/tests/fixtures/real_context` (3 `.spx`
files, TypeScript, Python), copied and `git init`-ed per test.

## Interop defects found and fixed while wiring the real tools

* The contract's context request was closed: adapters read `limit`, `path_prefix`
  and `refresh` that the host would refuse. The contract now has additive optional
  `refresh`, `exhaustive` (references), `in`, result `metadata`, coverage
  `extraction_errors` and item `edges`; adapters read the contract name `max_items`.
* Graft digested spans with line terminators; the broker re-hashes lines joined by
  LF without a trailing terminator, so every Graft item was `stale-digest` and
  unverified. Graft now uses the broker convention.
* `adopt` probed `graft --version` with `PATH=/usr/bin:/bin`; the upstream's
  `#!/usr/bin/env node` shebang failed (exit 127) and the install was recorded as
  incompatible. The probe now adds the directories of the explicitly named
  `HARNESS_NODE`/`HARNESS_PYTHON` runtimes (a profile change, see REQUESTS).
  The host itself needs no PATH: the adapter is started with `HARNESS_NODE` and
  runs graft through a private `node` symlink.

## Acceptance evidence (all `--ignored`, real tools)

| Criterion | Test |
| --- | --- |
| Real Graft queried by the development workflow on a mixed project; checked facts from the compiler | `graft_facts_and_spans` (native text equals standalone `semaprax context`) |
| Exact spans, digests re-verified against file bytes, `.spx` skipped/unsupported | same test |
| Renamed symbol gone, stale index refreshed, warm index reused (index file mtimes unchanged) | `graft_rename_stale_index_and_warm_reuse` |
| Worktree switch gets its own index and answers | `graft_worktree_switch` |
| Absent provider: auto falls back to native, required refuses (`SPX-HPB040`) | `graft_absent_provider_fallback_and_required` |
| Network denied (cold build and warm reuse under `sandbox-exec ... (deny network*)`, after proving the sandbox blocks a connect) | `graft_network_denied_cold_and_warm` |
| Planted `GRAFT_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GRAFT_PROVIDER`, proxy in the process and in `Environment.vars` not found in the index, cache or output | `graft_planted_secrets_not_inherited` |
| References are labelled, never exhaustive, no definitive absence | `graft_reference_labels_and_no_absence_claim` |
| No writes into the user tree (`git status --ignored`) | `graft_facts_and_spans` |

Not run by these tests: the host `IsolationRequest::Restricted` path (the context
integration requests `IsolationRequest::None`), so network denial is evidenced by
sandboxing the whole test process, not by the host.

## Measurement: model-facing bytes (criterion 4)

Corpora: the fixture project, and a repo snapshot copy of
`crates/semaprax-harness/src/context`, `packages/semaprax-harness-adapters/graft/lib`
and `packages/semaprax-harness-adapters/graphify/adapter.py`. "Full-source" is the
bytes of the files a person would paste (listed). Required facts are verified
independently: `Line` facts compute the line from file bytes and need some returned
item whose span covers it; `Text` facts need the string in an item. "Cold ms" is a
cold broker cache and, for the first task of each corpus only (T1, T4), a cold
index; "warm-index" purges the broker cache and restarts the adapter over the
on-disk index; "cache-hit" is a broker cache hit. Budget 16000 per query. The
native-only arm is the same document with no external provider (about 590 bytes of
fixed document overhead), so it is smaller when it carries nothing and then misses
the facts.

| Task | Arm | Bytes (byte-v1) | Required facts | Items (native/ext) | vs full-source | Cold ms | Warm-index ms | Cache-hit ms | Index bytes |
| --- | --- | ---: | :---: | :---: | :---: | ---: | ---: | ---: | ---: |
| T1 where is renderTotal defined and who calls it | full-source (web/render.ts) | 434 | 2/2 | - | 1.00x | - | - | - | - |
| T1 | native-only | 823 | 0/2 | 0/0 | 1.90x | 1 | 0 | 0 | 0 |
| T1 | native+graft | 1875 | 1/2 | 0/1 | 4.32x | 623 | 865 | 2 | 18313 |
| T1 | native+graphify | 1978 | 1/2 | 0/1 | 4.56x | 248 | 26 | 2 | 14249 |
| T2 compiler facts for ledger.core.split-evenly | full-source (src/core.spx, src/app.spx) | 502 | 2/2 | - | 1.00x | - | - | - | - |
| T2 | native-only | 1703 | 2/2 | 1/0 | 3.39x | 138 | 26 | 26 | 0 |
| T2 | native+graft | 2735 | 2/2 | 1/1 | 5.45x | 477 | 491 | 27 | 18313 |
| T2 | native+graphify | 2868 | 2/2 | 1/1 | 5.71x | 52 | 54 | 27 | 14249 |
| T3 totals parsing in report.py and rendering in render.ts | full-source (tools/report.py, web/render.ts) | 788 | 2/2 | - | 1.00x | - | - | - | - |
| T3 | native-only | 597 | 0/2 | 0/0 | 0.76x | 0 | 0 | 0 | 0 |
| T3 | native+graft | 5336 | 2/2 | 0/11 | 6.77x | 535 | 461 | 6 | 18313 |
| T3 | native+graphify | 5627 | 2/2 | 0/12 | 7.14x | 30 | 30 | 5 | 14249 |
| T4 where is span_digest defined (Rust) | full-source (crates/semaprax-harness/src/context/identity.rs) | 6689 | 1/1 | - | 1.00x | - | - | - | - |
| T4 | native-only | 589 | 0/1 | 0/0 | 0.09x | 2 | 1 | 1 | 0 |
| T4 | native+graft | 9232 | 1/1 | 0/20 | 1.38x | 689 | 604 | 13 | 875814 |
| T4 | native+graphify | 1259 | 1/1 | 0/1 | 0.19x | 570 | 29 | 2 | 857255 |
| T5 where is ensureFresh defined (JavaScript) | full-source (packages/semaprax-harness-adapters/graft/lib/project.mjs) | 13591 | 1/1 | - | 1.00x | - | - | - | - |
| T5 | native-only | 589 | 0/1 | 0/0 | 0.04x | 1 | 1 | 1 | 0 |
| T5 | native+graft | 4580 | 1/1 | 0/9 | 0.34x | 729 | 604 | 6 | 875814 |
| T5 | native+graphify | 1274 | 1/1 | 0/1 | 0.09x | 28 | 28 | 2 | 857255 |
| T6 where is extraction_errors defined (Python) | full-source (packages/semaprax-harness-adapters/graphify/adapter.py) | 21098 | 1/1 | - | 1.00x | - | - | - | - |
| T6 | native-only | 595 | 0/1 | 0/0 | 0.03x | 1 | 1 | 1 | 0 |
| T6 | native+graft | 5043 | 1/1 | 0/10 | 0.24x | 601 | 625 | 6 | 875814 |
| T6 | native+graphify | 1280 | 1/1 | 0/1 | 0.06x | 30 | 30 | 2 | 857255 |

Reading the table honestly:

* On the 3-file fixture no provider beats pasting the files (T1-T3: 4.3x to 7.1x
  larger): the files are smaller than the fixed document and item overhead.
* Where the reference is a real file (T4-T6, 6.7 to 21 KB) Graft is smaller for JS
  and Python (0.34x, 0.24x) but 1.38x larger for Rust (T4): Graft 0.18.0 has no Rust
  parser, so its 20 returned items (9.2 KB) are not Rust-structural; the fact was still covered.
* T1 counterexample: both providers returned one item and missed the caller fact
  (`Statement.summary` calling `renderTotal`); only the full source had both facts.
* Native-only always misses non-`.spx` facts (0/N on T1, T3-T6) and is the only arm
  that carries checked `.spx` facts (T2).
* Graft warm-index latency (about 0.45-0.9 s) is node start plus `graft check` on every
  call; Graphify's is about 0.03 s. Both broker-cache hits are a few ms.
* Index size: Graft 18 KB (fixture), 876 KB (repo snapshot).

Not measured: model answer accuracy (HP-17), large repositories, Linux.
