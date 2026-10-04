# Mixed retained affine callback v2

Status: additive private source profile; focused local execution evidence only.
Audience: compiler contributors and native Rust callback adapter authors.

This extends [Retained affine callback v1](AFFINE-CALLBACK-V1.md) with the
separate owned type `FnOnceI64() -> i64`. Its environment has exactly two
ordered captures: one owned `Bytes`, followed by one copied value `i64`.
The original `FnOnce() -> i64` type, graph v62 selection, cache tags, intrinsic
identities and native carrier remain unchanged.

```semaprax
module affine.mixed;

@id("mixed.consume")
fn consume(payload: own Bytes, offset: i64) -> i64
{
    offset + 2
}

@id("mixed.make")
fn make(offset: i64) -> FnOnceI64() -> i64
{
    let payload = bytes_zeroed(4usize);
    once fn() -> i64 { consume(payload, offset) }
}

@id("app.main")
fn main() -> i64
{
    let callback = make(40);
    callback()
}
```

## Admission and identities

The `once fn` literal's exact tail-call shape selects its type: one direct
`Bytes` binding selects v1; `Bytes` followed by one direct available `i64`
binding selects v2. The target must be a pure monomorphic local function with
exact `(own Bytes, i64) -> i64` signature. The scalar may be mutable, but
construction copies its current value into the retained environment, so later
writes to the outer binding cannot alias the callback. Computed scalar operands,
borrowed captures, extra captures and generic creation sites remain closed.
Unsupported scalar capture shapes use `SPX-T308`. The two callable
types cannot substitute for each other. Canonical source retains the distinct
type spelling, and AST/HIR cache codecs assign new leaf tags without changing
old tags.

Source and independent HIR validation authenticate both capture types, modes,
positions, outer bindings, expression identities and derived body parameter
identities. The retained body must pass those exact two parameters to its
checked target. The owned capture and callback each move once; reuse remains
`SPX-O101`. The scalar remains available in its enclosing scope.

Graph v63 records the `affine_function` profile `bytes-i64-to-i64.v2`, both
ordered captures, the checked derived body, and the ordinary cleanup plan.
Graphs containing only v1 affine values retain v62. Replay refuses altered
capture schemas and cleanup facts.

## Runtime and cleanup

The owning boundaries are `core.fn_once_i64.construct.v2`,
`core.fn_once_i64.invoke.v2`, and `core.fn_once_i64.drop.v2`. The constructor's
canonical owning argument is the `Bytes` value. After staging it, construction
reads the second capture as a direct, non-failing scalar snapshot,
and commits the sole owning argument. The snapshot has no cleanup epoch or
transfer authority. Restricting it to a direct scalar read makes
this boundary complete: no omitted operand can fail, allocate, mutate, borrow,
or transfer another owner. A future computed or owning capture needs a new
boundary and replay contract.

Native C uses the separate `spx_once_i64_v2` carrier and a typed entry accepting
`Bytes`, `i64`, and the ordinary checked output/status parameters. Core Wasm
uses its existing fixed environment, with the owned byte token in slot zero
and the scalar in slot one. The interpreter retains the same ordered pair.
Invocation transfers the byte owner and passes the stored scalar to the
derived body. Unused-drop settles only the byte owner. Postcondition failure
retains its selected status and settles the ordinary canonical plan.

Modules with only affine helper signatures still select the owned runtime and
carrier declarations. This selection does not fabricate a closure definition or
change the graph projection.

The inert `prepare_native_rust_affine_callback` projection accepts either exact
factory result type. V2 selects a separate source revision domain and typed C
bridge. The safe Rust owner and same-thread retained lease keep v1's consumption
and teardown rules. No foreign trait-object layout or cross-thread ABI is added.

## Focused evidence and remaining scope

`cleanup_backends::executable_owning_closure::affine_capture` owns canonical
round-trip, graph replay and hostile schema rejection, two distinct scalar
snapshots, mutable-snapshot non-aliasing, retained helper moves, duplicate-call
and computed-capture refusal, unused affine-parameter helpers without a factory, and
interpreter/native C O0/O2/Core Wasm execution with balanced owner
counts. The builder's `mixed_affine_capture_rust_consumer` executes generated
C and a separately compiled Rust adapter/consumer through `std::iter::once_with`
and a retained foreign registry. It includes body-result negative control,
factory and callback postcondition failures, unused-drop, unregister, and
rustc rejection of duplicate use, cloning and thread escape.

This bounded shape does not close RI-08. Synchronous borrowed source callbacks,
mutable captured environments that preserve state across calls, arbitrary capture
records, selected safe
trait synthesis, nested foreign re-entry and broader callback signatures retain
their own missing implementation and execution gates. No full quality profile
or hosted result is claimed for this batch.

Local gates use the worktree's private target, locked offline Cargo, jobs=1,
and disabled incremental/debug data:

```sh
cargo test --locked --offline -p semaprax --test cleanup_backends affine_capture -- --nocapture
cargo test --locked --offline -p semaprax-native-rust-interop --lib affine_capture_rust_consumer -- --nocapture
```

The core selector passed 6/6 and the physical selector passed 2/2 with no
ignored cases on macOS arm64, rustc/Cargo 1.98.0, Apple Clang 21.0.0 and
Node 24.3.0. These are local results; the full quality profile was not run.
