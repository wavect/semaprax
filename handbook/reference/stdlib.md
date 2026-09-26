# Standard library

The standard library ships **bundled with the compiler**: no source checkout,
cache, or network access needed. Add a dependency, import by stable id, set
the profile if one is required.

## Discover first, memorize never

The catalog is large and versioned — don't guess names. Ask the installed
compiler:

```sh
semaprax help library                 # full catalog (also offline)
semaprax help library compare         # one entry, ~200 bytes
semaprax help library std.core.compare
```

Each entry prints the stable identity, dependency row, required project
profile, signature, effects, and contracts. The machine-readable form is
`std/catalog.json` in a checkout. The generated human catalog is the
[Standard library catalog](https://github.com/wavect/semaprax/blob/main/docs/STANDARD-LIBRARY-CATALOG.md).

## Wire a dependency (3 steps)

```toml
# 1. semaprax.toml — depend on the package at 0.1.0
[dependencies]
std.num = "^0.1.0"

# 2. set the profile if the entry requires one (omit for `scalar`)
[package]
profile = "owned-data-api.v1"
```

```semaprax
// 3. import by stable identity, right after the module line
module app.using_std;

use function @id("std.num.abs") from std.num as abs;
```

Dependencies resolve from the compiler's closed bundled `std.*` inventory at
version `0.1.0`. Unknown packages or unsatisfied ranges fail with `SPX-J121`.
Always spell admitted Copy-scalar type arguments explicitly
(`vec_push<i64>(…)`), and in a project prefer the authenticated
`std.collections` aliases over ad-hoc declarations.

## Most-used packages

| Package | For | Profile |
| --- | --- | --- |
| `std.num` | Numeric helpers (`abs`, …) | `scalar` (omit) |
| `std.core` | Core guards (`compare`, …) | `scalar` (omit) |
| `std.collections` | Bounded `Vec` with `vec_*<T>` ops over Copy scalars | `owned-data-api.v1` |
| `std.fs` | Typed `Path`/`FileInfo`/reader-writer values over the `fs.*` effects | see entry |
| `std.agent` | Agent `Task`/`Context`/`Observation`/`Outcome` records | `owned-data-api.v1` |

`std.collections` mutators transfer and return the owner — thread the vector
through (`let v2 = vec_push<i64>(v1, x);`) rather than expecting in-place
mutation. Traverse immutably with `for item in values { … }` over a simple
`Vec<T>` binding of Copy scalars; the body result is discarded and the vector
is frozen during traversal.

Compiler-owned functions (`string_len`, `byte_get`, `stdout_write`, `box_new`,
…) are **not** stdlib — they're reserved names available in every file. See
[Ownership](../language/ownership.md) and the
[Agent quick reference](https://github.com/wavect/semaprax/blob/main/docs/AGENT-QUICK-REFERENCE.md)
table. Full contract: [Standard Library v1](https://github.com/wavect/semaprax/blob/main/docs/STANDARD-LIBRARY-V1.md).
