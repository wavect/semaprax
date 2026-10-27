# SEMAPRAX authoring guide

Executable qualification pending.

Source: `semaprax fmt <file>`, then `semaprax check <file>`.
Native: `semaprax build <file> --target native -o <path>`.
`fused-construction-v1` opt-in requires `--managed-bytes N`;
Web: `internal-strings-v1` or `text-toolkit-v1`.

Start: `semaprax help new`;
create: `semaprax new <directory> --template <template>`.
`semaprax.toml`: modules, tests, dependencies, capabilities, targets and
Project profile. `README.md`: `semaprax check <directory>`, test or run.
Project profiles are distinct from single-file web profiles.

UTF-8 file command: `source-command-file-text` declares `source-command.v1`,
`argv-utf8+file-text.v1`, and `native64`. README: invocation.
Web, Wasm and npm targets are refused.

Library signatures, profiles and dependencies: `semaprax help library <module|name|stable-id>`.
Examples: `semaprax help shapes <kind|stable-id|path#stable-id>`.
Syntax: `semaprax help language <topic>`; topics: `semaprax help language topics`;
full: `semaprax help language all`.
Routes: `semaprax help language author:routes`.
Native v31 JSON request/decision/response prerequisites (pending):
`semaprax help language author:task:stdin-json:language-command-io.collection-record.v1:native64:0`.
Operations: `semaprax help language author:find:<word>:0`.
Declarations: `semaprax help language find:<word>:0`. Follow continuations.
Statement tails: break/continue/return; `semaprax help language author:lexical-exits`;
qualification pending.
Help grants no capability or profile.
