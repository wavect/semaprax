# Harness Adapter SDK v1

Audience: adapter authors. Companion to
[HARNESS-PROVIDER-V1](HARNESS-PROVIDER-V1.md), which is authoritative for the
descriptor, envelopes and wire protocol; this page does not restate them.
Tracked by HP-16 (#432).

An adapter is a separate program plus a data descriptor. Adding one needs no
edit to the compiler, the host's verb table, or any provider-name switch.
Nothing here enters Cargo's default dependency graph; ordinary
`check`/`build`/`run` never start an adapter.

## Onboarding checklist

A provider for an existing capability slot consists of exactly:

1. **Descriptor** `harness-provider.json` (`semaprax.harness-provider.v1`):
   identity, runtime/entry, upstream versions, capabilities and operations,
   platforms, resource limits, permission *requests*.
2. **Capability adapter**: a program speaking `semaprax.harness-rpc.v1` over
   stdio. Helpers: `packages/semaprax-harness-adapters/sdk/python` and
   `sdk/node` (framing, handshake, envelope echo, cancel bookkeeping). The Rust
   example implements the same frames by hand with `serde_json`.
3. **Configuration schema**: the descriptor's `config.fields`. Project values
   come from `semaprax.harness.toml`; unknown keys are errors.
4. **Support and license manifest**: `support.license`, `support.isolation`,
   `upstream.versions`, `platforms`, and `support.tested` (a record of runs, not
   a claim; empty is legal and means unverified).
5. **Conformance tests**: the capability-specific suite (below) plus your own
   tests run by your language's runner.

Select the provider in project config by id; no host change is needed:

```toml
[capability."context.repository"]
mode = "auto"
provider = "org.example/source-index"
```

## Examples

All under `packages/semaprax-harness-adapters/examples/`:

| Directory | Capability | Runtime | Notes |
| --- | --- | --- | --- |
| `source-index-python/` | `context.repository/v1` | Python 3 stdlib | third context provider; `index.py` also runs as a standalone CLI |
| `decision-node/` | `decision.evaluate/v1` (`model-route/v1`) | Node ESM | threshold rule, abstains on missing features |
| `output-view-python/` | `command.view/v1` post-execution | Python 3 stdlib | dedupes repeats, always keeps `error|fail|panic` lines, never returns exit status |
| `context-rust.harness-provider.json` + `crates/semaprax-harness/examples/context_adapter_rust.rs` | `context.repository/v1` `search` | Rust (cargo example) | hand-written frames, same result shape as the Python provider |
| `hostile-python/` | any of the three | Python 3 stdlib | fixtures for conformance mutation tests; `HOSTILE_MODE` env selector; never a real provider |
| `tools/drive.py` | n/a | Python 3 stdlib | initialize + one invoke + shutdown, prints each frame |

The Rust example uses `serde_json` and `sha2`, both already dependencies of
`semaprax-harness`. It is a cargo *example*: it is built only on request and is
not part of any normal build artifact.

## Runnable journey

Run from the repository root. Python 3 only; no install step.

```sh
A=packages/semaprax-harness-adapters

# 1. The index works with no Semaprax involvement at all.
python3 $A/examples/source-index-python/index.py search $A/sdk/python AdapterError

# 2. Drive the adapter over real protocol frames.
python3 $A/examples/tools/drive.py \
  --kind context.repository --op search --payload '{"query":"AdapterError"}' \
  --root $A/sdk/python -- python3 $A/examples/source-index-python/adapter.py
```

`drive.py` sets `SEMAPRAX_HARNESS_PROJECT_ROOT` from `--root` (a real host
provides it; adapters must read the project only from there), sends
`harness/initialize`, one `harness/invoke` and `harness/shutdown`, and prints
each response. Step 2 printed (recorded 2026-10-04, Python 3.12.12, macOS
aarch64; the `project` ids are the driver's placeholders):

```json
{ "id": 1, "jsonrpc": "2.0", "result": {
    "accepted": [{"kind": "context.repository", "operations": ["orient", "search", "skeleton", "references"], "version": 1}],
    "protocol": "semaprax.harness-rpc.v1" } }
{ "id": 2, "jsonrpc": "2.0", "result": {
    "capability": {"kind": "context.repository", "version": 1},
    "diagnostics": [],
    "invocation_id": "inv-000001",
    "payload": {
      "coverage": {"complete": true, "exhaustive": true, "indexed_files": 1, "skipped": []},
      "items": [
        {"digest": "f2ca2678a33e039531304ea90ad71688a34c5ba9e91ab920ad28a5dd831616b1",
         "language": "python", "path": "semaprax_harness_adapter.py", "provenance": "structural",
         "rank": 150, "span": {"end_line": 16, "start_line": 16},
         "text": "class AdapterError(Exception):"},
        {"digest": "47d8f6c5ccb089ce99d51466a47deee06d839f35f48d1a653bd4e14c69593618",
         "language": "python", "path": "semaprax_harness_adapter.py", "provenance": "structural",
         "rank": 100, "span": {"end_line": 89, "start_line": 89},
         "text": "                    except AdapterError as err:"}] },
    "project": {"id": "pppp...", "revision": "rrrr...", "worktree": "wwww..."},
    "provenance": {"adapter_version": "0.1.0", "provider_id": "org.example/source-index", "upstream_version": "builtin-0.1.0"},
    "schema": "semaprax.harness-result.v1",
    "status": "complete" } }
{ "id": 3, "jsonrpc": "2.0", "result": {} }
```

(Output above is reformatted onto fewer lines; `drive.py` pretty-prints with
sorted keys. The 64-character ids are shortened here.) The driver exits 0 when
all three replies are JSON-RPC results.

Next steps once the host verbs from HP-02 exist:

```sh
semaprax harness adopt $A/examples/source-index-python/harness-provider.json
semaprax harness trust org.example/source-index
semaprax harness resolve
```

`adopt`, `trust` and `resolve` are owned by HP-02 and are not exercised by this
lane; no output for them is shown. The result of the conformance step is also
pending:

```sh
semaprax harness conformance $A/examples/source-index-python/harness-provider.json
```

**Pending:** `semaprax harness conformance <descriptor>` is provided by the
host (HP-16 conformance module, a separate lane). Until it lands, the evidence
for these examples is their own tests:

```sh
python3 -m unittest discover -s $A/examples/source-index-python
python3 -m unittest discover -s $A/examples/output-view-python
python3 -m unittest discover -s $A/examples/hostile-python
(cd $A/examples/decision-node && node --test)
cargo build --offline -p semaprax-harness --example context_adapter_rust
python3 -m unittest discover -s $A/examples/tools
```

## Conformance model

There is no generic ping suite. The host runs a suite per capability kind:

- `context.repository`: result paths relative and inside the project; spans
  within the file; digest matches the line; revision echoed unchanged;
  non-exhaustive coverage never reads as "no references".
- `command.view`: view is derived from the captured output; critical
  (`error|fail|panic`) lines survive; `lossless` is false when anything was
  dropped; no exit status in the payload.
- `decision.evaluate`: choice is in `options` or null with `abstain: true`;
  scores finite 0..1; deterministic for equal input.
- `skill.catalog`: bounded `list`; `load` by exact digest.
- `model.generate`: reuses the existing `ProviderAdapter` conformance.

Common hostility checks apply to every kind. `hostile-python` provides a
fixture per behaviour so a suite can prove it *fails* the mutant:

| `HOSTILE_MODE` | What the host must catch |
| --- | --- |
| `spoof_invocation`, `spoof_project`, `fake_revision` | result binding differs from the request |
| `wrong_protocol` | protocol/version mismatch at handshake |
| `flood`, `oversized_frame`, `stderr_flood` | frame count/size and stderr bounds |
| `malformed_frame` | non-JSON on stdout |
| `unsolicited_request`, `sampling_request` | adapter-originated requests |
| `path_escape`, `absolute_path` | path escalation in context items |
| `drop_critical_error` | command view dropping an error line |
| `forbidden_model` | decision choice outside `options` |
| `ignore_cancel` | cancellation; leaves a `sleep 300` grandchild that group kill must reap |
| `crash_on_invoke`, `hang_on_initialize` | crash and handshake timeout |
| `secret_probe` | reads `SECRET_PATH`; the host must not have granted it |

`hostile-python/test_hostile.py` verifies that each mode really misbehaves on
the wire (18 tests). It does not itself assert host rejection; that is the
host conformance runner's job.

## Identity versus trust

The descriptor digest, adapter entry digest and upstream executable digest
identify exactly which code ran. A checksum detects drift; it does not prove
the code is safe, correct or honest. Trust is a separate, machine-local grant
(`trust.json`) bound to those digests and to the permissions requested.
`support.tested` and `support.license` are author records; no descriptor can
mark itself production-supported. Unsupported or untested
(upstream version, OS, operation) cells fail or remain *unverified*.

## Update and revocation

- Any change to the descriptor, adapter entry or upstream executable
  invalidates the grant; re-adopt and re-trust.
- Widening a permission invalidates the grant even if the code is unchanged.
- `semaprax harness revoke <provider-id>` removes the grant; a frozen lock
  naming the provider then refuses to run until re-resolved.
- Bump `provider.version` for any behaviour change and `adapter.version` for
  adapter-only changes; never reuse a version for different bytes.

## Backward compatibility

- The protocol is `semaprax.harness-rpc.v1`; the descriptor declares
  `protocol.min/max`. A mismatch fails at negotiation.
- Capability schemas are fixed per `kind/version`. New fields arrive only in a
  new capability version; providers keep working against the version they
  declared.
- A published diagnostic code keeps its meaning.
- Adapters ignore nothing silently: unknown operations answer `unsupported`.

## Explicitly unsupported

- Loading adapters as native libraries / unstable Rust dylib ABI.
- An `execute(any_json)` generic interface.
- A marketplace, remote registry, automatic install or upgrade.
- New capability kinds without host support, a versioned contract and their
  own conformance; unknown kinds are shown as `inactive`, extension kinds
  (`x.<org>/<name>/v<N>`) always inactive.
- Network or secret access by default; permissions are requests, and
  restricted isolation is offered only where the host can enforce it.
- Claims of hosted, multi-platform or production support from a descriptor
  alone. The examples here were run only on macOS aarch64 with the versions
  noted above; Linux is listed in `platforms` but unverified.

## Test evidence (this lane, 2026-10-04, macOS aarch64)

See the commands under "Pending" above; results are recorded in the lane
report, not fabricated here.
