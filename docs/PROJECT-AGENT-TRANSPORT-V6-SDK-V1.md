# Project Agent Transport v6 SDK/discovery v1

Status: additive implementation and owning local tests; not transport or package promotion.

Audience: SDK consumers, transport integrators, compiler contributors, and reviewers.

The compiler exposes an authority-free, out-of-band discovery document with
schema `semaprax.project-agent-transport-v6-discovery.v1`. It describes the
unchanged `semaprax.agent-transport.v6` protocol, exactly
`project/api-describe` and `project/npm-build-inline`, and the closed Project,
descriptor, and carrier schema triples for Project v8 through v11. It records
the transport byte ceilings and explicitly identifies TypeScript, Python, and
Rust codecs.

Generated codecs only construct LF-delimited JSON-RPC request bytes and decode
caller-supplied response bytes. They never locate or launch `semapraxd`, read a
manifest, select a path or tool, inspect an environment, open a socket, or
perform filesystem, source, workspace, package, or publication effects. The
caller owns process and byte transport plumbing.

The decoders reject surplus wrapper/result fields, mismatched request IDs,
unknown profile triples, invalid descriptor digests, descriptor/profile
disagreement, and carrier/profile disagreement. Nested descriptor semantic
replay and carrier authentication remain server responsibilities; the SDK does
not relabel structural outer validation as authentication.

This artifact adds no protocol method and does not alter v2-v6 wire bytes. Its
owning gate is the `agent_transport_v6_sdk` module in the Project integration
harness.

The owning executable gate generates and executes the Python codec and embeds
an exact checked-in rendering of the generated Rust codec in the existing Rust
test harness. Byte-for-byte generator equality is checked before the embedded
Rust codec runs, so this gate needs no ambient Cargo, rustc, dependency home, or
network resolution. Both codecs drive a real retained v6 daemon for every
Project v8-v11 profile. A separate ignored gate requires explicitly selected
TypeScript 5.8.3 `lib/tsc.js` and Node 22-or-newer files before it compiles and
executes the TypeScript codec. Every external command runs with an empty
environment. The harness owns temporary files and a bounded byte relay; the
generated codec receives only revisions and response bytes. The relay caps
requests at 1 MiB, responses at 16 MiB, stderr at 64 KiB, and each direct-child
wait at 30 seconds.

The Python and TypeScript adapters and the admitted daemon route are checked
for process-launch primitives. The selected TypeScript entry and implementation
are checked for direct child-process calls before use. These source scans are
tripwires over the named files, not recursive proof over arbitrary imports.
The gate therefore requires the selected Python, Node, TypeScript, and
`CARGO_BIN_EXE_semapraxd` images and every path ancestor to remain immutable and
quiescent for the complete run; it proves neither tool provenance nor safety
under concurrent pathname replacement. Windows must provide an absolute
`SEMAPRAX_TEST_PYTHON`; Unix may use one of the fixed absolute system Python
locations when no explicit selection is supplied. TypeScript always requires
explicit absolute `SEMAPRAX_TEST_TSC` and `SEMAPRAX_TEST_NODE` selections.
Within that precondition, the tripwires support the gate's deliberately narrow
direct-child/no-descendant contract. They are not a general process-tree
supervisor or proof about arbitrary third-party interpreters. Every admitted
direct child is owned by an idempotent bounded settlement guard which kills
and repeatedly probes it to completion on timeout or unwinding; no blocking
wait is used after the deadline.

The executable cases cover the four closed profile triples, every foreign
cross-profile project/descriptor/carrier/build-schema substitution, per-profile
descriptor-digest corruption, exact LF framing, stale project and workspace
subjects for both methods with recovery, local request validation, mismatched
IDs, error envelopes, surplus keys, malformed JSON, and oversized response
rejection. Project fixture inventories are byte-identical before and after
every retained session. Static authority token checks and zero-write
inventories are regression tripwires; they are not an operating-system sandbox
or proof of network isolation.

Generated-client compilation and execution is evidence only for this codec
contract, not registry publication, installed-product support, daemon peer
authentication, hosted or cross-platform passage, or Project profile
promotion. The decoder validates the closed outer transport shape; descriptor
semantic replay and carrier authentication remain exclusively server-side.

Focused local commands:

```sh
cargo test --locked -p semaprax --test project \
  agent_transport_v6_sdk::live_conformance::generated_python_and_embedded_rust_clients_drive_all_retained_v6_profiles \
  -- --exact --test-threads=1

# Required on Windows; optional on Unix when a listed absolute system Python exists.
SEMAPRAX_TEST_PYTHON=/absolute/python \
cargo test --locked -p semaprax --test project \
  agent_transport_v6_sdk::live_conformance::generated_python_and_embedded_rust_clients_drive_all_retained_v6_profiles \
  -- --exact --test-threads=1

SEMAPRAX_TEST_TSC=/absolute/typescript-5.8.3/lib/tsc.js \
SEMAPRAX_TEST_NODE=/absolute/node-22-or-newer \
cargo test --locked -p semaprax --test project \
  agent_transport_v6_sdk::live_conformance::provisioned_typescript_client_drives_all_retained_v6_profiles \
  -- --exact --ignored --test-threads=1
```
