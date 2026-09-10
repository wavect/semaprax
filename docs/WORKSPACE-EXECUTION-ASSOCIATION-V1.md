# Workspace Execution Association v1

Status: **HOSTED GREEN** for the bounded v0.4.0 generation binding and runtime replay.

Audience: compiler, Project, ProgramRoot, semantic-service, and runtime
maintainers.

The [v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md) supersedes the former
local-only evidence status without widening the runtime or service contract.

This SEG-04 profile associates runtime execution with one exact immutable
generation retained by `SemanticWorkspaceService`. It does not change the
existing workspace snapshot, Project, ProgramRoot, execution, evidence, or
receipt structures.

## Exact selection and replay

`WorkspaceExecutionBinding::select(service, version, expected_workspace,
expected_root)` selects one retained current snapshot for
`WorkspaceExecutionRootVersion::V1`, `V2`, or `V3`. The selector chooses the
corresponding compiler-owned ProgramRoot and retains the complete immutable
generation, including its selected root and semantic image. Caller-supplied
root bytes are never decoded or adopted.

The binding receipt is a bounded public association of the selected workspace
revision, ProgramRoot schema and digest, Project revision, semantic-image
digest, and `authority: false`. It contains no private task or proposal bytes,
private proposals, or authority. Existing root and receipt bytes remain
unchanged.

`WorkspaceExecutionBinding::replay` first reselects the retained state through
the service, then exact-compares the canonical receipt bytes and digest. Replay
reconstructs compiler state from the fresh retained service generation; it does
not deserialize an arbitrary root or trust a self-consistent remint. Selection
or replay rejection reports the existing `SPX-G583` diagnostic.

## Consuming runtime producers

The binding exposes `bind_once`, `bind_iterative`, `bind_typed`, and
`bind_linked_typed`. Each
delegates to its existing runtime producer with the internally selected
Project and ProgramRoot. The binding itself is opaque and cloneable only as a
reference to the same immutable generation; it opens no store and gains no
handler, task, proposal, filesystem, network, or publication authority.

`bind_linked_typed` selects imported Agent roles from that exact retained
workspace generation and delegates to the linked Agent runtime v2 producer.
Imported roles therefore retain the workspace's exact Project and ProgramRoot
binding; the result uses the same closed `typed` runtime kind and
`semaprax.workspace-runtime-association.v1` association as an ordinary typed
binding. It reuses the existing currentness, run, durable, and migration
producer paths rather than introducing a second workspace association. Its
two focused `workspace_linked_typed_*` tests cover imported roles running
three turns with joined evidence and an in-memory refresh of an imported
helper rejecting the stale binding before any host call.

Binding computes a runtime association over the exact deployment, invocation,
and execution revision before execution. Only a consuming `run` can produce
the separate evidence association, which joins that bound runtime to its actual
returned EvidenceRoot. No constructor accepts caller-authored evidence.
Neither association authorizes execution by itself.

The additive schemas are `semaprax.workspace-execution-binding.v1`,
`semaprax.workspace-runtime-association.v1`, and
`semaprax.workspace-runtime-evidence.v1`. They use the execution-root canonical
JSON and schema-plus-NUL SHA-256 digest convention. Binding receipts are capped
at 16,384 bytes; runtime and evidence receipts contain only a fixed set of
compiler-produced digests and the closed runtime-kind label.

The typed durable path delegates to the existing runtime v2 durable producer.
The caller retains responsibility for the trusted checkpoint store and its
existing checkpoint contract. This profile does not claim a durable semantic
service or a new checkpoint or migration association. `into_evidence` keeps
the existing producer-evidence transfer semantics.

## Refresh and current execution

A binding remains coherent as a historical association after the service
refreshes to another generation. `require_current` reselects the binding's
workspace and root against the service and rejects drift with `SPX-G583`.
Ordinary `run` may execute the retained immutable generation selected by the
binding. Typed `run_current` borrows the service for the run, checks the
binding while that borrow is held, and rejects stale state before any retained
stage or injected operation executes; the service cannot refresh concurrently
through that call.

## Boundaries

The profile adds no wire, MCP, CLI, disk-store, or snapshot-structure route.
It owns no persistence and does not expose private task or proposal bytes.
Embedding hosts may persist and replay the public receipt, while trusted
checkpoint storage remains the existing caller-owned durable runtime boundary.
All existing root, image, service, execution, evidence, and receipt schemas
retain their prior meaning and bytes.

Focused evidence is selected by:

```sh
cargo test --locked -p semaprax --test agent_runtime_v1 execution_revision -- --nocapture
```

The original twelve-test selection included the direct runtime and migration
chains plus four workspace tests; later additions extend that selector. The
workspace cases exercise V1/V2/V3 binding and actual execution, fresh-service
receipt replay, independently reminted forgeries, same-source and changed
exact-v3 refresh, historical execution, three-turn iterative and typed dispatch,
zero-host durable replay, foreign source rejection, and stale `run_current`
rejection before host dispatch. Receipts are checked for private invocation-data
disclosure. The released injected-handler implementation has hosted-green
evidence; it does not add native/Wasm Agent execution or a durable semantic
service.

The additive [Workspace Execution Migration v1](WORKSPACE-EXECUTION-MIGRATION-V1.md)
profile composes two retained workspace producers through the existing migration
and trusted recovery paths without changing this single-generation contract.
