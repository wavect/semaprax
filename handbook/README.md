# The Semaprax Handbook

> This handbook describes **Semaprax v0.6.0** (workspace version `0.6.0`).
> v0.6.0 is installable from source; its release gate is not green and no
> signed v0.6.0 archive is published yet, so install from source to follow
> along. Semaprax is **alpha research software**: syntax, protocols, and
> binary interfaces can change. Experiment and prototype; don't ship
> production or safety-critical workloads on it yet.

## Semaprax in 60 seconds

Semaprax is a systems programming language designed so **humans and AI agents
can work on the same program**. You write readable `.spx` source files. The
compiler checks them and exposes a **semantic graph**: declarations, types,
contracts, effects, and who-calls-whom — queryable by stable identity instead
of by guessing from text.

Five ideas explain the whole language:

| Idea | What it means in practice |
| --- | --- |
| **Readable source in Git** | `.spx` files are the canonical artifact. One formatter, one canonical layout, no style debates. |
| **Stable identities** | Every declaration gets an `@id("math.add")` that survives renames, so tools and agents can track it across edits. |
| **Contracts and effects** | Functions state what they require, what they promise (`requires`/`ensures`), and which effects they use (`uses`). The compiler checks all three. |
| **Ownership** | Values are owned or borrowed. The compiler rejects use-after-move and data races at compile time instead of crashing at runtime. |
| **One meaning, three engines** | Checked code runs identically on the interpreter, native (C11), and WebAssembly backends. |

A program is verified before it ever runs: `check` proves it, `run` executes
`main`, `test` runs its test module, `build` targets native or web.

## How to use this handbook

This book is **task-oriented best practice**: short pages, copy-paste examples,
and do/don't guidance. It is not the exact contract — when you need the
letter of the law, each page links down to the versioned specification in
[`docs/`](https://github.com/wavect/semaprax/tree/main/docs), which remains the
authoritative reference for agents and integrators.

| You want to… | Go to… |
| --- | --- |
| Install and run something in 5 minutes | [Install](getting-started/install.md) → [First program](getting-started/first-program.md) |
| Start a real multi-file project | [First project](getting-started/first-project.md) |
| Learn the language fast | [Essentials](language/essentials.md) → [Types](language/types.md) → [Ownership](language/ownership.md) |
| Go deeper on one topic | [Functions](language/functions.md) · [Loops](language/loops.md) · [Collections](language/collections.md) · [Classes](language/classes.md) · [Matching](language/matching.md) · [I/O](language/io.md) · [Resources](language/resources.md) |
| Ship a project | [Manifests](projects/manifests.md) → [Targets](projects/targets.md) → [Shipping](projects/shipping.md) |
| Write correct, idiomatic code | [Style](practices/style.md) and the [Cookbook](practices/cookbook.md) |
| Fix a compiler error | [Debugging](practices/debugging.md) → [Diagnostics reference](reference/diagnostics.md) |
| Drive Semaprax from an AI agent | [Agents](practices/agents.md) |
| Look something up | [Cheatsheet](reference/cheatsheet.md) · [Stdlib](reference/stdlib.md) · [Built-ins](reference/builtins.md) |

## The 2-minute tour

One file, one module, one `main` returning `i64`:

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
semaprax check examples/meaning.spx   # verify: types, contracts, effects, ownership
semaprax run examples/meaning.spx     # prints 42
```

That's the whole loop: write it, `fmt` it, `check` it, `run` it. The rest of
this book makes each step idiomatic.
