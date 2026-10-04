# Harness Adapter SDK v1

Status: additive development-harness specification (HP-00); local macOS aarch64 evidence only.

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

## Adopt, trust, resolve

`adopt` records the descriptor and digests, `trust` approves its requested
permissions (bound to those digests), `resolve` writes the lock. Select the
provider in `semaprax.harness.toml` as shown above. An adapter whose
`upstream` is only its own bundled code (a `local:` package with no identity
probe) has no separate executable to adopt; `trust` refuses it with
`SPX-HPB033`, so a third-party provider that ships its own index should omit
the `upstream` block (the integration test `hp_hp16b_third_provider_journey_without_core_edits`
does exactly this from a copied directory and then runs `adopt`, `trust`,
`resolve` and a `context.repository` search through the host). The conformance
runner itself never needs global trust: it adopts and trusts in a throwaway
harness home and, for the bundled-upstream examples, derives the temporary grant
in memory.

## Conformance runner

```sh
semaprax harness conformance <descriptor> \
  [--suite context|command|decision|skill|common|all] \
  [--runtime <abs python|node>] [--upstream <abs>] [--hostile-runtime <abs python>] \
  [--isolation none|restricted] [--env NAME=VALUE]... [--json]
```

Exit 0 unless a case failed (1); usage errors exit 2. Paths are absolute where
they name executables; `PATH` is never searched. The runner launches the
adapter through the real host with a temporary harness home, project, cache and
grant (network, process and secret classes are never granted, whatever the
descriptor requests), runs one suite per *active negotiated* capability, plus
`common` (adapter identity, binding, declared `max_frame_bytes`, authority,
cancellation, recursion guard) and `common.hostility` (the host refusing the
`hostile-python` fixture). Every report states
`"support_decision": "not-a-support-decision"`; cells that were not executed
are `unverified`. Digest rule for context items: `sha256:` of the span's lines
joined with LF.

Real runs (2026-10-04, macOS aarch64, Python 3.12, node v24.3.0):

```text
$ semaprax harness conformance $A/examples/output-view-python/harness-provider.json --suite command --runtime $PY
conformance report for org.example/output-view (not-a-support-decision) - pass
suite command.view [adapter]: pass
  pass       critical-lines-survive-or-loss-is-declared
  pass       lossless-claim-is-honest
  pass       never-carries-exit-status
  pass       deterministic-for-equal-input
  unverified raw-recovery
             raw recovery is performed by the host's `recover` verb (HP-08); only handle presence is observed
  pass       cancellation-cooperative
(exit 0)

$ semaprax harness conformance $A/examples/source-index-python/harness-provider.json --suite common --runtime $PY
conformance report for org.example/source-index (not-a-support-decision) - pass
suite common [adapter]: pass
  pass       platform-declared
  pass       negotiation-visible
  pass       identity-and-binding
  pass       revision-rebinding
  pass       declared-frame-limit-enforced
  pass       no-ambient-authority
  pass       endpoint-escalation-refused
  pass       cancellation-cooperative
  pass       recursive-invocation-refused
suite common.hostility [host]: pass
  pass       spoofed-invocation-id        pass  spoofed-project-id     pass  stale-revision-refused
  pass       protocol-version-mismatch    pass  malformed-frame        pass  oversized-frame
  pass       response-flood               pass  unsolicited-host-request  pass  sampling-request
  pass       path-escape                  pass  absolute-path          pass  forbidden-choice
  pass       crash-is-contained           pass  handshake-timeout      pass  stderr-flood-bounded
  pass       budget-abuse-job-cap         pass  cancellation-group-kill-with-grandchild
  pass       secrets-unrestricted-recorded-honestly  pass  secrets-restricted-isolation
(exit 0; the hostility lines are shown two or three per row here, the runner prints one per line)
```

A bad adapter is caught. `HOSTILE_MODE=drop_critical_error` makes the hostile
fixture delete error lines from a command view:

```text
$ semaprax harness conformance $A/examples/hostile-python/harness-provider.json --suite command \
    --runtime $PY --env HOSTILE_MODE=drop_critical_error
conformance report for org.example/hostile (not-a-support-decision) - fail
suite command.view [adapter]: fail
  fail       critical-lines-survive-or-loss-is-declared
             a critical error line was dropped without a lossy marker covering it and a recovery handle
  ...
(exit 1)
```

`--json` prints the canonical report (`semaprax.harness-conformance-report.v1`),
byte-identical across runs. Trimmed, for source-index `--suite context`:

```json
{"schema": "semaprax.harness-conformance-report.v1", "support_decision": "not-a-support-decision",
 "subject": {"provider_id": "org.example/source-index", "adapter_version": "0.1.0", "license": "Apache-2.0",
             "os": "macos-aarch64", "runtime": {"kind": "python", "executable": "/abs/python3"},
             "upstream": {"name": "source-index", "declared_versions": ["builtin-0.1.0"]},
             "operations": {"context.repository": ["orient", "search", "skeleton", "references"]},
             "isolation": {"declared": "subprocess", "requested": "none", "observed": "not-recorded"}},
 "suites": [{"name": "context.repository", "subject": "adapter", "verdict": "pass",
             "cases": [{"name": "finds-planted-symbol", "verdict": "pass", "evidence": {"items": 1, "operation": "references"}}]}],
 "inactive_capabilities": [], "summary": {"pass": 7, "fail": 0, "unverified": 0}, "verdict": "pass"}
```

`isolation.observed` is filled from the `common` suite (it launches the
adapter and records the host's mode); other suites alone report `not-recorded`.
`--isolation restricted` runs the adapter under OS enforcement where the host
offers it and refuses (`SPX-HPC003`) where it does not.

`model.generate` is delegated: the runner reports
`"delegated": "provider-adapter-conformance"` (an `unverified` case) and does
not re-implement the root crate's `run_conformance_suite`
(`src/provider_adapter_sdk/conformance.rs`), which model adapters keep using
through the toolchain bridge.

Harness tests (all use `conformance::` as selector):

```sh
cargo build --offline -p semaprax-harness --example context_adapter_rust
cargo test --offline -p semaprax-harness --test harness_v1 conformance:: -- --test-threads=2
# 14 passed, 0 failed
```

The adapters' own tests remain:

```sh
python3 -m unittest discover -s $A/examples/source-index-python
python3 -m unittest discover -s $A/examples/output-view-python
python3 -m unittest discover -s $A/examples/hostile-python
(cd $A/examples/decision-node && node --test)
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
the wire (18 tests). Host rejection is asserted by the `common.hostility` suite;
the capability suites additionally fail when run *against* such a mode
(`drop_critical_error` fails `command`, `fake_revision` fails `context`,
`forbidden_model` and `ignore_cancel` fail `decision`).

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

## Known limits of this runner

- Skill, wrapper-form and raw-recovery behaviour that the host owns (`recover`,
  wrapper execution) is `unverified`, not passed.
- Only Python and Node runtimes given by absolute path, and native binaries,
  were exercised; Linux is listed in `platforms` but unverified.
- A descriptor that lists a `local:` bundled upstream cannot be trusted by the
  `trust` verb (see above).
