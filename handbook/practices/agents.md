# Driving Semaprax from an AI agent

The compiler answers questions about program meaning as small, bounded data. An
agent spends tokens on source and decisions, not on dumping a repository. After
this page you can set up a coding agent to write, check and change Semaprax
safely.

<img src="../assets/ernesto/ernesto-laptop.png" alt="Ernesto working at a laptop" width="180">

New to the CLI? The [visual tour](../getting-started/see-it-in-action.md) shows
the first commands. This page is for an assistant editing your project. An
[agent program](../agents/programs.md) written in Semaprax is a different thing.

## Give the agent its instructions

`semaprax new` writes an `AGENTS.md` into every project. It lists the commands
and the rules that differ from other languages, and points at one language topic
instead of the 33 KB card. For other tools:

```sh
semaprax skills get agent          # also: language, graph, stdlib, packages, effects
semaprax agent skill               # the installed agent skill: authority classes, verbs
semaprax query --capabilities      # what this binary's query and change commands accept
```

Each verb in the skill has an authority class: `read_only`, `candidate_only`,
`test_execute`, `source_write` or `publication`. Grant an agent the lowest class
that does its job.

## The edit loop

1. Write the file. Run `semaprax fmt <file> && semaprax run <file>` (one call).
2. On failure, fix the first diagnostic at its line and column. Match the
   `SPX-...` code, not the message. Plain output is smaller than `--json`.
3. Read small `.spx` files directly. Never fetch `graph` to look around: on the
   calculator it is about 40 times the source.

In a project use `semaprax check .`, `semaprax test .`. See
[Debugging](debugging.md).

## Ask bounded questions

```sh
semaprax query <project> --id <stable-id>        # find a declaration
semaprax query <project> --calls <stable-id>     # who calls it
semaprax query <project> --kind function --effect clock.read
semaprax context <project> <stable-id> --direction both --depth 1 --max-bytes 4096 --max-nodes 16
semaprax context <file> <stable-id> --depth 1 --filters contracts,ownership --max-bytes 4096
```

Check `truncation` before treating a result as complete. `--max-bytes` for
`context` is at least 2048. Project `context` does not accept `--filters`. Use
`graph` only when a tool needs the whole expression tree or cleanup plan, and
`--json` only when you need exact revision fields. `semaprax doc <file>` renders
declarations, contracts and effects as text or `--json`.

## Ask for narrow help

```sh
semaprax help <command>                         # one command's grammar
semaprax help language topics                   # then: help language <topic>
semaprax help diagnostic <SPX-code>             # one fix
semaprax help shapes <kind|stable-id>           # minimal declaration example
semaprax help library <module|name|stable-id>   # one stdlib entry
```

`help all` and the full card are for broad questions. One `help library` lookup
is about 200 bytes; the full catalog is about 22 KB.

## Write source an agent can change later

- Put an explicit `@id` on every declaration, field and case. Later `query`,
  `context` and patches address them by ID, and IDs survive renames.
- Put intent in `requires`/`ensures`, tests and ID names. `fmt` and single-file
  `patch` keep `//` comments, but workspace transactions do not promise to.
- Tell the agent the project's [profile](../projects/profiles.md) before it
  changes parameter or result types.

## Make checked changes

```sh
semaprax query <project> impact declaration <id> --depth 1 --max-bytes 4096
semaprax change preview <project> rename-display-name <id> <new-name>
semaprax review <project> <transaction.json>
semaprax verify <subject> <change> <evidence.json>
```

Impact, preview and review write nothing and are bound to exact source bytes:
drift fails closed. A preview, a review report and an applicable transaction are
different objects; do not feed one to a command that wants another. Evidence
carries no authority: replay it with `verify` before you apply anything. The
full flow, including managed workspaces, is in
[Shipping](../projects/shipping.md#change-with-review).

## Give an agent a live connection

| Command | What the agent gets |
| --- | --- |
| `semaprax service <project> [--mcp]` | One authenticated project over line-delimited JSON-RPC 2.0, or MCP, on stdin/stdout. Queries and transaction validation. |
| `semaprax serve-workspace <manifest> <host-policy.json>` (and `serve-workspace-mcp`) | Image-agent protocol with candidates, diagnostics, tests, builds and Git commits only as the host policy allows. |
| `semaprax serve <file>` | A single-file request loop. |
| `semaprax dev <manifest> --jsonl` | Hot reload control frames ([Targets](../projects/targets.md#edit-and-re-run-hot-reload)). |

The host decides authority. The client cannot widen the policy. An agent
inside VS Code uses saved-source candidate sessions; see
[Editor setup](../getting-started/editor.md).

## Smaller context

`semaprax compact ...` and the token report script shrink what a model reads.
See [Context and performance](context-performance.md). For a visual map use the
[semantic explorer](explorer.md).

## A coding agent is not an agent program

Agent programs written in Semaprax have task, state, proposal and authorization
roles. Inspect and replay them with `semaprax agent inspect|run|replay`
([Agent programs](../agents/programs.md)). The development harness
(`semaprax harness ...`) lets a coding agent propose changes under compiler
checks ([Harness](../tools/harness.md)). The release archive ships the full
build as `semaprax`, so the command works there. A standalone or crates.io
`semaprax` refuses it (exit 2); build `semaprax-full` from source instead. It is
development tooling, so do not build a product on it.

The complete agent contract is the
[Agent quick reference](https://github.com/wavect/semaprax/blob/main/docs/AGENT-QUICK-REFERENCE.md),
printed verbatim by `semaprax help language`.
