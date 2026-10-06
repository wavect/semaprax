# Project manifests

`semaprax.toml` says which files make up a project, where it starts, which
modules hold tests, and what it exports. After this page you can read, edit
and add to a manifest without breaking it.

## Start from a template

```sh
semaprax new my-app                        # calculator (default)
semaprax new my-lib --template library
semaprax new my-svc --template service
```

`new` creates the directory, a `semaprax.toml`, sources, tests, a `README.md`
and an `AGENTS.md`. It never writes into an existing path. Names are lowercase
(`[a-z][a-z0-9-]*`, `SPX-J115` otherwise).

| Template | You get |
| --- | --- |
| `calculator` | Entry, core and test modules; one web export. |
| `library` | A reusable module plus examples and tests. |
| `service` | A task-tracking scenario on ten bundled `std.*` packages (`useful-data.v1`). Every step is a deterministic fixture: no socket, file or clock. |

`semaprax project-scaffold --name <n> [--template ...] [--layout frozen|tables]`
prints the same project as one JSON capsule for tools instead of writing files.

## Read a manifest

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

| Table | What you put there |
| --- | --- |
| `schema` | `semaprax.manifest.v1` (tables) or `semaprax.manifest.v2` (adds `law_sources`). |
| `[package]` | `name`, `version`, optional `profile` (see [profiles](profiles.md)). |
| `[modules]` | `entry` (module with `main`), `sources` (2 to 16 `.spx` paths, sorted), `tests` (one test module). |
| `[exports] web` | Stable IDs a web or Rust build exposes. 1 to 32, sorted. |
| `[dependencies]` | `name = "range"`. Ranges: `=1.2.3`, `~1.2.3`, `^1.2.3`. |
| `[dependency-sources]` | Local Subject-v3 files for non-`std` packages (up to four). |
| `[rust-dependencies]` | Exact crates for a generated Rust SDK. |
| `[targets] matrix` | Allowed targets: `native64`, `wasm32`. Absent means both. |
| `[command]`, `[capabilities]` | Only for command profiles. |

A filename is not a module name, and a module name is not a declaration ID.
See [Modules](modules.md).

## Edit a manifest safely

The loader accepts one canonical byte layout. Keep the table order, one blank
line between tables, one-line arrays, and no comments.

| You see | Meaning and fix |
| --- | --- |
| `SPX-J100` | Not canonical, or a missing or mistyped key. `help` names the first differing line. |
| `SPX-J120` | Unknown table or key. |
| `SPX-J121` | Unknown bundled package, or a range the bundled `0.1.0` does not satisfy. |
| `SPX-J122` | You built a target outside `[targets] matrix`. |
| `SPX-J123` | A local dependency subject failed replay or resolution, or `semaprax.lock` is stale. |
| `SPX-J127` | `add` found the row already present, or the manifest is the frozen layout. |

New source file: add its path to `sources`. New test module: add it to
`sources` and `tests`. A file in `src/` is not part of the project until listed.

```sh
semaprax fmt . --check       # also reports manifest drift
semaprax add . std.num "^0.1.0"   # appends one [dependencies] row
```

`add` edits only the manifest. It fetches nothing. Next steps are in
[Shipping](shipping.md#resolve-dependencies).

## Three kinds of version

| Version | Example | Meaning |
| --- | --- | --- |
| Compiler | `0.9.0` | `semaprax version`. |
| Manifest schema | `semaprax.manifest.v1` | Grammar of this file. |
| Your package | `version = "0.1.0"` | Your project's own version. |

A spec named `PROJECT-MANIFEST-V18.md` describes one project profile. It does
not mean you change your manifest schema.

## Add laws

Native law files need `schema = "semaprax.manifest.v2"` and
`[modules] law_sources`. A law file also appears in `sources`. See
[Laws and proofs](../language/laws.md) and the
[native-law example](https://github.com/wavect/semaprax/tree/main/examples/native-law-project).

## Recognize the older frozen layout

Committed examples may use `schema = "semaprax.project.v1"` (or `v2` to `v13`)
with flat keys such as `entry`, `sources`, `web_exports`, `tests`. Each frozen
schema equals one profile of the table layout. Keep its key order. For new
projects use `semaprax new`. `add` and `[dependencies]` need the table layout
(`SPX-J127` otherwise).

**Next:** [Choose a profile](profiles.md), then [build a target](targets.md).
References: [Package Manifest v1](https://github.com/wavect/semaprax/blob/main/docs/PACKAGE-MANIFEST-V1.md),
[v2](https://github.com/wavect/semaprax/blob/main/docs/PACKAGE-MANIFEST-V2.md),
[Project Dependencies v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-DEPENDENCIES-V1.md).
