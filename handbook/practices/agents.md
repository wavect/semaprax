# Driving Semaprax from an AI agent

Semaprax is built for this workflow: the compiler exposes program meaning as
queryable data, so an agent spends tokens on source and decisions — not on
dumping whole repositories into context.

## The edit loop

1. Write the file, run `semaprax fmt <file>`, then
   `semaprax check <file> --json`. Run it with `semaprax run <file>` once the
   check succeeds.
2. Fix the first diagnostic at its reported line and column. Match the stable
   `SPX-…` code, not message wording.
3. Read small `.spx` files directly. On the calculator example the semantic
   graph is ~40× larger than source — never fetch it "just to look around."

## Ask bounded questions

For **one declaration**, ask for its neighborhood with explicit budgets and
check `truncation` before trusting the result:

```sh
semaprax context <file> <stable-id> --depth 1 --filters contracts,ownership --max-bytes 4096
```

In a **project**, locate first, then zoom:

```sh
semaprax query <project-dir> --id <stable-id>     # find the declaration
semaprax query <project-dir> --calls <stable-id>  # find its callers
semaprax context <project-dir> <stable-id> --direction both --depth 1 --max-bytes 2048 --max-nodes 16
```

Use `graph` only when a tool needs the whole expression tree or cleanup plan.
Use `--json` only when you need exact revision and relationship fields.

## Ask for narrow help

The installed compiler answers reference questions offline — no source
checkout needed:

```sh
semaprax help <command>              # one command's accepted shape
semaprax help language topics        # language card topics
semaprax help language <topic>       # scalars, ownership, …
semaprax help diagnostic <SPX-code>  # one diagnostic's fix
semaprax help shapes <kind|stable-id|path#stable-id>  # minimal declaration example
semaprax help library <module|name|stable-id>         # one stdlib entry
```

`semaprax help all` and the full language card are for broad questions only.
The guarded `std.core.compare` lookup is ~200 bytes; the full catalog is
~22 KB — prefer the exact lookup by 50×.

## Write agent-friendly source

- **Id everything.** An explicit `@id` on every declaration, field, and case
  is what makes later `query`/`context`/patch calls stable across renames.
- **Put intent in contracts, not comments.** `fmt` and single-file `patch`
  preserve `//` comments, but workspace transactions don't promise that —
  durable intent belongs in `@id` names, `requires`/`ensures`, and tests.
- **Keep signatures scalar in projects.** Rich types stay module-local so
  every backend and every query agrees on the interface.

## Checked changes

Preview semantic edits before applying them, and inspect their blast radius:

```sh
semaprax query <project> impact declaration <stable-id> --depth 1 --max-bytes 4096
semaprax change preview <project> rename-display-name <stable-id> <new-name>
semaprax review <project> <transaction.json>
```

Impact and review are read-only and bound to exact source bytes — source
drift fails closed rather than applying a stale edit. Evidence capsules
carry no authority: replay them with `verify` before staging anything.

The complete agent contract — every accepted shape, every diagnostic habit,
project manifests, and ownership profiles — is the
[Agent quick reference](https://github.com/wavect/semaprax/blob/main/docs/AGENT-QUICK-REFERENCE.md),
printed verbatim by `semaprax help language`.
