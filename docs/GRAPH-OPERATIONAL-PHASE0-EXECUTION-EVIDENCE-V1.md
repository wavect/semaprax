# Graph-operational Phase 0 execution evidence v1

Status: aggregate runner and independent MCP SDK profile authored, unrun.

Audience: release engineers, compiler contributors, and programme reviewers.

This contract executes the selected Phase 0 evidence set freshly at one clean
exact local HEAD. It does not combine or transfer the three historical archives.
The runner is:

```sh
python3 scripts/graph-operational-phase0-evidence.py \
  --node /absolute/path/to/node \
  --tsc /absolute/path/to/tsc-5.8.3 \
  --vscode-app '/absolute/path/Visual Studio Code.app' \
  --mcp-python /absolute/path/to/python-with-mcp-1.27.0
```

The runner invokes the canonical-Git, generated-client/MCP v2, and Visual Studio
Code host runners with fresh private output directories. It independently
builds an exact-subject compiler and runs the public Python MCP SDK 1.27.0 over
stdio. Every component must bind the same commit/tree and pass its closed
inventory. Any failure, subject drift, tool drift, malformed envelope, artifact
digest mismatch, or destination collision prevents publication.

## Independent SDK profile

`tools/mcp-sdk-conformance/harness.py` imports the public `mcp` SDK and no
SEMAPRAX client code. It negotiates MCP `2025-11-25`, drains the bounded paged
tool catalogue, requires candidate-only methods and absence of build, test, and
commit methods, opens the workspace and a candidate, and sends a `tools/call`
notification attempting to discard that candidate. A subsequent ordinary query
must still read the same candidate, proving notification nonexecution in this
ordered stdio flow. An ordinary discard then succeeds and later query rejects.
Saved manifest/source bytes must remain unchanged.

The aggregate records this as provisioned local Python MCP SDK interoperability,
not full MCP certification. The installed SDK version, bounded distribution
payload digest, runtime dependency versions, selected Python, fresh compiler,
negotiation, catalogue digest, revisions,
notification probe, and source-byte digest are machine-readable.

## One report and evidence dimensions

The canonical top-level schema is
`semaprax.graph-operational-phase0-execution-evidence.v1`. Its default private,
Git-ignored location is
`.semaprax/evidence/graph-operational-phase0/<commit>/<bundle-id>/`. It contains
one `evidence.json`, the three complete newly executed child bundles, and the
independent SDK build/stderr/observation artifacts. The aggregate authenticates
every nested artifact path, length, and digest.

`repository` records local exact HEAD/current-head-at-capture separately from
`exact_tag`. Observed tags do not become release evidence. `components` retains
each gate's own provisioning. `ignored_tests` distinguishes default source
ignores from their explicit provisioned execution and keeps the managed ACTIVE
test not selected. No ordinary/provisioned/ignored result is silently merged.

A passing report establishes one exact-subject selected local evidence set. It
does not establish remote-main or later-head status, an exact release tag,
managed ACTIVE, full MCP conformance, HTTP/cancellation, OS network isolation,
manual editor UI, VSIX/Marketplace, native/Wasm runtime, hosted/cross-platform,
full quality, completion-matrix promotion, or programme completion.
