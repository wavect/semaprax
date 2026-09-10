# IO Cursors v1

Status: implemented bounded source profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
The broader Everyday profile remains incomplete.

Audience: language users, compiler contributors, standard-library authors, and
backend implementers.

This profile defines the first source-authored `std.io` Reader and Writer
shapes. Each is an ordinary nongeneric record containing a caller-supplied
`Bytes` buffer and a `usize` cursor. The records are public source values with
ordinary constructors and field rules; they make no opaque or unforgeable
representation claim.

## Cursor transitions

`reader_from_bytes` and `writer_from_bytes` consume an existing buffer and
start at position zero. Borrowed `position` and `remaining` observers preserve
the owner; `reader_peek` returns one `u8` and requires a nonempty remainder.
`reader_advance` consumes its Reader and clamps the requested advance to the
remaining length. `writer_write_u8` requires spare capacity, consumes its
Writer, replaces that byte, and advances by one. Each `finish` consumes its
cursor and returns the original full-length buffer; unwritten suffix bytes
retain their original values. Zero-length buffers are valid exhausted cursors.

Cursor constructors remain ordinary record constructors. Observer and transition
preconditions reject a forged position beyond the buffer length. Borrowed
buffer access destructures with `match borrow`, keeping its view arm-scoped. The
buffer remains supplied and owned according to the ordinary source binding;
the profile introduces no hidden allocation, ambient file, stream, process,
network, or descriptor authority.

Every operation evaluates its arguments left to right. Source contracts verify
that the cursor is within the buffer's admitted bounds before a read or write,
that each accessed byte range is valid, and that a successful transition
advances the cursor exactly as specified. A failed operation retains its
selected status and does not publish a fabricated cursor or buffer state.
Ordinary complete binding transfers continue to use the existing ownership and
cleanup rules.

The same checked HIR meaning is consumed by the interpreter, native C11, and
Core Wasm lanes. No backend infers cursor validity from a carrier layout, and
no lane gains authority from the Reader or Writer record.

## Checked composition

The additive internal Project import lane admits explicitly identified,
nongeneric resource-free record trees whose fields are Bytes and Copy scalars.
An internal helper may additionally borrow `str` while transferring or
borrowing one of these authenticated byte-record shapes; the view cannot be
stored in the record or returned by this lane. Callers import the exact nominal type identities. The ordinary verifier and
independent HIR validator admit record matches returning Copy scalars or
transferring Bytes/record ownership from an owning match; a borrowed field may
not escape. Existing cleanup transitions govern these whole-owner transfers.
The graph retains exact calls, nominal identities and checked cleanup meaning.

The bundled `std.io` package uses the library-only Project v8 route with
`web_exports = []`. This route applies to ordinary owned-data libraries without
a package-name exception: entry, tests, and internal calls are checked and can
execute, while public descriptor/export routes remain absent. Selecting any
public export still invokes the unchanged public API admission rules, including
contract restrictions. Existing authenticated std.mem/std.collections aliases
retain their additional exact-source checks. No public nominal or generic
descriptor is widened.

## Boundaries

Reader and Writer remain source-authored, nongeneric `Bytes` plus `usize`
records. This profile does not add a public resource ABI, opaque handles,
ambient I/O, buffered streams, files, sockets, asynchronous operations, or
standard-stream authority. Existing `Bytes`, contracts, cleanup, graph, and
backend meanings remain unchanged outside the admitted cursor transitions.

The profile does not supply a disk/network service or complete `std.io`.
Arbitrary streaming interfaces and general mutable cursor replacement in loops
remain open. Line processing over these same cursors is the separate additive
[IO Lines v1](IO-LINES-V1.md) profile; it changes no shape, signature, or
contract defined here. [Typed paths](TYPED-PATH-V1.md),
[filesystem I/O](FILESYSTEM-IO-V1.md), [filesystem v2](FILESYSTEM-IO-V2.md),
[formatting](FORMAT-WRITER-V1.md), and [logging](LOG-WRITER-V1.md) are implemented
separate profiles; none is implied solely by a Reader or Writer value.

## Focused local evidence

The historical local witness used the following selectors. The implemented
release corpus is now hosted green:

```sh
cargo test --locked -p semaprax --lib workspace_graph::owned_function_import
cargo test --locked -p semaprax --test project standard_library::io_cursor
cargo test --locked -p semaprax --test project manifest_v8::
cargo test --locked -p semaprax --test project package_manifest_v1::
```

The import tests cover retained provider execution, repeated contract failure,
wrong nominal identities, missing explicit type imports, generic rejection,
scalar-boundary rejection and removed record/Bytes result transfers. Cursor
cases cover zero and 255, exhausted and forged positions, full writes,
borrow-escape refusal, no-public-descriptor behavior and bundled dependency use
without vendored source. The package conformance executes on the interpreter,
C11 at O0/O2 and Core Wasm, whose one-entry byte arena balances over four runs.
Catalog and metadata checks preserve canonical signatures and record identities.
These are cursor/import checks, not evidence of a physical filesystem provider.
