# Input and output

Every I/O operation is an explicit effect plus an explicit provider. No file,
socket, argument, or stream is reachable unless the module permits it, the
function declares it, and the execution environment injects it.

## Command I/O: args, stdin, stdout, stderr

```semaprax
module spxgrep_lines.app;

permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }

@id("spxgrep-lines.run")
fn run() -> bool
    uses { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
{
    if args_len() == 1usize { let needle = arg_utf8(0usize); let input = bytes_as_slice(stdin_read()); byte_len(input) > 0usize } else { false }
}

@id("main")
fn main() -> i64
{
    0
}
```

| Operation | Signature | Effect |
| --- | --- | --- |
| `args_len()` | `() -> usize` | `process.args.read` |
| `arg_utf8(i)` | `(usize) -> borrow str` | `process.args.read` |
| `stdin_read()` | `() -> own Bytes` | `process.stdin.read` |
| `stdout_write(v)` / `stderr_write(v)` | `(borrow Slice<u8>) -> usize` | `process.stdout.write` / `process.stderr.write` |

`arg_utf8` returns a borrowed `str` view; `stdin_read` returns owned `Bytes`
(view it with `bytes_as_slice`). The writers return the byte count — assert
it so short writes fail loudly.

**Profiles matter.** Single-file `run` admits exactly the
`process.stdout.write` transcript profile. `args_len`, `arg_utf8`,
`stdin_read`, and `stderr_write` need a **project** with the
`useful-data-command.v1` profile, built for the **native** target.

## Filesystem: bounded, explicit, never overwriting

| Operation | Signature | Effect |
| --- | --- | --- |
| `file_read(path, len, max)` | `(borrow Slice<u8>, usize, usize) -> own Bytes` | `fs.read` |
| `file_write_new(path, len, data, data_len)` | `(…) -> usize` | `fs.write` |
| `file_stat`, `file_create_dir`, `file_remove` | `(borrow Slice<u8>, usize) -> usize` | `fs.read` / `fs.write` |
| `file_list(path, len, max)` | `(borrow Slice<u8>, usize, usize) -> own Bytes` | `fs.read` |
| `file_write_atomic(path, len, data, data_len)` | `(…) -> usize` | `fs.write` |

Rules: paths are bounded **relative** byte prefixes against an injected
provider root — no absolute paths, no ambient filesystem. Writes create new
files and never overwrite (v1); v2 adds stat, sorted immediate-name listing,
directory creation, removal, and atomic replacement. Only stat/list accept an
empty path for the injected root. `std.fs` composes typed `Path`, `FileInfo`,
and reader/writer values over these boundaries.

## Network: effect-gated TCP client

```semaprax
module net_http_get.app;

permit { network.connect, network.read, network.write, process.stdout.write }

@id("net-http-get.fetch")
fn fetch() -> bool
    uses { network.connect, network.read, network.write, process.stdout.write }
{
    let host = [101u8, 120u8, 97u8, 109u8, 112u8, 108u8, 101u8, 46u8, 111u8, 114u8, 103u8];
    let handle = net_connect(array_as_slice(host), 80usize);
    let sent = net_send(handle, array_as_slice(host));
    let closed = net_close(handle);
    sent == 11usize && closed == 0usize
}

@id("app.main")
fn main() -> i64
{
    0
}
```

| Operation | Effect |
| --- | --- |
| `net_connect(host, port)` | `network.connect` |
| `net_send(handle, bytes)` / `net_stream_stdout(handle, max)` | `network.write` (+ `process.stdout.write` for streaming) |
| `net_wait(handle, ms)` / `net_recv(handle, …)` | `network.read` |
| `net_close(handle)` | — (settles the handle) |

Execution happens only through an injected provider. `net_recv` returns an
owned result, so it is not admitted inside `while` bodies (`SPX-T270`) —
receive outside the loop, inspect the bytes inside.

## Best practices

1. **Declare the narrowest effects you need.** `stdout`-only tools shouldn't
   permit the network; reviewers and the semantic graph both see the difference.
2. **Check counts and statuses.** Writers return bytes written, close returns
   a status — compare them instead of assuming success.
3. **Bytes at the boundary, scalars inside.** Convert I/O bytes to views at
   entry, walk them with `byte_get`/`byte_range`, and build owned results at
   exit.

Exact rules: [Bounded Language Command I/O v1](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-LANGUAGE-COMMAND-IO-V1.md),
[Filesystem I/O v1/v2](https://github.com/wavect/semaprax/blob/main/docs/FILESYSTEM-IO-V1.md),
[Bounded Language Network I/O v1](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-LANGUAGE-NETWORK-IO-V1.md).
