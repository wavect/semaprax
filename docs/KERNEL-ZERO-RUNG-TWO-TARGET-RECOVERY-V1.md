# Kernel-0 Rung-2 Target and Recovery Evidence v1

Audience: compiler and self-hosting contributors.

Status: local, private test gate for issue #188. Its required-tool scalar
target/recovery cases passed in the 124-case exact `f99c76dc2` selector recorded
by [Accepted-Revision Validation v1](KERNEL-ZERO-ACCEPTED-REVISION-VALIDATION-V1.md).
This document does
not promote a self-hosting rung, transfer formatter authority, define a public
target ABI, or introduce an owned `Bytes`/string buffer.

## Subject

The subject is five scalar byte lanes in authored bootstrap-v2 order: `char`,
`bool`, `int`, `operator`, and `string-scalar`. Rust formatter boundaries stay
authoritative. Target evidence lives in `target_execution.rs`; recovery lives
in `recovery.rs`.

For every selected input, the harness obtains the Rust boundary's bytes first.
It then checks all of the following against those bytes:

1. the retained bootstrap-v2 generated C11 source, compiled and executed at
   both `-O0` and `-O2` with a local driver for `render-length` and
   `render-byte`;
2. the retained private Core-Wasm scalar-export companion, bounded to one
   component payload, structurally validated during decode, and loaded by the
   local Node driver; and
3. the existing per-lane Rust byte oracle.

The retained v2 raw Core-Wasm field deliberately has no parameterized entry
wrapper. The separately retained private scalar-export companion is a closed
v2 component field, followed by its SHA-256 digest. The ordinary scalar-export
builder only supplies private Node bindings; the harness overwrites `app.wasm`
with the exact retained companion before Node loads it. Neither field is
published or a public ABI.

The exact v2 wire is specified in
[Rung-2 Bootstrap Artifact v2](KERNEL-ZERO-RUNG-TWO-BOOTSTRAP-V2.md). The v1
document remains the historical v1 contract; v2 intentionally refuses a
structurally valid v1 artifact rather than guessing or silently upgrading it.

## Bounds and hostility

Each companion is capped by the bootstrap component maximum (1 MiB). The
decoder bounds and digests it with the rest of the component wire, validates
the retained Core-Wasm structure, and exact-compares it with compiler
regeneration. It refuses a plus-one length, structurally invalid bytes, or a
different closed component's otherwise valid companion.

The target-execution test needs local `clang` and `node`. Every availability,
compiler, native-execution, and Node-execution command uses one 20-second
bounded runner. It drains piped stdout and stderr on reader threads while
retaining at most 64 KiB from each; deadline expiry kills and waits for the
direct child. On Unix each command has a fresh child-owned process group; the
runner kills that whole group before reader collection, even after the direct
child exits, so descendants cannot retain inherited pipes beyond the same
deadline. Its private scratch directory is a `create_dir` allocation with a
128-bit `getrandom` suffix and RAII cleanup. The test skips only when tools are
absent; setting `SEMAPRAX_REQUIRE_KERNEL_ZERO_RUNG_TWO_TARGETS` makes absence a
failure. No CI selector or hosted run is implied by this local contract.

## Candidate recovery

The `run_native` and `run_wasm` paths return `Result`, mapping tool spawn,
deadline, nonzero exit, output parse, I/O, and candidate-byte mismatch to a
lane-specific `CandidateRefusal`. Recovery always borrows the already-produced
Rust bytes. Even a matching candidate does not become the returned value, so a
successful test cannot accidentally make the target lane formatter authority.
The hostile cases inject corrupted C and a corrupted retained Wasm companion,
recover with pointer-identical Rust bytes, then execute the valid candidate
successfully to demonstrate re-entry. A malformed target transcript similarly
becomes a parse refusal rather than an assertion or panic.

This model is deliberately narrower than a production failover path. There is
no production target invocation, no target-selected buffer, no persistence,
and no retry authority. It proves only that the proposed evidence route does
not replace the Rust result when its candidate is missing or wrong.

## Non-claims and rung boundary

Exact regeneration authenticates reproducibility; it is not independent
semantic authentication. A deterministic bad producer can regenerate a
self-consistent but wrong artifact, and only the executed Rust target oracle
catches that case. This is local finite-corpus execution evidence, not a
backend-equivalence theorem. It does not execute the raw v2 Wasm field
directly, prove a general C/Wasm correctness property, add a component wrapper
to the language, or make an owned formatter buffer available to Kernel-0. Rust
remains the sole formatter authority, and rung 2 remains unreached until
maintainers decide the owned-buffer/authority contract and the required
independent hosted gate has an accepted result.
