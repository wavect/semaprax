# First program

You will write one file, format it, check it, and run it. [Install](install.md)
first, then work in an empty directory.

## 1. Save hello.spx

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module app.hello;

@id("app.main")
fn main() -> i64
{
    42
}
```

| Line | Meaning |
| --- | --- |
| `module app.hello;` | Names the module. Every file starts with one. |
| `@id("app.main")` | Gives the function a stable identity that tools can address. |
| `fn main() -> i64` | Declares a function with no arguments that returns a 64-bit integer. |
| `42` | The result. The last expression has no semicolon. |

The name `main` is for people. The ID `app.main` is for tools. For a
single-file `run`, Semaprax uses `fn main`.

## 2. Format, check, run

```sh
semaprax fmt hello.spx
semaprax check hello.spx
semaprax run hello.spx
```

```text
42
```

| Command | Does |
| --- | --- |
| `fmt` | Rewrites the file in the one canonical layout. |
| `check` | Parses, type-checks, and verifies contracts, effects, and ownership. |
| `run` | Runs `main` and prints its result. |

`semaprax fmt hello.spx --check` reports formatting drift and leaves the file
alone. The printed `42` is the return value, not the process exit status.

## 3. Change it

Replace `42` with `6 * 7` and run the three commands again. The result is the
same. A **tail expression** is the last expression in a block; its value is the
block's result. There is no `return` here.

## 4. Write text

Printing is an effect, so the module must `permit` it and the function must
declare `uses`. Save this as `hello-print.spx`:

<!-- handbook-smoke: {"stdout":"Hello, Semaprax!\n0\n"} -->
```semaprax
module app.print_greeting;

permit { process.stdout.write }

@id("app.main")
fn main() -> i64
    uses { process.stdout.write }
{
    let greeting = "Hello, Semaprax!\n";
    let view = string_as_str(greeting);
    let written = stdout_write(str_as_bytes(view));
    if written == 17usize { 0 } else { 1 }
}
```

```sh
semaprax run hello-print.spx
```

```text
Hello, Semaprax!
0
```

`stdout_write` prints the text and returns the byte count (17, including the
newline). The runner then prints the return value, `0`. `string_as_str` and
`str_as_bytes` borrow the text; [Ownership](../language/ownership.md) explains
them.

## When a command fails

Read the first diagnostic and its `help:` line, fix that, and run `check`
again. Each `SPX-` code has a stable fix page:

```sh
semaprax help diagnostic SPX-T208
semaprax help language topics
```

See [Debugging](../practices/debugging.md) for more.

**Next:** [Create a project](first-project.md).
