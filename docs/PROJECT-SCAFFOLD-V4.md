# Project Scaffold Capsule v4

Status: additive authority-free scaffold descriptor for the Project v25 native
stream-text starter.

The `stdin-stream-text` template emits a table manifest selecting Project v25
profile `language-command-io.stream-text.v1`. Its descriptor uses capsule schema
`semaprax.project-scaffold.v4` and records `project_schema` as
`semaprax.project.v25`. The v4 digest has its own domain; v2 and v3 descriptor
bytes, schema meanings, and digests remain unchanged.

## Template

```text
semaprax new <destination> [--name project-name] --template stdin-stream-text
semaprax project-scaffold --name <name> --template stdin-stream-text
```

The project manifest contains one stable command export, the exact
`argv-utf8+stdin-stream.v1` input, and the four capabilities required by the
stream-text profile. It includes a reusable streaming reader, a private
cross-module owned-String helper, and a test module that calls that helper.
`project-scaffold` selects the tables layout for this template when `--layout`
is omitted. An explicit frozen layout is refused.

The generated command is built for native only:

```sh
semaprax build --manifest-path semaprax.toml --target native --output app
```

`semaprax doctor --profile` reports compiler support; it does not select the
Project profile or command. `semaprax run .` executes the separate ordinary
`main` entry. Web, Wasm, and npm targets are refused by this profile.

## Descriptor contract

The descriptor keeps the v3 closed field inventory and limits. Only its
`schema`, `project_schema`, template, file inventory, and digest domain differ:

- `schema` is `semaprax.project-scaffold.v4`;
- `project_schema` is `semaprax.project.v25`;
- `template` is `stdin-stream-text`;
- files are `README.md`, `AGENTS.md`, `semaprax.toml`, `src/app.spx`,
  `src/input.spx`, and `src/tests.spx`, in that order; and
- the digest domain is `semaprax.project-scaffold.digest.v4\0`.

Replay binds the selected template and project name, validates every file
hash, and independently rederives the same descriptor. A v3 capsule cannot
claim the v25 project schema, and v4 cannot replay as a v1 template. The
capsule carries no filesystem, process, target-emission, or publication
authority.
