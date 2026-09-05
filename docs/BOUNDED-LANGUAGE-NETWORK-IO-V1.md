# Bounded Language Network I/O v1

Audience: language users, tool authors, and compiler contributors.

Status: locally evidenced implementation tranche. This document freezes the
reviewed contract for six compiler-owned TCP client operations, their closed
status domain, the deterministic fixture provider, and the target adapters. No
hosted or public product claim is made. TLS clients and listen sockets are an
additive hosted-only protocol in [Bounded Network Services v1](BOUNDED-NETWORK-SERVICES-V1.md); they do not mutate this v1 ABI. No DNS-policy, structured-task, or
production claim is made; the [completion matrix](COMPLETION-MATRIX.md) owns
status and its "Edge and server" row remains Partial.

## Objective

[Bounded Language Command I/O v1](BOUNDED-LANGUAGE-COMMAND-IO-V1.md) made a
process's arguments, stdin, stdout, and stderr visible to checked SEMAPRAX code
without granting ambient process authority. This tranche extends the same
closed host-command operation family with explicit, effect-gated TCP client
operations. Like command I/O, these are not authored imports: their complete
signature, authority, status space, and capacity are derived from one closed
operation table (`src/network_io_ops.rs`) that every backend consumes.

| Operation | Stable identity | Effect (`uses`) | Result |
| --- | --- | --- | --- |
| `net_connect(host: borrow Slice<u8>, port: usize) -> usize` | `core.host.net-connect` | `network.connect` | fallible; the new handle |
| `net_send(handle: usize, value: borrow Slice<u8>) -> usize` | `core.host.net-send` | `network.write` | fallible; bytes accepted, equal to `byte_len(value)` on success (blocking full write) |
| `net_recv(handle: usize, max: usize) -> own Bytes` | `core.host.net-recv` | `network.read` | fallible; one blocking read of at most `max` bytes; empty `Bytes` is end of stream |
| `net_stream_stdout(handle: usize, max: usize) -> usize` | `core.host.net-stream-stdout` | `network.read` and `process.stdout.write` | fallible; reads at most `max` bytes and appends them to the stdout transcript; returns the count, `0` is end of stream |
| `net_wait(handle: usize, timeout_ms: usize) -> usize` | `core.host.net-wait` | `network.read` | fallible; `0` timeout, `1` readable, `2` peer closed |
| `net_close(handle: usize) -> usize` | `core.host.net-close` | `network.connect` | fallible; `0`; the handle becomes unknown |

Arguments evaluate left to right before the host call. Every operation is
fallible: a nonzero status aborts the enclosing function exactly like any
other fallible host-command operation, failure selection stays sticky, and
cleanup cannot replace the selected status.

The names are reserved: declaring your own `net_connect` is `SPX-S113`. A
call with the wrong arity or argument types is `SPX-T270`.

## Handles

A handle is an invocation-scoped `usize` token. Successful `net_connect`
calls hand out dense handles `1`, `2`, … up to `8`; a handle is never reused
within one invocation, and closing one does not free its number. Handles are
not file descriptors and never leave the invocation: they cannot be stored
across invocations, and a forged, stale, or closed handle fails with
`UNKNOWN_HANDLE`.

The provider or adapter that granted network authority closes every handle
that is still open at settlement, on success and on failure alike. Program
outcome therefore never depends on `net_close` order, and a program that
omits `net_close` leaks nothing past its own invocation.

## Limits

| Bound | Value |
| --- | ---: |
| open handles per invocation | 8 |
| `host` bytes (RFC 1035 name length) | 253 |
| `port` | 1 to 65,535 |
| bytes one `net_recv` or `net_stream_stdout` may deliver (`max`) | 65,536 |
| cumulative sent plus received bytes per invocation | 1,048,576 |
| `net_wait` timeout | 30,000 ms |
| combined stdout plus stderr transcript | 65,536 |

The host must be nonempty, strict UTF-8, contain no NUL, and fit the length
bound; otherwise `net_connect` fails with `INVALID_ENDPOINT`. Exceeding any
other bound is `CAPACITY_EXCEEDED`, including a `net_stream_stdout` call whose
bytes would overflow the transcript.

### Three different bounds

"Thirty seconds" can name three unrelated things, and this tranche keeps them
apart.

| Bound | Scope | Selected by | Guarantees |
| --- | --- | --- | --- |
| per-syscall timeout | one blocking `recv`, `send`, `poll`, or `connect` | the provider, derived from the deadline | that *one* call returns |
| operation deadline | one whole provider operation: name resolution, every candidate address, the TLS handshake, every partial write, every retried read, and every readiness wait | the host, when it constructs the provider | the operation returns, whatever the peer or resolver does |
| invocation budget | one program invocation | the language | dense handles `1..=8`, `max` at most 65,536, 1,048,576 cumulative bytes |

The invocation budget bounds *how much* a program transfers, never *how long*
that takes; evaluator fuel likewise bounds language steps, not time spent
inside an injected provider.

The operation deadline is monotonic and caller-selected, clamped to a fixed
safe maximum of 30,000 ms. A host that asks for more is clamped rather than
refused, so no configuration path produces an unbounded operation. Every
sub-operation receives the *remaining* duration, so several failing candidate
addresses, several partial writes, or several interrupted calls draw down one
total instead of restarting it. An interrupted system call resumes against the
same deadline and preserves the same selected failure. A timed-out operation
retains no connection: no handle was issued, and settlement still runs on
every path.

Name resolution is inside the deadline. The Rust provider takes an explicitly
injected bounded resolver; the shipped one answers a literal address with no
name service at all, and otherwise runs the platform resolver on a worker it
*owns* — a worker whose caller stopped waiting stays registered and is reaped,
never detached and forgotten.

This is a bound on *waiting*, not forced cancellation. `getaddrinfo` and its
`std` wrapper cannot be aborted from another thread, so the honest guarantee
is that the caller stops waiting on time and the abandoned work stays
accounted for. Nothing here promises cancellation a selected host cannot
enforce.

The generated native C11 adapter carries the same aggregate deadline on a
monotonic clock across candidate addresses, partial writes, retried reads, and
readiness waits, and derives each per-syscall timeout from what is left.
`getaddrinfo` itself is the one step it cannot bound: the C11 adapter has no
owned resolver worker, so a native program's connect may still block in the
platform resolver before its deadline arithmetic begins. That gap is stated,
not closed, and the native adapter's resolution bound must not be described as
equivalent to the Rust seam's.

`net_recv` follows `stdin_read` exactly: its owned result slot is initialized
only on status zero, and each `net_recv` call site counts as one owned-byte
allocation site with the conservative payload `65,536`, so sixteen sites reach
the existing 1 MiB owned-byte payload bound. Reading the value uses the
ordinary `bytes_as_slice` and `byte_len` operations and the value is dropped
exactly once by the ordinary cleanup plan.

## Status domain

Every operation fails into the closed normalized domain `semaprax.network.v1`
(class Adapter, retryability known false):

| Code | Name | Meaning |
| --- | --- | --- |
| 1 | `CONNECT_FAILED` | refused, unreachable, timed out, or the fixture had no matching connection |
| 2 | `INVALID_ENDPOINT` | host empty, over 253 bytes, containing NUL, not UTF-8, or port `0`/over 65,535 |
| 3 | `UNKNOWN_HANDLE` | forged, stale, or already closed handle |
| 4 | `CAPACITY_EXCEEDED` | more than 8 handles, `max` over 65,536, cumulative bytes over 1,048,576, timeout over 30,000, or transcript overflow |
| 5 | `TRANSFER_FAILED` | connection reset, short write, read error, or a fixture `expect_send` mismatch |
| 6 | `AUTHORITY_DENIED` | the invocation was given no network provider |

OS errors, `errno` values, Winsock codes, and JavaScript exceptions are never
smuggled into this domain; adapters normalize them to one of the six codes.

## Streaming model

`net_stream_stdout` is the streaming primitive. It returns a Copy scalar,
borrows nothing that outlives the call, and appends the received bytes to the
same success-only stdout transcript that `stdout_write` uses, so it is the
operation a `while` body may call:

```text
let mut streamed = 1usize;
while streamed > 0usize {
    streamed = net_stream_stdout(handle, 4096usize);
    streamed > 0usize
}
```

`net_wait` and `net_close` are likewise admitted in loop conditions and
bodies. `net_recv` is not: it produces an owned `Bytes` value, and
[While Loops v1](WHILE-LOOPS-V1.md) admits only Copy-scalar operations inside
loops because an owned value created per iteration would need a cleanup edge
whose count is not statically known. A program that needs the received bytes
as a value calls `net_recv` once per path outside every loop; a program that
only forwards them streams. Placing `net_recv` in a loop is `SPX-T270`
(`command I/O operation `net_recv` is not admitted in while bodies`).

Streamed bytes are observable only after the root result and all cleanup
settle successfully; a failed path discards both transcripts, exactly as for
`stdout_write`.

## Async model

There are no tasks, threads, futures, callbacks, or schedulers in this
tranche. Asynchrony is expressed with bounded readiness polling:

- `net_wait(handle, timeout_ms)` blocks for at most `timeout_ms` (at most
  30,000) and returns `0` when nothing arrived, `1` when a read would return
  bytes, and `2` when the peer closed and an end-of-stream read is pending;
- a program owning several handles polls them in turn inside one `while`
  loop, acting on the readable ones and bounding its total wait with an
  explicit attempt counter, so termination follows from the counter rather
  than from the peer.

`std.async` (below) holds the pure arithmetic of such loops. This is
deliberately not structured concurrency: [Scoped Task Model v1](SCOPED-TASKS-V1.md)
is the design that future task, cancellation, and cleanup semantics must
preserve, and nothing here anticipates its syntax or runtime.

## HTTP

HTTP/1.1 over plain TCP is composed from these operations: `net_connect` to
port 80, one `net_send` per request piece (method, path, ` HTTP/1.1\r\nHost: `,
host, `\r\nConnection: close\r\n\r\n`), then `net_stream_stdout` or
`net_recv` for the response, then `net_close`. `std.http` parses the bytes:
status code, header terminator, body length, `Content-Length`, and method
validation. `examples/net_http_get.spx` is the committed request shape.

This v1 ABI has no TLS, so only `http://` endpoints are reachable through it.
The additive hosted service profile provides authenticated outbound TLS
without introducing a cleartext fallback.

## Authority

Programs declare `permit { network.connect, network.read, network.write }`
(plus `process.stdout.write` when they stream) and every function that calls
an operation, or calls a function that does, declares the effects under
`uses`. The ordinary rules apply: a missing module permit is `SPX-E101`, a
missing `uses` is `SPX-E102` (`call to `net_connect` requires effect
`network.connect`; add it to …`).

The compiler grants nothing ambient. A *provider* is injected by the host
adapter for one invocation:

- the **fixture provider** is a deterministic effect handler for tests and
  browsers, driven by the JSON document below; it is the first deterministic
  handler of the standard library's `test` tier and performs no I/O;
- the **TCP provider** (interpreter library seam and native runtime) uses
  blocking `std::net` or POSIX/Winsock sockets under one caller-selected
  aggregate operation deadline (see [three different
  bounds](#three-different-bounds)), no TLS, and either an injected bounded
  resolver or the platform resolver on an owned worker. It exists only when a
  host explicitly constructs it; Project v12's CLI and Web lanes construct
  only the deterministic fixture provider.

An invocation without a provider fails every operation with
`AUTHORITY_DENIED`. `semaprax capability-manifest` projects such a module
with `network` and `process` marked `declared`; the manifest remains a
read-only declaration and its enforcement nonclaim in
[Capability Manifest v1](CAPABILITY-MANIFEST-V1.md) is unchanged.

## Fixture schema

The fixture provider consumes one `semaprax.network-fixture.v1` document:

```json
{
  "schema": "semaprax.network-fixture.v1",
  "connections": [
    {
      "host": "example.org",
      "port": 80,
      "expect_send": "GET / HTTP/1.1\r\nHost: example.org\r\nConnection: close\r\n\r\n",
      "ready": true,
      "recv": ["HTTP/1.1 200 OK\r\ncontent-length: 5\r\n\r\n", "hello"]
    }
  ]
}
```

- Connections bind in `net_connect` order: the i-th successful connect binds
  the i-th entry, whose `host` and `port` must match exactly or the connect
  fails with `CONNECT_FAILED`.
- `recv` chunks are UTF-8 strings delivered in order. A chunk larger than the
  call's `max` is split and the remainder stays pending. After the last chunk,
  `net_recv` returns empty bytes, `net_stream_stdout` returns `0`, and
  `net_wait` returns `2`.
- `expect_send`, when present, is the exact cumulative byte sequence the
  program must have sent before its first read; a mismatch is
  `TRANSFER_FAILED`.
- `ready: false` makes the first `net_wait` return `0` once, after which the
  connection reports readable.

## Targets

- **Interpreter.** `semaprax::hosted_interpreter::execute_network_command`
  mirrors `execute_language_command`: the entry is one stable-ID function of
  exact signature `() -> bool`, the module's permits must be a subset of
  `process.args.read`, `process.stdin.read`, `process.stdout.write`,
  `process.stderr.write`, `network.connect`, `network.read`, and
  `network.write` and include at least one `network.*` token, and the
  operation profile `NetworkV1` must be satisfied. The caller passes the
  provider. `semaprax run` on such a module reports `SPX-B103`; the reference
  interpreter grants no network authority.
- **Native.** A C11 adapter over POSIX sockets, or Winsock on Windows, is
  compiled only when the network profile is selected; the ordinary native
  emitter for other profiles contains no socket code. Generated functions
  receive the provider through the explicit invocation context and never
  touch descriptors directly.
- **Wasm.** Network operations lower to synchronous, closed `env` imports
  that a host satisfies only from the fixture provider. There is no WASI
  socket import and no Node `net` binding: a Wasm program gaining real
  sockets from an ambient host module would be exactly the implicit authority
  this language refuses, and browsers have no raw TCP in any case. Under the
  earlier `LanguageV1` and `LineV1` command profiles a network operation is
  rejected with `SPX-W114`.
- **Project/npm/Web.** `network-command-io.v1` is the exact Project v12
  profile. It selects one `() -> bool` command, the existing
  `argv-utf8+stdin-bytes.v1` snapshot, and the sorted seven-capability
  inventory. Native builds use the TCP adapter. npm/Web builds expose
  `createFixture`, one-shot `createInvocation`, and authenticated
  `instantiate`; they accept only `semaprax.network-fixture.v1` data and never
  import Node sockets, WASI sockets, or browser fetch.
- **CLI fixture execution.** `semaprax network-run <project> --fixture file`
  executes the manifest command through the fixture provider. Repeated
  `--arg`, optional `--stdin`, and `--max-steps` configure the bounded command
  snapshot. The fixture is capped at 1 MiB and eight connections; combined
  argv/stdin remains capped at 65,536 bytes.

## Standard library

Three pure `portable`-tier packages accompany the operations. None declares an
effect; each composes with the operations by inspecting the scalars and byte
views they produce or consume, and each passes the standard-library gate on
the interpreter, native C11, and Core Wasm lanes with no provider present.

| Package | Functions |
| --- | --- |
| `std.net` | `port_is_valid(port: usize)`, `host_is_valid(host)` (nonempty, at most 253 bytes, dot-separated labels of 1–63 letters, digits, or interior hyphens), `is_ipv4(host)` (strict dotted quad), `wait_is_timeout`/`wait_is_readable`/`wait_is_closed(state: usize)` |
| `std.http` | `status_code(response) -> i64` (`-1` unless the bytes start `HTTP/1.x NNN ` with the required reason-phrase separator), `is_success(code)`, `has_header_end`, `header_end`, `body_len`, `content_length(response) -> i64` (`-1` when absent, malformed, or the header is unterminated), `method_is_valid(method)` |
| `std.async` | `clamp_wait_ms`, `next_timeout_ms(attempt, base_ms, cap_ms)` (doubling, capped at 30,000), `should_retry(state, attempts, max_attempts)`, `next_handle(current, count)` (round robin over `1..=count`), `remaining_ms`, `stream_ended(chunk)` |

The [standard library catalog](STANDARD-LIBRARY-CATALOG.md) lists every
signature with its contract. The packages take `usize` where the operations
produce `usize`; the language has no `usize`/`i64` conversion, so `i64`
helpers such as `std.time` do not compose with `net_wait` directly.

## Diagnostics

| Code | Trigger |
| --- | --- |
| `SPX-E101` | a module calls an operation without permitting its effect |
| `SPX-E102` | a function reaches an operation without declaring its effect under `uses` |
| `SPX-T270` | a network operation with the wrong call shape or argument types, or `net_recv` inside a `while` condition or body |
| `SPX-S113` | a user declaration named after a reserved operation |
| `SPX-W114` | a network operation under a Wasm command profile that does not admit it |
| `SPX-B103` | `semaprax run` on a module whose reachable code performs command or network I/O |

## Local evidence

The focused local gates are:

```sh
cargo test --locked -p semaprax --test useful_data -- bounded_language_network_io::
cargo test --locked -p semaprax --test useful_data -- network_io_interpreter::
cargo test --locked -p semaprax --test useful_data -- network_io_native::
cargo test --locked -p semaprax --test useful_data -- network_io_wasm::
cargo test --locked -p semaprax --test project -- standard_library::
cargo test --locked -p semaprax --test project -- manifest_v12::
cargo test --locked -p semaprax --test examples
cargo test --locked -p semaprax --lib network_provider::deadline::
cargo test --locked -p semaprax --lib network_provider::resolver::
cargo test --locked -p semaprax --lib network_provider::tcp::deadline_tests::
```

The three `--lib` gates cover the aggregate deadline itself: budget clamping,
remaining-duration propagation, a slice that is never zero and never restarts,
literal-address resolution without a worker, an abandoned resolver worker that
is reaped rather than detached, and — over real loopback sockets under a short
selected budget — a slow resolver, several candidate addresses, a silent
reader, a stalled writer, a capped readiness wait, and a bounded accept. Each
one finishes in under a second by advancing an injected clock or selecting a
short budget, never by waiting out thirty seconds.

The remaining gates cover the closed operation table, effect and reserved-name
diagnostics,
loop admission, the fixture provider's connection binding, chunk splitting,
`expect_send`, and readiness rules, the six status codes, handle and byte
capacities, transcript sealing, the interpreter seam, the native adapter, the
Wasm imports, the three standard-library packages on every listed lane, and
the canonical committed example. These are local artifact and execution facts;
they promote no completion row.

## Non-claims and remaining work

This v1 tranche does not add TLS or certificate policy, DNS policy beyond the
platform resolver, listen or accept sockets, UDP, HTTP/2 or HTTP/3, a
request/response type model, structured tasks or cancellation, threads,
  callbacks, timers other than the bounded `net_wait`, connection reuse across
invocations, proxies, or observability. Project v12, its CLI verb, and its
npm/Web package are developer-preview fixture lanes, not TLS or public socket
support; npm and browsers gain no real socket authority. Each later protocol
is sequenced in the
[roadmap](ROADMAP.md#concurrency-and-services). The separately versioned
[Bounded Network Services v1](BOUNDED-NETWORK-SERVICES-V1.md) now implements
the hosted-provider TLS/listen slice while preserving these bytes and targets.
