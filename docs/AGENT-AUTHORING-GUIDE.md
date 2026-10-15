# SEMAPRAX authoring guide

Status: executable qualification pending.

## Source

Use `semaprax fmt <file>`, then `semaprax check <file>`.
Native: `semaprax build <file> --target native -o <path>`.
The `fused-construction-v1` opt-in requires `--managed-bytes N`;
qualification pending. Web source profiles: `internal-strings-v1` or `text-toolkit-v1`.

## Project

Starters: `semaprax help new`; create: `semaprax new <directory>
--template <template>`. `semaprax.toml` declares modules, tests, dependencies,
capabilities, targets and Project profile. Follow `README.md`: run
`semaprax check <directory>` and its test or run command. Project profiles are distinct from single-file web
profiles.

For a native UTF-8 file command, `source-command-file-text` declares `source-command.v1`,
`argv-utf8+file-text.v1`, and `native64`. Its README documents invocation.
This route refuses Web, Wasm, and npm targets.

## Discovery

Library signatures, profiles and dependencies: `semaprax help library <module|name|stable-id>`.
Use
`semaprax help shapes <kind|stable-id|path#stable-id>` for checked examples.
Syntax: `semaprax help language <topic>`; topics: `semaprax help language topics`. Reference: `semaprax help language all`.

Application routes: `semaprax help language author:routes`.
Language operations: `semaprax help language author:find:<word>:0`.
Library declarations: `semaprax help language find:<word>:0`;
Pages include continuations.

For statement-tail break/continue/return contexts, use
`semaprax help language author:lexical-exits`; executable qualification is pending.

Help grants no capability or profile.
