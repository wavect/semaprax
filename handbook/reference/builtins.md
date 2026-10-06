# Built-in functions

Compiler-owned functions are reserved names available in every file: no import,
no dependency. Declaring your own `string_len` fails with `SPX-S113`. Spell
every type argument. Every borrowed view takes a plain `let` binding.

Print the same table from your compiler with `semaprax help language builtins`.
For library functions you import, see [Standard library](stdlib.md).

## Strings

| Function | Signature |
| --- | --- |
| `string_len`, `string_len_chars` | `(s: string) -> i64`: bytes, or Unicode scalars |
| `string_is_empty` | `(s: string) -> bool` |
| `string_concat` | `(a: string, b: string) -> string`; consumes both |
| `string_starts_with`, `string_contains` | `(s: string, other: string) -> bool` |
| `string_from_char` | `(c: char) -> string` |
| `string_from_i64` | `(value: i64) -> string`; canonical decimal text |
| `string_from_usize` | `(value: usize) -> string`; canonical decimal text |
| `string_as_str` | `(binding: string) -> borrow str`; a `let` binding, never a literal |

## String views

| Function | Signature |
| --- | --- |
| `str_len_bytes` | `(s: borrow str) -> i64` |
| `str_is_empty` | `(s: borrow str) -> bool` |
| `str_starts_with`, `str_contains` | `(s: borrow str, other: borrow str) -> bool` |
| `str_as_bytes` | `(s: borrow str) -> Slice<u8>` |

## Bytes and buffers

| Function | Signature |
| --- | --- |
| `byte_len` | `(v: borrow Slice<u8>) -> usize` |
| `byte_get` | `(v: borrow Slice<u8>, i: usize) -> Option<u8>` |
| `byte_range` | `(v: borrow Slice<u8>, start: usize, end: usize) -> Slice<u8>` |
| `bytes_copy` | `(v: borrow Slice<u8>) -> Bytes` |
| `bytes_zeroed` | `(count: usize) -> Bytes`; literal capacity |
| `bytes_set` | `(b: own Bytes, i: usize, v: u8) -> Bytes`; write-once chain |
| `bytes_as_slice` | `(b: borrow Bytes) -> Slice<u8>` |
| `array_as_slice` | `(a: borrow [u8; N]) -> Slice<u8>` |

## Vectors, iterators and boxes

`T` is one of the eight Copy scalars (`i64`, `i32`, `u8`, `usize`, `f64`, `f32`,
`bool`, `char`).

| Function | Signature |
| --- | --- |
| `vec_with_capacity<T>` | `(usize) -> Vec<T>` |
| `vec_push<T>` | `(own Vec<T>, T) -> Vec<T>`; thread the owner. Pushing past capacity fails at run time. |
| `vec_set<T>` | `(own Vec<T>, index: usize, value: T) -> Vec<T>` |
| `vec_reserve_exact<T>` | `(own Vec<T>, additional: usize) -> Vec<T>` |
| `vec_clear<T>` | `(own Vec<T>) -> Vec<T>` |
| `vec_len<T>`, `vec_capacity<T>` | `(borrow Vec<T>) -> usize` |
| `vec_get<T>` | `(borrow Vec<T>, usize) -> T` |
| `vec_into_iter<T>` | `(own Vec<T>) -> Iter<T>` |
| `iter_next<T>` | `(own Iter<T>) -> IterStep<T>`; match with `match own` on `IterStep::Done {}` and `IterStep::Yield { item, rest }` |
| `box_new<T>` | `(value: T) -> Box<T>` |
| `box_get<T>` | `(value: borrow Box<T>) -> T` |
| `box_into_inner<T>` | `(value: own Box<T>) -> T`; consuming |

Walk a vector with `for item in values { ... }` or consume an iterator with
`for own item in iterator { ... }` ([Loops](../language/loops.md)).

## Command I/O

Each needs its effect in `permit` and `uses`.

| Function | Signature | Effect |
| --- | --- | --- |
| `stdout_write`, `stderr_write` | `(v: borrow Slice<u8>) -> usize` | `process.stdout.write`, `process.stderr.write` |
| `stdout_append`, `stderr_append` | `(v: borrow Slice<u8>) -> usize`; cumulative, 65,536 bytes shared | same |
| `args_len` | `() -> usize` | `process.args.read` |
| `arg_utf8` | `(i: usize) -> borrow str` | `process.args.read` |
| `stdin_read` | `() -> own Bytes` | `process.stdin.read` |

Single-file `run` admits `stdout_write` only. `args_len`, `arg_utf8`,
`stdin_read` and `stderr_write` need a project with the `useful-data-command.v1`
profile on the native target. `stdout_append` and `stderr_append` belong to
the line-command profile (`line-command-io.v1`).

## Filesystem

Paths are bounded relative byte prefixes against a provider root the host
injects. Writes create new files unless you use the atomic forms.

| Function | Signature | Effect |
| --- | --- | --- |
| `file_read` | `(path: borrow Slice<u8>, length: usize, max: usize) -> own Bytes` | `fs.read` |
| `file_write_new` | `(path, length, data: borrow Slice<u8>, data_length: usize) -> usize` | `fs.write` |
| `file_stat`, `file_create_dir`, `file_remove` | `(path: borrow Slice<u8>, length: usize) -> usize` | `fs.read` or `fs.write` |
| `file_list` | `(path, length, max: usize) -> own Bytes` | `fs.read` |
| `file_write_atomic` | `(path, length, data, data_length) -> usize` | `fs.write` |
| `file_write_atomic_checked` | like `file_write_atomic`, but returns a classified outcome (not published, published or uncertain) instead of aborting. Use `std.fs.write_atomic_checked`. | `fs.write` |

## Network

Effect-gated operations that run only through a provider the host injects. A
handle is a `usize` token valid for one invocation, at most 8 open at once.

| Function | Effect | Notes |
| --- | --- | --- |
| `net_connect(host, port)` | `network.connect` | TCP client; returns a handle |
| `net_send(handle, bytes)` | `network.write` | blocking full write |
| `net_recv(handle, max)` | `network.read` | owned result; not allowed in `while` bodies (`SPX-T270`) |
| `net_stream_stdout(handle, max)` | `network.read` and `process.stdout.write` | appends to the stdout transcript |
| `net_wait(handle, ms)` | `network.read` | 0 timeout, 1 readable, 2 peer closed |
| `net_close(handle)` | `network.connect` | settles the handle |
| `net_tls_connect(host, port)` | `network.tls` | authenticated TLS client |
| `net_listen(host, port)`, `net_accept(listener)`, `net_close_listener(listener)` | `network.listen`, `network.accept` | explicit listener lifecycle |
| `net_tls_accept(listener)` | `network.accept` and `network.tls` | TLS server side |
| `https_get(url, max)` | `network.http` | `(borrow Slice<u8>, usize) -> own Bytes`; whole response |
| `https_post(url, body, max)` | `network.http` | HTTPS only; no redirects; the host sets an origin allow-list |

The TLS and listener operations run only through the hosted provider on the
interpreter; native and Wasm builds reject them before emission. `https_get` and
`https_post` belong to the `https-command-io.v1` profile. Try the network
operations against recorded replies with
`semaprax network-run <project> --fixture fixture.json`
([Input and output](../language/io.md)).

Usage patterns for each family are in [Ownership](../language/ownership.md),
[Collections](../language/collections.md) and [Input and output](../language/io.md).
Specs: [Network I/O](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-LANGUAGE-NETWORK-IO-V1.md),
[Network services](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-NETWORK-SERVICES-V1.md),
[HTTPS client I/O](https://github.com/wavect/semaprax/blob/main/docs/HTTPS-CLIENT-IO-V2.md).
