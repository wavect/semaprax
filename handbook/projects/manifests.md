# Project manifests

A manifest answers four questions: which files belong to this project, where
execution starts, which tests run, and what other code may call. Keep it open
beside your source when learning a multi-file project.

## Start with the generated layout

This complete example uses the extensible table layout:

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

The names refer to matching source files and declarations. Use this manifest
with that project, not beside an unrelated `hello.spx`.

| Field | What you put there |
| --- | --- |
| `schema` | The manifest format understood by the loader. |
| `[package] name` and `version` | Your package's identity and version. |
| `entry` | The module containing the application's `main`. |
| `sources` | Project-relative paths to the source files the loader should read. |
| `tests` | Module names containing the project's test functions. |
| `[exports] web` | Stable function IDs to expose through the selected web build. |
| `[dependencies]` | Declared package identities and accepted version ranges. |

A filename is not a module name, and a module name is not a declaration ID.
The [modules tutorial](modules.md) shows the relationship in a working project.

## Preserve canonical formatting

**Canonical** means the loader expects one accepted representation. Preserve
the generated table order, one blank line between tables, and one-line arrays.
Do not add comments or arbitrary TOML tables to a canonical manifest.

`SPX-J100` identifies a formatting mismatch and gives a location to fix.
Unknown tables or keys use `SPX-J120`; an unknown bundled dependency or an
unsatisfied range uses `SPX-J121`.

When adding a source file, put its path in `sources`. When adding a test
module, include it in both `sources` and `tests`. A file merely existing in
`src/` does not automatically include it in the project.

## Three different kinds of version

Do not treat all version numbers in the repository as the same thing:

| Version | Example | Meaning |
| --- | --- | --- |
| Compiler package | `0.8.0` | The installed Semaprax workspace/package version. |
| Manifest schema | `semaprax.manifest.v1` | The grammar of this configuration file. |
| Your package | `version = "0.1.0"` | The version assigned to the application or library. |

A specification filename such as `PROJECT-MANIFEST-V18.md` names a particular
project profile. It is not an instruction to change your compiler version or
to replace every manifest's schema string.

## Select a profile when your data needs it

The optional `[package] profile` chooses an admitted consumer profile.
Start with the default for the calculator. Move to an explicit data or command
profile when the interface you are building needs it.

| Profile choice | Starting use case |
| --- | --- |
| Omit `profile` | Scalar project interfaces and bundled scalar helpers. |
| `owned-data-api.v1` | The selected owned-data API and collection workflows. |
| `useful-data-command.v1` | The command-I/O workflow. Follow its target-specific example. |

The [profiles guide](profiles.md) explains why a helper that checks on its own
may need a different project boundary. A profile is more than a label: it
selects concrete type, ownership, execution, and packaging rules.

`[targets] matrix = ["wasm32"]`, in the schema that admits it, restricts the
allowed target set. A conflicting build request reports `SPX-J122`.

## Add laws explicitly

Projects that select native law files use `semaprax.manifest.v2` and
`[modules] law_sources`. A selected law file also appears in `sources`.

The complete [native-law example](https://github.com/wavect/semaprax/tree/main/examples/native-law-project)
shows the exact ordering. Naming a file `LAWS.spx` alone does not add it to a
project. See [Laws and proofs](../language/laws.md) for the declaration syntax
and inspection commands.

## Recognize the older frozen layout

Existing projects may use this six-line layout:

```toml
schema = "semaprax.project.v1"
name = "calculator"
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
web_exports = ["calculator.add"]
tests = ["calculator.tests"]
```

It is still useful to recognize this format when opening committed examples.
Keep its fields in the required order. For a new ordinary project, let
`semaprax new` produce the table layout rather than translating by hand.

**Next:** [Choose a project profile](profiles.md), then [build a target](targets.md).
References: [Package Manifest v1](https://github.com/wavect/semaprax/blob/main/docs/PACKAGE-MANIFEST-V1.md),
[Package Manifest v2](https://github.com/wavect/semaprax/blob/main/docs/PACKAGE-MANIFEST-V2.md),
and [Project Manifest v1](https://github.com/wavect/semaprax/blob/main/docs/PROJECT-MANIFEST-V1.md).
