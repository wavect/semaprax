# Project Manifest v18: Process I/O

Status: implemented private manifest/profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
Broader physical-provider platform support remains separately scoped.

Audience: compiler, project-tooling, and standard-library contributors.

Project v18 selects the private `process-io.v1` profile. It gives one authored
command entry a checked, bounded process provider while preserving the frozen
older Project manifest schemas and their authority. This document specifies
the manifest admission boundary; the process operation and provider contract
remain owned by [Bounded Process I/O v1](BOUNDED-PROCESS-IO-V1.md).

## Manifest

The frozen Project v18 projection has the following ordered assignments:

```toml
schema = "semaprax.project.v18"
name = "process-fixture"
version = "1.0.0"
profile = "process-io.v1"
entry = "process.app"
sources = ["src/app.spx", "src/tests.spx"]
web_exports = []
command = "process.run"
capabilities = ["process.execute"]
tests = ["process.tests"]
```

`version` is canonical SemVer text. The manifest requires 2..=16 explicit,
strictly ordered relative `.spx` source paths. `web_exports` is always empty:
this profile admits private command execution and does not construct a public
ABI or Web artifact. The command identifies an authored `fn() -> bool` entry;
it has no command input table or public export signature.

The profile's capability inventory is the six sorted names
`process.args.read`, `process.environment.read`, `process.execute`,
`process.stderr.write`, `process.stdin.read`, and `process.stdout.write`.
`capabilities` may select any nonempty sorted subset of that inventory, but it
must include `process.execute`; unrelated effects and unsorted lists are
rejected. The command may import `std.process` and `std.io` for checked
internal composition. Those imports do not create public ABI authority.

Schema/profile mismatches, nonempty Web exports, an input declaration, missing
`process.execute`, capabilities outside the six-name inventory, and malformed
source or command fields fail closed before publication. Project v17 and all
older schemas retain their existing parsing, canonical bytes, profiles, and
authority.

## Admission and non-claims

The manifest is authenticated as one Project snapshot. Linking and command
preparation retain ordinary source, dependency, effect, ownership, cleanup,
and provider checks. The empty export list keeps this route private; it is not
a public process API and does not establish production support.

The package gate covers example, conformance, and bundled-consumer commands
on the interpreter, native C11 `-O0`/`-O2`, and Core Wasm. The historical local
witness also included five physical Darwin provider cases; those observations
retain their original host scope. The implemented manifest, process and replay
corpus has hosted-green release evidence. Broader public/process functionality
and additional physical-host claims remain separate from that accepted baseline;
a backend fixture does not establish an unselected physical-provider scenario.

## Owning implementation

`src/project/manifest.rs` owns the frozen v18 line shape and canonical render;
`src/project/manifest/tables.rs` owns lowering from `semaprax.manifest.v1`;
`src/project/profile.rs` owns `ProcessIoV1`, its six capability names, and
subset validation. Process operation and physical-provider ownership are
specified in [Bounded Process I/O v1](BOUNDED-PROCESS-IO-V1.md).
