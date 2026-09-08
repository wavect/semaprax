# Filesystem I/O v2

Status: private, bounded additive implementation. This document records the
current `filesystem-io.v2` contract and makes no hosted, release, completion,
or cross-platform physical-filesystem claim.

Filesystem I/O v2 extends the frozen Filesystem I/O v1 profile with metadata,
directory listing, directory creation, removal, and explicit atomic replacement.
The v1 operation identities, signatures, status domain, limits, provider
authority model, and projections remain frozen. A v1 program continues to
select Graph v41 and the v1 facts; reaching any v2 operation selects Graph v42
and the additive v2 facts.

## Closed operation set

The compiler owns these stable operation identities. They are not authored
imports and cannot be redefined by name or ID:

| Source operation | Stable identity | Effect | Signature |
| --- | --- | --- | --- |
| `file_stat` | `core.host.file-stat` | `fs.read` | `(borrow Slice<u8>, usize path_length) -> usize` |
| `file_list` | `core.host.file-list` | `fs.read` | `(borrow Slice<u8>, usize path_length, usize max) -> own Bytes` |
| `file_create_dir` | `core.host.file-create-dir` | `fs.write` | `(borrow Slice<u8>, usize path_length) -> usize` |
| `file_remove` | `core.host.file-remove` | `fs.write` | `(borrow Slice<u8>, usize path_length) -> usize` |
| `file_write_atomic` | `core.host.file-write-atomic` | `fs.write` | `(borrow Slice<u8>, usize path_length, borrow Slice<u8>, usize data_length) -> usize` |

`std.fs` composes these as `metadata`, `list`, `create_dir`, `remove`, and
`write_atomic`. The typed `metadata` wrapper returns a Copy `FileInfo` record
with `kind` and `size` fields. The compiler primitive returns a packed `usize`: a regular file is
`size * 4 + 1`; an admitted directory is `2` (`size` is zero). Arithmetic
overflow is `CAPACITY_EXCEEDED`. `create_dir` and `remove` return `0` on
success. `write_atomic` returns the exact data length on success.

## Paths, listing, and accounting

Paths are bounded byte prefixes with a logical `path_length`; bytes outside the
prefix are ignored. Non-root paths use v1's relative slash-separated grammar:
components are nonempty, and `.`, `..`, NUL, backslash, and colon are rejected.
The logical path is at most 4,096 bytes and need not be UTF-8. An empty logical
path is admitted only by `file_stat` and `file_list`, where it denotes the
provider's explicitly selected root. Empty paths remain invalid for create,
remove, atomic write, read, and create-new write.

`file_list` returns immediate entry names as raw bytes, each followed by one
NUL byte. Names are unique and sorted by unsigned raw-byte order; the result is
canonical and contains no path separators. There are at most 1,024 names, each
at most 4,096 bytes, and the complete NUL-terminated payload is at most 65,536
bytes. The requested `max` is also at most 65,536 bytes. The empty listing is
the empty byte sequence.

Every attempted operation reserves first and never refunds. Reads and lists
reserve their requested `max`; writes reserve `data_length`; stat, create-dir,
and remove reserve zero. The invocation-wide limits are 64 operations and
1,048,576 reserved bytes. The reservation precedes logical-slice extent and
path validation, provider dispatch, and result publication, preserving the
same failure priority across backends. A provider result is checked against
the requested bounds; listing results are additionally checked for the
canonical raw-byte wire form and stat results for the packed representation.

All failures use the frozen `semaprax.filesystem.v1` status domain and codes:
`INVALID_PATH` (1), `NOT_FOUND` (2), `ALREADY_EXISTS` (3),
`CAPACITY_EXCEEDED` (4), `IO_FAILURE` (5), `AUTHORITY_DENIED` (6), and
`INVALID_FILE_TYPE` (7). Zero is success. Failure selection is sticky and
settlement occurs before result publication, as in v1.

## Provider and target boundary

The caller injects a mutable `FileProvider` for one invocation. Its retained
root or fixture state is the explicit authority; there is no current-directory,
home-directory, WASI, Node `fs`, libc path lookup, or ambient callback. The
fixture provider is deterministic in-memory state, and the denied provider
returns `AUTHORITY_DENIED`.

The Unix `ScopedFileProvider` retains an opened directory descriptor, walks
relative parent descriptors without following symlinks, and admits regular
files and directories only. `write_atomic` writes a private temporary file and
uses the provider's explicit atomic rename within the selected parent. This
provides replacement atomicity at that provider boundary; it makes no
durability, fsync, crash-recovery, or rollback claim. A failed physical write
or later language failure cannot be treated as a transaction that restores
prior filesystem state.

The checked interpreter, native C path, and Core Wasm lowering all retain the
same operation set, validation order, status normalization, and ownership
rules. Core Wasm appends the v2 imports
`spx_filesystem_stat_v2`, `spx_filesystem_list_v2`,
`spx_filesystem_create_dir_v2`, `spx_filesystem_remove_v2`, and
`spx_filesystem_write_atomic_v2` after the frozen v1 filesystem prefix. Import
presence does not grant a provider or filesystem authority.

## Graph v42 and Project v15

Reachable v2 calls select `semaprax.graph.v42`. Its filesystem fact uses
`semaprax.filesystem.v2` and records the retained checked calls, the v1 status
domain, the 4,096/65,536/1,048,576/64 limits, root-capable stat/list
operations, `kind-plus-four-times-size-file1-directory2` stat encoding,
`sorted-unique-immediate-raw-names-nul-terminated` listing encoding, and
reserve-first no-refund accounting. Graph verification binds these facts to
the exact retained HIR calls and rejects a v1 schema or forged fact.

The private `std/fs` manifest selects Project v15 with profile
`filesystem-io.v2`, command shape `fn () -> bool`, no web exports, and exactly
the explicit capabilities `["fs.read", "fs.write"]`. It uses the existing
`std.io` and `std.path.value` dependencies. Project v14 and
`filesystem-io.v1` remain frozen and separate.

