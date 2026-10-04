# ADR 0001: Defer Graphify repository indexing

Audience: maintainers and compiler contributors.

Status: accepted decision about Graphify; the current repository navigation
tool is Graft, as described in [AGENTS.md](../../AGENTS.md).

## Decision

Do not add Graphify to the build or agent bootstrap. Use `graft ask --source`
for repository code navigation, and SEMAPRAX's own `graph` and `context`
commands for checked `.spx` meaning. Graft's code index is not SEMAPRAX's
semantic graph.

## Evidence

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

## Revisit gate

Re-evaluate Graphify only if it adds value beyond the current Graft workflow.
Adoption would require:

1. A pinned, audited tool version installed outside the Cargo dependency graph.
2. Local code-only extraction by default; no model-backed document ingestion without explicit capability approval.
3. Generated graphs and caches excluded from Git.
4. Benchmarks showing lower tokens and equal-or-better answer accuracy on real maintenance tasks.
5. An adapter that merges SEMAPRAX graph nodes into the repository index instead of treating `.spx` as opaque text.

Relevant upstream references: [repository](https://github.com/Graphify-Labs/graphify), [documentation](https://graphify.com/docs), and [security model](https://graphify.com/security).
