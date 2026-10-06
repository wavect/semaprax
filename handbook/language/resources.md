# Resources and cleanup

After this page you can declare a value with a defined end of life, a
`resource`, and describe a state machine with a `session protocol`. The
compiler proves that every owned resource is settled exactly once on every
exit path. There is no leak, no double free, and no finalizer that fails
halfway.

## Declare a resource

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

`pipeline` borrows first (`inspect`), then transfers (`consume`). A borrow never
changes ownership. The final `own` transfer settles the value. Using `buffer`
after `consume(buffer)` is `SPX-O101`, the same error as for a moved `string`,
see [Ownership](ownership.md).

A single-file `semaprax run` rejects modules that declare resources
(`SPX-B104`). Verify them with `check`, and run them through a Project's native
or Wasm build or with `run file.spx --native`.

## Choose how a resource ends

| Drop | Meaning |
| --- | --- |
| `drop trivial;` | Nothing to finalize. The value ends at scope exit. |
| `drop import "host.symbol";` | A host function finalizes the value when it goes out of scope. |

An imported finalizer is declared in an `interface` that states its effect,
its failure mode, and the value it consumes:

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

A finalizer that runs automatically must be `infallible` and must consume the
token. A teardown that can fail is an explicit `close` function that returns a
status for the caller to handle. A destructor never reports an error.

Cleanup runs in a fixed order set by the cleanup plan, once for each owned
resource that was not transferred, on every exit. Failure selection is sticky:
cleanup cannot replace the status that was already selected.

```sh
semaprax context examples/ownership.spx buffer.pipeline --depth 1 --filters ownership
```

`context --filters ownership` lists each parameter as `own` or `borrow` and
where each loan starts and ends.

## Describe a state machine

A `session protocol` names states and the moves between them. It is checked and
then erased: native and Wasm output do not change, and it grants no authority.

```semaprax
module app.checkout;

@id("checkout.session")
session protocol "checkout-v1" {
    states { Idle, Open, Committed, Failed }
    initial Idle;
    terminal Committed cleanup {}
    terminal Failed cleanup {}
    on Idle begin: send BeginRequest via "checkout.begin" -> Open;
    on Idle abort: fail Unit -> Failed;
    on Open commit: send CommitRequest via "checkout.commit" -> choice { committed: Committed, refused: Failed };
    on Open lost: fail Unit -> Failed;
}

@id("checkout.begin")
fn begin() -> i64
{
    1
}

@id("checkout.commit")
fn commit() -> i64
{
    2
}

@id("app.main")
fn main() -> i64
{
    0
}
```

- `states` lists the states and `initial` picks the start.
- Each `terminal` state names its cleanup, and a terminal has no moves out.
- `on <state> <label>: <kind> <Payload> … -> <state>` is one move. The kind is
  `send`, `receive`, `call`, `return`, `cancel`, `timeout`, or `fail`. Use
  `choice { label: state, … }` for several outcomes.
- `via "<function-id>"` ties a move to a function in the same module by `@id`
  (`SPX-K104`).
- Every non-terminal state needs a `cancel`, `timeout`, or `fail` exit.
  Violations are `SPX-K101` to `SPX-K106`.

A function can opt in with `follows session protocol` to have its call order
checked (`SPX-K107` to `SPX-K109`). Status: local evidence, see
[Session Protocol Types v1](https://github.com/wavect/semaprax/blob/main/docs/SESSION-PROTOCOL-TYPES-V1.md).

## Borrow, then own

Helpers borrow. Only sinks, builders, and `close` own. Structure a consuming
function as a borrow phase followed by one transfer.

Exact rules: [RFC 0003](https://github.com/wavect/semaprax/blob/main/docs/RFC-0003-CLEANUP-AND-RESOURCE-ABI.md),
[Shared Loan Plan v1](https://github.com/wavect/semaprax/blob/main/docs/SHARED-LOAN-PLAN-V1.md).
