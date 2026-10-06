# The Semaprax Handbook

<img src="assets/ernesto/ernesto.png" alt="Ernesto, the official Semaprax mascot" width="180">

Ernesto, the Semaprax mascot, guides you from one `.spx` file to a checked,
tested project. This handbook matches Semaprax **0.9.0**.

> Semaprax is **beta software**. Syntax, protocols, and binary interfaces can
> change. Use it to experiment and prototype, not for production or
> safety-critical work.

## Start here

1. [Install Semaprax](getting-started/install.md). It has a one-command
   installer and a Homebrew formula.
2. [Write and run your first program](getting-started/first-program.md).
3. [Create a project](getting-started/first-project.md) with modules and tests.

Prefer to watch first? See the [recorded walkthrough](getting-started/see-it-in-action.md).

## What Semaprax is

Semaprax is a systems language where people and AI agents work on the same
program. You write `.spx` source. The compiler checks it and exposes a
**semantic graph** of declarations, types, contracts, effects, and calls.

| Idea | What it means |
| --- | --- |
| Readable source in Git | `.spx` files are the source of truth. One formatter, one layout. |
| Stable identities | Each declaration has an `@id("math.add")` that survives renames. |
| Contracts and effects | `requires`, `ensures`, and `uses` are checked by the compiler. |
| Ownership | The compiler rejects use-after-move and data races. |
| One meaning, three engines | Checked code behaves the same in the interpreter, native C11, and WebAssembly. |

## The whole loop in one example

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module examples.meaning;

@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result == left + right
{
    left + right
}

@id("app.main")
fn main() -> i64
    ensures result == 42
{
    add(19, 23)
}
```

```sh
semaprax fmt meaning.spx     # canonical layout
semaprax check meaning.spx   # types, contracts, effects, ownership
semaprax run meaning.spx     # prints 42
```

## What do you want to do?

| Goal | Read |
| --- | --- |
| Install | [Install](getting-started/install.md) |
| Run one file | [First program](getting-started/first-program.md) |
| Build a multi-file project | [First project](getting-started/first-project.md) → [Modules](projects/modules.md) |
| Learn the language | [Essentials](language/essentials.md) → [Types](language/types.md) → [Ownership](language/ownership.md) |
| Use functions, loops, collections | [Functions](language/functions.md) · [Loops](language/loops.md) · [Collections](language/collections.md) |
| Model data and behavior | [Classes](language/classes.md) · [Matching](language/matching.md) · [Contracts and effects](language/contracts-effects.md) · [I/O](language/io.md) · [Resources](language/resources.md) |
| Prove properties | [Laws and proofs](language/laws.md) |
| Configure a project and pick a target | [Manifests](projects/manifests.md) → [Profiles](projects/profiles.md) → [Targets](projects/targets.md) |
| Call Semaprax from Rust or JavaScript | [Integrations](projects/integrations.md) |
| Use VS Code | [Editor setup](getting-started/editor.md) |
| Let a coding agent edit your code | [Agent workflow](practices/agents.md) → [Semantic explorer](practices/explorer.md) |
| Build an agent as a Semaprax program | [Agent programs](agents/programs.md) → [Budgets and recovery](agents/recovery.md) |
| Measure context size | [Token reports and caches](practices/context-performance.md) |
| Test, debug, release | [Testing](practices/testing.md) · [Debugging](practices/debugging.md) · [Shipping](projects/shipping.md) |
| Run the agent harness, check trust limits, find a specialist command | [Harness](tools/harness.md) · [What Semaprax verifies](tools/trust.md) · [Specialist commands](tools/specialist-commands.md) |
| Find a command | [Command reference](reference/commands.md) |
| Look something up | [Cheatsheet](reference/cheatsheet.md) · [Standard library](reference/stdlib.md) · [Built-ins](reference/builtins.md) · [Cookbook](practices/cookbook.md) · [Glossary](reference/glossary.md) |

For implementation details, use the [source map](reference/source-map.md) and
the specifications in `docs/`.

## Get help from the compiler

```sh
semaprax help                     # commands you need first
semaprax help all                 # every command
semaprax help language topics     # language topics, one at a time
semaprax help diagnostic SPX-T208 # the fix for one error code
```
