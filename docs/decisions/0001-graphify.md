# ADR 0001: Defer Graphify repository indexing

> Updated 2026-10-04: see [2026-10-04 re-evaluation](#2026-10-04-re-evaluation).
> The sections before it are the original decision and its historical evidence.

Audience: maintainers and compiler contributors.

Status: accepted decision about Graphify (default adoption still no-go; opt-in adapter supported since 2026-10-04); the current repository navigation
tool is Graft, as described in [AGENTS.md](../../AGENTS.md).

## Decision

Do not add Graphify to the build or agent bootstrap. Use `graft ask --source`
for repository code navigation, and SEMAPRAX's own `graph` and `context`
commands for checked `.spx` meaning. Graft's code index is not SEMAPRAX's
semantic graph.

## Evidence (historical)

A historical, local assessment tested Graphify 0.9.25 at commit `7e3d294`.
It kept generated output in `/private/tmp`:

```sh
graphify extract . --code-only --no-cluster --out /private/tmp/semaprax-graphify
graphify query Parser --budget 500 --graph /private/tmp/semaprax-graphify/graphify-out/graph.json
graphify benchmark /private/tmp/semaprax-graphify/graphify-out/graph.json
```

| Observation | Result |
| --- | --- |
| Indexed corpus | 17 code files; `.spx`, `.spatch`, and `Cargo.toml` skipped |
| Extracted structure | 232 nodes and 712 edges |
| Size | 254,636-byte graph for 120,266 bytes of indexed source |
| Bounded query | The 500-token `Parser` slice was useful |
| Benchmark | Failed with `KeyError: 'links'` against the newly generated graph |

The query was useful, but the assessment found that:

- `.spx`, `.spatch`, and `Cargo.toml` were not indexed;
- the generated graph was larger than the indexed source;
- the tested pre-1.0 tool's benchmark command failed on its newly generated graph;
- SEMAPRAX already owns the authoritative semantic graph for the language it compiles.

That result did not justify committing another incomplete generated graph.
The current Graft index is ignored buildable context, not the authoritative
representation of `.spx` meaning.

## Revisit gate (original)

Re-evaluate Graphify only if it adds value beyond the current Graft workflow.
Adoption would require:

1. A pinned, audited tool version installed outside the Cargo dependency graph.
2. Local code-only extraction by default; no model-backed document ingestion without explicit capability approval.
3. Generated graphs and caches excluded from Git.
4. Benchmarks showing lower tokens and equal-or-better answer accuracy on real maintenance tasks.
5. An adapter that merges SEMAPRAX graph nodes into the repository index instead of treating `.spx` as opaque text.

Relevant upstream references: [repository](https://github.com/Graphify-Labs/graphify), [documentation](https://graphify.com/docs), and [security model](https://graphify.com/security).

## 2026-10-04 re-evaluation

Scope: HP-07 (#423). Everything below is a current run, not the historical one above.

Pinned: `graphifyy` 0.9.25 (`/Users/kevin/.local/bin/graphify`) and `@nanonets/graft`
0.18.0, macOS arm64, Python 3.12 / Node v24.3.0, compiler `semaprax 0.7.0`, harness
branch `hp/hp0607` on `wavect/v090` (`7db320ffa` plus the lane commit). Adapter:
`packages/semaprax-harness-adapters/graphify` (provider `com.graphify-labs/graphify-context`),
queried through the common host, selected per project by `semaprax.harness.toml` alone.

Commands:

```sh
graphify extract crates/semaprax-harness/src/context --code-only --no-cluster --out <d>
graphify benchmark <d>/graphify-out/graph.json        # KeyError: 'links' (still)
graphify extract crates/semaprax-harness/src/context --code-only --out <d2>
graphify benchmark <d2>/graphify-out/graph.json       # "No matching nodes found for sample questions"
cargo test --offline -p semaprax-harness --test real_tools_v1 -- --ignored --test-threads=1 graft graphify
cargo test --offline -p semaprax-harness --test real_tools_v1 -- --ignored --nocapture measure_model
```

Measured comparison (six bounded tasks, byte-v1 of the harness document, same required
facts checked independently; full table and counterexamples in
[graft/EVIDENCE.md](../../packages/semaprax-harness-adapters/graft/EVIDENCE.md)):

| Arm | 3 fixture tasks (3-file project) | 3 real-file tasks (6.7-21 KB references) | Required facts |
| --- | --- | --- | --- |
| native-only | 0.76x-3.39x of full-source | 0.03x-0.09x | 2 of 9 (only the `.spx` task) |
| native + Graft | 4.3x-6.8x | 0.24x-1.38x | 8 of 9 (T1 caller missed) |
| native + Graphify | 4.6x-7.1x | 0.06x-0.19x | 8 of 9 (T1 caller missed) |

Graphify's warm-index call is about 0.03 s versus Graft's 0.45-0.9 s; its index is
14 KB (fixture) and 857 KB (repo snapshot), Graft's 18 KB and 876 KB. Cold first query:
Graphify 0.25-0.57 s, Graft 0.62-0.69 s. Counterexamples retained: on the fixture both
providers are larger than the files they summarise, both miss the caller of `renderTotal`,
and Graft is 1.38x larger than the Rust file because 0.18.0 has no Rust parser.

Gates: local-only (network denied cold and warm, planted keys not inherited), `.spx`
and manifests skipped with reasons, generated graph kept in the harness cache, native
`.spx` facts byte-identical to standalone `semaprax context`, provider switch with no
code change. Not evidenced: answer accuracy on real maintenance tasks (HP-17), large
repositories, Linux, Graphify versions other than 0.9.25.

### Outcome

Supported opt-in adapter; default adoption remains no-go. Graphify is a per-project
alternative to Graft with a measured latency advantage and a smaller result on
real-file tasks, but it adds no information the compiler does not already own for
`.spx`, shares Graft's miss on the caller fact, still ships a failing `benchmark`
command, and no accuracy evidence exists. Revisit default adoption when HP-17 shows
equal-or-better answer accuracy on maintenance tasks.

### Revisit-gate checklist (status 2026-10-04)

1. Pinned, audited version outside Cargo: met (0.9.25, identity read from dist-info; audit is the adapter's allowlist and the tests, not a third-party review).
2. Local code-only by default, no model ingestion without approval: met.
3. Generated graphs and caches excluded from Git: met (harness cache, project tree clean).
4. Benchmarks showing lower size and equal-or-better accuracy on real tasks: partly met (smaller on real-file tasks, byte-v1 not tokens; accuracy not measured).
5. Adapter merging SEMAPRAX nodes rather than treating `.spx` as text: met by composition (compiler facts via the broker, `.spx` reported skipped); Graphify itself does not extract `.spx`.
