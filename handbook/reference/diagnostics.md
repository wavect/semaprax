# Diagnostics reference

Every diagnostic carries a stable `SPX-…` code: match on the code, not the
wording. The letter after `SPX-` names the family — it tells you which part
of the compiler rejected the program.

```sh
semaprax help diagnostic <SPX-code>   # one code's fix
semaprax help diagnostic codes        # all codes (case-sensitive)
semaprax explain <SPX-code> --json    # structured explanation
```

## P: parsing and shapes

The program isn't shaped like Semaprax. Fixes are syntactic.

| Code | Meaning | Fix |
| --- | --- | --- |
| `SPX-P003` | Bad minimum literal | Write `-9223372036854775808` as one literal (no parens) |
| `SPX-P104` | Foreign keyword | `record`/`variant` instead of `struct`/`enum`; no `pub`/`const` |
| `SPX-P105`/`P106` | Wrong shape | Covers `return`, `else if`, `for`, tuples, `f(x);`, `a[0]`, `fn f()`, missing trailing `,` |
| `SPX-P130` | `own fn` with params | Owning closures take zero explicit parameters |
| `SPX-P201` | `x += 1` | `x = x + 1;` |
| `SPX-P203` | Block missing tail | End `while` bodies with the continuation condition; value-branches with an expression |
| `SPX-P207` | Nesting past 128 | Extract a named helper |

## T: types and profiles

The shape parses but the types or the admitted profile reject it.

| Code | Meaning | Fix |
| --- | --- | --- |
| `SPX-T104` | Bad `main` | Exactly `fn main() -> i64` |
| `SPX-T203`/`T202` | Bad method/constructor | Only classes have methods; `Option<i64>::Some { value: v }` |
| `SPX-T205` | Owned where borrow expected | Bind, then `string_as_str(binding)` |
| `SPX-T208` | Mixed-type operation | `index + 1usize` — operators never mix types |
| `SPX-T209` | Shadowing | Pick a new name |
| `SPX-T213` | Missing record fields | Construction names every field |
| `SPX-T221` | Generic construction spelling | `Option<i64>::Some` (with args) vs `Option::Some` (match) |
| `SPX-T225` | Generic call without args | `identity<i64>(4)` |
| `SPX-T232` | Unsuffixed literal | `let a: i32 = 5i32` |
| `SPX-T250` | `+` on strings | `string_concat(a, b)` |
| `SPX-T252` | Aggregate in `while` body | Compute scalars in the loop, construct after |
| `SPX-T257` | Match without catch-all | Final `_` or unguarded binding |
| `SPX-T258` | Aggregate from match arm | Bind scalars out first, or build with `if` |
| `SPX-T263`/`T266` | View from wrong source | `str_as_bytes` takes `str`; `string_as_str` takes a binding |
| `SPX-T265`/`T267`/`T271`/`T272` | Buffer misuse | No live view across replacement; `zeroed` outside loops; no re-opening; index in range |
| `SPX-T270` | Owned result in `while` | `net_recv` outside the loop |
| `SPX-T288`/`T291` | Closure placement | No nested generic-collection closures; no owning closures in generic fns |

## O: ownership

| Code | Meaning | Fix |
| --- | --- | --- |
| `SPX-O101` | Use after move | Callee takes `borrow`, or pass a fresh value per `own` call |

## E: effects

| Code | Meaning | Fix |
| --- | --- | --- |
| `SPX-E101` | Effect without `permit` | Grant it at module level |
| `SPX-E102` | Effect without `uses` | Declare it on the function (and transitively on callers) |

## U: mutation

| Code | Meaning | Fix |
| --- | --- | --- |
| `SPX-U101` | Assigning immutable binding | Declare `let mut` before assigning |

## S: identities and shapes

| Code | Meaning | Fix |
| --- | --- | --- |
| `SPX-S103` | Missing `@id` | Give the declaration a stable identity (warning) |
| `SPX-S113` | Reserved name | Pick another name — built-ins can't be redeclared |

## J: manifests and JSON

| Code | Meaning | Fix |
| --- | --- | --- |
| `SPX-J100` | Non-canonical manifest | `help` names the first differing line — match it |
| `SPX-J102` | Path alias in write `fmt` | Use the real path |
| `SPX-J120` | Unknown table/key | Remove it or fix the spelling |
| `SPX-J121` | Bad dependency | Bundled `std.*` at `0.1.0` with a satisfied range |
| `SPX-J122` | Target outside matrix | Build a matrix-listed target |

## G: projects and graphs

| Code | Meaning | Fix |
| --- | --- | --- |
| `SPX-G174` | Non-scalar project signature | Rich types stay module-local; boundaries are Copy-scalar |

## B: backends

| Code | Meaning | Fix |
| --- | --- | --- |
| `SPX-B104` | `run` on resource module | `check` it; execute via project native/Wasm build or `run --native` |

The debugging workflow around these codes — `fmt` first, fix the first
diagnostic, read the `help` line — is in
[Debugging](../practices/debugging.md).
