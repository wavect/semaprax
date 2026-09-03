# Project Agent Transport v6

Audience: agent and tool authors, plus compiler contributors.

Status: additive implementation with focused local evidence; transport and
Project v9-v11 package promotion are not claimed.

## Scope

`semapraxd --stdio --allow-project-public-api` selects the read-only
`semaprax.agent-transport.v6` profile over one host-selected Project v8, v9,
v10, or v11 public owned-data subject. The authenticated manifest selects the
exact profile. Requests cannot select or widen it. V6 adds the same two
semantic method names as v5 to the unchanged read-only inspection surface:

- `project/api-describe`
- `project/npm-build-inline`

The flag is mutually exclusive with every other daemon authority profile.
Startup enumerates and admits exactly Project v8 `owned-data-api.v1`, Project
v9 `flat-owned-record-api.v1`, Project v10 `owned-utf8-api.v1`, and Project v11
`nested-owned-record-api.v1`. Earlier and future Project schemas fail closed.
V2-v5 schemas, method inventories, requests, responses, and authority remain
unchanged.

## Bound subject and response

Both methods require the exact Project and Workspace revisions returned by
`workspace/open`. The common held-input pre- and post-authentication surrounds
the complete read. Drift is absorbing and prevents payload publication.
Notifications do not execute either semantic method.

Both results identify their closed profile with:

```json
{
  "project_schema": "semaprax.project.v11",
  "descriptor_schema": "semaprax.public-nested-owned-record-api.v1",
  "carrier_schema": "semaprax.project-npm-build.v10",
  "descriptor": {},
  "descriptor_digest": "sha256:..."
}
```

`project/npm-build-inline` adds the canonical `build` object. The exact mapping
is v8 to owned-data descriptor/npm v7, v9 to flat-record descriptor/npm v8,
v10 to owned-UTF-8 descriptor/npm v9, and v11 to nested-record descriptor/npm
v10. The descriptor is freshly derived and independently replayed from the
retained typed Project subject.

The build request accepts only the two revisions and optional `max_bytes`.
There is no path, target, output, artifact, tool, environment, process, or
publication selector. The complete JSON-RPC wrapper, profile discriminants,
descriptor, digest, keys, braces, and terminal LF are subtracted before the
carrier ceiling is admitted. Zero, more than 40 MiB, or more than the effective
response allowance rejects before a carrier is built.

Before publication, the ordinary Project build replays its full carrier,
semantic recipe, Wasm, fixed artifact inventory, artifact hashes, cumulative
byte count, and payload digest. A profile-specific typed verifier then recovers
the descriptor only from the authenticated metadata artifact and compares its
canonical bytes, digest, Project revision, Workspace revision, and Project
graph digest to the separately replayed retained descriptor. A carrier for one
profile cannot authenticate a descriptor from another.

## Evidence and nonclaims

The focused gate is:

```sh
cargo test --locked -p semaprax --all-features \
  --test project agent_transport_v6:: -- --test-threads=1
```

The local evidence covers all four exact profiles, equality with direct
retained descriptors and carriers, complete carrier replay, stale subjects,
surplus authority selectors, legacy method exclusion, and zero-write fixture
inventories. Carrier unit tests cover profile mismatch and authenticated
metadata binding. Full current-tree, hosted, cross-language generated-client,
and promotion evidence remain open.

V6 grants no filesystem write, package materialization, source or workspace
mutation, rename, patch, change, target or output selection, native build,
publication, process launch, tool or environment access, target execution,
network service, persistence, cache, concurrency, batch, recovery, provenance,
approval, signature, or reusable authorization.
