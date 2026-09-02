# Project graph-operational workflow v1

Status: integrated managed-generation scenario selected by the authored Phase 0
v2 evidence runner; exact-subject execution is pending.

Audience: compiler contributors and agent workflow integrators.

The scenario in `tests/project_graph_operational_workflow_v1.rs` connects the
existing calculator Project, source-derived image, stable-ID selection, typed
signature change and cross-file caller migration. It merges a sibling function
display rename, obtains independently replayable semantic deltas and human source
diffs, requests fixed-policy interpreter tests, then prepares and applies a
separately approved managed Workspace publication. It also rejects a competing
signature and repeated publication against the stale managed base.

Candidate admission owns identity and manifest preservation, complete source
rebuilding, contract/ownership checks and native-C11/structural-Wasm projection.
Those checks are not runtime target conformance or external ABI compatibility.
The test requests interpreter execution when the evidence runner executes it.
Target programs remain outside this managed-generation scenario.

Publication changes only the authenticated immutable managed generation through
`ACTIVE`. The scenario asserts original `.spx` files remain unchanged. This is
therefore an integrated bounded precursor to the requested twelve-step scenario,
not its canonical Git-source commit or completion evidence. Full source commit,
general signature evolution, hostile race coverage, native/Wasm execution and
measured task-level benchmarks remain outstanding in the
[programme ledger](GRAPH-OPERATIONAL-PROGRAMME.md).

## Explicit host entry points

`project-image-store <manifest> <store-root>` derives an authenticated image and
persists source-backed inputs through the secure Project revision store. Its
stdout is a bounded receipt; a final Project drift rejection can leave a
disposable store entry but grants no source authority. The store is explicitly
selected by the host, never discovered from ambient project files.

`project-image-load <store-root> <receipt.json> <expected-image-digest>` reads a
bounded regular receipt without following a leaf symlink, rebuilds stored source
through Project admission, checks the exact receipt/image binding and prints the
canonical image. This is not serialized-HIR restoration or a warm compiler cache.

`serve-diagnostics <manifest>` selects diagnostic candidate protocol v4 without
test execution. `serve-diagnostics-tested <manifest>` explicitly adds the fixed
host test policy. Neither grants store, raw-source, publication, build, network
or artifact-materialization authority. Earlier image/candidate/test sessions
retain their existing selected capabilities.
