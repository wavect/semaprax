# Graph-operational client and MCP execution evidence v1

Status: executed locally for exact subject
`85084537bfe41fd1c2d1691b19a25a9955d76731`; all 23 explicitly selected tests
passed and the ordinary run retained its one provisioned TypeScript ignore row.

Audience: release engineers, compiler contributors, and programme reviewers.

This contract defines a private, machine-readable local evidence bundle for the
existing generated-client and workspace MCP tests. It is a sibling of
`semaprax.graph-operational-execution-evidence.v1`; it does not amend, inherit,
or promote the historical canonical-Git workflow result.

## Exact runner and subject

The sole runner is:

```sh
python3 scripts/graph-operational-client-mcp-evidence.py \
  --tsc /absolute/path/to/tsc \
  --node /absolute/path/to/node
```

`--tsc` must report exactly `Version 5.8.3`; `--node` must report a semantic
version with major version 22 or later. `--output <new-bundle-directory>` may
select an exact fresh destination. Otherwise the runner writes beneath
`.semaprax/evidence/graph-operational-client-mcp/<commit>/<bundle-id>/`. The
directory is private derived evidence and is never source or authority.

The reviewed invocation produced bundle
`a20ed6b8b39d8c4e48cd277a4a6936e5acc4b8d20be55ac381d3ae22e26d8ddb`.
Its [archived envelope](evidence/graph-operational-client-mcp/85084537bfe41fd1c2d1691b19a25a9955d76731/a20ed6b8b39d8c4e48cd277a4a6936e5acc4b8d20be55ac381d3ae22e26d8ddb/evidence.json)
and four authenticated Cargo logs are evidence for that exact subject, not for
this later record commit. The host was Darwin arm64 with Rust/Cargo 1.98.0,
Python 3.14.2, Node 24.3.0 and TypeScript 5.8.3.

Before execution the runner requires a clean worktree and binds exact `HEAD`,
its tree object, and the regular-file bytes, lengths, and SHA-256 digests of
`Cargo.toml` and `Cargo.lock`. It rechecks all of them after tool inspection,
after every selected suite, and after atomically publishing the bundle. Drift
fails closed and a post-publication drift removes the new bundle.

The runner records the selected and resolved absolute path, version output,
regular-file byte length, and SHA-256 digest for Git, Cargo, rustc, Python,
Node.js, and `tsc`. It passes the recorded absolute tools through `RUSTC`,
`SEMAPRAX_TEST_CARGO`, `SEMAPRAX_TEST_PYTHON`, `SEMAPRAX_TEST_NODE`, and
`SEMAPRAX_TEST_TSC`. It also sets `CARGO_INCREMENTAL=0`,
`CARGO_NET_OFFLINE=true`, and `CARGO_TERM_COLOR=never`.
The resolved executable bytes and selected-path resolution are rechecked after
each suite and after bundle publication.

## Closed envelope and artifacts

`evidence.json` is canonical compact JSON without a terminal newline. Its
schema is
`semaprax.graph-operational-client-mcp-execution-evidence.v1`; its exact
top-level fields are `schema`, `bundle_id`, `repository`, `runner`,
`executions`, `observations`, `artifacts`, and `claims`. Unknown or omitted
fields define a different schema and are not accepted as this evidence.

The bundle contains exactly:

- `evidence.json`;
- `clients-cargo.log`;
- `typescript-cargo.log`;
- `mcp-adapter-cargo.log`;
- `mcp-cli-cargo.log`.

Each log has a bounded byte length and an authenticated SHA-256 row. The bundle
ID is derived from the four ordered log digests with the domain
`semaprax.graph-operational-client-mcp-execution-evidence.bundle.v1`. The
envelope does not treat paths or digest possession as execution authority.

## Exact executions

The ordinary client execution is exactly:

```sh
cargo test --locked --offline -p semaprax \
  --test image_typed_request_clients_v5 \
  --test image_typed_response_clients_v5 \
  --test image_recursive_repair_response_clients_v5 -- \
  --test-threads=1 --nocapture
```

It must identify the exact nine ordinary test names embedded in the runner,
pass all nine, fail none, and report the provisioned TypeScript test once as
source-ignored. Its three exact summaries are 3/0/0, 3/0/0, and 3/0/1 for
passed/failed/ignored.

The ignored TypeScript execution is selected explicitly and separately:

```sh
cargo test --locked --offline -p semaprax \
  --test image_recursive_repair_response_clients_v5 \
  provisioned_typescript_harness_checks_actual_recursive_repair_payloads_and_hostile_nested_values \
  -- --exact --ignored --nocapture
```

It must pass exactly that one test with zero failures or ignored rows and three
filtered tests. This is provisioned local evidence: the test compiles the
generated TypeScript with the recorded `tsc` and runs it with the recorded
Node.js executable.

The MCP adapter execution is exactly:

```sh
cargo test --locked --offline -p semaprax \
  --test image_mcp_transport_v1 -- --test-threads=1 --nocapture
```

It must pass exactly the eight embedded adapter test names with 8/0/0 and no
filtered tests. Two publication cases use mock publication authority.

The MCP CLI stdio execution is exactly:

```sh
cargo test --locked --offline -p semaprax \
  --test workspace_mcp_cli_v1 -- --test-threads=1 --nocapture
```

It must pass exactly the five embedded CLI test names with 5/0/0 and no
filtered tests. The inventory includes
`real_stdio_catalogue_paging_and_notification_nonexecution_are_explicit`, which
launches the locally built `semaprax serve-workspace-mcp` process and exchanges
authored JSON-RPC frames over real stdin/stdout pipes.

No passing bundle is written if a command, exact-name inventory, exact summary,
UTF-8 log, repository binding, tool requirement, size bound, or destination
condition disagrees.

## Observations and strict nonclaims

The envelope keeps execution observations orthogonal. A qualifying bundle may
record generated TypeScript/Python/Rust source checks, actual Python runtime,
actual offline Rust compile/runtime, provisioned local TypeScript
compile/runtime, the in-process MCP adapter, and the local MCP stdio subprocess
as passed. It records independent MCP client conformance, HTTP transport,
editor hosting, native/Wasm target execution, full quality, and programme
completion as `not_selected`; hosted cross-platform execution is
`not_observed`. Neither value means success.

Even a passing bundle does **not** prove:

- a later commit, hosted run, cross-platform result, or full quality profile;
- an independent MCP SDK or conformance client, HTTP/authentication transport,
  editor host, cancellation, or asynchronous scheduling;
- real Git publication, a physical CAS race, durability, remote publication,
  or crash/power-loss behavior from the mock adapter cases;
- native or Wasm target runtime behavior or backend equivalence;
- every generated schema, report shape, external consumer, or heterogeneous
  deployment contract;
- OS-level network isolation merely because Cargo was offline;
- Python standard-library, dynamic-loader, Cargo cache/registry, Node module
  closure, or TypeScript package-lock provenance from executable hashes;
- model, agent, human, latency, cost, correctness, or productivity improvement;
- any completion-matrix row or completion of the graph-operational programme.

A claim about this evidence must cite the exact subject commit, bundle path,
schema, four commands, local host/tool facts, and artifact digests. “Current
head” remains accurate only while that exact subject is the head under review.
