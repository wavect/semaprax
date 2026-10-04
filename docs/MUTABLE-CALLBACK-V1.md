# Mutable Callback v1

Status: implemented for the narrow source-level RI-08 profile below.

## Narrow profile

`FnMutI64(i64) -> i64` is a same-thread, non-copyable callback with exactly
one captured `i64` state and one `i64` invocation argument. Its only literal
form is:

```semaprax
mut fn(value: i64) -> i64 { transition(state, value) }
```

`state` must be one direct available `i64` binding. `transition` must be a
local, monomorphic, pure function with exact signature
`fn(i64, i64) -> i64`. There are no borrowed, owned, computed, record, generic,
or additional captures in this profile.

## Transaction

Construction snapshots `state` into the callback carrier. Invocation performs
these ordered steps:

1. Reject a dead or active receiver before reading state.
2. Read the current state and stage the invocation argument.
3. Call the checked transition body.
4. Commit the returned state only after the body returns success.
5. Return the committed state as the invocation result.

A contract, arithmetic, host, or reentry failure leaves the prior state live
and unchanged. The carrier is same-thread and has a per-receiver active guard;
nested entry to that receiver fails before executing its body. Copy, clone,
ordinary `Fn`, `FnOnce`, foreign-thread transfer, and escape through aggregate
or public signatures are refused.

## Required implementation agreement

Parser and formatter must retain the distinct type and `mut fn` literal.
Source verifier, HIR, graph, cache codecs, cleanup plan, and callable validation
must identify the same state capture and transition function. Interpreter uses
one mutable receiver cell. Native C and Core Wasm use a mutable receiver carrier
with an active bit and update its state only after the status result is success.
Generated Rust exposes the same receiver as a local `FnMut` adapter with the
same reentry and commit ordering.

Focused evidence must execute repeated success, failure rollback, reentry
refusal, canonical source/graph replay, native C at `-O0` and `-O2`, Core Wasm,
and a separately compiled generated Rust trait consumer. It must also reject
copying, state aliasing, second-thread use, and all captures outside the fixed
profile.

## Executable evidence

The `language` harness selector `mutable_closures` covers canonical source and
Graph replay, unique ownership facts, snapshot isolation, repeated calls in the
interpreter, native C at `-O0`/`-O2`, Core Wasm, and source/retained-HIR refusals.
The core `mutable_` selector includes rollback, active and foreign-thread guards,
and a Wasm failure probe that inspects the actual receiver slot after failure.
The native Rust interop crate's `mutable_callback` selector compiles the generated
owner and a separate Rust consumer. It exercises success, contract/arithmetic
rollback, source-body and factory-failure controls, drop accounting, and compiler
refusals for Copy, Clone, Send, Sync, overlapping borrows and adapter escape.

The scalar state needs no resource finalizer. Its semantic type is non-Copy and
uses unique ownership; invocation borrows the available mutable local without
consuming it. Direct receiver value reads and same-receiver entry while staging
an argument are refused. Private factories may return newly constructed receivers;
callback parameters and public/aggregate callback signatures remain outside this
profile. The generated Rust owner controls the source-created native receiver's
lifetime and starts a fresh checked status context for each call.
