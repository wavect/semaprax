# Unified CLI v1: `verify`, `agent inspect`, `query`, `package`, `add`, and `fetch`

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

## `semaprax agent`

```sh
semaprax agent inspect <definition.json> [--profile]
semaprax agent run <definition.json> <task.json> <transcript.json> [--evidence|--trace]
semaprax agent replay <definition.json> <task.json> <transcript.json> <evidence.json>
```

`agent` is the 1.0 verb for the agent lifecycle. Three subcommands are
admitted, each a pure function of its input documents that grants no provider,
tool, filesystem, process, network, clock, approval, or publication authority.

`inspect` compiles a canonical AgentDefinition v1 document through
`agent_definition::compile_agent_definition` and prints the deterministic
AgentGraph v1 canonical JSON (the compiler's projection, including its
terminal LF), or the byte-preserved Agent Runtime Profile v1 projection with
`--profile`. Diagnostics are the agent compiler's own (`SPX-G501`,
`SPX-G502`); an unreadable file reports `SPX-I001`.

`run` compiles the definition, derives its Runtime v1 profile, and executes
one canonical `semaprax.agent-runtime-task.v1` document through the bounded
runtime against a scripted host. The host is the transcript:

```json
{"schema":"semaprax.agent-runtime-transcript.v1","policy_epoch":7,
 "provider":[{"disposition":"succeeded","response":"{\"schema\":\"semaprax.agent-runtime-action.v1\",...}\n"},
             {"disposition":"definitely_not_started"},{"disposition":"failed_uncertain"}],
 "tools":[{"result":"{\"value\":\"alpha\"}"},{"result":null}]}
```

Provider attempts and tool invocations consume the two arrays in order; a
succeeded attempt streams its `response` bytes, an exhausted provider script
answers `failed_uncertain`, and a `null` or exhausted tool script fails the
invocation. Elapsed time is always zero and the policy epoch is the
transcript's, so every observation the runtime makes is a function of the
three documents and the run is deterministic. The transcript is a closed
object of at most 4 MiB and 256 entries per array; anything else reports
`SPX-V221`. By default `run` prints one receipt line naming the agent, the
terminal status (`completed`, `cancelled`, `deadline_exceeded`,
`budget_exhausted`, `provider_failed`, `tool_failed`, `policy_rejected`), the
final message or `null`, and the trace and evidence digests; `--evidence`
prints the canonical `semaprax.agent-runtime-evidence.v1` document instead and
`--trace` the `semaprax.agent-runtime-trace.v1` document. A run that ends in a
runtime failure status is still a successful invocation; only rejected input
documents exit with status one.

`replay` re-runs the same three documents and requires the recomputed evidence
to equal the supplied capsule byte for byte, printing a
`semaprax.agent-replay-receipt.v1` line with `verified: true`; any difference
is `SPX-V222`. Nothing in the capsule is trusted, so a replay proves that this
compiler, this definition, this task, and this transcript produce exactly this
evidence.

`resume` and `reconcile` are not admitted: the runtime's non-claims exclude
durable memory, persistence, recovery, and resume, and the verb rejects them
with a usage error that names the three admitted forms.

## `semaprax query`

```sh
semaprax query <file|project> [--kind <kind>[,<kind>]] [--name <text>] [--id <prefix>]
                      [--effect <effect>] [--calls <stable-id>] [--called-by <stable-id>] [--json]
```

`query` selects declarations of one checked module or one authenticated
Project from the documentation model of
[Documentation Projection v1](DOC-PROJECTION-V1.md). Every match carries the
identity and canonical signature `doc` renders. A Project match additionally
names its canonical source path, module, and source revision, while the result
binds the exact Project and semantic-graph revisions. Filters are a conjunction:

| Filter | Holds when |
| --- | --- |
| `--kind` | the declaration's kind is one of the listed kinds: `record`, `variant`, `class`, `method`, `resource`, `interface`, `protocol`, `implementation`, `function` |
| `--name` | the display name contains the text |
| `--id` | the stable identity starts with the prefix |
| `--effect` | the declaration's `uses` clause names the effect |
| `--calls` | the declaration calls the named persistent callable |
| `--called-by` | the named persistent callable calls the declaration |

Call relations come from the persistent call index that `impact` and the
use, so `--calls` is "semantic references" by stable identity, not a text
search. Project queries use the already authenticated cross-file graph, so a
library declaration can name callers in entry and test modules without reading
the full graph. Without `--json`, a file query prints kind, identity, and
canonical header; a Project query prepends the owning path. `--json` prints
`semaprax.query.v1` for a file or `semaprax.project-query.v1` for a Project.
The Project result includes its revisions and each match's path, module, source
revision, kind, identity, name, persistence, signature, location (the one-based
line and column followed by the start and end byte offsets of the name token,
encoded as a four-integer array to keep the Project result compact), effects,
callees, and callers. The VS Code adapter's go-to-declaration, callers, and
code-lens features use the single-file route
([VS Code adapter](VSCODE-SAVED-SOURCE-ADAPTER-V1.md)).

The query fails closed rather than matching nothing: an unknown kind reports
`SPX-V211`, and an unknown `--calls`/`--called-by` identity reports `SPX-V212`.
`SPX-V213` names an impossible mismatch inside an already authenticated Project
query. A file query still follows no `use` lines and still requires a standalone
checked module; Project inspection must select its directory or manifest.
Neither route performs writes or grants source authority.

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

## `semaprax add`

```sh
semaprax add <dir>|semaprax.toml <package> <range>
```

`add` appends one `[dependencies]` row to a Package Manifest v1 table manifest
([Package Manifest v1](PACKAGE-MANIFEST-V1.md)). It parses the manifest,
inserts the row in strict byte order, renders the canonical table layout, and
re-parses the result before its one write, so a rejected package identity or
range surfaces as the manifest's own `SPX-J100` and the file is untouched. A
frozen `semaprax.project.v*` manifest has no `[dependencies]` table and is
rejected with `SPX-J127`, which names `project-scaffold --layout tables`; a
dependency already present is also `SPX-J127`. `add` fetches, resolves,
and contacts nothing; the next steps stay explicit (`fetch`, then `resolve`).

## `semaprax fetch`

```sh
semaprax fetch <cache-dir> <subject.json>...
```

`fetch` is the caller-populating step [Project Dependency Resolution v1](PROJECT-DEPENDENCY-RESOLUTION-V1.md)
leaves outside the compiler's implicit actions. Each operand is a Semantic
Package Subject-v3 envelope file (at most the resolver's subject size). Every
subject is independently replayed through the same verifier the resolver uses
before anything is written; its cache address is `<hex>.json` for its own
`digest` `sha256:<hex>`. An operand that fails replay, carries a non-canonical
digest, or would overwrite an entry holding different bytes rejects the whole
run with `SPX-J128` before any write; an identical entry is reported as
`present`. The cache directory is created when missing and may not exceed the
resolver's 64-subject bound. One receipt line is printed:

```json
{"schema":"semaprax.fetch-receipt.v1","cache":"cache","subjects":[{"package":"examples.meaning","version":"1.0.0","digest":"sha256:...","state":"added"}]}
```

`fetch` reads only the paths it is given. It performs no network access, no
registry lookup, no version selection, and no build; `resolve` remains the
only reader of the cache and still replays every subject itself.

## Guided help

The guided page lists `verify` under `Change by meaning`, `query` under
`Inspect meaning`, and `agent inspect` under a new `Agents` group; `package`,
`add`, and `fetch` appear in the exhaustive catalog only. Both capability pages stay within the 2048-byte
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
  tampered graph fails with `SPX-G503`; `agent run` follows a tool-then-final
  transcript to a `completed` receipt, prints deterministic evidence and trace
  documents, reports an exhausted script as `provider_failed`, and rejects
  malformed transcripts with `SPX-V221`; `agent replay` verifies the printed
  evidence and rejects a tampered capsule with `SPX-V222`; `resume`,
  `reconcile`, and malformed grammars exit with status two.
- `tests/projections/query_projection.rs` (projections harness): filters
  select by kind, name, identity prefix, effect, callers, and callees on the
  committed examples; authenticated Project queries find library declarations,
  owning paths and cross-file entry/test callers while a raw library file keeps
  `SPX-T105`; the exact calculator result stays below 1 KiB, 256 lexical units,
  and one eighth of its full Project graph; directory and manifest selectors
  print the library's same one-line JSON; unknown kinds and identities fail
  closed with `SPX-V211`/`SPX-V212`; malformed grammars exit with status two.
- `tests/offline_package/package_namespace.rs` (offline-package harness):
  every `package` subcommand's stdout, stderr, and status equal its long
  form's, and a missing or unknown subcommand exits with status two.
- `tests/project/add_fetch_v1.rs` (project harness): `add` appends
  byte-sorted rows that re-parse canonically through both operand forms and
  leaves the manifest untouched on duplicate, grammar, frozen-layout, and
  missing-file rejections; `fetch` files replayed subjects by digest with an
  exact receipt, reports refetches as `present`, feeds `resolve` directly, and
  rejects tampered, foreign, missing, and colliding subjects before any write.
- Unit tests pin the closed grammars, the route table, and the namespace map.
