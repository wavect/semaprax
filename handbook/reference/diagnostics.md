# Diagnostics reference

Every diagnostic carries a stable code, `SPX-` plus a family letter and digits.
Match on the code, not the message wording. The letter says which part of the
toolchain refused your input.

```text
error[SPX-U103]: `mut` is only allowed on local `let` bindings; parameters are immutable at u103.spx:4:6
  help: drop `mut` and copy the parameter into a new mutable local: `let mut current = <parameter>;`
```

The `help:` line is usually the fix. Fix the first diagnostic first; later ones
often follow from it.

## Look a code up

| You want | Command |
| --- | --- |
| The fix for a common mistake | `semaprax help diagnostic SPX-T208` prints what you wrote and the fix |
| The list of codes with an indexed fix | `semaprax help diagnostic codes` (also bare `semaprax help diagnostic`) |
| Whether this compiler emits a code | `semaprax explain SPX-T208 [--json]` prints the family and how many places emit it |
| The exact grammar of a command | `semaprax help <command>` |
| One diagnostic per line for tools | add `--json` to `check`, `build`, `run` or `test` |

`help diagnostic` indexes 24 codes, the mistakes people make most. For every other code,
read the message and its `help:` line, then the spec for the feature. Codes are
case-sensitive.

## Common mistakes and fixes

Habits from other languages cause most first errors. The full habit table with
runnable examples is in `semaprax help language mistakes-code`.

| Code | You wrote or hit | Fix |
| --- | --- | --- |
| `SPX-P003` | `9223372036854775808` or `-(9223372036854775808)` | Write the minimum as one literal: `-9223372036854775808` |
| `SPX-P104` | `struct`, `enum`, `pub`, `const` | `record`, `variant`; there is no visibility keyword |
| `SPX-P105`, `SPX-P106` | `return`, `else if`, `for i in 0..n`, tuples, `f(x);`, `a[0]`, `fn f()`, `c ? a : b`, `break`, `as`, a last field or arm without `,` | Use the tail expression, nested `else { if ... }`, `while`, a record, `let _ = f(x);`, `byte_get(...)`, `if`; end every field and arm with `,` |
| `SPX-P130` | `own fn` with parameters | An owning closure takes none |
| `SPX-P201` | `x += 1`, a Rust or JavaScript closure | `x = x + 1;`; `fn(x: i64) -> i64 { x + 1 }` |
| `SPX-P203` | A block with no final expression | End a `while` body with its continuation condition; end an `if` branch with a value |
| `SPX-P207` | Nesting deeper than 128 | Extract a named helper |
| `SPX-S103` | A declaration without `@id` (warning) | Add `@id("your.name")`; `semaprax fix --plan` can plan it |
| `SPX-S113` | Your own `string_len` | Built-in names are reserved; rename yours |
| `SPX-T001`, `SPX-T281` | `String`, `int`, an unsupported `Vec` element type | `string`, `i64`, `i32`, `u8`, `usize` |
| `SPX-T104` | `fn main() -> bool` | Exactly `fn main() -> i64` |
| `SPX-T202`, `SPX-T203` | `Some(1)`, `None`, `s.len()`, a method on a record | `Option<i64>::Some { value: 1 }`; `string_len(s)`; only classes have methods |
| `SPX-T205` | An owned value or literal where a `borrow str` goes | Bind it, then pass `string_as_str(binding)` |
| `SPX-T207`, `SPX-T208` | `index + 1` with a `usize`, or two different integer types | The message names both types. Suffix the literal: `index + 1usize` |
| `SPX-T209` | `let x = x + 1;` reusing a name | No shadowing; pick a new name |
| `SPX-T213` | A record literal missing a field | Name every field |
| `SPX-T218` | `f()?` outside a `Result` function | `match` the result in `main` |
| `SPX-T221`, `SPX-T225` | `Option::Some { ... }` to construct, or `identity(4)` | `Option<i64>::Some { ... }`; `identity<i64>(4)`. Matching omits the arguments. |
| `SPX-T232` | `let a: i32 = 5` | `5i32` |
| `SPX-T250` | `"a" + "b"`, `string_concat("n=", 5)` | `string_concat(a, b)`; use `string_from_i64(5)` for numbers |
| `SPX-T252`, `SPX-T258` | A record built in a `while` body or yielded from a match arm | Compute scalars in the loop and build after; or build with `if` |
| `SPX-T257` | A scalar `match` with no catch-all | Add a final `_` arm without a guard |
| `SPX-T262` | `[1, 2, 3]` | Array literals hold bytes (`[1u8, 2u8]`); use a `Vec<i64>` for numbers |
| `SPX-T263`, `SPX-T266` | `str_as_bytes(text)` on an owned string, `string_as_str("lit")` | `let view = string_as_str(text); str_as_bytes(view)`; bind a literal first |
| `SPX-T265`, `SPX-T267`, `SPX-T271`, `SPX-T272` | Buffer misuse | No live view across a replacement; `bytes_zeroed` outside loops; do not re-open a named buffer; keep the index in range |
| `SPX-T270` | `net_recv` in a `while` body | Receive outside the loop |
| `SPX-T284` | A `for` over a `let mut` vector | Move the finished vector into an immutable binding first |
| `SPX-T288`, `SPX-T291` | Nested generic-collection closures; owning closures in generic functions | Name a helper; keep the closure out of the generic |
| `SPX-O101` | Using a moved `string` or `Bytes` | The callee takes `borrow`, or pass a fresh value or a copy |
| `SPX-O116` | A function returning `str` | Return an owned `string` |
| `SPX-U101` | Assigning an immutable binding | `let mut` first |
| `SPX-U103` | `mut` on a parameter | Copy it into a new `let mut` local |
| `SPX-E101`, `SPX-E102` | Missing `permit` or `uses` | `permit { ... }` at module level, `uses { ... }` on the function and its callers. The message names both edits. |
| `SPX-F102` | `run` refused to admit the program | Try `run --native`, or build the project |
| `SPX-G170` | `use std::io;` | Built-ins need no import; import one declaration with `use function @id("...") from module as name;` |
| `SPX-G174` | A rich type in a project function signature | Keep records module-local; cross boundaries with Copy scalars |
| `SPX-B104` | `run` on a module with `resource` | `check` it, or run through a native or Wasm project build, or `run <file> --native` |
| `SPX-J100` | Non-canonical manifest | The help names the first differing line |
| `SPX-J120`, `SPX-J121`, `SPX-J122` | Unknown manifest key; bad dependency; target outside the matrix | Remove or fix the key; bundled `std.*` at `0.1.0` with a satisfied range; build a listed target |
| `SPX-J102` | A path alias given to a writing `fmt` | Use the real path |

An unknown function name that a project module or the standard library provides
gets the exact `use function @id("...") from ... as ...;` line and, for the
library, the dependency to add. A `string_concat("n=", 5)` or a wrong integer
width names the fix.

## Which part refused it?

| Letter | Area | Where to read next |
| --- | --- | --- |
| `P` | Parsing, plus size limits of reports | [Essentials](../language/essentials.md) |
| `S`, `J` | Identities and declarations; manifests | [Manifests](../projects/manifests.md) |
| `T`, `O`, `U`, `E`, `M`, `N` | Types, ownership, mutation, effects, matches, unsafe boundaries | [Types](../language/types.md), [Ownership](../language/ownership.md), [Contracts and effects](../language/contracts-effects.md) |
| `K` | Session protocols (`K1xx`) and capability manifests (`K2xx`) | `semaprax help language` (the full card) |
| `F`, `B`, `H` | Interpreter admission, backends, replay of retained HIR | [Targets](../projects/targets.md) |
| `G` | Projects, graphs, patches and workspaces: `G409` stale patch, `G530` stale revision, `G150` wrong workspace kind | [Shipping](../projects/shipping.md#change-with-review) |
| `I` | Workspace, candidate and agent-runtime I/O | [Agent programs](../agents/programs.md) |
| `W` | Wasm and web export profiles (`W115` signature outside the profile) | [Targets](../projects/targets.md) |
| `A`, `D`, `X`, `Y`, `Q`, `V` | ABI report, C header, C++ shim, hygienic generation, plugin manifest and verify front, SIMD report | [Integrations](../projects/integrations.md), [Specialist commands](../tools/specialist-commands.md) |
| `L`, `PKR`, `Z926`, `Z927` | Package locks, registry rules, registry reads | [Shipping](../projects/shipping.md#use-a-package-registry-file) |
| `Z70x` | Release verification (`Z701` shape, `Z702` binding, `Z703` identity, `Z704` artifact, `Z705` missing, `Z707` cryptographic) | [Shipping](../projects/shipping.md#verify-a-release-you-downloaded) |
| `HP` + letter | Harness: `HPB` config and lock, `HPD` run pipeline, `HPE` context, `HPJ` routing, `HPM` skills, `HPN` bridge | [Harness](../tools/harness.md) |

Other families exist for specialist features (generic ownership, WIT, laws).
`semaprax explain <code>` tells you if your compiler emits a code and which family
it belongs to.

The debugging workflow around these codes (`fmt` first, fix the first
diagnostic, read the `help:` line) is in [Debugging](../practices/debugging.md).
