# Typed Path v1

Status: implemented bounded lexical-path profile; **HOSTED GREEN** under the
[v0.4.0 release baseline](RELEASE-0.4.0-STATUS.md).
The Everyday profile remains incomplete.

Audience: standard-library authors, compiler contributors, and agents working
with lexical paths.

This profile adds the `std.path.value` library over an ordinary source-authored
`Path` record. A Path contains a `Bytes` backing value and a `usize` logical
length. Its backing storage and fields use ordinary source ownership and
constructors; the record is neither opaque nor unforgeable.

## Lexical path contract

Admitted paths are NUL-free POSIX lexical byte paths. The profile defines no
UTF-8 interpretation, platform normalization, filesystem lookup, symlink
policy, or filesystem authority. Separators and component boundaries are
handled as bytes under the package's explicit lexical rules. An empty prefix is
valid and relative. Only `/` separates components; backslashes, colons, dots,
and non-UTF-8 bytes retain their byte meaning. This is lexical composition,
not directory confinement: `..` is not resolved or rejected.

`path_valid` returns false for a forged length beyond capacity without scanning
that length. Other observers and transitions require a valid path. The prefix
validator accepts an explicit in-bounds byte slice extent. Bytes after the
logical length may contain any value and never participate in path queries.

Every constructor and operation validates that the logical length is within
the backing `Bytes` bounds and that each inspected or written range is valid.
Invalid NUL-containing or out-of-bounds values fail through the ordinary
checked contract path before a result is published.

## Consuming operations

The parent and finish operations consume their `Path` input and return the
checked successor/result according to their source contracts. A consuming
transition settles the prior owner exactly once; it does not copy or revive a
backing buffer. Join uses a caller-supplied output buffer and validates its
capacity and resulting logical length before writing. No operation allocates
an ambient buffer or acquires filesystem authority.

Join keeps both input views inside nested borrowed-match scopes and fills a
separate caller-owned output. Those matches return a Copy value; the owning
Path is constructed only after both view scopes end. Thus join introduces no
borrow escape or owning result from a borrowed match.

The source-authored records lower through the existing checked HIR and retain
equivalent behavior on the interpreter, native C11, and Core Wasm lanes. The
backends consume checked bounds and ownership facts rather than treating Path
layout as authority.

Borrowed queries expose length, capacity, absolute status, nonempty segment
count, file-name start, parent end, extension start, and one checked byte. A
missing extension is represented by the logical length; the first dot of a
file name does not itself begin an extension. Trailing separators make the
file-name start equal the logical length.

Join replaces the base when the child is absolute. Otherwise it appends the
child, inserting one `/` only when both paths are nonempty and the base does
not already end in `/`. Existing separators are preserved. `path_join_length`
provides the required output capacity before ownership of a buffer is passed.
The result's logical length is the number of copied bytes; any remaining
output-buffer suffix is unchanged. `path_finish` returns the entire backing
buffer, so callers must retain the logical length when that prefix matters.

The parent drops the last nonempty component and trailing separators. Relative
single-component paths have an empty parent. An absolute path's parent keeps
its root: `/` and all-separator paths produce `/`. `path_parent_end` reports
the same new logical length used by the consuming transition.

## Package boundary

`std.path.value` is an additive internal nongeneric library profile. It does
not widen the original public `std.path` lexical helper package, add a public
generic or nominal ABI, or change the existing `std.path` declarations and
existing package descriptors or catalog entries. The original package remains the source of its
allocation-free byte inspection helpers; this profile supplies the typed Path
record composition separately.

Path normalization, safe joining beyond the admitted caller-buffer operation,
filesystem conversion, and platform-specific path policy remain outside this
profile. Its focused source, contract, projection, and cross-engine release
corpus is hosted green. [Filesystem I/O v1](FILESYSTEM-IO-V1.md) and
[v2](FILESYSTEM-IO-V2.md) are implemented separate profiles with a stricter
relative-path grammar and caller-selected provider authority. A lexical Path
alone is not filesystem permission or confinement.

## Focused verification

The focused selectors are:

```sh
cargo test --locked -p semaprax --lib loan_plan::
cargo test --locked -p semaprax --test project standard_library::typed_path
```

The package backend runner executes the committed main and every authored
zero-parameter boolean conformance function in separate authenticated snapshots.
It changes only the fixture entry call, then performs ordinary source and HIR
verification for each snapshot. This preserves the 16-allocation-site limit per
invocation while exercising every case on the interpreter, native C11 O0/O2,
and Core Wasm. Each Wasm case repeats four times against a three-entry Bytes
arena and requires zero live entries afterward. A source file's presence or
successful type checking alone does not count as runtime coverage.

Additional focused cases check constructor and observer contract failures,
undersized join output, borrowed-owner escape, conflicting join arguments,
canonical graph replay with forged field identity and source drift, bundled
dependency composition, and absent public export authority. Catalogs and
package metadata are generated from the canonical authored declarations.

Historical local results: all 10 focused loan-plan tests and all 8 typed-path
Project tests passed. The latter included the committed main plus all 11 authored
conformance cases on every listed engine. The 7 existing cursor tests also
passed, and catalog/metadata checks retained every original `std.path` entry.
These counts describe that local witness; current implementation evidence follows
the hosted-green release baseline, without broadening the lexical-path contract.
