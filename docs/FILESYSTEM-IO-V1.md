# Filesystem I/O v1

Status: bounded implementation with focused local evidence. This specification
freezes the admitted local contract; it makes no hosted, registry, release, or
cross-platform physical-filesystem support claim.

Audience: language users, standard-library authors, compiler contributors, and
host-adapter implementers.

## Scope

Filesystem I/O v1 is a closed, invocation-scoped host-operation profile. It
does not give a checked program ambient access to a current directory, a home
directory, environment path lookup, WASI, Node's `fs`, libc file functions, or
an arbitrary host callback. A host supplies a `FileProvider` explicitly for
one invocation and settles it before it publishes that invocation's result.

The compiler-owned operations are not authored imports:

| Operation | Stable identity | Effect | Signature |
| --- | --- | --- | --- |
| `file_read` | `core.host.file-read` | `fs.read` | `(borrow Slice<u8>, usize path_length, usize max) -> own Bytes` |
| `file_write_new` | `core.host.file-write-new` | `fs.write` | `(borrow Slice<u8>, usize path_length, borrow Slice<u8>, usize data_length) -> usize` |

The source name, stable identity, arity, parameter ownership, result
ownership, effect, status table, and limits come from the one closed operation
table. Programs may not replace these operations with a declaration of the
same name or ID. A filesystem command is an explicit stable-ID `fn () -> bool`
whose reachable host operations use Filesystem I/O v1 and whose permits are a
nonempty subset of exactly `fs.read`, `fs.write`. Command, stdout/stderr,
network, HTTPS, native-Rust-import, and public-interface authority are not
part of this profile.

Arguments evaluate left to right. Both operations are fallible. A nonzero
normalized status selects the language failure before a result is initialized;
ordinary sticky failure and canonical cleanup then apply. `true` and `false`
are both successful semantic results.

## Path and payload bounds

The first slice is carrier storage and `path_length` is its logical extent.
Only bytes in `[0, path_length)` name a file. A logical extent beyond the
carrier, an empty extent, or an extent over 4,096 bytes fails with
`INVALID_PATH`; bytes after it never affect host path selection.

The logical prefix is a byte grammar, not UTF-8 or platform normalization:

- it is relative and nonempty;
- `/` separates nonempty components;
- `.` and `..` components are rejected;
- NUL, `\`, and `:` are rejected;
- non-UTF-8 bytes are otherwise permitted.

This grammar is intentionally stricter than `std.path.value`'s lexical Path
value. `Path` can represent an absolute or dot-containing lexical value; the
filesystem boundary rejects that value rather than resolving it.

`max` and `data_length` are each at most 65,536 bytes. A provider may return a
read value only within `max`; an over-limit successful result normalizes to
`CAPACITY_EXCEEDED`. A successful `file_write_new` must report exactly
`data_length`; a short or otherwise mismatched successful count normalizes to
`IO_FAILURE`. `file_read` initializes its owned `Bytes` result only on status
zero. `file_write_new` borrows its input and transfers no caller Bytes owner.

Each attempted operation first reserves its requested read maximum or write
length, with no refund. It then validates the logical path extent and grammar,
the per-file maximum and write-data extent, and finally dispatches to the
provider. This order selects the same failure when several inputs are invalid. One invocation admits at most 64
operations and 1,048,576 reserved bytes. The bound is aggregate across reads
and writes, not an allowance per call. An over-limit request or exhausted
aggregate budget is `CAPACITY_EXCEEDED`; a malformed logical slice extent is
also rejected before a provider receives it.

## `std.fs` composition

`std.fs` is an additive package which imports the ordinary source-authored
`std.path.value::Path`, `std.io::Reader`, and `std.io::Writer` identities. Its
two public source functions are:

| Function | Signature | Behavior |
| --- | --- | --- |
| `std.fs.read` | `(own Path, usize max) -> Reader` | consumes Path, sends its backing view and logical length to `file_read`, then constructs a Reader from the returned exact-length Bytes |
| `std.fs.write-new` | `(own Path, own Writer) -> usize` | consumes both inputs, sends the Writer's initialized prefix `[0, position)` to `file_write_new`, and creates a new file only |

Both wrappers require `path_valid(path)`. The host grammar above still applies,
so lexical validity does not itself grant filesystem authority. `write-new`
uses the Writer's cursor as `data_length`; `writer_finish` returns the full
backing buffer, while the host receives only that initialized prefix. The
unwritten suffix is neither inspected nor written.

Neither wrapper fabricates a Reader, Writer, Path, or Bytes on failure. Ordinary owning matches and function transfers consume the wrapper inputs;
existing cleanup plans settle staged owners exactly once.

## Provider, native, and Wasm boundaries

`FileProvider` is borrowed mutably for one invocation. Its retained root or
fixture files may persist across invocations; settlement does not revoke that
host-selected scope. Its `read` transfers a bounded byte vector
on success; `write_new` borrows bytes only for the synchronous call; and
`settle` runs once on success or failure before result publication. The
deterministic fixture provider is explicit in-memory state and claims no
physical host access. The denied provider always returns `AUTHORITY_DENIED`.

The Unix `ScopedFileProvider` is an explicitly selected physical-provider
implementation. It retains an opened directory descriptor as its authority
root, walks relative parent descriptors with no symlink following, and opens
regular files only. It does not derive authority from cwd or an input pathname.
`write_new` uses create-exclusive semantics: an existing file is never
overwritten. Physical writes are not transactional: a failed write may leave a
partial newly-created file, and a later semantic failure cannot roll that
physical state back. This is a host-side effect boundary, not a claim of
durability or atomic replacement.

Generated native semantic functions receive only `spx_context`; they contain
no libc path or directory calls. The generated runner accepts a callback table
with explicit context, read, create-new write, and settle entries. The read
callback writes into a generated bounded allocation and reports a count, so a
host allocation never becomes an unauthenticated owned-Bytes carrier. The
adapter poisons and frees the slot on failure or invalid count, maps only the
closed status range, and settles before publication.

Core Wasm adds two synchronous filesystem imports,
`env.spx_filesystem_read_v1` and `env.spx_filesystem_write_new_v1`, alongside
checked scalar/byte support and the existing command import prefix; it has no WASI or ambient Node filesystem
import. The wrapper validates status, logical extents, provider-owned result
membership, bounds, and exact write counts before cleanup commits. Repeated
invocations reset the filesystem marker and settle live owned-byte arena
entries. A Wasm host must inject its provider explicitly; importing these names
does not grant filesystem authority.

## Status domain

Every fallible operation uses `semaprax.filesystem.v1` with adapter class and
known-false retryability:

| Code | Name | Meaning |
| ---: | --- | --- |
| 1 | `INVALID_PATH` | invalid logical extent or rejected relative byte grammar |
| 2 | `NOT_FOUND` | no readable selected file |
| 3 | `ALREADY_EXISTS` | create-new target exists |
| 4 | `CAPACITY_EXCEEDED` | per-file, aggregate, operation, or provider result bound exceeded |
| 5 | `IO_FAILURE` | physical/provider I/O failure or successful write-count mismatch |
| 6 | `AUTHORITY_DENIED` | host withheld read/write authority |
| 7 | `INVALID_FILE_TYPE` | selected Unix object is not an admitted regular file or safe directory path |

Status zero is success. Any other provider status is a target invariant failure
rather than an extension of this domain. A physical error is normalized to one
of these codes; raw errno, JavaScript exception text, paths, or host receipts
never enter semantic output.

## Semantic projections and Project v14

Reachable filesystem operations select additive `semaprax.graph.v41`. Its
`filesystem` fact records each retained checked call's function and expression
identity, operation ID/effect, closed status domain and codes, all four bounds,
logical-path policy, create-new mode, and no-refund accounting rule. Graph v41
is verified against retained HIR; changing a call, operation, bound, or schema
does not replay as the same graph. Earlier graph bytes remain frozen when no
filesystem call is retained.

Project v14 selects `filesystem-io.v1`, requires exactly sorted
`["fs.read", "fs.write"]`, one explicit command ID with `fn () -> bool`, and
no web exports. The package route admits checked `std.fs` composition and
produces explicit interpreter, callback-native, or injected-Wasm execution
products. It creates no public nominal ABI, web descriptor, filesystem receipt,
or implicit physical provider. Earlier Project schemas and profiles remain
unchanged.

## Focused evidence

The local selector below passes 29 tests: nine library/provider/graph tests,
four typed Project tests, and sixteen interpreter/native/Wasm operation tests.
Project conformance executes all three authored `std.fs` commands on the
interpreter, C11 at `-O0` and `-O2`, and repeated Core Wasm under Node. Cases
cover actual Unix files, create-new preservation, invalid paths before dispatch,
logical prefixes, empty files, cumulative bounds, multiple-invalid-input failure
priority, provider miscounts and exact owned-result cleanup. Metadata, catalog,
module-size and source-reader checks pass separately.

```sh
SPX_REQUIRE_CLANG=1 SPX_REQUIRE_NODE=1 cargo test --locked -p semaprax --lib --test useful_data --test project filesystem -- --nocapture
```

This is local evidence, not an observed hosted run or cross-platform physical
provider promotion. Full `std.fs` and Everyday profile completion remain open;
this tranche supplies bounded whole-file read and create-new write composition.
