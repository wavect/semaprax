# Graph-operational execution evidence v2

Status: runner and closed inventory authored; exact-subject execution pending.

Audience: release engineers, compiler contributors, and programme reviewers.

This contract extends [v1](GRAPH-OPERATIONAL-EXECUTION-EVIDENCE-V1.md) without
transferring its result to a later commit. The runner remains:

```sh
python3 scripts/graph-operational-evidence.py
```

It requires a clean exact local commit and invokes three locked, offline Cargo
integration binaries serially. A qualifying envelope uses schema
`semaprax.graph-operational-execution-evidence.v2` and contains three Cargo logs
plus the SHA-1 and SHA-256 task-economics reports.

## Closed selected inventory

| Gate | Selected passing rows |
| --- | ---: |
| `graph_operational_git_workflow_v1` | 4 |
| `candidate_managed_publication_v1` | 4 |
| `graph_operational_managed_workflow_v1` | 1 |

The Git gate retains both twelve-step object-format workflows and the real
stale-ref displacement case. Its fourth row delegates a real local Git
compare-and-swap, injects loss of the successful provider result, and requires
terminal `publication_uncertain` behavior with no retry. The managed-publication
gate checks read-only preparation, exclusive-lock ordering, hostile proof and
host binding, raw-source drift, exact immutable-generation bytes, and unchanged
raw source. The integrated managed workflow joins signature evolution, sibling
merge, impact/diff review, interpreter tests, separate managed publication, and
stale-base rejection.

Every row must be nonignored and pass. The runner rejects additional rows,
missing reports, dirty input or output state, subject drift, malformed summaries,
and artifact inventory or digest mismatches. `Cargo.toml` and `Cargo.lock` remain
bound repository inputs. Output is private derived evidence, never source or
publication authority.

## Boundaries

A passing v2 bundle may claim only the selected local workflows on its recorded
host. It does not prove a physical crash, power loss, remote-repository result
loss, durability, deployment, native or Wasm runtime execution, hosted or
cross-platform behavior, full quality, a release tag, general ownership-sensitive
signature evolution, comparative productivity, or programme completion.
