# Project Agent Transport v5

Audience: agent and tool authors, plus compiler contributors.

Status: additive implementation and focused nonignored evidence are exact-tag
hosted green at v0.2.0; transport promotion is not claimed.

## Scope

`semapraxd --stdio --allow-project-owned-data` selects the read-only
`semaprax.agent-transport.v5` profile over one host-selected Project Manifest
v8 `owned-data-api.v1` subject. It adds exactly two semantic methods to the
unchanged read-only v2 inspection surface:

- `project/api-describe`
- `project/npm-build-inline`

The startup flag is mutually exclusive with `--allow-project-rename` and
`--allow-project-workflow`, and startup rejects any non-v8/non-owned-data
subject before a protocol response. Default v2 and opt-in v3/v4 retain their existing
schemas, method inventories, request grammar, response bytes, and authority.
V5 reports neither the v3 rename methods nor the v4 change/build methods.

## Revision and framing contract

Both v5 methods are admitted only in `open` state and require the exact
`project_revision` and `workspace_revision` returned by `workspace/open`.
Missing, stale, foreign, or surplus revision facts reject. Every request uses
the common pre-render and post-render held-input authentication; observed drift
is absorbing and prevents the response payload from being written.

The common NDJSON and JSON-RPC grammar remains closed. Notifications do not
execute either v5 semantic method. The configured response budget includes the
complete JSON-RPC wrapper and terminal LF. A complete response that cannot fit
is replaced as a whole by the existing `-32001` response and terminates the
session; no response is truncated.

## `project/api-describe`

Parameters contain only the two exact revisions. The result is exactly:

```json
{
  "descriptor": {},
  "descriptor_digest": "sha256:..."
}
```

`descriptor` is the canonical `semaprax.public-owned-data-api.v1` object,
derived from retained validated HIR and independently replayed against the
same Project schema, Project revision, Workspace revision, Project graph
digest, and selected stable IDs. The transport never rediscovers a target
signature.

## `project/npm-build-inline`

Parameters contain the exact revisions and optional `max_bytes`. There is no
target, output, path, artifact selection, tool, environment, or publication
parameter. `max_bytes` is an unsigned host integer within both the fixed
40 MiB Project carrier ceiling and the id-sensitive effective response
allowance. The effective allowance subtracts the complete JSON-RPC wrapper,
canonical descriptor, descriptor digest, result keys, braces, and terminal LF
from the configured response limit before carrier construction. The default is
the smaller of 8 MiB and that allowance. Zero and any widening reject before a
carrier is built.

The result is exactly:

```json
{
  "descriptor": {},
  "descriptor_digest": "sha256:...",
  "build": {}
}
```

`build` is the canonical `semaprax.project-npm-build.v7` carrier. Before the
response becomes observable, the implementation:

1. derives and independently replays the retained descriptor;
2. builds through the ordinary Project v8 npm API;
3. independently replays the complete carrier, semantic recipe, descriptor,
   Wasm, fixed artifact inventory, per-artifact hashes, cumulative byte count,
   and payload digest; and
4. recovers the carrier's typed descriptor binding and exactly compares its
   canonical bytes, digest, Project revision, Workspace revision, and Project
   graph digest with the separately retained descriptor.

`ProjectNpmBuild::inspect_envelope` checks context-free compiler consistency.
`ProjectNpmBuild::verify_public_api_descriptor` additionally checks the exact
descriptor subject. Neither method grants build, materialization, or
publication authority.

## Evidence and nonclaims

The focused gate is:

```sh
cargo test --locked -p semaprax --all-features \
  --test project_agent_transport_v5 -- --test-threads=1
```

The authored evidence covers the exact method set; v2 method exclusion;
canonical descriptor and carrier equality with direct retained-Project APIs;
exact and minus-one carrier limits; exact response framing; stale and surplus
parameters; request-selected target/path/output rejection; silent
notifications; zero-write inventory; independently replayed returned bytes;
duplicate-key and string-decoy carriers; and self-consistent foreign carrier
remints rejected by the typed descriptor binding.

The focused suite ran as part of exact tag commit
`5f6fb9655fdec92c57ab71615cfd7bfa8cc76051` in
[release run 33608662244](https://github.com/wavect/semaprax/actions/runs/33608662244).
That execution does not supply cross-language client validation or an explicit
transport-support decision.

This profile grants no filesystem write, package materialization, source or
workspace mutation, rename, patch, change, output selection, target selection,
native or Rust build, publication, process launch, tool or environment access,
target execution, network service, persistence, caching, concurrency, batch,
recovery, provenance, approval, signature, or reusable authorization. Formal
promotion remains open until the remaining owning gates and support decision
are complete.
