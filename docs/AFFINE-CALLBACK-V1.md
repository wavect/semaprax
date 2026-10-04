# Retained affine callback v1

Status: bounded local RI-08 retained-callback profile.
Audience: language and native interop implementers.

This additive RI-08 profile gives one owned source capture a real retained
carrier. The existing `own fn` lexical profile and Copy `fn` signatures keep
their semantics. The admitted new source type is exactly `FnOnce() -> i64`:

```semaprax
module affine.example;
@id("affine.consume") fn consume(payload: own Bytes) -> i64 { 42 }
@id("affine.make") fn make() -> FnOnce() -> i64 {
    let payload = bytes_zeroed(4usize);
    once fn() -> i64 { consume(payload) }
}
@id("affine.run") fn run(callback: own FnOnce() -> i64) -> i64 { callback() }
@id("app.main") fn main() -> i64 {
    let callback = make();
    run(callback)
}
```

## Closed admission

A literal captures exactly one available owned `Bytes` binding. Its body is
one direct call transferring that binding to a pure, monomorphic function
with signature `(own Bytes) -> i64`. The named target's full checked body,
contracts, and canonical cleanup still execute. The callback can move through
monomorphic helper results and `own` parameters. Invoking it consumes it;
using either the captured buffer or callback after a move is `SPX-O101`.
Construction and invocation in contracts are refused. Unsupported callable
signatures or parameter modes are `SPX-T308`.

This profile does not admit mutable or borrowed captures, mixed captures,
callback arguments, generic captures, aggregate fields, foreign imported
callable signatures, cross-thread calls, or an ambient source callback
registry. Its generated Rust projection has a separate same-thread retained
lease for one foreign Rust registry; that lease does not change source
admission. It therefore advances RI-08 without closing its broader acceptance
criteria.

## Authority and cleanup

HIR retains the capture's owned type and exact source identity. Independent
validation authenticates the derived closure body and capture transfer.
`core.fn_once.construct` and `core.fn_once.invoke` are compiler-owned
boundaries using ordinary left-to-right owned argument staging and canonical
commit. `core.fn_once.drop` settles an uncalled capture. Result transfer uses
the ordinary provisional result and publication plan; backends do not invent
or reorder a cleanup vector.

The native carrier contains a checked entry and owned byte buffer. The Core
Wasm carrier contains the checked table identity and owned host byte token.
The interpreter moves its unique capture into the derived body frame.
Graph v62 records the affine type and its existing closure and cleanup facts;
older source continues to select its previous graph schema.

## Generated Rust projection

`prepare_native_rust_affine_callback` selects an explicit pure factory and
renders the checked C translation unit plus a safe Rust `AffineCallback`.
The generated owner contains the actual source-created environment. Its
`into_fn_once` method returns a Rust `FnOnce`; ordinary Rust ownership rejects
a second invocation, cloning, or cross-thread transport. Dropping an uncalled
owner settles the environment. A caller can implement its own consuming safe
trait using this owner; this is not selected-index trait implementation
synthesis.

`AffineCallback::retain` moves that same unique owner into
`RetainedAffineCallback`, an opaque same-thread registration lease. A foreign
Rust registry may retain the lease, invoke it once, or call `unregister`.
`invoke` marks the lease closed before entering C and `unregister` drops an
uncalled owner, so a later invocation is
`AffineCallbackError::RegistrationClosed` without entering the native
environment. Lease Drop performs the same unregister path. The caller's
registry state remains foreign and is not an ABI, trait-object-layout, or
cross-thread claim.

Projection is inert and grants no compiler, filesystem, process, or network
authority. The caller compiles and links the generated source. Foreign code
must obey the generated C ABI; arbitrary pointer misuse is outside the safe
Rust surface. Abort, process death, native UB, and allocator exhaustion are
not made recoverable by this profile. A callback failure maps to the explicit
`AffineCallbackError::Call`; no foreign unwind crosses C.

## Owning gates

`cleanup_backends::executable_owning_closure::affine_capture` owns canonical
round-trip, graph replay, source move/signature negatives, and interpreter /
native C / Core Wasm retained-call and unused-drop behavior. The native and
Wasm probes count the captured buffer allocation and settlement.

The builder's `affine_capture_rust_consumer_retains_source_owner_and_invokes_actual_body`
selector compiles a separate Rust adapter crate and physical consumer. It
retains the source-created owner after factory return, invokes it through
`std::iter::once_with`, drops another uncalled owner, and exercises a consuming
safe trait. It also compiles a foreign stateful Rust registry that retains the
generated affine lease, adds its non-zero state after one dispatch, and
unregisters another lease before any dispatch. The teardown control rejects
before registry callback-state access, while allocation counters show the
retained source environment is settled once. Changing the authored target from
42 to 43 must fail the unchanged consumer expectation. Cross-crate rustc
controls reject duplicate invocation, clone, and thread escape for both the
owner and retained lease.

The focused core selector passed 2/2 and the physical Rust selector passed 1/1
on the submitted implementation. The Rust gate also injects callback and
factory postcondition failures; both select the explicit error and settle all
captured storage. The full quality profile was not run for this batch.

The separate [Mixed affine callback v2](AFFINE-CALLBACK-V2.md) adds one copied
`i64` snapshot under its own source type and carrier identity; it may read a
mutable outer binding at construction without retaining an alias to it.
