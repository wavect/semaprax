# SEMAPRAX authoring guide

Status: executable qualification pending.
Audience: coding agents.

## Source

Use `semaprax fmt <file>`, then `semaprax check <file>`.
Native: `semaprax build <file> --target native -o <path>`.
The `fused-construction-v1` opt-in requires `--managed-bytes N`;
qualification pending. Web source profiles: `internal-strings-v1` or `text-toolkit-v1`.

## Project

Choose a starter with `semaprax help new`; create it with `semaprax new <directory>
--template <template>`. `semaprax.toml` declares modules, tests, dependencies,
capabilities, targets, and its Project profile. Follow `README.md`: run
`semaprax check <directory>` and its test or run command. Project profiles are distinct from single-file web
profiles.

For a native UTF-8 file command, `source-command-file-text` declares `source-command.v1`,
`argv-utf8+file-text.v1`, and `native64`. Its README documents invocation.
This route refuses Web, Wasm, and npm targets.

## Discovery

Use `semaprax help library <module|name|stable-id>` for library signatures
and profile or dependency requirements. Use
`semaprax help shapes <kind|stable-id|path#stable-id>` for checked examples.
For syntax, ask `semaprax help language <topic>`; use
`semaprax help language topics` to list topics. The quick reference is `semaprax help language all`.

Choose an application route with `semaprax help language author:routes`.
Find language operations with `semaprax help language author:find:<word>:0`.
Find library declarations with `semaprax help language find:<word>:0`;
Each page prints its continuation.

For statement-tail break/continue/return contexts, use
`semaprax help language author:lexical-exits`; executable qualification is pending.

Help grants no capability or profile.
