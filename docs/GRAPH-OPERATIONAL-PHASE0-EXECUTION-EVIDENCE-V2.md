# Graph-operational Phase 0 execution evidence v2

Status: reviewed local aggregate passed at exact subject
`4e6751f92525ed8e4bb5e859233616df7adc86d1`; bundle
[`76b2e7fab8a5c90fac6ed9c06fff8debe6c97bef015ff26ac53daf8b6ae0eeff`](evidence/graph-operational-phase0/4e6751f92525ed8e4bb5e859233616df7adc86d1/76b2e7fab8a5c90fac6ed9c06fff8debe6c97bef015ff26ac53daf8b6ae0eeff/evidence.json).

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

The reviewed Darwin arm64 aggregate freshly executed every component on the
same subject. It used explicit Node 24.3.0 and TypeScript 5.8.3 tools, the
selected Visual Studio Code 1.135.0 product, and Python 3.14.2 with a provisioned
`mcp` SDK distribution 1.27.0. The archived aggregate contains 20 authenticated
child artifacts plus its canonical envelope.

All child bundles must bind the same commit and tree. The runner replays their
closed inventories, artifacts and bundle identifiers; records selected
executable and launcher identities, versions, and the `mcp` package payload;
executes the independent provisioned Python `mcp` SDK 1.27.0 flow; and publishes
a new aggregate only after repository, tool, and artifact checks remain stable.

A passing aggregate is local exact-subject Phase 0 evidence. It does not claim
an exact release tag, remote-main or later-head status, hosted/cross-platform
execution, network isolation, full MCP certification, manual editor UI,
VSIX/Marketplace distribution, native/Wasm runtime, full quality, completion
matrix promotion, or graph-operational programme completion.
