# Cheatsheet

Use these commands as working starting points. The first group runs inside a
generated project. Later groups run from the Semaprax repository root so their
`examples/` paths resolve.

## Inside your project

```sh
semaprax fmt .
semaprax fmt . --check
semaprax check .
semaprax test .
semaprax run .
semaprax query . --kind function
semaprax build . --target web -o dist/web
```

`fmt` writes source; `fmt --check` does not. `check` inspects the program, while
`run` and `test` execute it. Build and runtime choices depend on the
[project profile](../projects/profiles.md).

## Create a project

Run each creation command from the directory that should contain the new project.
Each destination must be unused.

```sh
semaprax project-scaffold --name demo
semaprax new demo
semaprax new my-library --template library
semaprax new my-service --template service
```

## Inspect a committed example

```sh
semaprax doc examples/meaning.spx
semaprax query examples/calculator-project --id calculator.add
semaprax query examples/calculator-project --calls calculator.add
semaprax context examples/meaning.spx math.add --depth 1 --filters contracts,ownership --max-bytes 4096
semaprax graph examples/meaning.spx
```

`doc` is for a readable overview. `query` locates declarations. `context`
selects a neighborhood. `graph` emits the full representation.

## Preview a semantic rename

```sh
semaprax change preview examples/calculator-project rename-display-name calculator.add sum
```

This previews the change; it does not publish it. Keep the stable ID and check
the revision before a separately authorized application step.

## Inspect laws and compact context

```sh
semaprax query examples/native-law-project --kind law --json
semaprax project-assurance-manifest examples/native-law-project/semaprax.toml
semaprax compact context examples/meaning.spx math.add --max-bytes 4096 --encoding model-text
```

Continue with [Laws](../language/laws.md) and
[Token reports](../practices/context-performance.md) for interpretation.

## Lock and verify project inputs

Inside the project:

```sh
semaprax lock . --write
semaprax lock . --verify
```

A lock and a resolution cache have different purposes. Read
[Shipping](../projects/shipping.md) before preparing external dependency inputs.

## Ask your installed compiler

```sh
semaprax --version
semaprax help build
semaprax help language topics
semaprax help diagnostic SPX-T208
semaprax help library std.num
semaprax doctor --target native
```

When consulting a command synopsis, angle brackets name values to replace and
square brackets indicate optional parts. Do not paste those brackets into a
shell command. A `|` in a synopsis means “choose one,” not a shell pipe.

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

| Need | Spelling |
| --- | --- |
| Mutable local | `let mut count = 0;` then `count = count + 1;` |
| Final result | A tail expression, without `return` or a trailing semicolon. |
| Index literal | `3usize`, not an unsuffixed `3`. |
| Read-only text view | `string_as_str(binding)`. |
| Byte view of text | `str_as_bytes(view)`. |
| Construct a generic case | `Option<i64>::Some { value: 1 }`. |
| Match that case | `Option::Some { value: v } => v`. |
| Generic function call | `identity<i64>(4)`. |
| Repeat on a condition | `while condition { ... }`; the body's final value is discarded. |
| Traverse a vector | `for item in values { ... }`. |
| Consume an iterator | `for own item in iterator { ... }`. |

**Need an explanation?** Open the [glossary](glossary.md) or start with
[Essentials](../language/essentials.md).
