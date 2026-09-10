# Path Normalization v1

Status: implemented bounded source profile; local evidence only. Platform path
conversion and filesystem authority remain out of scope.

Audience: language users, compiler contributors, standard-library authors, and
backend implementers.

This profile is the bundled `std.path.normalize` package: lexical
normalization of the typed `Path` values [Typed Path v1](TYPED-PATH-V1.md)
owns. It adds no type, no allocation beyond the caller's buffer, and no host
authority. Normalization is a sibling package for the packaging reason
[Standard Library v1](STANDARD-LIBRARY-V1.md) records for `std.data.json` and
`std.path.value`: one library module holding both halves pushes ordinary
multi-package consumers past the `SPX-G171` workspace-graph pre-bound.

## Normalization policy

Normalization is purely lexical over the Path's logical prefix. It never reads
a filesystem, resolves a symbolic link, or interprets a platform prefix.

- A run of one or more `/` separators is one separator, so `a//b` normalizes to
  `a/b` and a leading `//` is one root. POSIX leaves a leading `//`
  implementation-defined; this profile collapses it, which is the same choice
  Go's `path.Clean` makes and differs from a C library that keeps it.
- A `.` segment is removed.
- A `..` segment cancels the nearest retained segment to its left.
- A `..` that cannot cancel is retained for a relative path (`../a` stays
  `../a`) and dropped for an absolute one (`/..` normalizes to `/`).
- A trailing separator is removed (`a/` normalizes to `a`), except that the
  root itself stays `/`.
- An empty result is `.` for a relative path and `/` for an absolute one, so a
  normalized Path is never zero bytes.

The retained set is decided without a stack. A right-to-left walk over the
segments would raise a skip count on each `..` and let an ordinary segment
either consume one skip or survive; this profile computes exactly that state as
the clamped maximum prefix sum of a forward walk, where `..` contributes `+1`,
an ordinary segment `-1`, and `.` nothing. `skip_from` is that state, so
`seg_retained` is one comparison against zero, and the count of leading `..`
units a relative path must emit is the same function applied at offset zero.

## Operations

`length` is the exact byte length of the normalized form of a borrowed view and
`byte-at` is its byte at one index, both computed from the source alone with no
buffer, in the pull-based shape [JSON Cursors v1](JSON-CURSORS-V1.md) already
uses for encoding. `path-length` lifts `length` to a borrowed `Path`.

`into(borrow Path, own Bytes) -> Path` writes the normalized form into
caller-supplied capacity and returns the normalized `Path` over that buffer.
Its precondition preflights the exact normalized length against the buffer, so
a short buffer fails before any byte is written, and the borrowed input keeps
its bytes and logical length. Bytes beyond the returned length retain their
previous values, exactly as the existing `join` transition specifies.

The segment helpers (`seg-start`, `seg-end`, `seg-is-dot`, `seg-is-dotdot`,
`seg-retained`, `skip-from`, `kept-count`, `kept-bytes`, `leading-parents`,
`parent-region`, `emitted-offset`, `body-owner`, `absolute`) are the checked
steps of that policy and are individually admitted, executed and covered.

## Boundaries

This profile adds no public export or descriptor, no platform path conversion,
no UTF-8 or Unicode interpretation, no symbolic-link or filesystem resolution,
no Windows prefix or separator handling, and no traversal-containment claim
beyond the lexical rules above. A normalized path that still begins with `..`
is a valid relative result, not an escape decision; a caller that must contain
a path inside a root checks that itself.
