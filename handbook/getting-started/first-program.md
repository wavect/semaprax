# First program

One file is enough. Create `hello.spx`:

```semaprax
module app.hello;

@id("app.main")
fn main() -> i64
{
    42
}
```

Three rules already visible: one `module dotted.name;` per file, first line;
every declaration carries a stable `@id`; the entry point is exactly
`fn main() -> i64` — no arguments, no other return type.

## The edit loop

Run these three commands after every edit, in this order:

```sh
semaprax fmt hello.spx --check   # is it canonical? no output = clean
semaprax check hello.spx         # verify: types, contracts, effects, ownership
semaprax run hello.spx           # prints 42
```

`fmt` without `--check` rewrites the file into canonical form — let it handle
layout and never hand-format. `check` prints `verified hello.spx (sha256:…)`
on success. `run` evaluates `app.main` in the bounded interpreter.

**Best practice:** fix the *first* diagnostic at its reported line and column,
then re-run. Diagnostics carry a stable `SPX-…` code; look any code up with
`semaprax help diagnostic <code>`. See [Debugging](../practices/debugging.md).

## Print something

`main` returns a number; printing goes through an explicit effect. To print
`42`, render the integer to a string, borrow it, and write its bytes:

```semaprax
module app.print_count;

permit { process.stdout.write }

@id("app.main")
fn main() -> i64
    uses { process.stdout.write }
{
    let count = 42usize;
    let text = string_from_usize(count);
    let view = string_as_str(text);
    let written = stdout_write(str_as_bytes(view));
    if written == 2usize { 0 } else { 1 }
}
```

```sh
semaprax run hello-print.spx   # prints 42
```

The pattern is always the same: the module **permits** an effect, each function
using it **declares** it, and the call itself is an ordinary function. Nothing
prints, reads, or connects without saying so in the signature. Read more in
[Contracts and effects](../language/contracts-effects.md).

## Next step

Grow to multiple files and tests: [First project](first-project.md).
