# Project Manifest v4 and Useful Data Command v1

Audience: language users, tool authors, and compiler contributors.

Status: locally evidenced. Hosted promotion and registry publication remain
open, so the completion claim remains Partial.

## Closed profile

Project v4 is additive: v1-v3 canonical bytes and behavior are unchanged. Its
canonical manifest has exactly ten assignments and selects only
`useful-data-command.v1`:

```toml
schema = "semaprax.project.v4"
name = "spxgrep"
version = "0.1.0"
profile = "useful-data-command.v1"
entry = "spxgrep.app"
sources = ["src/app.spx", "src/tests.spx"]
web_exports = ["spxgrep.contains"]
command = "spxgrep.contains"
capabilities = ["process.stdout.write"]
tests = ["spxgrep.tests"]
```

`web_exports` contains exactly the command stable ID. The command has exact
signature `(borrow Slice<u8>, borrow Slice<u8>) -> bool`; its complete linked
closure admits only `process.stdout.write`. The entry module alone may declare
that permit. The test closure remains effect-free. The additive command linker
reconstructs the permit and effect rather than weakening the legacy
effect-free Useful Data linker.

## Compiler-free command product

The npm build uses the independently replayed
`semaprax.project-npm-build.v3` carrier and exactly seven artifacts:

1. `app.wasm`
2. `semaprax.js`
3. `semaprax.bindings.js`
4. `semaprax.bindings.d.ts`
5. `semaprax.command.json`
6. `semaprax.command.js`
7. `package.json`

The fixed Node adapter snapshots one UTF-8 argument and stdin bytes into one
combined 65,536-byte budget. The command may write only a selected external
Slice parameter or its immutable alias; local arrays, owned data, and helper
writes are rejected. Wasm records only that stable scratch pointer/length and
copies bytes into the transcript page after semantic success. The adapter then
flushes the sealed guest transcript only after arena settlement.
Match exits 0, no match exits 1, and input, linking, settlement, or physical
flush failure exits 2. No compiler, source tree, current-working-directory
lookup, network, registry, WASI, stdout callback, or general host capability is
needed after package generation.

The reference `examples/spxgrep-project` performs a real nested indexed-byte
search. A match writes the original stdin bytes exactly once; absence writes
nothing. NUL, `0xff`, invalid UTF-8 stdin, empty needles, exact-capacity input,
combined overflow, carrier tamper, and forced post-return settlement failure
are part of the local boundary evidence.

## Authority boundary and nonclaims

The language operation is the success-published semantic transcript specified
by [Bounded Stdout Transcript v1](BOUNDED-STDOUT-TRANSCRIPT-V1.md). Only the
fixed command adapter owns the final physical stdout write, and an adapter
failure cannot turn into semantic success.

This profile does not add language-level stdin or argv, files, arbitrary
commands, environment access, stderr, multiple writes, streaming, async,
dependencies, lockfiles, Windows-safe npm publication, registry publication,
signing, provenance, or a general CLI framework.
