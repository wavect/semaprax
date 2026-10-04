# Harness Provider Host v1

Audience: toolchain contributors and adapter authors.

Status: additive specification for the downstream development harness tracked
by HP-00 (#416) and its work items HP-01..HP-17 (#417..#433). It does not
change `semaprax.plugin-manifest.v1` (`src/plugin_manifest.rs`), which stays a
read-only compiled-export descriptor and is **not** a harness-provider
descriptor.

## Position in the toolchain

```text
semaprax harness <verb>          (full toolchain, Availability::Private)
semaprax-harness <verb>          (same code, standalone host binary)
        |
        v
crates/semaprax-harness          host-only crate; no dependency on the compiler crate
  contract/   descriptors, capability payloads, envelopes, negotiation   (HP-01)
  profile/    semaprax.harness.toml, .lock, trust store, resolution     (HP-02)
  host/       stdio JSON-RPC adapter lifecycle, budgets, isolation      (HP-03)
  workflow/   development pipeline + compiler service                   (HP-04)
  context/    native-first context broker and revision-safe cache       (HP-05)
  command_view/ authoritative command result vs model-facing view       (HP-08)
  decision/   decision registry, model-route/v1, rules provider         (HP-10)
  endpoint/   local endpoint / gateway catalog adoption                 (HP-12)
  skills/     skill.catalog/v1 lazy catalog                             (HP-13)
  bridge/     external-host handshake, ownership negotiation            (HP-14)
  observe/    stage attribution over token/attempt observations         (HP-15)
  conformance/ capability conformance + hostility suites                (HP-16)
  bench/      journey benchmark contract and report                     (HP-17)
packages/semaprax-harness-adapters/<name>/   out-of-tree adapters (graft, graphify,
                                              rtk, laya/jev, examples)
```

The compiler is a **service used by the harness**: the workflow invokes the
real `semaprax` executable (an explicit path; the full toolchain passes its own
executable) for diagnostics, context, candidate preview and publication.
Ordinary `check`/`build`/`run` never start an adapter, read harness files, or
need any adapter runtime. No harness code is linked into the standalone
crates.io compiler.

Adapters are separate processes. The host contains no brand-specific dispatch:
a provider for an existing capability kind is added with a descriptor plus an
adapter program. Activating a *new capability kind* requires a host
implementation and a versioned contract in `contract/`.

## Identifiers

| Thing | Value |
| --- | --- |
| Provider descriptor schema | `semaprax.harness-provider.v1` |
| Adapter wire protocol | `semaprax.harness-rpc.v1` (JSON-RPC 2.0, LF-delimited frames on stdio) |
| Request envelope | `semaprax.harness-request.v1` |
| Result envelope | `semaprax.harness-result.v1` |
| Project configuration | `semaprax.harness.toml`, schema `semaprax.harness-config.v1` |
| Frozen lock | `semaprax.harness.lock` (canonical JSON), schema `semaprax.harness-lock.v1` |
| Machine-local state | `$SEMAPRAX_HARNESS_HOME` (default `$HOME/.config/semaprax/harness`): `installations.json`, `trust.json`, `cache/`, `retention/` |
| Capability kinds (first wave) | `context.repository/v1`, `command.view/v1`, `decision.evaluate/v1`, `model.generate/v1`, `skill.catalog/v1` |
| Extension kinds | namespaced `x.<org>/<name>/v<N>`; visible, always inactive |

`$SEMAPRAX_HARNESS_HOME` is the only machine-local location read. Discovery
never walks `$HOME`, `PATH`, `node_modules`, Python environments or editor/agent
configuration.

## Diagnostics

Harness diagnostics use `SPX-HP<letter><3 digits>`, one letter per work item:
A=HP-01 contract, B=HP-02 profile, C=HP-03 host, D=HP-04 workflow,
E=HP-05 context, F=HP-06 Graft, G=HP-07 Graphify, H=HP-08 command view,
I=HP-09 RTK, J=HP-10 decision, K=HP-11 Laya/Jev, L=HP-12 endpoints,
M=HP-13 skills, N=HP-14 bridge, O=HP-15 observation, P=HP-16 SDK,
Q=HP-17 benchmark. A code, once published, keeps its meaning.

## Provider descriptor

A descriptor is a strict JSON object (duplicate keys, unknown top-level
members, non-finite or non-integer numbers where integers are required, and
invalid UTF-8 are refused). Canonical digest: SHA-256 over
`semaprax.harness-provider.v1\0` + canonical JSON (sorted keys, no whitespace).

```json
{
  "schema": "semaprax.harness-provider.v1",
  "provider": {"id": "org.nanonets/graft-context", "version": "0.1.0"},
  "adapter": {"runtime": "node", "entry": ["adapter.mjs"], "version": "0.1.0"},
  "upstream": {"name": "graft", "package": "npm:@nanonets/graft",
               "repository": "https://github.com/NanoNets/Graft",
               "versions": ["0.18.0"], "identity_probe": ["--version"]},
  "protocol": {"name": "semaprax.harness-rpc.v1", "min": 1, "max": 1},
  "capabilities": [
    {"kind": "context.repository", "version": 1, "required": true,
     "operations": ["orient", "search", "skeleton", "references"]}
  ],
  "extensions": [{"kind": "x.example/notes", "version": 1}],
  "platforms": ["macos-aarch64", "linux-x86_64"],
  "config": {"fields": {"deep": {"type": "bool", "default": false}}},
  "permissions": {"read": ["project"], "write": ["cache"], "network": [],
                  "process": ["upstream"], "secrets": []},
  "resources": {"handshake_timeout_ms": 5000, "invoke_timeout_ms": 30000,
                "max_frame_bytes": 1048576, "max_concurrency": 1,
                "idle_shutdown_ms": 60000},
  "cancellation": "cooperative",
  "support": {"license": "MIT", "isolation": "subprocess",
              "tested": [{"upstream": "0.18.0", "os": "macos-aarch64", "result": "pass"}]}
}
```

Rules:

- `provider.id` is `<reverse-dns-or-org>/<name>`, ASCII `[a-z0-9._-]` plus one
  `/`, at most 128 bytes. Two descriptors with the same id in one resolution
  are refused (`duplicate identity`).
- `permissions` are **requests**. A descriptor never creates a grant; grants
  live only in the machine-local trust store (HP-02) and are bound to the
  descriptor digest, the adapter entry digest and the upstream executable
  digest. Any change, or any widened permission, invalidates the grant.
- `required: true` capability with an unknown kind or unsupported version fails
  negotiation **before process launch**. Unknown optional kinds and every
  `extensions` entry are reported as `inactive`.
- `support.tested` is a record, not a claim: the host never treats a
  declaration as production support (HP-16 conformance decides).
- `runtime: native` is an adapter executable built natively; its `entry[0]` is
  a path relative to the descriptor's directory (absolute or `..` refused).
- `runtime: builtin` descriptors describe host-implemented providers
  (native context, raw command view, rules decision, plain skill roots); they
  have no `entry` and no process.

## Envelopes

Request (host -> adapter, `params` of `harness/invoke`):

```json
{"schema": "semaprax.harness-request.v1",
 "invocation_id": "inv-000001",
 "project": {"id": "<sha256 of canonical project root identity>",
             "worktree": "<sha256 of worktree root>", "revision": "<snapshot digest>"},
 "lock_digest": "<sha256>",
 "capability": {"kind": "context.repository", "version": 1},
 "operation": "search",
 "deadline_ms": 30000,
 "budget": {"max_result_bytes": 65536, "remaining_calls": 8},
 "lineage": ["inv-000000"],
 "payload": {}}
```

Result (adapter -> host, JSON-RPC `result`):

```json
{"schema": "semaprax.harness-result.v1",
 "invocation_id": "inv-000001",
 "project": {"id": "...", "worktree": "...", "revision": "..."},
 "capability": {"kind": "context.repository", "version": 1},
 "status": "complete",
 "payload": {},
 "diagnostics": [{"code": "...", "message": "..."}],
 "provenance": {"provider_id": "...", "adapter_version": "...", "upstream_version": "..."}}
```

`status` is exactly one of `complete`, `partial`, `stale`, `unavailable`,
`unsupported`, `refused`, `failed`. The host refuses a result whose
`invocation_id`, `project` or `capability` differ from the request (spoofing),
whose payload fails the capability's payload validator, or that exceeds
`budget.max_result_bytes`. Results are untrusted data: no payload field can
grant execution, publication, filesystem, network or secret authority.

## Wire protocol `semaprax.harness-rpc.v1`

LF-delimited JSON-RPC 2.0 frames on the adapter's stdin/stdout; stderr is a
bounded diagnostic log, never protocol. Frame limit is the descriptor's
`max_frame_bytes`, capped by the host at 4 MiB.

| Method | Direction | Purpose |
| --- | --- | --- |
| `harness/initialize` | host -> adapter (request) | params `{protocol, host_version, descriptor_digest, offered: [{kind, version}], project}`; result `{protocol, accepted: [{kind, version, operations}]}` |
| `harness/invoke` | host -> adapter (request) | request envelope -> result envelope |
| `harness/cancel` | host -> adapter (notification) | `{invocation_id}`; a request, never proof |
| `harness/shutdown` | host -> adapter (request) | adapter answers `{}` then exits |

An adapter that sends a request or notification to the host, answers an
unknown id, or emits a non-JSON line on stdout is a protocol violation: the
host terminates its process group and quarantines it for the session with a
structured reason.

## Lifecycle

`prepared -> negotiated -> active -> draining -> closed`, plus `unavailable`
(restartable failure) and `quarantined` (terminal for the session). Start is
lazy (first invocation); an idle shutdown returns to `prepared`. Each
`(project, provider)` has its own process; concurrency and queue length are
bounded by the descriptor and host caps; deadlines and cancellation kill the
whole process group (`process_group(0)` + group `SIGKILL`) and every child is
reaped before return. A crash circuit breaker quarantines after repeated
failures. A protocol violation quarantines immediately with a structured
`SPX-HPC` reason. Only `SafeRead`/`Decision` invocations may fall back after a
crash, and only when no response arrived; a side-effecting invocation whose
request was sent is `Uncertain` and never retried. Restricted mode
(network/file isolation) is offered only where the host can enforce it
(`sandbox-exec` on macOS, `bwrap` on Linux when present); otherwise a request
for restriction is refused (`SPX-HPC003`), never silently downgraded. Plain
subprocess execution is never labelled sandboxed. The full host contract,
diagnostics and platform evidence are in `docs/HARNESS-HOST-V1.md`.

## Capability payloads (first wave)

Each kind has its own payload validator in `contract/payload/`. Shapes that
are valid for one kind are refused for another.

- `context.repository/v1` — operations `orient`, `search`, `skeleton`,
  `references`. Result items: `{path (relative), span {start_line, end_line},
  digest, provenance: compiler-verified|structural|inferred, language,
  rank, text?}` plus `coverage {complete: bool, indexed_files, skipped:
  [{path, reason}], exhaustive: bool}`. A non-exhaustive result can never state
  "no references".
- `command.view/v1` — forms `post-execution` (transform captured output) and
  `wrapper` (return a validated argv plan the host executes exactly once).
  Results carry `view {text, lossless, omissions, recovery_handle?}`; they
  never carry an exit status (the host owns the authoritative result).
- `decision.evaluate/v1` — `{task: "model-route/v1", features, options:
  [ids]}` -> `{choice: <option id>|null, scores: {id: finite 0..1}, abstain:
  bool}`. A choice outside `options` is refused.
- `model.generate/v1` — host-side wrapper over the existing
  `ProviderAdapter` SDK; payload is opaque bytes plus a logical model id that
  must be in the approved catalog.
- `skill.catalog/v1` — operations `list` (bounded metadata) and `load`
  (content by exact digest).

Every content digest in a payload (context items, skill entries, artifact
references, requested skill digests) has the form `sha256:<64 lowercase hex>`.
Paths are project-relative: no leading `/`, drive prefix, backslash, NUL, empty
or `.`/`..` segment. Shapes are closed (unknown members refused,
`SPX-HPA040`). Operations per kind: `context.repository` orient/search/
skeleton/references; `command.view` view/wrap; `decision.evaluate` evaluate;
`model.generate` generate; `skill.catalog` list/load. Result payloads are also
checked against their request: a decision `choice` and every score key must be a
request option (`SPX-HPA043`), and a loaded skill digest must equal the
requested one.

Stable diagnostics of this layer: HPA001 invalid UTF-8, 002 oversize, 003
depth, 004 node count, 005 duplicate key, 006 invalid number, 007 trailing
data, 008 raw line break in a frame, 009 JSON syntax; 010 wrong descriptor
schema, 011 plugin-manifest document, 012 unknown member, 013 malformed field,
014 provider id, 015 entry/runtime mismatch, 016 protocol range, 017 duplicate
capability, 018 resource bound, 019 unknown required kind, 020 duplicate
identity, 021 downgrade, 022 capability kind/operation, 023 unsupported
version, 030 request envelope, 031/032/033 spoofed invocation/project/
capability, 035 status/payload mismatch, 036 authority-like member, 037
malformed result envelope, 040 payload shape, 041 path, 042 exit status in a
command view, 043 choice outside options, 044 score range, 045 model id, 046
operation.

## Project configuration (`semaprax.harness.toml`)

A strict TOML subset (tables, strings, integers, booleans, string arrays).
Unknown keys and misspellings are errors.

```toml
schema = "semaprax.harness-config.v1"

[profile]
enabled = true

[capability."context.repository"]
mode = "auto"            # disabled | auto | required
provider = "org.nanonets/graft-context"   # optional explicit pin

[capability."command.view"]
mode = "auto"

[budget]
context_max_bytes = 16384
```

Resolution precedence per capability: explicit project pin, approved user
preference, the single compatible approved installation, built-in fallback.
More than one unmatched candidate yields a deterministic explanation, never a
random choice. `auto` is resolved and written to the lock before a workflow
runs and is not re-evaluated mid-step. `--frozen` refuses any difference from
the committed lock and names the exact missing/incompatible identity. The
committed files contain no absolute paths, secrets or trust grants.

Further keys: `[capability."<kind>"] scope = ["src"]` (project-relative
prefixes), `[budget] command_view_max_bytes`, and `[skills] enabled | select |
max_bytes`. A capability without a table is `auto`. `required` never falls back
to a builtin: it must be met by a pin, a user preference or the single compatible
trusted installation, otherwise `resolve` fails naming the identity. A table
named `[capability."x.<org>/<name>"]` is visible and always inactive. Absolute
paths and secret-looking values are refused (`SPX-HPB007`).

Machine-local state in `$SEMAPRAX_HARNESS_HOME`: `installations.json` (adopted
descriptor path, descriptor/entry/upstream digests, probed upstream version),
`trust.json` (granted permissions bound to those digests) and
`preferences.json`. `adopt` is the only action that runs anything: the
descriptor's `identity_probe` against an absolute `--upstream` path, with a
cleared environment (`PATH=/usr/bin:/bin`, private `HOME` and cwd), 5 s and
64 KiB bounds. An upstream inside the project is refused without
`--allow-project-local`. `trust` is the user's approval; `grant_for` is the
only issuer of a `Grant`, and `check_grant_current` re-verifies it before every
dispatch so `revoke` takes effect on the next call. Resolution reads only these
files and the project; it never scans `PATH` or `$HOME`.

Diagnostics `SPX-HPB`: 001 syntax, 002 duplicate, 003 unknown key/table, 004
type/value, 005 schema, 006 capability kind, 007 path/secret, 008 read; 010-014
lock (011 refused content, 012 missing for `--frozen`, 013 config changed, 014
identity mismatch); 020-024 local state and adoption (024 project-local
upstream); 030-034 trust (030 untrusted, 031 digest changed, 032 widened, 033
upstream missing/incompatible, 034 stale grant); 040-042 required capability
(040 pin unusable, 041 none, 042 ambiguous); 050 usage.

## CLI

`semaprax harness <verb>` (and `semaprax-harness <verb>`):

| Verb | Owner |
| --- | --- |
| `status [--json]`, `explain <kind>`, `resolve [--frozen]`, `adopt <descriptor> [--upstream <abs-path>]`, `trust <provider-id>`, `revoke <provider-id>`, `inspect <provider-id>` | HP-02 |
| `run <project> [--task <task.json>] [--apply-policy <policy.json>] [--disable]` | HP-04 |
| `context <project> <query> [--max-bytes N]` | HP-05 |
| `exec <project> -- <argv...>`, `recover <handle>` | HP-08 |
| `decide <project> <task.json>` | HP-10 |
| `endpoints <project>` | HP-12 |
| `skills <project> [list|load <digest>]` | HP-13 |
| `bridge <project> [--host <name>]` | HP-14 |
| `report <observations.jsonl>` | HP-15 |
| `conformance <descriptor> [--suite <kind>]` | HP-16 |
| `bench <corpus> [--profile <name>]` | HP-17 |

Exit codes: 0 success, 1 refused/failed with diagnostics, 2 usage error.
Every verb accepts `--json` for machine output; human output is derived from
the same report.

## Non-goals

No dynamic native library loading into the compiler, no `execute(any_json)`
interface, no marketplace or remote registry, no automatic installation or
upgrade, no rewriting of global editor/agent/shell configuration, and no
claim of hosted, multi-platform or production support without the recorded
executable evidence.

## Machine-local additions (hpwire)

`adopt <descriptor> --runtime <abs>` records the adapter runtime; `adopt --skills <abs-dir> [--origin l]` approves a
skill root. Both live in `installations.json` (`runtime`, `skill_roots`), never in a project. A bundled upstream
(`local:` package, no probe) needs no adopted executable to resolve.
