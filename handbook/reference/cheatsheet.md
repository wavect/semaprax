# Cheatsheet

The language and the command line of Semaprax 0.9.0 on one page. Each row links
to the page that explains it.

## The daily loop

Run these in your project directory.

```sh
semaprax fmt .            # rewrite source to the one canonical form (--check only reports)
semaprax check .          # parse, resolve, type-check, verify
semaprax test .           # run the project's test functions
semaprax run .            # run main and print its i64 result
semaprax build . --target web -o dist/web
```

`fmt` writes files; `fmt --check` does not. `check` never runs your code; `run`
and `test` do. For one file, `semaprax fmt f.spx && semaprax run f.spx` is the
whole loop. Add `--json` to `check`, `run` or `test` for one JSON object per
diagnostic.

## Commands by task

Run `semaprax help all` for the full list, or see the
[Command catalog](commands.md).

| I want to | Command | Page |
| --- | --- | --- |
| Start a project | `semaprax new demo` (`--template library\|service`) | [First project](../getting-started/first-project.md) |
| Read a declaration's meaning | `doc <file>`, `query <project> --id <id>`, `context <input> <id> --depth 1 --max-bytes 4096`, `graph <file>` | [Explore](../practices/explorer.md) |
| Find who calls what | `query <project> --calls <id>`, `--called-by <id>` | [Explore](../practices/explorer.md) |
| Look at a project visually | `explore <manifest> --format html --output out.html` | [Explore](../practices/explorer.md) |
| Change code by meaning | `change preview`, `patch`, `impact`, `review` | [Shipping](../projects/shipping.md#change-with-review) |
| Pin and compare an interface | `lock . --write\|--verify\|--compare base.lock` | [Shipping](../projects/shipping.md) |
| Build for a target | `build . --target native\|web\|wasm\|npm\|oci` | [Targets](../projects/targets.md) |
| Check my toolchain or a download | `doctor`, `version`, `release verify <dir>` | [Targets](../projects/targets.md#check-the-environment), [Shipping](../projects/shipping.md#verify-a-release-you-downloaded) |
| Reload code while editing | `dev semaprax.toml --jsonl` | [Targets](../projects/targets.md#edit-and-re-run-hot-reload) |
| Serve a project to an agent | `service <project> [--mcp]` | [Shipping](../projects/shipping.md#serve-a-project-to-tools) |
| Run an agent in a coding harness | `harness setup`, `harness run`, `harness bridge` | [Harness](../tools/harness.md) |
| Run an agent or check its definition | `agent inspect\|run\|replay` | [Agent programs](../agents/programs.md) |
| Get an error's fix | `help diagnostic SPX-T208` | [Diagnostics](diagnostics.md) |
| Look up a library function | `help library compare` | [Standard library](stdlib.md) |
| See a language topic | `help language topics`, `help language ownership` | [Essentials](../language/essentials.md) |
| Copy a declaration shape | `help shapes record` | [Types](../language/types.md) |

## The language at a glance

| Need | Spelling | Page |
| --- | --- | --- |
| A file | `module app.name;` first, then declarations | [Essentials](../language/essentials.md) |
| Stable identity | `@id("app.name.fn")` before every declaration | [Essentials](../language/essentials.md) |
| Entry point | exactly `fn main() -> i64` | [Essentials](../language/essentials.md) |
| Result of a block | A tail expression, with no `return` and no trailing `;` | [Essentials](../language/essentials.md) |
| Bindings | `let x = 1;` immutable; `let mut n = 0;` then `n = n + 1;` | [Essentials](../language/essentials.md) |
| Number types | `i64` (default), `i32`, `u8`, `usize`, `f64`, `f32`; suffix `5i32`, `3usize`; operators never mix types | [Types](../language/types.md) |
| Other scalars | `bool`, `char` (`'a'`), `string` (owned UTF-8), `str` (borrowed view) | [Ownership](../language/ownership.md) |
| Conditional | `if c { a } else { b }`; always an expression, no `else if` | [Essentials](../language/essentials.md) |
| Loop on a condition | `while cond { ...; cond }`; the last line is the continuation test | [Loops](../language/loops.md) |
| Loop over a vector | `for item in values { ...; 0 }` over an immutable `Vec` binding | [Loops](../language/loops.md) |
| Consume an iterator | `for own item in it { ... }`; `match own` on `IterStep` | [Loops](../language/loops.md) |
| Record | `record P { @id("p.x") x: i64, }`; build `P { x: 1 }`; update `p with { x: 2 }` | [Types](../language/types.md) |
| Variant | cases `Name,` or `Name { f: i64, }`; build `Shape::Dot {}` | [Types](../language/types.md) |
| Match | `match v { Shape::Box { width: w } => w, _ => 0, }`; guards `n if n < 0`; or-patterns `-1 \| -2` | [Matching](../language/matching.md) |
| Option and Result | `Option<i64>::Some { value: 1 }`; match `Option::Some { value: v }`; `?` in a `Result` function | [Matching](../language/matching.md) |
| Class | `class Dog : Animal { fn m(self: Dog) -> i64 { ... } }`; call `d.m()`; `super.m()` | [Classes](../language/classes.md) |
| Generics | `fn id<T>(v: T) -> T`; call `id<i64>(4)` | [Functions](../language/functions.md) |
| Function values | `fn(x: i64) -> i64 { x + 1 }`; parameter `f: fn(i64) -> i64` | [Functions](../language/functions.md) |
| Contracts | `requires x >= 0` and `ensures result >= 0` between signature and body | [Contracts](../language/contracts-effects.md) |
| Effects | `permit { process.stdout.write }` on the module, `uses { ... }` on each function | [Contracts](../language/contracts-effects.md) |
| Ownership | `own T` consumes; `borrow T` reads; a moved value cannot be reused | [Ownership](../language/ownership.md) |
| Resources | `resource R { drop trivial; }` or `drop import "host.symbol";` | [Resources](../language/resources.md) |
| Strings | `string_concat(a, b)`; view `string_as_str(binding)`; bytes `str_as_bytes(view)` | [Built-ins](builtins.md) |
| Vectors | `vec_with_capacity<i64>(4usize)`; `v = vec_push<i64>(v, 1);` | [Collections](../language/collections.md) |
| Laws | `@id("l.order") law relational (a: i64, b: i64) a <= b \|\| b < a evidence smt_proved;` | [Laws](../language/laws.md) |
| Session protocol | `session protocol "name" { states {...} initial S; on S label: send T via "id" -> S2; }` | `semaprax help language` |
| Import across files | `use function @id("pkg.fn") from other.module as name;` right after `module` | [Modules](../projects/modules.md) |
| A test | `fn test_add() -> i64` with an `@id` in a test module; return `0` to pass | [Testing](../practices/testing.md) |
| Standard library | `[dependencies] std.core = "^0.1.0"` then `use function @id("std.core.min") ...` | [Standard library](stdlib.md) |

Habits that fail: `return`, `else if`, `x += 1`, tuples, `a[0]`, `"a" + "b"`,
`Some(1)`, `i++`, `break`, `as` casts, macros. The fix for each is in
[Diagnostics](diagnostics.md).

## A complete syntax reminder

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module app.reminder;

@id("reminder.double")
fn double(value: i64) -> i64
    requires value >= 0
    ensures result == value * 2
{
    value * 2
}

@id("app.main")
fn main() -> i64
{
    let answer = double(21);
    if answer == 42 { answer } else { 0 }
}
```

## Reading a command synopsis

Angle brackets name values to replace, square brackets are optional parts, and `|`
means choose one. Do not paste the brackets into a shell. If a command says
`<input>`, give it a `.spx` file, a project directory or `semaprax.toml`.

**Need a word explained?** Open the [glossary](glossary.md).
