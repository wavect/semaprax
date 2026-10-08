# Project Scaffold Capsule v6

Status: additive authority-free scaffold descriptor for the Project v27 native
stream-data command starter.

Audience: CLI users, compiler contributors, and authors of bounded native data
commands.

The `stdin-stream-data` template emits a canonical table manifest for Project
v27, profile `language-command-io.stream-data.v1`, and input
`argv-utf8+stdin-stream.v1`. It declares the exact process command capability
set and one stable command route. Its descriptor uses capsule schema
`semaprax.project-scaffold.v6` and records `project_schema` as
`semaprax.project.v27`. The v6 digest has its own domain; every v2-v5 byte,
schema meaning, and digest remains unchanged.

## Template

```text
semaprax new <destination> [--name project-name] --template stdin-stream-data
semaprax project-scaffold --name <name> --template stdin-stream-data
```

`project-scaffold` selects the tables layout when `--layout` is omitted. An
explicit frozen layout is `SPX-J115`: it cannot represent the package, command,
and capabilities tables.

The project has three canonical modules and one test module: `app`, `input`,
and `tests`. The selected command and ordinary `main` both return `i64`.
`input.spx` shows one private immutable `borrow Vec<i64>` helper and its fixed
capacity caller. It does not buffer stdin or define an application data format;
borrowed chunks must be processed within their loan block.

Build the selected command on its admitted native route:

```sh
semaprax build --manifest-path semaprax.toml --target native --output app
./app
```

Project v27 refuses Web, Wasm, and npm targets with `SPX-W120`. Its selected
streaming command requires the native runtime. The inherited authority-free
Project interpreter can evaluate the ordinary pure `main` and test closures,
without a stdin provider or command adapter; derivation checks the full source
revision and tests that pure closure. Native Project v27 gates own command
execution. A `web` export records the closed command identity for manifest
admission and does not admit a Web artifact. See [Stream Data Command v1](STREAM-DATA-COMMAND-V1.md).

## Descriptor contract

The descriptor keeps the v3 closed field inventory and limits. Its changed
identity is exact:

- `schema` is `semaprax.project-scaffold.v6`;
- `project_schema` is `semaprax.project.v27`;
- `template` is `stdin-stream-data`;
- files are `README.md`, `AGENTS.md`, `semaprax.toml`, `src/app.spx`,
  `src/input.spx`, and `src/tests.spx`, in that order; and
- the digest domain is `semaprax.project-scaffold.digest.v6\0`.

Replay binds the selected template and project name, validates every file hash,
and independently rederives the same descriptor. Capsules from another schema
or template cannot replay as v6. The capsule carries no filesystem, process,
target-emission, native-build, or publication authority.

The generated `AGENTS.md` states the bounded streaming and helper boundary.
It does not relax the 4096-byte stream-reader capacity or the profile's private
Copy-scalar vector signature allowlist.
