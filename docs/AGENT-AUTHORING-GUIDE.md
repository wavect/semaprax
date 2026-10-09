# SEMAPRAX authoring guide

Start with the route that matches the work.

## One source file

Use `semaprax fmt <file>`, then `semaprax check <file>` or `semaprax run <file>`.
Single-file native builds use `semaprax build <file> --target native -o <path>`;
there is no native `--profile` option. A single-file web build may select
`internal-strings-v1` or `text-toolkit-v1` with `--profile`; these are source
build profiles, not Project profiles.

## A Project

Use `semaprax help new` to choose a starter and `semaprax new <template>
<directory>` to create it. The generated `semaprax.toml` declares modules,
tests, dependencies, capabilities, targets, and its Project profile. Follow the
generated `README.md`, then run `semaprax check <directory>` and the listed
test or run command. Project profiles are distinct from single-file web
profiles.

For a native command that reads UTF-8 arguments and a text file, the
`source-command-file-text` starter declares `source-command.v1`,
`argv-utf8+file-text.v1`, and `native64`. Its generated README documents
build and invocation. This route refuses Web, Wasm, and npm targets.

## Find an admitted shape

Use `semaprax help library <module|name|stable-id>` for a standard-library
signature and its profile or dependency requirements. Use
`semaprax help shapes <kind|stable-id|path#stable-id>` for a checked source
example. For exact syntax, ask `semaprax help language <topic>`; use
`semaprax help language topics` to list topics. The complete quick reference
is `semaprax help language all`.

The compiler's diagnostics and exact selectors are authoritative. Help does
not grant a capability or expand a profile.
