# Graphify adapter evidence (HP-07)

Raw local measurements for the ADR 0001 revisit. They are not a benchmark of
task accuracy (that is HP-17). The first part (to "Not measured") is the original adapter
run; the second part, "2026-10-04 host run", compares Graphify with Graft and native-only
through the common host.

## Environment

| Item | Value |
| --- | --- |
| Worktree base commit | `a540885b43ab2125013ac3d56b83f2b7f49cd4ba` (branch `hp/hp07a`; the adapter commit follows it) |
| Upstream | `graphifyy` 0.9.25 (`Name: graphifyy`, `Project-URL: Repository, https://github.com/Graphify-Labs/graphify`, `License-Expression: Apache-2.0`) read from `~/.local/share/uv/tools/graphifyy/lib/python3.12/site-packages/graphifyy-0.9.25.dist-info/METADATA` |
| Executable | `/Users/kevin/.local/bin/graphify` -> uv tool venv; `graphify --version` prints `graphify 0.9.25` |
| Platform | macOS 26.5.1, arm64, Python 3.12.12 |

## Extraction command (what the adapter runs)

```sh
graphify extract <project-root> --code-only --out <cache-dir>/graphify-index
```

Run with cwd = cache dir, `HOME=<cache-dir>/home`, and a four-variable
allowlisted environment (see README). `--no-cluster` is deliberately NOT used,
see the benchmark section.

## Cold extract vs warm query

Reproduce with `python3 evidence.py <root> <scratch-cache> <query> <symbol>`
(runs the real adapter, which runs the real graphify). "cold" is the first
`search` call, which includes extraction, graph load and the source-set digest;
"warm" is the identical second call in the same adapter process.

| Corpus | Command | Cold search | Warm search | Warm references |
| --- | --- | --- | --- | --- |
| `<worktree>/src` (Rust compiler sources, 2,351 indexed files) | `python3 evidence.py $WT/src $CACHE Parser Parser` | 46.766 s | 0.129 s | 0.095 s |
| `<worktree>/packages` (9 indexed files, Python/JS) | `python3 evidence.py $WT/packages $CACHE serve serve` | 0.519 s | 0.001 s | 0.001 s |

Warm numbers include re-hashing the whole source set for the staleness check
(0.1 s at 2,351 files / 39 MB), so staleness cost scales with corpus size.

## Graph size vs source size

| Corpus | `graph.json` bytes | Indexed source bytes | Ratio | Whole cache dir bytes | Nodes / edges |
| --- | --- | --- | --- | --- | --- |
| `src` | 87,918,282 | 39,120,302 | 2.25x | 204,641,605 | 52,854 / 169,755 |
| `packages` | 230,094 | 125,315 | 1.84x | 442,239 | 224 / 410 |

The historical ADR run recorded 254,636 graph bytes for 120,266 source bytes
(2.12x). The ratio is the same order. A larger graph is not itself
disqualifying: the model never sees `graph.json`; the adapter returns bounded
items (search: 20 items = 14,773 result bytes on `src`).

## Coverage and edge provenance observed

* `src`: 61 skipped files, 0 extraction errors, status `partial` because skips
  exist. Skips include non-code files (`.md`, `.pem`, license text) and `.spx`
  files where present. `.spx`, `.spatch` and `Cargo.toml` are never indexed.
* `src` edge mix (graph.json `links`): references/EXTRACTED 81,129; calls/EXTRACTED
  36,999; contains 25,870; method 11,765; calls/INFERRED 8,423; imports_from 4,708;
  implements 838; inherits 22; indirect_call/INFERRED 1.
* 12,071 of 52,854 nodes are unresolved stubs (`source_file: ""`, e.g. `Vec`).
  The adapter keeps them only as reference targets and labels such edges
  `inferred` even when graphify says EXTRACTED, because the target was matched by
  name only.
* `references Parser` on `src` returned 2 items (both `inferred`, stub target) and
  `partial`/non-exhaustive: the graph contains 13 distinct `Parser` structs, so a
  name-only lookup is weak evidence. This is a measured limitation of name-based
  resolution, not a claim about the real call count.

## `graphify benchmark` (current result, 0.9.25)

The ADR's historical `KeyError: 'links'` was re-checked, not assumed.

1. Graph written WITH clustering (the default) at `src`:
   `graphify benchmark <graph.json>` succeeds:
   `Corpus: 2,642,700 words -> ~3,523,600 tokens (naive); Graph: 52,854 nodes,
   169,755 edges; Avg query cost: ~562,189 tokens; Reduction: 6.3x fewer tokens
   per query` (per-question 3.3x, 8.7x, 396.0x, 3.2x, 14.1x). The average query
   cost shown is graphify's own heuristic on this large graph; it is not a
   measurement of the adapter's bounded results.
2. Graph written with `--no-cluster`
   (`graphify extract $WT/packages --code-only --no-cluster --out nc`) then
   `graphify benchmark nc/graphify-out/graph.json` still fails with
   `KeyError: 'links'` (`networkx/readwrite/json_graph/node_link.py`,
   `for d in data[edges]`). Cause: `--no-cluster` writes the raw extraction
   (`nodes`, `edges`, `hyperedges`, `input_tokens`, `output_tokens`) while the
   clustered output uses the NetworkX node-link keys (`directed`, `multigraph`,
   `graph`, `nodes`, `links`, `hyperedges`). The historical failure used
   `--no-cluster`; the failure is reproduced for that flag only, and benchmark
   works on clustered output. The adapter therefore omits `--no-cluster` and
   pins the `links` schema.

## Test run

```sh
cd packages/semaprax-harness-adapters/graphify/test && python3 -m unittest -v test_adapter
```

12 tests, real graphify 0.9.25, OK in about 2.8 s. They include the
`sandbox-exec -p '(version 1)(allow default)(deny network*)'` run (the test
first proves the sandbox blocks a socket connect), the planted-environment
shim, stale-after-edit, restart reuse and unsupported-version/identity refusal.

## Not measured

Task accuracy, model-token cost of answers, comparison against Graft or
native-only context, Linux, any Graphify version other than 0.9.25. Model-backed
extraction (docs, PDFs, media, LLM providers) was never run.

# 2026-10-04 host run (HP-07 revisit)

Environment, commands and the full six-task table are in
[../graft/EVIDENCE.md](../graft/EVIDENCE.md) (same run, same snapshots, same
required-fact checks). Graphify 0.9.25, `/Users/kevin/.local/bin/graphify`, macOS arm64.

## Fixes made for the real host path

* Digests were bare hex; now `sha256:<hex>` (contract). Line digests hash one line
  without terminator, which is the broker's convention, so items verify.
* `coverage.extraction_errors` was a list of strings; the contract requires
  `[{path, reason}]`. Only log lines that name a project file are reported.
* The adapter read `limit`; it now reads the contract's `max_items`. Default refresh is
  now `auto` (rebuild when the graph is behind the tree) because the broker never
  resends a refresh hint; `never` returns `stale`, `rebuild` forces.
* Items from `references` carry `edges` `[{target, relation, provenance}]`;
  provenance is `structural` for resolved EXTRACTED edges and `inferred` otherwise,
  never `compiler-verified`.

## Results against the revisit gates (macOS arm64, 0.9.25)

| Gate | Result |
| --- | --- |
| 1 pinned version outside Cargo | met: adapter verifies `graphifyy` 0.9.25 from dist-info metadata |
| 2 local code-only by default | met: `extract --code-only`, four-variable allowlisted env; planted `*_API_KEY`/`GRAFT_PROVIDER`/proxy values not found in index, cache or output; cold and warm runs pass with network denied |
| 3 generated graphs outside Git | met: graph lives in the harness cache; `git status --ignored` of the project is clean |
| 4 lower size at equal-or-better accuracy on real tasks | partial: smaller than full-source on the three real-file tasks (0.19x Rust, 0.09x JS, 0.06x Python) and all required facts present; larger on the 3-file fixture (4.6x-7.1x) and missed the T1 caller fact; accuracy is not a model-answer measurement |
| 5 merge SEMAPRAX nodes, `.spx` not opaque | met by composition, not by extraction: `.spx`/`.spatch`/`Cargo.toml` are reported skipped with a reason, checked `.spx` facts come from the compiler (identical across Graft and Graphify, `project_switches_graft_to_graphify_by_config_only`) |

## `graphify benchmark` today (0.9.25)

`graphify extract crates/semaprax-harness/src/context --code-only --no-cluster --out <d>`
then `graphify benchmark <d>/graphify-out/graph.json`: still `KeyError: 'links'`
(`networkx/.../node_link.py`, `for d in data[edges]`). Without `--no-cluster`, benchmark
on the same 9-file corpus reports `Benchmark error: No matching nodes found for
sample questions.` (the earlier `src` run in this file benchmarked successfully, 6.3x).
The adapter does not use `--no-cluster` and does not depend on `benchmark`.
