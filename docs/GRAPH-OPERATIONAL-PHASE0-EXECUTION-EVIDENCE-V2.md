# Graph-operational Phase 0 execution evidence v2

Status: aggregate runner and closed inventory authored; exact-subject execution
pending.

Audience: release engineers, compiler contributors, and programme reviewers.

This contract reruns the selected Phase 0 evidence at one clean exact local
HEAD. It does not inherit the [v1 aggregate](GRAPH-OPERATIONAL-PHASE0-EXECUTION-EVIDENCE-V1.md).
The command remains:

```sh
python3 scripts/graph-operational-phase0-evidence.py \
  --node /absolute/path/to/node \
  --tsc /absolute/path/to/tsc \
  --vscode-app '/absolute/path/Visual Studio Code.app' \
  --mcp-python /absolute/path/to/python-with-mcp-1.27.0
```

The v2 envelope requires these freshly executed dimensions:

| Dimension | Passing rows |
| --- | ---: |
| Canonical Git workflows | 4 |
| Candidate managed-publication boundaries | 4 |
| Integrated managed workflow | 1 |
| Generated clients and authored MCP | 25 |
| VS Code standalone controllers | 50 |
| Real Visual Studio Code Extension Host | 1 |
| Independent Python MCP SDK | 1 |

The aggregate therefore records 86 selected passing rows. Two default-ignored
TypeScript cases remain separately provisioned and must pass explicitly. The
managed `ACTIVE` tests are ordinary selected rows in v2 and may no longer be
reported as ignored or not selected.

All child bundles must bind the same commit and tree. The runner replays their
closed inventories, artifacts and bundle identifiers; pins tool identities;
executes the independent public Python MCP SDK 1.27.0 flow; and publishes a new
aggregate only after repository, tool, and artifact checks remain stable.

A passing aggregate is local exact-subject Phase 0 evidence. It does not claim
an exact release tag, remote-main or later-head status, hosted/cross-platform
execution, network isolation, full MCP certification, manual editor UI,
VSIX/Marketplace distribution, native/Wasm runtime, full quality, completion
matrix promotion, or graph-operational programme completion.
