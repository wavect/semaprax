# Cheatsheet

One page: the v0.6.0 commands and syntax you'll reach for daily.

## Commands

```sh
# Scaffolding
semaprax project-scaffold --name demo [--template calculator|library|service]
semaprax new demo [--template service] [--layout tables|frozen]

# Everyday loop (file, dir, or manifest)
semaprax fmt <target> [--check]
semaprax check <target> [--json]
semaprax run <target> [--native] [--max-steps N] [--max-bytes N] [--json]
semaprax test semaprax.toml [--json]

# Understanding
semaprax graph <target>                                   # whole checked graph (large!)
semaprax context <target> <id> --depth 1 --max-bytes 4096 # one declaration
semaprax query <target> --id <id> | --calls <id> | --kind function
semaprax doc <file> [--json]

# Shipping
semaprax build semaprax.toml --target web|native -o dist/...
semaprax lock semaprax.toml --write | --verify | --compare base.lock
semaprax resolve semaprax.toml --target native64|wasm32 --cache <dir> --write
semaprax doctor [--profile <id>] [--target native|web|all] [--json]

# Help (all offline)
semaprax help all | help <command>
semaprax help language [topics|<topic>] | help shapes <kind>
semaprax help diagnostic <SPX-code> | help diagnostic codes
semaprax help library [<module|name|stable-id>]
```

## Syntax

```semaprax
module app.demo;                              // one per file, first line

permit { process.stdout.write }               // file-level effect grants

use function @id("calc.add") from calc.core as add;  // project import

@id("demo.helper")                            // stable identity, always
fn helper(value: i64) -> i64                  // no (), no unit, always returns
    requires value >= 0                       // precondition
    ensures result >= 0                       // postcondition; result = return
    uses { process.stdout.write }             // declared effects
{
    let doubled = value + value;              // immutable
    let mut acc = 0;                          // mutable
    acc = acc + doubled;                      // assignment is a statement
    let sign = if acc > 0 { 1 } else { 0 - 1 };  // if is an expression, else required
    while acc > 100 {                         // while repeats on tail condition
        acc = acc - 100;
        acc > 100
    }
    match acc { 0 => 0, n if n < 0 => 0 - n, _ => acc, }  // catch-all required
}
```

| Types | Literals |
| --- | --- |
| `i64` / `i32` / `u8` / `usize` | `42` / `42i32` / `255u8` / `3usize` (no mixing) |
| `f64` / `f32` / `bool` / `char` | `1.5` / `1.5f32` / `true` / `'a'` |
| `string` → `str` → `Slice<u8>` | `string_as_str(b)` → `str_as_bytes(v)` → `byte_len`/`byte_get` |
| `Option<T>` / `Result<T, E>` | Build `Option<i64>::Some { value: v }`, match `Option::Some { value: v }` |
| `record` / `variant` / `class` | `Point { x: 1 }` / `Shape::Box { … }` / `counter.bumped(1)` (methods only on classes) |
| Generics | Always explicit: `identity<i64>(4)` |

`main` is exactly `fn main() -> i64`. `0` means success. Trailing commas
everywhere. When stuck: `fmt`, `check`, read the first `SPX-…` code — see
[Debugging](../practices/debugging.md).
