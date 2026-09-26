# Built-in functions

Compiler-owned functions are reserved names available in every file — no
import, no dependency. Declaring your own `string_len` fails (`SPX-S113`).
Every type argument is explicit; every borrowed view takes a plain `let`
binding.

## Strings

| Function | Signature |
| --- | --- |
| `string_len`, `string_len_chars` | `(s: string) -> i64` — bytes / scalar count |
| `string_is_empty` | `(s: string) -> bool` |
| `string_concat` | `(a: string, b: string) -> string` — consumes both |
| `string_starts_with`, `string_contains` | `(s: string, other: string) -> bool` |
| `string_from_char` | `(c: char) -> string` |
| `string_from_i64` | `(value: i64) -> string` — canonical decimal text |
| `string_from_usize` | `(value: usize) -> string` — canonical decimal text |
| `string_as_str` | `(binding: string) -> borrow str` — binding only, never a literal |

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
| `bytes_zeroed` | `(count: usize) -> Bytes` — literal capacity |
| `bytes_set` | `(b: own Bytes, i: usize, v: u8) -> Bytes` — write-once chain |
| `bytes_as_slice` | `(b: borrow Bytes) -> Slice<u8>` |
| `array_as_slice` | `(a: borrow [u8; N]) -> Slice<u8>` |

## Vectors and boxes

| Function | Signature |
| --- | --- |
| `vec_with_capacity<T>` | `(usize) -> Vec<T>` — admitted Copy scalar `T` |
| `vec_push<T>` | `(own Vec<T>, T) -> Vec<T>` — thread the owner |
| `vec_len<T>` | `(borrow Vec<T>) -> usize` |
| `vec_get<T>` | `(borrow Vec<T>, usize) -> T` |
| `vec_clear<T>` | `(own Vec<T>) -> Vec<T>` |
| `vec_into_iter<T>` | `(own Vec<T>) -> Iter<T>` |
| `iter_next<T>` | `(own Iter<T>) -> IterStep<T>` — match with `match own` |
| `box_new<T>` | `(value: T) -> Box<T>` — explicit admitted Copy scalar |
| `box_get<T>` | `(value: borrow Box<T>) -> T` — synchronous Copy access |
| `box_into_inner<T>` | `(value: own Box<T>) -> T` — consuming extraction |

## Command I/O

| Function | Signature | Effect |
| --- | --- | --- |
| `stdout_write`, `stderr_write` | `(v: borrow Slice<u8>) -> usize` | `process.stdout.write` / `process.stderr.write` |
| `args_len` | `() -> usize` | `process.args.read` |
| `arg_utf8` | `(i: usize) -> borrow str` | `process.args.read` |
| `stdin_read` | `() -> own Bytes` | `process.stdin.read` |

`args_len`/`arg_utf8`/`stdin_read`/`stderr_write` need a project with the
`useful-data-command.v1` profile on the native target.

## Filesystem

| Function | Signature | Effect |
| --- | --- | --- |
| `file_read` | `(path: borrow Slice<u8>, length: usize, max: usize) -> own Bytes` | `fs.read` |
| `file_write_new` | `(path, length, data: borrow Slice<u8>, data_length: usize) -> usize` | `fs.write` |
| `file_stat`, `file_create_dir`, `file_remove` | `(path: borrow Slice<u8>, length: usize) -> usize` | `fs.read` / `fs.write` |
| `file_list` | `(path: borrow Slice<u8>, length: usize, max: usize) -> own Bytes` | `fs.read` |
| `file_write_atomic` | `(path, length, data: borrow Slice<u8>, data_length: usize) -> usize` | `fs.write` |

Bounded relative paths against an injected provider root. Writes create new
files, never overwrite (v1); v2 adds stat, listing, mkdir, removal, and
atomic replacement.

## Network

| Function | Effect |
| --- | --- |
| `net_connect(host, port)` | `network.connect` |
| `net_send(handle, bytes)`, `net_stream_stdout(handle, max)` | `network.write` |
| `net_wait(handle, ms)`, `net_recv(handle, …)` | `network.read` |
| `net_close(handle)` | — |

Effect-gated TCP client operations via an injected provider. `net_recv`
(owned result) is not admitted in `while` bodies.

Usage patterns for each family live in [Ownership](../language/ownership.md),
[Collections](../language/collections.md), and [Input and output](../language/io.md).
