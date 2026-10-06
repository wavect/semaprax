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
(`SPX-T270`): receive first, then inspect the bytes in the loop. HTTPS and
services are covered in the specifications below.

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
