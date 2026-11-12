# Invocation-selected file input v1

Status: focused executable qualification passed on macOS arm64.
Audience: compiler implementers and authors of bounded native file commands.
No interpreter, Wasm, Web, npm, Windows, hosted, physical-device, or fresh
LogLens improvement claim is made. Historical CLI boundary failures motivated
this addition; they are prior-head evidence, not performance measurements.

## Selection and usage

The additive Project profile `source-command.selected-file.v1` selects semantic
contract `semaprax.project.v33`, input
`argv-utf8+selected-file.follow.v1`, and target matrix `["native64"]`.
The ordinary explicit entry, source linkage, capabilities, HIR validation,
publication and 0..255 exit-status contract of Project v26 still apply.
`fs.read` and `process.args.read` must both be declared. Other admitted effects
are the existing staged stdout/stderr effects. No new source operation or
filesystem write authority is introduced.

The generated launcher accepts an explicit host selection prefix:

```
./app --spx-selected-file-v1 1 -- report /absolute/input.log --summary
```

The decimal index is zero-based in the application arguments following `--`.
The checked source sees precisely `report`, `/absolute/input.log`, `--summary`,
in that order. The launcher consumes only its fixed four-argument prefix.
It does not interpret, reorder, remove, or validate application modes/options.
The selected path spelling has a separate 4,096-byte bound.
Application argv retains the existing limit of 16 UTF-8 arguments and 65,536
aggregate bytes. A selector is one canonical decimal index in 0..15; missing
separator/index, overflow, leading zeros, or an index outside supplied argv is
usage status 2 before any file acquisition or content read.

Without the prefix all arguments still reach source unchanged, but there is no
file grant. This permits application-owned usage/help paths without acquiring
input. The checked application must validate its complete usage before calling
`file_read_text`. The launcher only binds the selected argv value at invocation
start; the first authorized read lazily acquires its descriptor. A selected
missing file therefore cannot mask an earlier application usage result.

## Authority and named link policy

A host-selected argv position binds exactly one path spelling. Checked source
receives ordinary argv and can reference that binding by supplying its exact
text to `file_read_text`; string equality is a lookup within an already granted
invocation, never permission to choose another path. No matching grant means
`AUTHORITY_DENIED`; a different path means `UNSELECTED_INPUT`. The provider
never interprets another source string as an OS pathname and never opens cwd as
an authority root. Absolute paths, parent-relative paths, dot names, and spaces
are admitted only at this explicit host selection boundary.

The named `follow` policy follows symbolic links in the explicitly selected
host path using the platform's bounded pathname-resolution policy. It does not
remove `O_NOFOLLOW` from any existing provider. Kernel resolution loops fail
closed. This is a separate opt-in authority contract; canonical relative roots
and no-follow component traversal in Project v26/v28 remain unchanged.

The descriptor is opened once, read-only, nonblocking and close-on-exec. After
acquisition every read uses that held descriptor at offset zero; a replacement
of the path or symlink cannot substitute another inode. Failed acquisition is
sticky for this grant: it is never retried against a subsequently changed path.
The host may have changed the selected path before the first authorized read;
the lazy acquisition boundary selects the file as it exists at that instant.
This contract promises no earlier path snapshot or immutable filesystem.

## Bounds and failures

Only regular files are admitted. The per-read limit is exactly 65,536 bytes;
an additional byte detects overflow. An empty regular file is valid. Input
must be valid UTF-8 under the existing command text predicate. Every attempt
reserves one operation and 65,536 bytes before key validation or descriptor
access; the frozen 64-attempt / 1,048,576-byte reservation ceilings are retained.
A reservation is not refunded.

The held file's device/inode, size, modification time and change time are
checked before and after a read. Detectable descriptor-content drift fails
closed. This is detection, not an atomic snapshot or a guarantee against a
privileged writer restoring metadata. The invocation has no concurrent-file
coordination or locking authority.

Failures use normalized adapter class, never retryable,
`semaprax.selected-file.v1`: 1 absent grant; 2 unselected input; 3 capacity;
4 nonregular input or link-resolution refusal; 5 acquisition/read/settlement
I/O failure; 6 invalid UTF-8; 7 detected file drift. Failure selects before
owned String result initialization. The existing ordinary String allocation runtime is retained; physical String
allocation recovery remains the separate incomplete allocation-failure tranche.
This profile does not claim to close that gap.
Live canonical cleanup remains responsible
for language values; provider failure does not fabricate a String or replace
an already selected status.

## Lifetime and publication

The descriptor belongs to exactly one native invocation. It is retained across
reads, closed once after root postconditions and language cleanup, and never
exported to source or inherited by another process. Provider settlement occurs
before stderr/stdout publication. Settlement failure selects I/O failure only
when the root succeeded; an existing failure stays selected. Failure discards
staged output. No file grant is persisted in Project graphs, locks, manifests,
cache artifacts or semantic evidence.

## Owning qualification

The Project source-command suite passed on compiler source
`7d27641abb9a80b357beaf602b96ee3ffc4fd752` using Rust 1.97.1 and Apple
Clang 21.0.0. Native O0/O2 witnesses exercised argument order and modes,
usage before lazy acquisition, absolute and parent-relative selections,
hidden names and spaces, explicitly selected symlinks, and reads through the
held descriptor after pathname replacement. They covered nonregular files,
missing grants, wrong keys, invalid UTF-8, inclusive file and cumulative
read bounds, detectable drift, root failure and settlement before publication.
Project graph and lock checks confirmed the v33 selector and portable-route
refusals. The ordinary v26 and v28 native controls exercised direct relative
file reads and refusal of both final-component and intermediate symlinks.

The [v34 adoption](INVOCATION-SELECTED-FILE-RESOURCE-V1.md) passed the
original CLI49 acceptance. Fresh matched trials remain pending.
