# First program

You will create one file, run it, and understand every line. Start in a
scratch directory where you can save your own files.

## 1. Save hello.spx

Create a file named `hello.spx` with this content:

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module app.hello;

@id("app.main")
fn main() -> i64
{
    42
}
```

Read it from top to bottom:

| Line | What it does |
| --- | --- |
| `module app.hello;` | Gives this file's module a name. A module groups declarations. |
| `@id("app.main")` | Gives the entry function a stable identity that tools can address. Keep this ID for the single-file runner. |
| `fn main() -> i64` | Declares a function with no arguments that returns a 64-bit integer. |
| `42` | Supplies the function's result. The final expression has no semicolon. |

The display name `main` and stable ID `app.main` serve different jobs. Keep both
as shown while learning the single-file execution route.

## 2. Format, check, and run

Run these commands in the directory containing `hello.spx`:

```sh
semaprax fmt hello.spx
semaprax check hello.spx
semaprax run hello.spx
```

`fmt` writes the standard layout. `check` reports whether the file passes the
compiler's checks. `run` evaluates the entry function and displays its result:

```text
42
```

Use `semaprax fmt hello.spx --check` when you only want to check formatting.
It leaves the file unchanged and reports a failure when formatting differs.

The displayed `42` is the program's return value. It is not the CLI's exit
status. A successful interpreter run can return a nonzero value; a test case
uses a different convention, where `0` means pass.

## 3. Make a small change

Replace `42` with `6 * 7`, then repeat the three commands. The result stays `42`.
Try a different arithmetic expression before moving on.

A **tail expression** is the last expression in a block. Semaprax uses its
value as the block's result. You do not write `return 42;` here. This rule also
explains the value-producing branches in [Essentials](../language/essentials.md).

## 4. Write a line of text

Returning a number and writing to the terminal are separate operations. Save
this complete program as `hello-print.spx`:

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
semaprax fmt hello-print.spx
semaprax check hello-print.spx
semaprax run hello-print.spx
```

The output is:

```text
Hello, Semaprax!
0
```

The first line comes from `stdout_write`. The runner prints the returned `0`
on the next line. The greeting contains 17 UTF-8 bytes, including `\n`.
`stdout_write` returns the number of bytes written, so the program can check it.

`permit` lists an effect allowed in the module. `uses` lists the effect used
by this function. `string_as_str` borrows a read-only text view, and
`str_as_bytes` exposes the bytes that the output function accepts. You will
learn those conversions in [Ownership](../language/ownership.md).

## When a command fails

Read the first diagnostic and its `help` text. Fix that issue, then run
`check` again. A code such as `SPX-T208` is a stable name for the diagnostic:

```sh
semaprax help diagnostic SPX-T208
```

Keep [Debugging](../practices/debugging.md) nearby. The
[recorded walkthrough](see-it-in-action.md) also shows the tools in use.

**Next:** [Create a project with multiple files and tests](first-project.md).
