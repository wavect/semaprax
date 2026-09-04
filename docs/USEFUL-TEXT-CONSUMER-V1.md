# Useful Text Consumer v1

Status: versioned bounded reference; the completion matrix owns product status.

Audience: language users, tool authors, and compiler contributors.

Useful Text Consumer v1 is a deliberately restrictive borrowed UTF-8 input
profile. Its nonignored regression inventory is exact-tag hosted green at
v0.2.0 commit `5f6fb9655fdec92c57ab71615cfd7bfa8cc76051` in
[run 33608662244](https://github.com/wavect/semaprax/actions/runs/33608662244).
That does not widen the profile or imply broader text support.

## Language boundary

`str` is a non-owning invocation-bounded view. It is admitted only as an
explicit `borrow str` parameter and cannot be returned, stored in a record or
variant, moved, shared, cloned into owned `string`, or placed in cleanup
storage. The compiler-owned operations are `str_len_bytes`, `str_is_empty`,
`str_starts_with`, and `str_contains`, with stable identities under
`core.str.*`. They operate on UTF-8 bytes, so `len_bytes` is not a character
count and embedded NUL is ordinary data.

One invocation admits at most 65,536 cumulative borrowed-text bytes; each
external argument is charged exactly once. Internal forwarding and
aliasing do not charge the same admitted invocation-root view again. `contains`
uses fixed-capacity Knuth-Morris-Pratt prefix search in native C and Wasm, and
the same byte-KMP semantics in the interpreter, so periodic hostile inputs are
linear in value plus needle length rather than quadratic.

The interpreter retains an invocation-root provenance identity with shared
borrow evidence rather than manufacturing an owned `String`. Native C uses a
pointer-plus-`u64`-length carrier and checks null, length bounds, and UTF-8 for
storage that the host is responsible for making readable; C cannot authenticate
an arbitrary non-null pointer. Native root-call admission charges all borrowed
parameters once in invocation-local context, while nested calls retain that
root depth. Callbacks and host imports are excluded from this profile, so there is
no reentrant host entry inside that depth. The Wasm public adapter additionally validates
the public 64-KiB scratch range and cumulative byte charge before the call.
The module has fixed three-page memory: page zero is the only public scratch
region and pages one and two are a caller-visible reserved fixed `u16` KMP
table whose read-before-use sentinel is reset on every call. It never
grows memory, scans for NUL, or retains the borrowed bytes after return.

Workspace `use function` linking admits the same exact non-escaping `borrow
str` parameters across source files. The import stub must match the provider's
ownership and type exactly; `borrow string`, shared/owned text, borrowed
nominals, and borrowed results remain rejected. This is the boundary used by
the `std.text` package and does not widen the public host ABI above.

## Public export profile

Only explicit stable-ID, monomorphic, effect- and contract-free functions in
the closed text profile are exportable. Parameters are borrowed `str`; results
are `i64` or `bool`. The exact compiled function inventory must have an acyclic
call graph; direct and mutual recursion reject before emission. Export-root linking includes every selected manifest root,
including roots disconnected from the entry function, while preserving the
legacy scalar and Project-v1 Web bytes when the profile is absent.

Generated JavaScript maps JavaScript strings through checked UTF-8 encoding
into fixed Wasm scratch and maps results to `bigint` or `boolean`. Generated
TypeScript exposes `string`, `bigint`, and `boolean` for exactly those admitted
signatures.

## Evidence and nonclaims

Local tests cover canonical source/HIR/Graph, hostile ownership and escape
rejection, cleanup-inertness, interpreter provenance, Unicode and embedded
NUL, native O0/O2 pointer-plus-length execution, adversarial periodic KMP
match/miss cases at the exact 65,536-byte cumulative bound, over-budget
rejection, Wasm validation and fixed scratch behavior, JavaScript/TypeScript
consumption, and v1 byte preservation.

This is not a general text processor. `usize`, arrays, slices, indexing,
iteration, mutable text, substring views, owned-text conversion, dynamic
allocation, callbacks, async, and general Component or resource ABI remain
open.
