# Graph-operational Phase 1 product workflow execution evidence v1

Status: the bounded exact-subject local gate passed at commit
`3c605fe3055539a9a5f2bf83e98c8c2a521ff741`; bundle
[`8e18e9dea2050844c554a826e9485394ef44381c24915602ff52952448862cfa`](evidence/graph-operational-phase1-product-workflow/3c605fe3055539a9a5f2bf83e98c8c2a521ff741/8e18e9dea2050844c554a826e9485394ef44381c24915602ff52952448862cfa/evidence.json).
Full Phase 1 and the graph-operational programme remain **Partial**.

Audience: agent-client authors, embedding hosts, release engineers, and
programme reviewers.

## Scope

This record owns one closed `function_signature_review_publish_v1` execution.
It selects the calculator fixture's scalar `calculator.add` signature change,
the exact review and publication profiles in the capability document, three
generated client languages, and isolated local Unix bare SHA-256 Git
repositories. It is evidence for that bounded composition, not general SDK,
signature-evolution, publication-provider, or Phase 1 support.

The owning contract is [Supported graph-operational product workflow
v1](IMAGE-SUPPORTED-PRODUCT-WORKFLOW-V1.md).

## Reproduction

Run the exact runner from a clean committed worktree:

```sh
python3 scripts/graph-operational-phase1-product-workflow-evidence.py \
  --python /absolute/path/to/python3 \
  --node /absolute/path/to/node \
  --tsc /absolute/path/to/typescript-5.8.3/bin/tsc \
  --typescript-package-root /absolute/path/to/typescript-5.8.3 \
  [--output /absolute/path/to/new-bundle-directory]
```

The runner requires Node 22 or newer and TypeScript exactly 5.8.3. Its envelope
records and rechecks the selected Python, Node, Cargo, Rust, Git, TypeScript
entry point, and the complete 132-file TypeScript package payload. It clears
recorded compiler/linker and Node overrides, forces the selected Node for the
TypeScript compiler, builds under a fresh worktree-local target, and rejects
dirty or moving repository/tool inputs.

## Captured subject

| Binding | Captured value |
| --- | --- |
| Commit | `3c605fe3055539a9a5f2bf83e98c8c2a521ff741` |
| Tree | `3bebe82eabc5d496883c4ac400f871dc0339c872` |
| Subject relation | clean local `HEAD` before and after; unchanged during execution |
| Tag selection | none; no exact-tag claim |
| Host | Darwin arm64 |
| Cargo / Rust | 1.98.0 / 1.98.0 |
| Python / Node / TypeScript | 3.14.2 / 24.3.0 / 5.8.3 |

The envelope also binds the exact `Cargo.toml` and `Cargo.lock` bytes and the
selected executable bytes, resolved paths, versions, generated libtest
executables, capability profiles, test policy, handoffs, transcripts, Git
objects, receipts, and source inventories.

## Executions

| Gate | Selected result |
| --- | --- |
| Generated Python and Rust workflow target | 2 passed; the provisioned TypeScript row remained default-ignored |
| Hostile workflow target | 1 passed; 9 default hostile transitions recorded |
| Explicit provisioned TypeScript workflow | 1 passed; 2 sibling workflow rows filtered; malformed-TypeScript response became hostile case 10 |

Each generated client was compiled or checked and executed as its own bounded
subprocess codec. The harness fed its framed requests directly to an in-process
v5 session; this was not an MCP execution. Python, Rust, and TypeScript each
completed separate review and publication sessions over an isolated repository,
produced a complete handoff and transcript, published one real SHA-256 Git ref,
and independently matched the receipt, commit, parent, tree, source objects,
unrelated executable entry, and unchanged raw Project source.

The ten hostile transitions cover stale reference, source drift, failed tests,
tampered recovery, wrong approval, definite pre-pivot failure, injected result
loss after a real ref update, and malformed Python, Rust, and TypeScript
responses. Publication uncertainty remains terminal and does not authorize a
blind retry.

## Archive and replay

The tracked archive contains 18 authenticated artifacts plus the canonical
evidence envelope: three Cargo logs; the nine-case hostile intermediate
snapshot; final hostile observation and transcript; and generated client,
handoff, observation, and transcript artifacts for each language. The runner
replays every length and SHA-256 digest, the canonical inventory, and the bundle
identifier before publishing the directory.

The tracked archive is byte-identical to the private runner output. This later
documentation commit does not transfer the result to a new subject.

## Bounded conclusion

The external exact-subject gate qualifies only the frozen fixture and selected
profiles recorded here. Static discovery exposes the composition and its exact
profile binding, but never embeds or infers executed support.

The workflow-level runtime ledger records
`partial_bounded_reference_interpreter` because it binds the passing
reference-interpreter report. Both the base and candidate analysis-coverage
payloads retain deployment configuration, generated provenance, generated
artifacts, external APIs, and external consumers as `not_inspected`.

The envelope preserves these nonclaims:

- no general signature or owned-resource migration;
- no dynamic or external caller or behavioral-compatibility proof;
- no deployment-configuration or generated-provenance evidence;
- no provider, external-API, or installed-consumer validity;
- no native or Wasm runtime equivalence;
- no filesystem, checkout, or remote-Git publication;
- no physical crash, power-loss, or multi-writer atomicity proof;
- no cancellation, deduplication, retry, exactly-once, or session durability;
- no approval or authority transfer through the handoff;
- no packaged SDK, editor UI, or MCP certification;
- no network-isolation, hosted, cross-platform, or exact-release-tag claim;
- no comparative economics, full-quality, completion-matrix promotion, or
  programme completion.
