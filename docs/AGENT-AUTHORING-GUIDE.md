# SEMAPRAX authoring guide

Audience: authors.

Status: executable qualification pending.

## Source

Run `semaprax fmt <file>` before `semaprax check <file>`.
Native: `semaprax build <file> --target native -o <path>`.
The `fused-construction-v1` opt-in requires `--managed-bytes N`.
Web source profiles: `internal-strings-v1` or `text-toolkit-v1`.

## Project

Starters: `semaprax help new`; create: `semaprax new <directory>
--template <template>`. `semaprax.toml` declares modules, tests, dependencies,
capabilities, targets, Project profile. Then run
`semaprax check <directory>` and the README test or run command. Project profiles are distinct from single-file web
profiles.

Native UTF-8 file command: `source-command-file-text` declares `source-command.v1`,
`argv-utf8+file-text.v1`, and `native64`; it refuses Web, Wasm, and npm targets.

## Discovery

Library signatures, profiles and dependencies: `semaprax help library <module|name|stable-id>`.
Checked examples: `semaprax help shapes <kind|stable-id|path#stable-id>`.
Syntax: `semaprax help language <topic>`; `topics` lists topics. Reference: `semaprax help language all`.

Application routes: `semaprax help language author:routes`.
JSON task prerequisites:
`semaprax help language author:task:stdin-json:language-command-io.collection-record.v1:native64:0`.
Language operations: `semaprax help language author:find:<word>:0`.
Library declarations: `semaprax help language find:<word>:0`;
Pages continue.

Statement-tail exits:
`semaprax help language author:lexical-exits`.

Help grants no capability or profile.
