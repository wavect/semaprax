# Contracts and effects

After this page you can state what a function promises (`requires`,
`ensures`) and what it touches (`permit`, `uses`). Both sit in the signature,
the compiler checks both, and tools read them without reading the body.

## Add a contract

<!-- handbook-smoke: {"stdout":"42\n"} -->
```semaprax
module examples.meaning;

@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    requires right >= 0
    ensures result == left + right
{
    left + right
}

@id("app.main")
fn main() -> i64
    ensures result == 42
{
    add(19, 23)
}
```

- `requires` is what the caller must satisfy. `ensures` is what the function
  guarantees. `result` names the return value.
- Clauses go between the signature and the body. They are part of the
  function's meaning, so changing one is a change to the interface.
- Contracts are checked at run time, on every backend. A violation stops the
  program and names the clause, the function, and the arguments:

```text
SEMAPRAX contract failure
  contract: requires left >= 0 in math.add
  arguments: left = -1, right = 2
```

Start with bounds (`value >= 0`), exact results for small pure helpers
(`result == left + right`), and non-empty results for builders.

## Contract or Result?

Use `requires` for something the caller must already have checked. Use a
[`Result`](types.md#handle-missing-values-and-errors) for an outcome callers
should expect, such as bad user input. A failed contract is a bug report, not
control flow. To keep deliberate rejection checks out of the passing suite, see
[Testing](../practices/testing.md). For a rule with its own identity and
solver evidence, see [Laws and proofs](laws.md).

## Declare effects

A function that performs an effect, or calls one that does, must say so. The
module permits effects first, then each function lists the ones it uses:

<!-- handbook-smoke: {"stdout":"hi0\n"} -->
```semaprax
module app.hello;

permit { process.stdout.write }

@id("app.main")
fn main() -> i64
    uses { process.stdout.write }
{
    let text = "hi";
    let view = string_as_str(text);
    let written = stdout_write(str_as_bytes(view));
    if written == 2usize { 0 } else { 1 }
}
```

`run` prints `hi` from `stdout_write`, then `0` from `main`.

A missing `permit` is `SPX-E101`. A missing `uses` is `SPX-E102`. Each message
names both edits.

Effects pass up through callers. A function that calls an effectful function
lists that effect too:

<!-- native-checked: {"stdout":"42\n"} -->
```semaprax
module app.ticking;

permit { audit.log }

@id("flow.tick")
fn tick(value: i64) -> i64
    uses { audit.log }
{
    value + 1
}

@id("app.main")
fn main() -> i64
    uses { audit.log }
{
    tick(41)
}
```

`audit.log` is a name you chose. Declaring an effect adds a visible
requirement and nothing more. The compiler-owned operations need these exact
names:

| Effect | Operations |
| --- | --- |
| `process.stdout.write` | `stdout_write` |
| `process.stderr.write` | `stderr_write` |
| `process.stdin.read` | `stdin_read` |
| `process.args.read` | `args_len`, `arg_utf8` |
| `fs.read`, `fs.write` | `file_read`, `file_write_new`, `file_stat`, `file_list`, … |
| `network.connect`, `network.read`, `network.write` | `net_connect`, `net_send`, `net_recv`, … |

The operations are in [Input and output](io.md). Installing the compiler grants
no filesystem, process, network, or signing authority. A declared effect still
needs a host that provides it. A test host can return fixed answers, and a
configured runtime host does the real work. Check the
[profile](../projects/profiles.md) before you move an effectful helper to a
new target.

Single-file `semaprax run` evaluates declared effects only for
`process.stdout.write`. Other declared effects stop with `SPX-F102`; add
`--native`.

## Mark an unsafe boundary

`unsafe` marks code that a reviewer must read. It adds no raw memory access. It
needs a module `permit { unsafe }` and a one-line audit note on each block
(`SPX-N102`, `SPX-N103`). The body is ordinary checked code:

<!-- native-checked: {"stdout":"4\n"} -->
```semaprax
module app.audited;

permit { unsafe }

@id("app.main")
fn main() -> i64
{
    let mut x = 1;
    @audit("bump the counter")
    unsafe {
        x = x + 3;
        x
    }
    x
}
```

Each boundary appears as a node in the semantic graph. Status: partial, see
[Unsafe Boundaries v1](https://github.com/wavect/semaprax/blob/main/docs/UNSAFE-BOUNDARIES-V1.md).

## Resumable effects (preview)

A function can declare `yields Request -> Response` and `yield` one request at
a time so a driver can answer it later. Only the interpreter engine and a Rust
driver run this. Ordinary native and Wasm builds refuse it (`SPX-B116`,
`SPX-W126`). Read
[Resumable Effects v1](https://github.com/wavect/semaprax/blob/main/docs/RESUMABLE-EFFECTS-V1.md)
before you use it.

## Ask for contracts and effects

```sh
semaprax context examples/meaning.spx math.add --depth 1 --filters contracts
semaprax doc examples/meaning.spx
```

`context` returns one declaration's neighborhood as bounded JSON. `doc` renders
signatures, contracts, and effects as documentation. See
[Driving Semaprax from an AI agent](../practices/agents.md).

Exact rules: [RFC 0001](https://github.com/wavect/semaprax/blob/main/docs/RFC-0001.md).
