# Resources and cleanup

A `resource` is a value with a declared end of life. The compiler proves every
owned resource is settled exactly once on every exit path — no leaks, no
double-frees, no finalizer that can fail halfway.

## Declaring a resource

```semaprax
module buffer.app;

@id("buffer.type")
resource Buffer {
    @id("buffer.type.drop")
    drop trivial;
}

@id("buffer.inspect")
fn inspect(buffer: borrow Buffer) -> i64
{
    1
}

@id("buffer.consume")
fn consume(buffer: own Buffer) -> i64
{
    inspect(buffer)
}

@id("buffer.pipeline")
fn pipeline(buffer: own Buffer) -> i64
    ensures result == 2
{
    inspect(buffer) + consume(buffer)
}

@id("app.main")
fn main() -> i64
{
    0
}
```

`pipeline` shows the core discipline: borrow first (`inspect`), then transfer
(`consume`). Borrowing never affects ownership; the single `own` transfer at
the end settles the value.

## Drops and finalizers

Two drop strategies exist:

- `drop trivial;` — no finalization needed; the value just ends.
- `drop import "host.symbol";` — an imported host finalizer runs at scope exit.

An imported finalizer is declared through an `interface` stating its
capability, effects, failure mode, and consumed value:

```semaprax
module platform.app;

@id("platform.token")
resource Token {
    @id("platform.token.drop")
    drop import "platform.token.finalize";
}

@id("platform.token.host")
interface TokenHost
    permits { platform.token.release }
{
    @id("platform.token.finalize")
    import fn finalize(token: own Token) -> unit
        effects { platform.token.release }
        failure infallible
        consumes token always;
}

@id("app.main")
fn main() -> i64
{
    0
}
```

Automatic finalization **must be infallible** and consume the token. A
fallible operation must be an explicit consuming `close` the caller handles —
destructors never report errors. Every initialized owned resource that isn't
transferred is finalized exactly once on each language-level exit, in
canonical cleanup-plan order that downstream tools must never reorder.

## Running resources

Modules declaring resources are rejected by single-file `run` (`SPX-B104`):
they verify with `check`, and execute through a **project** native or Wasm
build (or the explicit `run <file> --native` lane). Keep interpreter-run
examples free of `resource` declarations.

```sh
semaprax check examples/ownership.spx
semaprax context examples/ownership.spx buffer.pipeline --depth 1 --filters ownership
```

`context --filters ownership` reports which parameters are `own`/`borrow` and
where each loan begins and ends — the fastest way to see the plan.

## Best practices

1. **Borrow for inspection, own for settlement.** Helpers take `borrow`;
   only sinks, builders, and `close` take `own`.
2. **Make failure impossible in finalizers.** Push fallible teardown into an
   explicit `close` returning a status the caller must handle.
3. **Transfer once, at the end.** Structure consuming functions as
   borrow-phase then transfer-phase, like `pipeline` above.

Exact rules: [RFC 0003](https://github.com/wavect/semaprax/blob/main/docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md).
