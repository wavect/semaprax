# Graph-operational client and MCP execution evidence v2

Status: runner and expanded request-admission evidence authored, unrun.

Audience: release engineers, compiler contributors, and programme reviewers.

V2 replaces the runner's v1 output for new executions while preserving every
archived v1 bundle as historical evidence for its exact subject. It adds actual
Rust and provisioned TypeScript request construction and compiler admission to
the existing generated-source, Python request, three-language response, MCP
adapter, and real stdio child gates.

The runner remains:

```sh
python3 scripts/graph-operational-client-mcp-evidence.py \
  --tsc /absolute/path/to/tsc-5.8.3 \
  --node /absolute/path/to/node-22-or-newer
```

It requires a clean exact commit and writes schema
`semaprax.graph-operational-client-mcp-execution-evidence.v2`. The bundle now
contains `evidence.json` and five authenticated Cargo logs. The ordinary client
selection requires ten passes and records two source-ignored TypeScript cases.
Each ignored case is then run separately with the exact provisioned tools:
request admission and recursive response conversion. Eight adapter cases and
five real MCP stdio cases remain separate gates.

The Rust and TypeScript request consumers use generated public
`CandidateApplyIntentTypedParams` and
`request_candidate_apply_intent_typed` APIs. Each external compile/runtime emits
one exact LF-terminated JSON-RPC frame, which the live compiler session admits.
A host-derived unbound-place variant must reject with `-32000` and `SPX-G225`.
Neither generated client receives filesystem, process, source, test, build, or
publication authority, and the fixture bytes must remain unchanged.

V2 proves only the selected request and response surfaces. It does not prove a
packaged SDK, every generated method/report, full MCP conformance, HTTP,
generated-client publication, target runtime, hosted/cross-platform behavior,
full quality, or programme completion. Independent MCP SDK interoperability is
owned by the Phase 0 aggregate contract rather than inferred from these
project-authored clients.
