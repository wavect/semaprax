# Native Rust Local Future Bridge v1

Audience: maintainers and contributors.

Status: implemented for the bounded RI-09 same-thread Future bridge and
Project-selected source `yield` profile. This is not an authored `import rust
fn` syntax or public SDK package claim.

## Boundary

`render_local_future_bridge()` returns deterministic Rust source. The caller
stages it in a separately authorized package and supplies an executor. The
render operation reads no Project, index, lock, Cargo configuration, network,
process, or home state. It grants no effect or publication authority. The
standalone source forbids unsafe code and depends only on `std`.

`LocalFutureLimit::start` accepts one `Future<Output = T> + 'static`; the future
is pinned in a Rust `Box` before its first poll. Captures must therefore be
owned or independently proven `'static`. The handle deliberately contains
`Rc` and has no `Send` or `Sync` implementation. Only the thread owning the
handle may poll, cancel, or take output. The current profile never serializes
the future, its output, or any waker to a durable checkpoint.

No runtime is created by the adapter. A caller-owned `Context` supplies the
executor waker on every poll. A safe `Arc<Wake>` relay provides a Rust waker to
the selected future. The relay retains only the latest executor waker, clones
it under a mutex, and calls it after releasing the lock. A wake arriving
before registration occupies one pending bit. Multiple wakes may coalesce;
the adapter allocates no queue entry per wake. Cloned relay wakers remain
memory-safe after cancellation or handle drop, but become inert then.

## State and settlement

The handle owns one of `Pending`, `Ready(T)`, `Taken`, `Cancelled`, or
`OutputTooLarge`. `poll` rejects reentry before touching the pinned future and
rejects any poll after `Ready`. On `Ready`, an explicit caller-supplied byte
weigher checks the output against a per-handle maximum before publication.
`take_output` transfers the value once. The owned `LocalFuture<T>` and
`&LocalFuture<T>` also implement Rust `Future<Output = Result<T,
FutureBridgeError>>`: awaiting either uses the caller's executor, transfers a
ready value once, and reports a settled error on a later poll. The shared form
lets another local task request cancellation while the waiter is pending.
Exceeding the limit discards the output. A
panic in the future or weigher settles as `Panicked` and cannot cause a second
poll of a completed future.

`cancel` drops a pending future or an untaken output once. If cancellation
arrives during a poll, it records a request; the outer poll discards any
result before publication, then drops the future. Cancellation wakes a parked
executor so it can observe the terminal state. Drop of the handle invalidates
the relay and returns one slot to the explicitly constructed
`LocalFutureLimit`. The limit bounds live handles. The output maximum bounds
one result; the relay's pending bit bounds queued wake state. It does not
bound allocations performed by the selected Rust future or the number of
waker clones that future retains.

Dropping a future does not undo an external effect. A request already received
by a server remains an observed request after local cancellation. No retry is
authorized by this bridge.

## Executable gates and profile limits

The native Rust builder's `future_bridge_tests` module compiles the exact
rendered source with `rustc --test` and executes deterministic waker,
settlement, capacity, panic, and cancellation cases. A compile-fail control
requires `Send` from the local handle and must report `E0277`.

The explicitly selected `generated_local_future_bridge_runs_locked_reqwest_and_cancels_received_request`
gate stages the exact generated source with the checked-in Cargo manifest and
lock. It runs `reqwest` against a local TCP server under a caller-created Tokio
current-thread runtime and directly awaits the shared bridge handle. The server
confirms receipt before cancellation and
counts requests to detect an implicit retry. It also keeps a pending adapter
through actual `LocalSet` and runtime shutdown: the future drops once, live
capacity returns to zero, and a cloned late waker is inert after shutdown.
This gate is ignored by default until a checkout-private Cargo target is
explicitly supplied.

The separate ignored toolchain gate
`ri09_async::locked_reqwest_response_enters_checked_semaprax_bytes_export`
builds an authenticated Project v8 owned-data SDK from `.spx` source, then
runs a locked reqwest consumer against a local server. A caller-created Tokio
current-thread runtime awaits the Rust bridge and passes the response bytes
to the generated SDK. The authored Semaprax function copies the slice after
its first byte, so the observed `hello` response becomes the checked `ello`
value. A second response exceeds the bridge output bound and is refused
before any Semaprax call. This is a Rust-owned suspension followed by a
checked synchronous Semaprax export; the source program does not await Rust.

The bounded RI-09 source import is the exact Project-selected `yield` site:
the generated Rust module pins its checked source plan and registers one
caller-owned Rust Future callback before returning an awaitable checked-source
export. The [Project contract](PROJECT-SOURCE-LOCAL-FUTURE-V1.md) owns the
locked, physically compiled generated-module and source-drift gate. Authored
`import rust fn` syntax, published SDK packaging and RI-08 retained callback
registration require separate admission. No Stream, implicit Tokio startup,
background executor thread, or network effect is admitted here.

## Checked source interpreter adapter

`resumable_effects::source_local_future::SourceLocalFuture` is a separate,
ephemeral adapter over the checked source interpreter. Its constructor takes
exact canonical `.spx` bytes, checks and resolves them, validates the HIR,
derives the compiler-owned source effect signature, and admits exactly one
direct `i64 -> i64` yield in a function with one `i64` argument and result.
It evaluates the pure prefix to the suspension before returning. The handler
is invoked only on the first Rust poll, returns one caller-owned Future, and
receives the checked request. On ready, the interpreter resumes against the
same in-memory program, argument, state, binding, and request. The adapter
does not publish a Project SDK, authenticate a Project lock, or turn source
`yield` into ordinary native emission.

The adapter is deliberately `!Send`; the caller supplies every wake and the
executor. A pending Rust Future is dropped on cancellation. Any external
effect already performed by that Future remains external and is never
reported as rolled back. The source continuation, Rust Future, and waker are
never serialized or attached to the durable source journal. Handler error,
source language failure, fuel exhaustion, and rejected evaluation have
separate outcomes. The exact unit selector
`resumable_effects::source_local_future::tests::` exercises pending/wake/ready,
single host dispatch, checked result, noncanonical source refusal, and pending
future drop. A second test uses Rust `.await` on the selected checked-source
adapter while its injected Rust Future returns `Pending`, wakes the explicit
caller waker, and then returns the checked source result. This unit selector
exercises the interpreter-backed export; the generated Project module has a
separate physical consumer gate. Handler or host-Future panic settles as
`Panicked` and cannot
leave a half-consumed handle available for another poll.

An opt-in [Project source local Future](PROJECT-SOURCE-LOCAL-FUTURE-V1.md)
profile separately binds this interpreter route to one authenticated Project
selection. It keeps Rust-only async export inventory outside Web exports and
refuses ordinary target emission. It can render a generated Rust module that
pins the selected source yield and checks one-shot callback registration at
runtime. A published Rust SDK package and source-authored `import rust fn`
declaration remain open.
