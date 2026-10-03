# Native Rust Local Future Bridge v1

Status: detached RI-09 implementation batch. This is a local Rust adapter
profile, not a Semaprax source async import or public SDK claim.

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
`take_output` transfers the value once. Exceeding the limit discards it. A
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

## Executable gates and remaining work

The native Rust builder's `future_bridge_tests` module compiles the exact
rendered source with `rustc --test` and executes deterministic waker,
settlement, capacity, panic, and cancellation cases. A compile-fail control
requires `Send` from the local handle and must report `E0277`.

The explicitly selected `generated_local_future_bridge_runs_locked_reqwest_and_cancels_received_request`
gate stages the exact generated source with the checked-in Cargo manifest and
lock. It runs `reqwest` against a local TCP server under a caller-created Tokio
current-thread runtime. The server confirms receipt before cancellation and
counts requests to detect an implicit retry. This gate is ignored by default
until a checkout-private Cargo target is explicitly supplied.

RI-09 remains open until the real locked gate passes at the claimed commit,
the response becomes a checked Semaprax value, and a source-authenticated
async import and reverse async export have executable evidence. RI-08's
callback registration and the source suspension owner must be connected
without weakening their authority or checkpoint rules. No Stream, implicit
Tokio startup, background executor thread, or network effect is admitted here.
