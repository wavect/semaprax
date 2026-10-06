# Harness: let an agent propose changes under checks

After this page you can set up the development harness, run one
agent-proposed repair through compiler checks, and connect it to an outside
coding agent such as Claude Code.

The harness is optional tooling around the compiler. Ordinary `check`, `build`
and `run` never start it, read its files or need its adapters. The compiler is a
service the harness calls; the harness cannot overrule a compiler verdict.

**Where it lives.** `semaprax harness <verb>` exists in the release archive's
`semaprax` and in a source-built `semaprax-full`. The standalone `semaprax` build
answers `harness is unavailable in the standalone crates.io package` and exits 2.
`semaprax-harness <verb>` is the same code as its own binary in a checkout.

**Status.** Development harness. The specs record local macOS arm64 evidence;
Linux is untested. Treat model routing as rules-based unless you select a
provider yourself (see [Routing](#routing-models)).

## Set up

```sh
semaprax harness setup --project . --preset native
semaprax harness setup --project . --preset native --yes
```

Without `--yes`, `setup` prints a plan and changes nothing. With it, setup unpacks
the bundled adapters into the harness home (`$SEMAPRAX_HARNESS_HOME`, default
`~/.config/semaprax/harness`), adopts and trusts the ones you chose, sets your
preference, and writes `semaprax.harness.toml` into the project if none exists
(provider ids only: no paths, no trust). It keeps an existing project file and
never overwrites it. Running it twice reports `noop`. It never scans `PATH` or
`$HOME` and downloads nothing; pass tools with `--path-dirs` or
`--tool graft=/abs/path`. Presets: `native` (built-in context and command view,
official skills) and `local-efficient` (adds RTK and one repository provider,
Graft by default). A missing optional tool falls back to the built-in and
exits 0.

`semaprax harness status [--json]` shows what resolved, and
`explain <kind>` says why.

## Run one repair

```sh
semaprax harness run . --task task.json --proposal proposal.json
```

`run` follows one pipeline on one revision: authenticate the source, run
`check` and `test`, gather context within a byte budget, get a proposal (from your
`--proposal` file or a model you selected), preview it as a candidate, check the
candidate in a scratch copy, and stop. It stops at `approved-candidate-ready`
unless you pass `--apply-policy policy.json`, which publishes the candidate to a
local Git repository under that policy.

The harness refuses a proposal that deletes a `requires`/`ensures` line, adds a
`uses` effect, or weakens the requirements (`SPX-HPD042` to `HPD044`). A candidate
that fails checks is `SPX-HPD050`, never success. A publication that may have
happened is `uncertain` and is never retried. Exit 0 means
`approved-candidate-ready`, `published` or `no-repair-needed`. `--disable` runs
with built-ins only and no external calls. Spec:
[Harness workflow](https://github.com/wavect/semaprax/blob/main/docs/HARNESS-WORKFLOW-V1.md).

## All verbs

| Verb | Use it to |
| --- | --- |
| `setup` | Plan or apply the one-time setup above. |
| `status`, `explain`, `resolve [--frozen]`, `inspect <provider-id>` | See and freeze which providers handle each capability, per `semaprax.harness.toml` and its lock. |
| `adopt <descriptor> [--upstream <abs>]`, `trust <id>`, `revoke <id>` | Record, approve and withdraw a provider. `adopt` is the only verb that runs an upstream tool, with a cleared environment. Revocation applies on the next call. |
| `run`, `apply` | Run the repair pipeline. `apply <project> --session <result-dir> --expected-revision <digest>` re-verifies a finished session, requires your project to still match the session's baseline, and writes the changed files. It never publishes. |
| `context <project> <query> [--max-bytes N]` | Get compiler-verified context plus optional repository-provider results, within a byte budget. |
| `exec <project> -- <argv...>`, `recover <handle>` | Run one command once, return a short model-facing view, and keep the full output retrievable by handle. |
| `decide <project> <task.json>` | Ask the routing layer which model route a task gets. |
| `endpoints <project> [adopt\|bind\|reprobe\|litellm-config]` | Register loopback model endpoints (Ollama, LiteLLM, OpenAI-compatible) and bind logical models to them. |
| `skills <project> [list\|load <digest>]` | List and load instruction skills from approved roots. |
| `updates ...` | Keep skills and adapter packages current from pinned sources, by exact commit. |
| `bridge <project> --stdio\|--mcp\|--setup claude-code\|--host claude-code` | Connect an outside coding agent ([Bridge](#connect-claude-code)). |
| `report <observations.jsonl> [--json]` | Attribute token use across stages of a run. |
| `conformance <descriptor>` | Test an adapter against the provider contract and a hostile-input suite. |
| `bench ...` | Run journey and routing benchmarks. Results are evidence, not a support claim. |
| `evolve run\|promote\|status` | Experimental: propose a skill revision from traces. Gated and off by default. |

Exit codes: 0 success, 1 refused or failed with diagnostics, 2 usage error.
Most verbs accept `--json`.

## Project configuration

`semaprax.harness.toml` sits in the project and holds no paths, secrets or trust
grants:

```toml
schema = "semaprax.harness-config.v1"

[profile]
enabled = true

[capability."context.repository"]
mode = "auto"            # disabled | auto | required

[budget]
context_max_bytes = 16384
```

Capability kinds are `context.repository`, `command.view`, `decision.evaluate`,
`model.generate` and `skill.catalog`. For each, an explicit pin beats a user
preference, which beats the single compatible trusted installation, which beats
the built-in. `required` never falls back to a built-in. `resolve` writes
`semaprax.harness.lock`; `--frozen` fails on any difference. Permissions in an
adapter descriptor are requests. Grants live only in the harness home and are
bound to the descriptor, adapter and upstream digests, so a changed or widened
adapter loses its grant. Spec:
[Harness provider host](https://github.com/wavect/semaprax/blob/main/docs/HARNESS-PROVIDER-V1.md).

## Adapters

Bundled under `packages/semaprax-harness-adapters/` and embedded in the binary.
Each is a separate process on JSON-RPC over stdio, and none enters the compiler
build.

| Adapter | Capability | Needs |
| --- | --- | --- |
| `graft`, `graphify` | `context.repository` (orient, search, skeleton, references) | Your own installed [Graft](https://github.com/trailhq/Graft) or Graphify. Graphify is opt-in. |
| `rtk`, `caveman` | `command.view` (shorter command output) | RTK; Caveman is opt-in and needs a user-started local runtime. |
| `laya`, `jev`, `minijev-local`, `clef-local` | `decision.evaluate` (model routing) | Their own runtimes. Experimental; no learned adapter is live-tested on the reference host. |
| `wikiskill` | Skill evolution | A pinned community tool. Experimental. |

To write your own, start from the SDK in `packages/semaprax-harness-adapters/sdk/`
and check it with `semaprax harness conformance`. Spec:
[Adapter SDK](https://github.com/wavect/semaprax/blob/main/docs/HARNESS-ADAPTER-SDK-V1.md).

## Routing models

Routing is policy-first. By default simple rules pick the route with zero router
calls. Modes are `rules` (default), `pin`, `experimental` and `auto`; `auto` is used only for a profile that passed
its qualification gate, and none has yet. Every report carries `route.explain`.
`semaprax harness status --routing` lists profiles; `--check` validates them
without cost, and `--probe` makes one announced, metered call. The current
support table is in
[Harness decision](https://github.com/wavect/semaprax/blob/main/docs/HARNESS-DECISION-V1.md#current-status-2026-10-05).

## Connect Claude Code

```sh
semaprax harness bridge . --setup claude-code            # shows what it would write
semaprax harness bridge . --setup claude-code --write    # writes the host settings
semaprax harness bridge . --stdio                        # protocol over stdin and stdout
```

The bridge is a host-side surface: LF-delimited JSON-RPC (`semaprax.harness-bridge.v1`)
that begins with a `bridge/handshake`. The outside agent keeps its own model.
Semaprax does not claim to route it, and the handshake says who owns the model.
Phase routing applies to Semaprax-owned worker requests only. The bridge adds no
authority to semantic methods. Spec:
[Harness bridge](https://github.com/wavect/semaprax/blob/main/docs/HARNESS-BRIDGE-V1.md).

## Related

- Installed guidance for agents without the harness: [Driving Semaprax from an AI agent](../practices/agents.md).
- Serve a project to an agent over MCP: [Shipping](../projects/shipping.md#serve-a-project-to-tools).
- Measure token use: [Context performance](../practices/context-performance.md).
- Trust limits of grants and evidence: [What Semaprax verifies](trust.md).
