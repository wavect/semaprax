# Project Scaffold Capsule v5

Status: additive authority-free scaffold descriptor for the Project v26 native
file-text source-command starter.

Audience: CLI users, compiler contributors, and authors of native source
commands.

The `source-command-file-text` template emits a canonical table manifest for
[Project v26](PROJECT-MANIFEST-V26.md), profile `source-command.v1`, input
`argv-utf8+file-text.v1`, and target matrix `native64`. It declares the four
available command effects and pins the bundled source dependency
`std.int.decimal = "=0.1.0"`. Its descriptor uses capsule schema
`semaprax.project-scaffold.v5` and records `project_schema` as
`semaprax.project.v26`. The v5 digest has its own domain; every v2-v4 byte,
schema meaning, and digest remains unchanged.

## Template

```text
semaprax new <destination> [--name project-name] --template source-command-file-text
semaprax project-scaffold --name <name> --template source-command-file-text
```

`project-scaffold` selects the tables layout when `--layout` is omitted. An
explicit frozen layout is `SPX-J115` because it cannot represent the package,
command, capabilities, dependencies, or targets tables.

The project contains one selected `fn main() -> i64`. It reads one relative
UTF-8 file, canonicalizes its decimal text, adds one, divides by three, and
writes the result. The application imports `canonicalize`, `add`, and `divide`
by their `std.int.decimal.*` stable IDs; it contains no copied arithmetic. The
sample `digits` file makes the documented native journey immediately runnable:

```sh
semaprax check .
semaprax help library std.int.decimal
semaprax build --manifest-path semaprax.toml --target native --output app
./app digits
```

Project v26 deliberately refuses interpreter test/run with `SPX-F102` and
Web, Wasm, and npm targets with `SPX-W120`. Scaffold derivation performs the
full project check before accepting that exact interpreter refusal; the native
Project v26 gate owns executable behavior.

## Descriptor contract

The descriptor keeps the v3 closed field inventory and limits. Its changed
identity is exact:

- `schema` is `semaprax.project-scaffold.v5`;
- `project_schema` is `semaprax.project.v26`;
- `template` is `source-command-file-text`;
- files are `README.md`, `AGENTS.md`, `semaprax.toml`, `src/app.spx`,
  `src/tests.spx`, and `digits`, in that order; and
- the digest domain is `semaprax.project-scaffold.digest.v5\0`.

Replay binds the selected template and project name, validates every file
hash, and independently rederives the same descriptor. Capsules from another
schema or template cannot replay as v5. The capsule carries no filesystem,
process, target-emission, native-build, or publication authority.

The generated `AGENTS.md` points directly to the compact `std.int.decimal`
help card and gives profile-specific repairs for `SPX-G174` and the exact
`SPX-H006` loan-program-point exhaustion. Those repairs preserve profile
admission and verifier limits.
