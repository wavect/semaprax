# Input and output

After this page you can print, read arguments and input, touch files, and open
a network connection from Semaprax, and you will know what each one needs. Every
operation needs three things: the module permits the effect, the function
declares it, and the host running the program provides it.

## Print a number

<!-- handbook-smoke: {"stdout":"420\n"} -->
```semaprax
module app.print_count;

permit { process.stdout.write }

@id("app.main")
fn main() -> i64
    uses { process.stdout.write }
{
    let count = 42usize;
    let text = string_from_usize(count);
    let view = string_as_str(text);
    let written = stdout_write(str_as_bytes(view));
    if written == 2usize { 0 } else { 1 }
}
```

`run` prints `42` from `stdout_write`, then `0`, the value `main` returns.
`stdout_write` returns the number of bytes written, so compare it and fail
loudly on a short write. Use `string_from_i64` for signed values.

## Read arguments and standard input

| Call | Returns | Effect |
| --- | --- | --- |
| `args_len()` | `usize`, the argument count | `process.args.read` |
| `arg_utf8(i)` | `borrow str`, argument `i` | `process.args.read` |
| `stdin_read()` | `own Bytes`, all of standard input | `process.stdin.read` |
| `stdout_write(v)` | `usize`, bytes written | `process.stdout.write` |
| `stderr_write(v)` | `usize`, bytes written | `process.stderr.write` |

```semaprax
module spxgrep_lines.app;

permit { process.args.read, process.stdin.read }

@id("spxgrep-lines.run")
fn run() -> bool
    uses { process.args.read, process.stdin.read }
{
    if args_len() == 1usize {
        let needle = arg_utf8(0usize);
        let data = stdin_read();
        let input = bytes_as_slice(data);
        byte_len(input) > 0usize
    } else {
        false
    }
}

@id("main")
fn main() -> i64
{
    0
}
```

`stdout_append` and `stderr_append` take the same `borrow Slice<u8>` and
return the bytes accepted. They add to the output instead of writing once, so a
line command can print many times. All appends share one 65,536-byte budget
and appear only when the command ends with a settled result. They belong to
the `line-command-io.v1` profile.

Bind `stdin_read()` to a name before you borrow it: `bytes_as_slice(stdin_read())`
is `SPX-T266`. A single-file `run` supports only `process.stdout.write`. The
others need a Project with the `useful-data-command.v1` profile built for the
native target. See [Profiles](../projects/profiles.md).

## Read and write files

| Call | Returns | Effect |
| --- | --- | --- |
| `file_read(path, len, max)` | `own Bytes` | `fs.read` |
| `file_write_new(path, len, data, data_len)` | `usize` status | `fs.write` |
| `file_stat(path, len)`, `file_create_dir`, `file_remove` | `usize` status | `fs.read` or `fs.write` |
| `file_list(path, len, max)` | `own Bytes`, sorted names | `fs.read` |
| `file_write_atomic(path, len, data, data_len)` | `usize` status | `fs.write` |

```semaprax
module app.load;

permit { fs.read }

@id("load.size")
fn size() -> usize
    uses { fs.read }
{
    let path = [100u8, 97u8, 116u8, 97u8];
    let bytes = file_read(array_as_slice(path), 4usize, 64usize);
    let view = bytes_as_slice(bytes);
    byte_len(view)
}

@id("app.main")
fn main() -> i64
{
    0
}
```

Paths are bounded, **relative** byte strings (here `data`), resolved under a root
the host injects. There is no absolute path and no ambient filesystem.
`file_write_new` creates a new file and never overwrites. `file_write_atomic`
replaces one atomically. Only stat and list accept an empty path for the root.
The `std.fs` package builds typed `Path` and reader/writer values on top. See
[Standard library](../reference/stdlib.md).

### Know whether a write landed

`file_write_atomic` aborts the command when something fails.
`file_write_atomic_checked` returns a classified code instead: `0` published,
`1` not published (the target is untouched), `2` uncertain (the replace started
and the host cannot say whether it finished). Check for uncertain before you
retry. It needs `fs.write` and the private `filesystem-io.v3` profile, so use it
through `std.fs.write_atomic_checked`, which returns a `WriteOutcome`:

```semaprax
module save.app;

use type @id("std.fs.write-outcome") from std.fs as WriteOutcome;
use type @id("std.io.writer") from std.io as Writer;
use type @id("std.path.value.path") from std.path.value as Path;
use function @id("std.fs.write-atomic-checked") from std.fs as fs_write_atomic_checked;
use function @id("std.io.writer.from-bytes") from std.io as writer_from_bytes;
use function @id("std.io.writer.write-u8") from std.io as writer_write_u8;
use function @id("std.path.value.from-bytes") from std.path.value as path_from_bytes;

permit { fs.write }

@id("save.app.run")
fn run() -> bool
    uses { fs.write }
{
    let name = [110u8, 111u8, 116u8, 101u8];
    let path = path_from_bytes(bytes_copy(array_as_slice(name)));
    let w0 = writer_from_bytes(bytes_zeroed(2usize));
    let w1 = writer_write_u8(w0, 111u8);
    let w2 = writer_write_u8(w1, 107u8);
    match fs_write_atomic_checked(path, w2) { WriteOutcome::Published {} => true, WriteOutcome::NotPublished {} => false, WriteOutcome::Uncertain {} => false, }
}

@id("save.app.main")
fn main() -> i64
{
    0
}
```

The project around it sets `profile = "filesystem-io.v3"`,
`[capabilities] required = ["fs.read", "fs.write"]`, and depends on `std.fs`,
`std.io` and `std.path.value`. Its test module needs its own `main`. This
project passes `semaprax check` and `semaprax test`. Spec:
[Host operation outcome v1](https://github.com/wavect/semaprax/blob/main/docs/HOST-OPERATION-OUTCOME-V1.md).

## Connect over TCP

```semaprax
module net_http_get.app;

permit { network.connect, network.read, network.write }

@id("net-http-get.fetch")
fn fetch() -> bool
    uses { network.connect, network.read, network.write }
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

| Call | Effect |
| --- | --- |
| `net_connect(host, port)` | `network.connect` |
| `net_send(handle, bytes)` | `network.write` |
| `net_stream_stdout(handle, max)` | `network.write`, `process.stdout.write` |
| `net_wait(handle, ms)`, `net_recv(handle, …)` | `network.read` |
| `net_close(handle)` | none; settles the handle |

Calls run only through an injected provider, so tests can use a fixed one.
`net_recv` returns an owned value and is not allowed inside a `while` body
(`SPX-T270`): receive first, then inspect the bytes in the loop.

## TLS and listeners

Five more calls open encrypted connections and accept inbound ones:

| Call | Effect | Returns |
| --- | --- | --- |
| `net_tls_connect(host, port)` | `network.tls` | handle to an authenticated TLS client connection |
| `net_listen(host, port)` | `network.listen` | listener handle |
| `net_accept(listener)` | `network.accept` | handle to an accepted connection |
| `net_tls_accept(listener)` | `network.accept`, `network.tls` | handle to an accepted TLS connection |
| `net_close_listener(listener)` | `network.listen` | `0` |

The call checks the server name and the host owns the certificates; there is no
cleartext fallback and no implicit bind address. Handles share the same 8-slot
space as `net_connect`. These calls run only on the interpreter through a hosted provider.
Native and Wasm builds reject them before emission, and `network-run` fixtures
(v2) replay them with `tls: true` and a `listeners` queue. Spec:
[Bounded Network Services v1](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-NETWORK-SERVICES-V1.md).

## HTTPS in one call

`https_get(url, max)` and `https_post(url, body, max)` return the whole
response as owned bytes, shaped like an HTTP/1.1 message, so the `std.http`
parsers read it. Each needs `network.http` and the `https-command-io.v1`
profile. This function checks and builds:

```semaprax
module https_status.app;

permit { network.http }

@id("https-status.fetch")
fn fetch() -> usize
    uses { network.http }
{
    let url = [104u8, 116u8, 116u8, 112u8, 115u8, 58u8, 47u8, 47u8, 101u8, 120u8, 97u8, 109u8, 112u8, 108u8, 101u8, 46u8, 111u8, 114u8, 103u8];
    let reply = https_get(array_as_slice(url), 4096usize);
    byte_len(bytes_as_slice(reply))
}

@id("app.main")
fn main() -> i64
{
    0
}
```

The URL is HTTPS only, at most 2,048 bytes, with no credentials or fragment.
`max` is 1 to 65,536 and counts the headers. `https_post` sends a body of up to
65,536 bytes as `application/octet-stream`, follows no redirects, and only
reaches origins the host allow-lists (at most eight). A failure aborts the
command and leaves no partial response, and a POST error does not prove the
server did nothing, so do not retry blindly. Test against recorded replies with
`semaprax network-run`. Start from `examples/https-project`.

## Wire a standard-library package

The packages `std.fs` and `std.http` supply typed values and parsers over these
calls. Three steps: depend on the package, set its profile, import by stable id,
as in the `std.fs` example above. Use `semaprax help library` to list the
exact signatures; for example `semaprax help library std.fs.read`.

## Narrow the effects

Declare only the effects a function needs. A tool that prints should not
permit the network, and reviewers and the semantic graph both see the
difference. Convert I/O bytes to views at the edge, scan them with
`byte_get` and `byte_range`, and build owned results at the end.

Exact rules: [Bounded Language Command I/O v1](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-LANGUAGE-COMMAND-IO-V1.md),
[Filesystem I/O v1](https://github.com/wavect/semaprax/blob/main/docs/FILESYSTEM-IO-V1.md) and
[v2](https://github.com/wavect/semaprax/blob/main/docs/FILESYSTEM-IO-V2.md),
[Bounded Language Network I/O v1](https://github.com/wavect/semaprax/blob/main/docs/BOUNDED-LANGUAGE-NETWORK-IO-V1.md),
[HTTPS Client I/O v2](https://github.com/wavect/semaprax/blob/main/docs/HTTPS-CLIENT-IO-V2.md).
