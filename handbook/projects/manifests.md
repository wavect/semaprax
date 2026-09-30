# Project manifests

`semaprax.toml` is the project's identity: what it is, what it contains,
what it needs, and what it exposes. Two layouts are admitted — prefer the
extensible table layout.

## The table layout (preferred)

```toml
schema = "semaprax.manifest.v1"

[package]
name = "calculator"
version = "0.1.0"

[modules]
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
tests = ["calculator.tests"]

[exports]
web = ["calculator.add"]

[dependencies]
std.num = "^0.1.0"
```

- `entry` names the one module declaring `main`.
- `sources` lists every file, in order. `tests` lists test modules.
- `[exports]` names stable ids per target (`web`, …).
- `[dependencies]` links dotted package identities with `^`/`~`/`=` ranges
  against the compiler's closed bundled inventory (`0.1.0`).

**Manifest bytes are canonical**: table order as shown, one blank line
between tables, one-line arrays, no comments. Violations fail with `SPX-J100`
(its `help` names the first differing line); unknown tables or keys fail
with `SPX-J120`; unknown packages or unsatisfied ranges with `SPX-J121`.

## The frozen layout (also admitted)

```toml
schema = "semaprax.project.v1"
name = "calculator"
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["calculator.add"]
tests = ["calculator.tests"]
```

One line per key, six lines in this order. The committed
`calculator-project` uses it. New projects should use the table layout;
`scaffold --layout frozen|tables` and `new --layout` select either.

## Profiles and targets

`[package] profile` selects the admitted consumer profile for the project's
dependencies:

| Profile | For |
| --- | --- |
| `scalar` (omit the key) | Plain scalar code, `std.num`, `std.core` |
| `owned-data-api.v1` | Bounded `Vec`/`Box`/bytes via `std.collections` |
| `useful-data-command.v1` | `args`/`stdin`/`stderr` command I/O, native target only |

`[targets] matrix = ["wasm32"]` restricts builds to Wasm; a native build
against it fails with `SPX-J122`.

## Modules and imports

- Import by stable identity, directly after the `module` line:
  `use function @id("calculator.add") from calculator.core as add;`
- Project function signatures are **Copy scalars only**. Records, variants,
  `Option`/`Result`, and classes work as module-local implementation
  details, but crossing a function boundary with one is `SPX-G174`.

Exact rules: [Package Manifest v1](https://github.com/wavect/semaprax/blob/main/docs/PACKAGE-MANIFEST-V1.md),
[Project Manifest v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-MANIFEST-V1.md)
(frozen), and the specialized profiles
([v16](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-MANIFEST-V16.md),
[v18](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-MANIFEST-V18.md),
[v19](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-MANIFEST-V19.md)).
