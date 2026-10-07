# Draft: Bounded Streaming Standard Input

Status: Draft; design proposal only. No compiler, host adapter, profile, ABI,
schema, or benchmark support is implemented by this document.

Audience: SEMAPRAX compiler contributors, host-adapter implementers, and
reviewers.

The assigned compiler/profile contract now lives in
[Bounded Standard Input Streaming v1](BOUNDED-STDIN-STREAM-V1.md). This draft
retains the original motivation and design questions; its placeholder API and
version language are historical, not the assigned contract.

## Summary

This draft proposes a new, explicitly selected command-input profile for
source-authored programs that must consume arbitrarily long standard input
with bounded live input memory. A single invocation-scoped linear reader would
advance through a fixed-size reusable chunk; source code would parse and
discard consumed bytes rather than first receiving a complete `Bytes` value.
The host supplies bytes only. Parsing, validation, and application behavior
remain checked SEMAPRAX code.

The proposal exists because a complete-input byte cap rejects otherwise valid
documents whose JSON whitespace is unbounded. For example, a request with more
than 65,536 whitespace bytes before a small valid JSON value must remain
acceptable when the application specification places no raw-byte limit on
whitespace. A streaming profile must not fix this by narrowing that
specification, moving the parser or algorithm into a host wrapper, or silently
switching the program to a file argument.

This is additive. [Bounded Language Command I/O v1](BOUNDED-LANGUAGE-COMMAND-IO-V1.md)
and its `argv-utf8+stdin-bytes.v1` input remain frozen with their existing
complete-snapshot limit and ownership rules. The API spellings, operation
identities, effects, chunk size, failure envelope, profile names, manifest
version, and graph schema for a future implementation are all undecided. Any
strings shown below are descriptive placeholders, not assigned registry IDs.

## Proposed execution model

A selected streaming command invocation receives one host-owned input source
and one fixed-size reusable byte buffer. The source operation exposes an
opaque, non-cloneable reader whose ownership moves linearly through source
bindings. Conceptual operations might open the reader once, advance it, inspect
whether the current chunk is empty or EOF, and borrow the current chunk. These
are operation roles, not proposed source signatures or names.

Opening is available only through an explicit `process.stdin.read` capability
and produces a reader owned by the current invocation. The reader is not a
file descriptor, general-purpose stream, or authority to open another source.
The invocation provider retains the underlying input source; generated code
receives only the closed provider calls selected by the profile. A program
cannot duplicate a reader, create another reader for the same invocation, or
reopen standard input after settlement.

Advancing refills the same bounded buffer and transfers the unique reader
ownership to its successor state. A chunk view is read-only and borrows that
state. It expires before the next advance and cannot escape through a return,
capture, aggregate, retained callback, asynchronous continuation, or invocation
settlement. Ordinary synchronous helpers may borrow the current chunk within
the call's lifetime; the reader cannot advance while that loan remains live.
Source parsing must finish
with a borrowed chunk before advancing. The checker, interpreter, native
backend, and Wasm provider must agree on this lifetime rather than relying on
buffer-layout conventions.

An advance may return fewer bytes than requested without signaling EOF. Only
the provider's explicit end-of-input state means EOF; an empty input begins at
EOF. A provider error is a closed normalized input failure, not an OS error or
JavaScript exception exposed to source. The exact status domain is open. The
selected error remains sticky, the result is not published, and reader/buffer
state settles exactly once. Cleanup cannot replace the selected failure.
Settlement releases invocation-owned buffer/provider state; the language
reader does not own or close a process-global descriptor.

All source-visible reads retain left-to-right evaluation order and the
existing explicit-effect rules. A helper can consume or advance a reader only
by receiving and returning its unique ownership. Re-entering the same reader
operation in a loop must remain visible to effect and resource analysis; it
must not be treated as an unbounded number of separately owned `Bytes`
allocations. This draft does not decide whether a reader owner may cross a
loop backedge or whether helper/call-cycle restrictions should change.

## Memory and work bounds

The input staging buffer has a fixed maximum size selected by the eventual
profile. The illustrative 4 KiB size is not a requirement. The provider does
not retain the complete input or mint a fresh owned byte allocation for each
chunk. Thus input staging memory is constant with respect to total input
length. The parser may retain only the bounded state or application values
required by its source-level algorithm; ordinary SEMAPRAX ownership and
capacity analysis still applies to those values.

This memory bound does not imply a total-work or total-input bound. Consuming
N bytes takes work proportional to N, and an input source that never reaches
EOF can keep a synchronous invocation active. The adapter's cancellation or
execution deadline policy, if any, must be specified separately and must not
be presented as a raw input-size acceptance limit. No requirement is added to
benchmark run counts, oracle comparisons, request validity, or output checks.

The implementation must define how the reusable provider buffer and live
reader count against existing capacity accounting. Charging the entire input
length would reintroduce the incompatible aggregate cap; exempting arbitrary
source-owned values would weaken the current memory contract. A proposed
accounting model should charge one bounded runtime resource for the staging
buffer and continue charging source-owned retained allocations by the existing
rules.

## Backend and adapter boundary

The host boundary only supplies the next bounded byte range, EOF, or a closed
read failure. It does not scan JSON, skip whitespace on behalf of the program,
select events, compute a report, or implement benchmark-specific policy.
Native generated code must use explicit invocation context and closed provider
calls, never direct descriptor reads. Core Wasm must use a closed import and
authenticated reader/chunk state; no WASI, ambient JavaScript process access,
or arbitrary callback is implied.

Current Wasm command imports are synchronous, while the Node command adapter
currently collects stdin asynchronously before invoking Wasm. A synchronous
bounded Node read path (for example, a reviewed fd-0 adapter) might fit a
synchronous Wasm call, but its portability and interaction with the current
adapter contract need review. An asynchronous/resumable Wasm design is a
different and substantially broader option. Neither is selected here.

The profile and package envelope must be additive and explicitly versioned.
The frozen Project v6 input and package command envelopes keep their existing
complete-input meaning. Candidate labels such as
`language-command-io.stream.v1` or `argv-utf8+stdin-stream.v1` are merely
illustrative strings and must not be treated as registry reservations. Do not
assign a Project manifest or graph schema number until owners choose the exact
projection and compatibility boundary. Graph facts for a future profile
should identify the input mode, bound, EOF/error behavior, effect, and selected
entrypoint without implying that v1 changed.

## Candidate implementation files and focused evidence

After the exact contract is resolved, likely compiler/host ownership points are:

- `src/command_io_ops.rs` for operation metadata, effects, signatures, and
  closed outcomes;
- source type resolution and verification, `src/hir/nodes.rs`, HIR validation
  and semantic-image encoding for the authenticated opaque reader and loans;
- `src/cleanup_plan/` for reader transfer, live-state joins, exactly-once
  settlement and independent replay; new runtime state cannot bypass it;
- `src/byte_data_capacity.rs` and `src/hir/byte_capacity/` for loop-aware
  dynamic read/resource accounting and call-closure rules;
- `src/interpreter.rs`, `src/interpreter/command_state.rs`, and
  `src/hosted_interpreter.rs` for invocation-scoped reader state and lifetime
  behavior;
- `src/codegen/native_command_io.rs` and
  `src/codegen/native_emit/expression/host_command.rs` for native provider
  calls and settlement;
- `src/wasm/command_io.rs`, `src/wasm/aggregate/host_command.rs`, and
  `src/wasm/environment_provider.mjs` for the closed Wasm import, carrier
  authentication, and bounded provider buffer;
- `src/project/profile.rs`, `src/project/manifest.rs`,
  `src/project/manifest/tables.rs`, and `src/project/npm/command_v3.rs` /
  `command_v4.rs` for explicit additive profile and package-envelope selection;
- `src/graph.rs` for the additive semantic projection; and
- a new owning versioned specification plus an updated completion-matrix row
  only after the executable gate exists.

Focused tests should live in their owning harnesses, not as a new top-level
integration binary:

- `tests/useful_data/bounded_language_command_io.rs` and
  `tests/useful_data/language_command_io_native.rs`: interpreter/native parity,
  repeated reads in a loop, helper ownership transfer, O0/O2, explicit effects,
  duplicate-open/clone rejection, view escape rejection, sticky read failure,
  and exactly-once settlement;
- `src/wasm/command_io/tests.rs`: identical EOF/error/ownership behavior,
  authenticated bounded chunk carriers, and no use-after-advance; and
- `src/project/npm/command_v3/tests.rs` or `command_v4/tests.rs`: selected
  streaming envelope, chunked Node input with irregular short reads, provider
  failure, and a request with more than 65,536 leading/inter-token whitespace
  bytes accepted when its parsed value is valid.

The oversized-whitespace case is the compatibility sentinel: it must reach the
source-authored parser and produce the same semantic result as its compact
equivalent. Add parser tests for escapes, multibyte UTF-8, numbers, and tokens
split at every chunk boundary. These tests must not require a weaker benchmark
oracle or move algorithmic work into an adapter.

## Open design questions

1. **Owner movement through loops and helpers:** Can a unique reader be
   reassigned across a loop backedge and returned from a helper, or should the
   first profile admit only one direct owner in a loop body? What proves that a
   reader cannot be reopened or aliased through call cycles?
2. **Resource accounting:** Is the fixed provider buffer one separately
   budgeted live resource, or does the compiler account for its bytes in the
   existing allocation ledger? How are cumulative source allocations kept
   distinct from the reusable buffer while preserving a finite live-memory
   bound?
3. **Synchronous Wasm stdin:** Can the Node adapter provide bounded chunks
   synchronously without weakening portability or host isolation, or does
   correct streaming require an explicitly asynchronous/resumable Wasm profile?
4. **Short reads and failures:** Which closed result shape distinguishes a
   short read, true EOF, and provider failure? How is the linear reader settled
   if failure occurs during refill?
5. **Profile and registry selection:** Should this be a new Project command
   profile, an additive package profile, or both? Which exact schema and
   envelope versions represent the new input mode without changing Project v6
   or existing package behavior?
6. **Nontermination policy:** Which existing execution bound, if any, limits a
   source loop reading from a pipe that never reaches EOF, without imposing a
   raw-byte cap that rejects a valid finite input?

Until these questions and an exact versioned contract are resolved, this
document is design input only. Existing v1 behavior remains authoritative.
