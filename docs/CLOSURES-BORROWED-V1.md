# Synchronous Borrowed Text Closures v1

Audience: language and native interop implementers.

Status: bounded implementation; owning executable gates are the language
`borrowed_closures` selector and interop-builder `borrowed_callback` selector.

This additive RI-08 profile borrows one direct, unshadowed `borrow str` parameter
of a pure monomorphic ordinary function. Its exact literal is:

```semaprax
fn(value: i64) -> i64 { transition(view, value) }
```

`transition` is a local pure monomorphic ordinary function with exact signature
`fn(view: borrow str, value: i64) -> i64`. The literal must initialize one
immutable local directly in its creator body; requires/ensures expressions
cannot create it. That callback may only be invoked directly while the
creator frame is live. Returning, copying, selecting, storing or passing the
callback, capturing local owned views, extra captures, generic creation and
nested literals are outside this profile. A borrowed callback is never retained
by a registry. Mutable borrows remain closed.

The callable retains the ordinary `fn(i64) -> i64` signature. Its HIR capture
records `borrow str`, Borrow ownership, the exact outer parameter place and the
canonical derived body parameter. Source checking and independent HIR replay
prove the direct local creation/use boundary. The outer borrow parameter is
already protected for the whole synchronous call by its caller's ordinary loan
rules. No owner-root loan is shortened, copied or invented for the capture.

Construction borrows the existing view descriptor; it does not copy text bytes
or run the callback body. Invocation stages its argument once, calls the checked
body, and publishes only successful results. The interpreter retains its abstract
borrow representation. Native C retains the pointer and length in two fixed
carrier cells; Core Wasm retains its existing authenticated borrowed-text scalar
carrier. Neither runtime owns or frees the borrowed storage.

The generated Rust projection binds one checked source function whose body
creates and immediately invokes this callback. A higher-ranked synchronous
scope supplies the borrowed text and prevents callback escape. Each Rust
invocation enters that actual checked source function; it does not replace the
body with a Rust implementation or an owned text snapshot. RI-06's typed
owner-to-view relation and invocation guard remain authoritative. The API is
same-thread and has no general public borrowed-callable ABI or stored lifetime.

The owning evidence includes a real Rust iterator under an RI-06 owner-view scope,
unchanged borrowed pointer identity, actual captured-byte reads, empty/different
text cases, authored-body and introduced-copy controls,
contract failure without result publication, same-owner reentry refusal and scope
release on panic. Rust must reject owner move/drop/mutation, callback/view/async
escape and thread transfer. Source/HIR must reject all lifetime escapes and
forged captures. Canonical source/graph plus interpreter/native O0/O2/Core Wasm
must agree before admission is committed.

## Local executable evidence

The focused language selector passed **5/5**, including canonical graph replay,
owner-loan and escape refusals, independent HIR checks, and interpreter/native
C O0/O2/Core Wasm execution with actual captured-byte reads. The builder selector
passed **2/2**, including the pinned URL consumer, changed-body/copy controls,
failed-output preservation and seven Rust lifetime/thread/owner refusals. These
were serial, locked/offline, one-job runs in a private worktree target. The
builder physical test ran directly from its built harness so nested Cargo did
not overlap an outer Cargo test process. No full-workspace or hosted gate is
claimed by this focused evidence.
