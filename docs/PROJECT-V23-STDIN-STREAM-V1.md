# Project v23: Native Streaming Standard Input

Status: proposed profile specification; implementation and executable gates
are in progress. This document does not claim interpreter, native, Wasm, npm,
hosted, or release support. Project v6 and its complete-input contract remain
frozen.

Audience: SEMAPRAX project authors and compiler, linker, and host-adapter
contributors.

Project v23 is the explicit Project manifest route for a native command that
reads standard input incrementally. The selected profile is
`language-command-io.stream.v1`; its input contract is
`argv-utf8+stdin-stream.v1`. Source code owns the parsing and application
algorithm. The host supplies bounded chunks and EOF or a normalized read
failure.

The source-level reader and chunk lifetime are defined by the streaming stdin
contract in [Bounded Stdin Stream v1](BOUNDED-STDIN-STREAM-V1.md).
This Project profile does not change that contract's application semantics or
relax any source-language ownership, effect, or capacity rule.

## Frozen manifest

The frozen layout is UTF-8 with LF line endings and exactly eleven ordered
assignments followed by one terminal LF:

```toml
schema = "semaprax.project.v23"
name = "stream-check"
version = "0.1.0"
profile = "language-command-io.stream.v1"
entry = "stream.app"
sources = ["a/app.spx", "b/tests.spx"]
web_exports = ["stream.command"]
command = "stream.command"
input = "argv-utf8+stdin-stream.v1"
capabilities = ["process.args.read", "process.stderr.write", "process.stdin.read", "process.stdout.write"]
tests = ["stream.tests"]
```

All Project v6 field constraints remain in force, including canonical package
version, source ordering and bounds, stable IDs, and the command result
contract. Only the schema, profile, and input values differ. The capability
list is the exact v6 command inventory; `process.stdin.read` identifies the
stream operation family and does not grant direct descriptor access. The
manifest may not add, omit, reorder, or reinterpret capabilities.

The extensible `semaprax.manifest.v1` table layout selects the same profile
using `[package].profile`, `[command].function` and `[command].input`, and
`[capabilities].required`. It lowers to `semaprax.project.v23` with the same
fixed capability list. The frozen Project v6 and v7 schemas reject the stream
profile and input value; they retain their existing snapshot and line-input
contracts.

## Admission and targets

The `command` stable ID selects one public `fn() -> bool` function from the
linked entry-plus-command closure. Profile admission independently validates
the transitive command closure against the closed
`CommandOperationProfile::StdinStreamV1` operation inventory. Calls to helpers
in other declared project modules remain subject to the same closure check.
Disconnected declarations do not widen command authority.

Native generation is an explicitly selected profile route. It uses the
invocation-scoped stream provider and does not preload or aggregate stdin into
the v6 65,536-byte snapshot. The native host adapter may read process stdin
only through the injected provider. It does not close the process-global
descriptor.

Project v23 has no Wasm or npm bridge in this proposal. Web and npm build
requests fail with `SPX-W120` before preparing or publishing artifacts. They
must never fall through to the v6 snapshot route. A later Wasm adapter requires
its own explicit bounded provider contract and executable gate.

## Focused evidence required

Before this profile can be marked implemented, its owning tests must establish:

- canonical Project v23 parsing and table-layout lowering, with v6 and v7
  rejecting the v23 input/profile pair;
- admission of the exact selected command and transitive helper closure, with
  unrelated or disallowed operations rejected;
- interpreter/native parity for EOF, positive short reads, normalized read
  failure, cleanup, and reader loans across `next`;
- native input larger than the v6 snapshot limit reaching the source parser;
- Web and npm refusal before candidate artifacts are created; and
- canonical graph projection and versioned profile facts without changing
  earlier graph, Project, or package bytes.

The root completion matrix remains the authority for implementation status.
