# Unified CLI v1: `verify`, `agent inspect`, `query`, and `package`

Status: authored with local executable evidence; unpublished and unpromoted.
The completion matrix and release evidence own product status.

Audience: SEMAPRAX users, coding agents, editor and tooling authors, and
compiler contributors.

## Purpose

The 1.0 command surface converges public workflows on one `semaprax` binary
and a short list of verbs. Two of those verbs, `verify` and `agent`, front
capabilities the compiler already had under long protocol-specific names. This
revision admits them without changing any verifier or the agent compiler: the
new verbs select and delegate, add no verification of their own, and grant no
authority the long forms did not have.

## `semaprax verify`

```sh
semaprax verify <subject> <change> <capsule.json>
semaprax verify <manifest> <image.json>
```

The last operand is always the capsule to replay. The front reads it once,
takes its top-level string `schema`, and selects the verifier that owns that
schema for that operand count. It then hands the same paths, unchanged and in
the same order, to that verifier, which re-reads and independently replays
them exactly as the long-form route does. The receipt is the verifier's own
bytes; the front never rewrites, wraps, or summarizes it.

| Capsule schema | Operands | Verifier |
| --- | --- | --- |
| `semaprax.semantic-patch-evidence.v1` | `<file> <patch.spatch> <capsule>` | `verify-patch-evidence` ([Semantic Patch Evidence v1](SEMANTIC-PATCH-EVIDENCE-V1.md)) |
| `semaprax.semantic-patch-evidence.v2` | `<file> <patch.spatch> <capsule>` | `verify-patch-evidence-v2` ([Semantic Patch Evidence v2](SEMANTIC-PATCH-EVIDENCE-V2.md)) |
| `semaprax.semantic-workspace-patch-evidence.v1` | `<root> <patch.wspatch> <capsule>` | `verify-workspace-patch-evidence` ([Semantic Workspace Patch Evidence v1](SEMANTIC-WORKSPACE-PATCH-EVIDENCE-V1.md)) |
| `semaprax.workspace-semantic-change-evidence.v1` | `<root> <proposal.json> <capsule>` | `verify-semantic-workspace-change-evidence` ([Semantic Workspace Change v1](SEMANTIC-WORKSPACE-CHANGE-V1.md)) |
| `semaprax.workspace-semantic-structural-change-evidence.v1` | `<root> <proposal.json> <capsule>` | `verify-semantic-workspace-structural-change-evidence` |
| `semaprax.semantic-workspace-operations-evidence.v1` | `<root> <proposal.json> <capsule>` | `verify-semantic-workspace-operations-evidence` ([Semantic Workspace Operations v1](SEMANTIC-WORKSPACE-OPERATIONS-V1.md)) |
| `semaprax.agent-graph.v1` | `<definition.json> <profile.json> <graph.json>` | `agent_definition::verify_agent_graph_bundle` ([Language-Native Agent Object v1](LANGUAGE-NATIVE-AGENT-OBJECT-V1.md)) |
| `semaprax.semantic-workspace-image.v1` | `<manifest> <image.json>` | `project-image-verify` ([Semantic Workspace Image v1](SEMANTIC-WORKSPACE-IMAGE-V1.md)) |

The agent graph bundle is the one route without a long-form command. On
success it prints one receipt line:

```json
{"schema":"semaprax.agent-graph-verification.v1","agent_id":"...","definition_digest":"sha256:...","graph_digest":"sha256:...","verified":true,"authority":false}
```

The identities and digests come from an independent recompilation of the
definition after the bundle comparison passed; the receipt asserts equality of
bytes, not fitness of the agent.

### Fail-closed selection

Selection happens before any verifier runs, so a rejected capsule leaves no
verifier receipt and no side effect:

| Code | Meaning |
| --- | --- |
| `SPX-V201` | The capsule's schema is admitted for no verifier at this operand count. The message lists the schemas admitted for that count. |
| `SPX-V202` | The capsule cannot be read, exceeds 16 MiB, is not a JSON document, or has no top-level string `schema`. |

A capsule whose schema selects a verifier but whose contents that verifier
rejects produces exactly the long-form route's diagnostics and status; the
front adds nothing. The grammar is closed: two or three operands, none empty
and none starting with `-`; anything else exits with status two.

### Non-claims

`verify` performs no verification itself, does not authenticate the capsule
beyond reading `schema`, does not apply, publish, or lock anything, and does
not introduce a new evidence format. Evidence capsules still carry no
authority ([AGENTS.md](../AGENTS.md)); a passing `verify` is proof data for the
route that owns it, nothing more.

## `semaprax agent inspect`

```sh
semaprax agent inspect <definition.json> [--profile]
```

`agent` is the 1.0 verb for the agent lifecycle. This revision admits exactly
one subcommand, `inspect`, which compiles a canonical AgentDefinition v1
document through `agent_definition::compile_agent_definition` and prints:

- the deterministic AgentGraph v1 canonical JSON (the compiler's projection,
  including its terminal LF) by default; or
- the byte-preserved Agent Runtime Profile v1 projection with `--profile`.

Diagnostics are the agent compiler's own (`SPX-G501`, `SPX-G502`); an
unreadable file reports `SPX-I001`. Inspection is pure: it grants no provider,
tool, filesystem, process, network, approval, or publication authority, and it
runs nothing. `agent run`, `agent resume`, `agent replay`, and
`agent reconcile` are not admitted; the runtime's non-claims still exclude
durable memory, resume, and replay, and the verb rejects them with a usage
error naming the one admitted form.

## `semaprax query`

```sh
semaprax query <file> [--kind <kind>[,<kind>]] [--name <text>] [--id <prefix>]
                      [--effect <effect>] [--calls <stable-id>] [--called-by <stable-id>] [--json]
```

`query` selects declarations of one checked module from the documentation
model of [Documentation Projection v1](DOC-PROJECTION-V1.md), so every match
carries the identity, canonical signature, and facts `doc` renders, and the
result names the same graph revision. Filters are a conjunction:

| Filter | Holds when |
| --- | --- |
| `--kind` | the declaration's kind is one of the listed kinds: `record`, `variant`, `class`, `method`, `resource`, `interface`, `protocol`, `implementation`, `function` |
| `--name` | the display name contains the text |
| `--id` | the stable identity starts with the prefix |
| `--effect` | the declaration's `uses` clause names the effect |
| `--calls` | the declaration calls the named persistent callable |
| `--called-by` | the named persistent callable calls the declaration |

Call relations come from the persistent call index that `impact` and the
graph use, so `--calls` is "semantic references" by stable identity, not a
text search. Without `--json` the output is one tab-separated line per match:
kind, identity, and the first non-`@id` line of the canonical signature.
`--json` prints one `semaprax.query.v1` line with the module, revision, the
filters as given, and every match's kind, identity, name, persistence,
signature, effects, callees, and callers.

The query fails closed rather than matching nothing: an unknown kind reports
`SPX-V211` with the admitted list, and a `--calls`/`--called-by` identity that
is not a declaration of the module reports `SPX-V212`. It documents one file,
performs no writes, and follows no `use` lines into other modules.

## `semaprax package`

```sh
semaprax package report <file> [--max-bytes N]
semaprax package lock <subject.json>... [--max-bytes N]
semaprax package resolve <subject.json>... --require <package>:<range> [--require ...] --target <native64|wasm32> [--allow-capability <capability>]... [--max-bytes N]
```

`package` is the 1.0 namespace over the offline package routes. Each
subcommand is rewritten to its long-form command (`package-report`,
`package-lock`, `package-resolve`) with the same operands and re-enters the
dispatcher, so stdout, stderr, and status are the owning route's own; the
grammar, bounds, and diagnostics of [Package Report v1](PACKAGE-REPORT-V1.md),
[Offline Package Lock v1](OFFLINE-PACKAGE-LOCK-V1.md), and
[Offline Package Resolver v1](OFFLINE-PACKAGE-RESOLVER-V1.md) are unchanged.
The usage recovery hint printed after a rejected invocation names the verb
as typed (`semaprax package --help`), which is the one line that differs from
the long form. A missing or unknown subcommand exits with status two before
any route runs.
`add` and `fetch` are not admitted by this revision.

## Guided help

The guided page lists `verify` under `Change by meaning`, `query` under
`Inspect meaning`, and `agent inspect` under a new `Agents` group; `package`
appears in the exhaustive catalog only. Both capability pages stay within the 2048-byte
bound of [Guided CLI Help v4](CLI-HELP-V4.md); the shapes and several summaries
were shortened to make room, and the exhaustive catalog remains the grammar
authority.

## Executable gates

- `tests/semantic/verify_front.rs` (semantic harness): for patch evidence v1
  and v2, workspace patch evidence, semantic workspace change evidence, and a
  project image, the `verify` receipt is byte-identical to the long-form
  route's; a v2 capsule handed to the v1 subject reproduces the owning
  verifier's rejection exactly; wrong operand counts, foreign schemas,
  schema-less documents, non-JSON, and missing capsules fail closed with
  `SPX-V201`/`SPX-V202` and no stdout; the source is unchanged afterwards.
- `tests/agent_runtime_v1/agent_inspect_cli.rs` (agent runtime harness):
  `agent inspect` prints the exact AgentGraph v1 bytes and, with `--profile`,
  the exact profile bytes of the library compiler; `verify` over the
  definition, profile, and graph prints the receipt with the pinned digests; a
  tampered graph fails with `SPX-G503`; malformed grammars exit with status
  two.
- `tests/projections/query_projection.rs` (projections harness): filters
  select by kind, name, identity prefix, effect, callers, and callees on the
  committed examples; the CLI prints the library's text and one-line JSON
  naming the graph revision; unknown kinds and identities fail closed with
  `SPX-V211`/`SPX-V212`; malformed grammars exit with status two.
- `tests/offline_package/package_namespace.rs` (offline-package harness):
  every `package` subcommand's stdout, stderr, and status equal its long
  form's, and a missing or unknown subcommand exits with status two.
- Unit tests pin the closed grammars, the route table, and the namespace map.
