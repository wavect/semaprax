# Text Toolkit v1

Audience: language users, agent authors, and compiler contributors.

Status: Partial — implemented for the reference interpreter and generated C11
(`run`, `run --native`, `build --target native`). Every Core Wasm lane refuses
the family with one stable diagnostic (`SPX-W116`). It builds on
[Owned String Loops v1](OWNED-STRING-LOOPS-V1.md) and the
[string operations](STRING-OPS-V1.md).

## Objective

An ordinary command-line program reads a file named by an argument, cuts its
text into lines and fields, parses numbers, counts, and prints a report. Before
v1 a single `.spx` file could do none of that: `semaprax run` passed no program
arguments, nothing turned file contents into a `string`, and nothing cut a
`string` into pieces. v1 adds five pure text operations, one file operation,
and a command-line profile for single-file programs.

## Operations

All six are compiler-owned and reserved like the other `string_*` names
(declaring one is `SPX-S113`). They resolve to ordinary monomorphic calls with
the stable identities below; no prelude bytes, graph schema version, or
earlier program's projection changes.

| Function | Stable identity | Signature |
| --- | --- | --- |
| `string_slice` | `core.string.slice` | `(s: string, start: i64, end: i64) -> string` |
| `string_find` | `core.string.find` | `(s: string, needle: string, from: i64) -> i64` |
| `string_to_i64` | `core.string.to_i64` | `(s: string) -> Option<i64>` |
| `string_trim` | `core.string.trim` | `(s: string) -> string` |
| `string_byte_at` | `core.string.byte_at` | `(s: string, index: i64) -> i64` |
| `file_read_text` | `core.host.file-read-text` | `(path: borrow str) -> string`, effect `fs.read` |

Offsets and indexes are **byte** offsets into the UTF-8 encoding.

- `string_slice` copies bytes `[start, end)`. It fails with
  `semaprax.text.v1` code 1 unless `0 <= start <= end <= string_len(s)`, and
  with code 2 when `start` or `end` splits a multi-byte character.
- `string_find` returns the offset of the first occurrence of `needle` at or
  after `from`, or `-1`. An empty needle matches at `from`. `from` outside
  `0..=string_len(s)` fails with code 1.
- `string_to_i64` accepts an optional leading `-` followed by one or more
  ASCII digits and nothing else (leading zeros are allowed, `-0` is `0`). Any
  other text, including `+5`, spaces, and values outside the `i64` range, is
  `Option::None {}`. It never fails.
- `string_trim` removes leading and trailing ASCII whitespace: space, tab,
  line feed, vertical tab, form feed, and carriage return.
- `string_byte_at` returns the byte at `index` as `0..=255`; `index` outside
  `0..string_len(s)` fails with code 1. It classifies characters without
  allocating.
- `file_read_text` reads one whole file, at most 65,536 bytes, and returns it
  as a `string`. Bytes that are not valid UTF-8 fail with code 3. File
  failures select the existing [Filesystem I/O v1](FILESYSTEM-IO-V1.md)
  statuses: `semaprax.filesystem.v1` code 1 (invalid path), 2 (not found), 4
  (over 65,536 bytes or the aggregate budget), 5 (I/O failure), 6 (authority
  denied), 7 (not a regular file). The path grammar, the per-file bound, and
  the 64-operation / 1 MiB reservation budget are those of `file_read`.

### Status domain

`semaprax.text.v1`, adapter class, never retryable:

| Code | Meaning |
| ---: | --- |
| 1 | offset or index out of range, or `start > end` |
| 2 | a slice bound splits a UTF-8 character |
| 3 | file text is not valid UTF-8 |

A failure is the ordinary checked language status: it is sticky, cleanup
releases every live value exactly once, and no partial result is published.

### Ownership

Like `string_len`, every `string` operand is borrowed: a read of a `string`
binding allocates a clone that the call's cleanup region releases, and the
binding stays usable. `file_read_text` borrows a `str` view, exactly as the
`str_*` operations do; bind it first (`let path = arg_utf8(0usize);`).
Results are new owned strings. The operations are admitted in `while` and
`for` bodies exactly like the other `string_*` operations; each iteration
releases its own strings.

### Matching `string_to_i64`

The result is the compiler-owned `Option<i64>`. Match it directly:

```text
let value = match string_to_i64(field) { Option::Some { value: n } => n, Option::None {} => 0, };
```

A `while` body admits this match only as the exact pair of unguarded arms
`Option::Some { value }` and `Option::None {}` whose scrutinee is the
`string_to_i64` call, the same rule as `byte_get` in Indexed Byte Loop v2
(`SPX-T252` names the wrong detail of a near miss). The reference interpreter
admits an `Option<i64>` match only with the call as scrutinee; binding the
result first (`let o = string_to_i64(s);`) is outside its profile (`SPX-F102`),
while native C accepts it.

Independent matches in one function do not multiply its cleanup-replay cost:
a field parser may test each field with its own `match` in one function.
Cleanup replay compares such functions factored by cleanup state
([RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md#factored-skeleton-comparison)).

## Command-line programs

A single `.spx` file whose `permit` set is a nonempty subset of
`fs.read`, `process.args.read`, `process.stderr.write`, and
`process.stdout.write` — anything except exactly `process.stdout.write`,
which keeps the [stdout transcript](BOUNDED-STDOUT-TRANSCRIPT-V1.md)
behavior — is a command-line program:

- `semaprax run <file> [--native] -- <arg>...` passes the arguments after
  `--` to `args_len()` and `arg_utf8(i)`. A built program
  (`semaprax build <file> --target native`) receives its own argv. At most 16
  arguments of strict UTF-8, 65,536 bytes in total.
- `stdout_write` and `stderr_write` stage at most one write per channel and
  path (the existing capacity rule); both are published, stderr first, only
  after `main` returns and its cleanup settles.
- `main`'s `i64` result is the process exit status and is not printed. It
  must be in `0..=255`; anything else is reported (`SPX-F116` on the
  interpreter) and exits with 1.
- A checked failure (`semaprax.text.v1`, `semaprax.filesystem.v1`,
  arithmetic, contract) discards both channels, prints one line naming the
  status to stderr, and exits with 1.
- With `fs.read` permitted, `file_read_text` reads regular files **below the
  directory the program was started in**: relative paths only, no `.` or
  `..` components, no symbolic links (each component is opened relative to
  its parent without following links). Without `fs.read`, or on Windows, the
  call fails with `AUTHORITY_DENIED`. No write, directory, network, process,
  environment, or stdin authority is reachable.
- `semaprax run` refuses `-- <arg>` for any other program and treats `-h` or
  `--help` after `--` as program arguments.

The profile admits exactly `stdout_write`, `stderr_write`, `args_len`,
`arg_utf8`, and `file_read_text` as host operations; anything else, such as
the byte-oriented `file_read`, is `SPX-T273`. `--json` on the interpreter
route prints a `semaprax.single-file-command.v1` envelope with `fuel`,
`outcome`, `stdout`, and `stderr`.

The interpreter reports a failure as
`single-file execution failed with language status {…} (meaning)`; native C
prints `SEMAPRAX operation failure: <domain>/<code>`. Both exit with 1.

## Backends

| Backend | Behavior |
| --- | --- |
| Reference interpreter | Executes all six; the command-line route uses a read-only scoped provider rooted at the current directory (Unix). |
| Native C11 | Executes all six in length-delimited String profiles; `file_read_text` only in the command-line profile (`SPX-B103` elsewhere). |
| Core Wasm (every lane) | `SPX-W116` before any operand is emitted. |

## Evidence

`tests/language/text_toolkit_v1.rs` round-trips the corpus through the
canonical formatter and the graph (asserting each `callee` identity), runs it
on the interpreter and on C11 at `-O0` and `-O2` under an allocation-counting
allocator that requires zero live allocations after every case, including
text failures inside loops, pins the Wasm refusal and the misuse diagnostics,
and drives `semaprax run` and `semaprax run --native` with arguments, files,
exit statuses, usage errors, missing and escaping paths, and invalid UTF-8.

## Not in v1

Sorting, sets, `<` on strings, number formatting with padding or decimals,
appending output in several writes, reading stdin, and the Core Wasm lowering
of these operations. [String Collections v1](STRING-COLLECTIONS-V1.md) adds
`Map<string, i64>` and `string_compare`.
